use crate::app::RUNTIME;
use crate::backend::media;
use crate::util::get_border_radius;
use either::Either;
use gdk::prelude::GdkContextExt;
use gdk_pixbuf::Pixbuf;
use gdk_pixbuf::PixbufAnimation;
use gdk_pixbuf::PixbufAnimationExt;
use gio::prelude::FileExt;
use glib::source::Continue;
use gtk::prelude::*;
use gtk::DrawingArea;
use log::error;
use matrix_sdk::{identifiers::MxcUri, Client as MatrixClient};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Image {
    pub path: Either<MxcUri, PathBuf>,
    pub local_path: Arc<Mutex<Option<PathBuf>>>,
    pub max_size: Option<(i32, i32)>,
    pub widget: DrawingArea,
    pub pixbuf: Arc<Mutex<Option<Pixbuf>>>,
    /// useful to avoid the scale_simple call on every draw
    pub scaled: Arc<Mutex<Option<Pixbuf>>>,
    pub zoom_level: Arc<Mutex<Option<f64>>>,
    pub shrink_to_fit: bool,
    pub thumb: bool,
    pub fixed_size: bool,
    pub centered: bool,
}

impl Image {
    /// Image constructor this return an Image but not initialized, to
    /// have a working image you should call to the build method
    ///
    /// # Examples
    ///
    /// ```
    /// let img = Image::new("mxc://matrix.org/HASDH")
    ///           .rounded(true)
    ///           .fixed(true)
    ///           .size(Some((50, 50)))
    ///           .build();
    /// ```
    pub fn new(path: Either<MxcUri, PathBuf>) -> Image {
        let da = DrawingArea::new();
        da.add_events(gdk::EventMask::ENTER_NOTIFY_MASK);
        da.add_events(gdk::EventMask::LEAVE_NOTIFY_MASK);

        da.connect_enter_notify_event(move |da, _| {
            da.get_style_context().add_class("image-hover");
            da.queue_draw();
            Inhibit(false)
        });
        da.connect_leave_notify_event(move |da, _| {
            da.get_style_context().remove_class("image-hover");
            da.queue_draw();
            Inhibit(false)
        });

        Image {
            path,
            local_path: Arc::new(Mutex::new(None)),
            max_size: None,
            widget: da,
            pixbuf: Arc::new(Mutex::new(None)),
            scaled: Arc::new(Mutex::new(None)),
            zoom_level: Arc::new(Mutex::new(None)),
            thumb: false,
            fixed_size: false,
            centered: false,
            shrink_to_fit: false,
        }
    }

    /// When the image is drawn, shrink it (if necessary) to fit inside the
    /// allocated space, both width and height. This is used in the Media
    /// Viewer, for example, to make sure the image fits the screen.
    /// Contrast with images in the message feed, which fit to the width but
    /// expand vertically.
    pub fn shrink_to_fit(mut self, f: bool) -> Image {
        self.shrink_to_fit = f;
        self
    }

    pub fn center(mut self, c: bool) -> Image {
        self.centered = c;
        self
    }

    pub fn size(mut self, size: Option<(i32, i32)>) -> Image {
        self.max_size = size;
        self
    }

    pub fn build(self, session_client: MatrixClient) -> Image {
        self.draw();
        self.load_async(session_client);

        self
    }

    pub fn draw(&self) {
        let da = &self.widget;
        let ctx = da.get_style_context();

        match self.max_size {
            Some(size) => {
                let w = size.0;
                let h = size.1;

                da.set_hexpand(false);
                da.set_vexpand(false);

                if self.fixed_size {
                    da.set_size_request(w, h);
                } else {
                    da.set_hexpand(true);
                    if let Some(ref pb) = *self.pixbuf.lock().unwrap() {
                        let h = pb.get_height();
                        da.set_size_request(1, h);
                    } else {
                        // No image yet, square image
                        da.set_size_request(1, h);
                    }
                }
            }
            None => {
                da.set_hexpand(true);
                da.set_vexpand(true);
            }
        }

        let max_size = self.max_size;
        let pix = self.pixbuf.clone();
        let scaled = self.scaled.clone();
        let zoom_level = self.zoom_level.clone();
        let fixed_size = self.fixed_size;
        let centered = self.centered;
        let shrink_to_fit = self.shrink_to_fit;
        da.connect_draw(move |da, g| {
            let widget_w = da.get_allocated_width();
            let widget_h = da.get_allocated_height();

            let width = widget_w as f64;
            let height = widget_h as f64;

            let mut rw = widget_w;
            let mut rh = widget_h;
            if let Some(size) = max_size {
                rw = i32::min(size.0, widget_w);
                rh = i32::min(size.1, widget_h);
            }

            let arc_1 = 0.0;
            let arc_2 = std::f64::consts::PI * 0.5;
            let arc_3 = std::f64::consts::PI;
            let arc_4 = std::f64::consts::PI * 1.5;

            let border_radius = get_border_radius(&ctx) as f64;

            let widget_x = 0.0;
            let widget_y = 0.0;

            let context = da.get_style_context();
            gtk::render_background(&context, g, 0.0, 0.0, width, height);

            if context.has_class("image-spinner") {
                // TODO: draw a margin
            }

            if let Some(ref pb) = *pix.lock().unwrap() {
                let (mut pw, mut ph) = if shrink_to_fit {
                    adjust_shrink_to_fit(pb.get_width(), pb.get_height(), rw, rh)
                } else {
                    adjust_to(pb.get_width(), pb.get_height(), rw, rh)
                };

                if border_radius > 0.0 {
                    g.new_sub_path();
                    g.arc(
                        widget_x + pw as f64 - border_radius,
                        widget_y + border_radius,
                        border_radius,
                        arc_4,
                        arc_1,
                    );
                    g.arc(
                        widget_x + pw as f64 - border_radius,
                        widget_y + ph as f64 - border_radius,
                        border_radius,
                        arc_1,
                        arc_2,
                    );
                    g.arc(
                        widget_x + border_radius,
                        widget_y + ph as f64 - border_radius,
                        border_radius,
                        arc_2,
                        arc_3,
                    );
                    g.arc(
                        widget_x + border_radius,
                        widget_y + border_radius,
                        border_radius,
                        arc_3,
                        arc_4,
                    );
                    g.close_path();
                    g.clip();
                }

                if let Ok(zoom_level_guard) = zoom_level.lock() {
                    if let Some(zl) = *zoom_level_guard {
                        pw = (pb.get_width() as f64 * zl) as i32;
                        ph = (pb.get_height() as f64 * zl) as i32;
                    }
                }

                if fixed_size {
                    da.set_size_request(pw, ph);
                } else if !shrink_to_fit {
                    da.set_size_request(1, ph);
                }

                let mut scaled_pix: Option<Pixbuf> = None;

                if let Some(ref s) = *scaled.lock().unwrap() {
                    if s.get_width() == pw && s.get_height() == ph {
                        scaled_pix = Some(s.clone());
                    }
                }

                if scaled_pix.is_none() {
                    scaled_pix = pb.scale_simple(pw, ph, gdk_pixbuf::InterpType::Bilinear);
                }

                if let Some(sc) = scaled_pix {
                    let x = if centered {
                        ((width / 2.0) - (pw as f64 / 2.0)).round()
                    } else {
                        0.0
                    };
                    let y = if centered {
                        ((height / 2.0) - (ph as f64 / 2.0)).round()
                    } else {
                        0.0
                    };
                    g.set_source_pixbuf(&sc, x, y);
                    g.rectangle(x, y, pw as f64, ph as f64);
                    g.fill();
                    *scaled.lock().unwrap() = Some(sc);
                }
            } else {
                gtk::render_activity(&context, g, 0.0, 0.0, rw as f64, height);
            }

            Inhibit(false)
        });
    }

    /// If `path` starts with mxc this func download the img async, in other case the image is loaded
    /// in the `image` widget scaled to size
    pub fn load_async(&self, session_client: MatrixClient) {
        match self.path.as_ref() {
            Either::Left(mxc) => {
                let mxc = mxc.clone();
                // asyn load
                let response = if self.thumb {
                    RUNTIME.spawn(async move { media::get_thumb(session_client, &mxc).await })
                } else {
                    RUNTIME.spawn(async move { media::get_media(session_client, &mxc).await })
                };
                let local_path = self.local_path.clone();
                let pix = self.pixbuf.clone();
                let scaled = self.scaled.clone();
                let da = self.widget.clone();

                da.get_style_context().add_class("image-spinner");
                glib::MainContext::default().spawn_local(async move {
                    match response.await {
                        Err(_) => return,
                        Ok(Ok(fname)) => {
                            *local_path.lock().unwrap() = Some(fname.clone());
                            load_pixbuf(pix.clone(), scaled.clone(), da.clone(), &fname);
                            da.get_style_context().remove_class("image-spinner");
                        }
                        Ok(Err(err)) => {
                            error!("Image path could not be found due to error: {:?}", err);
                        }
                    }
                });
            }
            Either::Right(path) => {
                load_pixbuf(
                    self.pixbuf.clone(),
                    self.scaled.clone(),
                    self.widget.clone(),
                    &path,
                );
            }
        }
    }
}

pub fn load_pixbuf(
    pix: Arc<Mutex<Option<Pixbuf>>>,
    scaled: Arc<Mutex<Option<Pixbuf>>>,
    widget: DrawingArea,
    fname: &Path,
) {
    if is_gif(&fname) {
        load_animation(pix, scaled, widget, fname);
        return;
    }

    match Pixbuf::from_file(fname)
        .ok()
        .and_then(|pb| pb.apply_embedded_orientation())
    {
        Some(px) => {
            *pix.lock().unwrap() = Some(px);
            *scaled.lock().unwrap() = None;
        }
        _ => {
            let pixbuf = match gtk::IconTheme::get_default() {
                None => None,
                Some(i1) => match i1.load_icon(
                    "image-x-generic-symbolic",
                    80,
                    gtk::IconLookupFlags::empty(),
                ) {
                    Err(_) => None,
                    Ok(i2) => i2,
                },
            };
            *pix.lock().unwrap() = pixbuf;
            *scaled.lock().unwrap() = None;
        }
    };
}

pub fn load_animation(
    pix: Arc<Mutex<Option<Pixbuf>>>,
    scaled: Arc<Mutex<Option<Pixbuf>>>,
    widget: DrawingArea,
    fname: &Path,
) {
    let res = PixbufAnimation::from_file(fname);
    if res.is_err() {
        return;
    }
    let anim = res.unwrap();
    let iter = anim.get_iter(glib::get_current_time());

    glib::timeout_add_local(iter.get_delay_time() as u32, move || {
        iter.advance(glib::get_current_time());

        if widget.is_drawable() {
            let px = iter.get_pixbuf();
            *pix.lock().unwrap() = Some(px);
            *scaled.lock().unwrap() = None;
            widget.queue_draw();
        } else {
            return Continue(false);
        }
        Continue(true)
    });
}

pub fn is_gif(fname: &Path) -> bool {
    if !fname.is_file() {
        return false;
    }

    if let Ok(info) = gio::File::new_for_path(fname).query_info(
        &gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
        gio::FileQueryInfoFlags::NONE,
        gio::NONE_CANCELLABLE,
    ) {
        match info.get_content_type() {
            Some(mime) => mime == "image/gif",
            _ => false,
        }
    } else {
        false
    }
}

/// Adjust the `w` x `h` to `maxw` x `maxh` keeping the Aspect ratio
fn adjust_to(w: i32, h: i32, maxw: i32, maxh: i32) -> (i32, i32) {
    let mut pw = w;
    let mut ph = h;

    if pw > ph && pw > maxw {
        ph = maxw * ph / pw;
        pw = maxw;
    } else if ph >= pw && ph > maxh {
        pw = maxh * pw / ph;
        ph = maxh;
    }

    (pw, ph)
}

/// Adjust the `w` x `h` to fit in `maxw` x `maxh`, keeping the aspect ratio.
/// Do not make `w` x `h` bigger, only smaller.
fn adjust_shrink_to_fit(w: i32, h: i32, maxw: i32, maxh: i32) -> (i32, i32) {
    let ratio = w as f64 / h as f64;
    let t_ratio = maxw as f64 / maxh as f64;

    let (nw, nh) = if t_ratio < ratio {
        (maxw, (maxw as f64 * (1.0 / ratio)) as i32)
    } else {
        ((maxh as f64 * ratio) as i32, maxh)
    };

    if nw < w {
        (nw, nh)
    } else {
        (w, h)
    }
}
