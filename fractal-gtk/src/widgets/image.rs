use fractal_api::url::Url;
use gdk::prelude::GdkContextExt;
use gdk_pixbuf::Pixbuf;
use gdk_pixbuf::PixbufAnimation;
use gdk_pixbuf::PixbufAnimationExt;
use gio::prelude::FileExt;
use glib::source::Continue;
use gtk::prelude::*;
use gtk::DrawingArea;
use log::error;
use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::backend::BKCommand;
use crate::error::Error;
use std::sync::mpsc::TryRecvError;

#[derive(Clone, Debug)]
pub struct Image {
    pub path: String,
    pub local_path: Arc<Mutex<Option<String>>>,
    pub server_url: Url,
    pub max_size: Option<(i32, i32)>,
    pub widget: DrawingArea,
    pub backend: Sender<BKCommand>,
    pub pixbuf: Arc<Mutex<Option<Pixbuf>>>,
    /// useful to avoid the scale_simple call on every draw
    pub scaled: Arc<Mutex<Option<Pixbuf>>>,
    pub zoom_level: Arc<Mutex<Option<f64>>>,
    pub shrink_to_fit: bool,
    pub thumb: bool,
    pub circle: bool,
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
    /// let img = Image::new(backend, "mxc://matrix.org/HASDH")
    ///           .circle(true)
    ///           .fixed(true)
    ///           .size(Some((50, 50)))
    ///           .build();
    /// ```
    pub fn new(backend: &Sender<BKCommand>, server_url: Url, path: &str) -> Image {
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
            path: path.to_string(),
            local_path: Arc::new(Mutex::new(None)),
            server_url,
            max_size: None,
            widget: da,
            pixbuf: Arc::new(Mutex::new(None)),
            scaled: Arc::new(Mutex::new(None)),
            zoom_level: Arc::new(Mutex::new(None)),
            thumb: false,
            circle: false,
            backend: backend.clone(),
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

    #[allow(dead_code)]
    pub fn fixed(mut self, f: bool) -> Image {
        self.fixed_size = f;
        self
    }

    #[allow(dead_code)]
    pub fn circle(mut self, c: bool) -> Image {
        self.circle = c;
        self
    }

    #[allow(dead_code)]
    pub fn thumb(mut self, t: bool) -> Image {
        self.thumb = t;
        self
    }

    pub fn size(mut self, size: Option<(i32, i32)>) -> Image {
        self.max_size = size;
        self
    }

    pub fn build(self) -> Image {
        self.draw();
        self.load_async();

        self
    }

    pub fn draw(&self) {
        let da = &self.widget;

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
        let is_circle = self.circle;
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

                if let Ok(zoom_level_guard) = zoom_level.lock() {
                    if let Some(zl) = zoom_level_guard.clone() {
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
                    if is_circle {
                        use std::f64::consts::PI;

                        g.arc(
                            pw as f64 / 2.0,
                            ph as f64 / 2.0,
                            pw.min(ph) as f64 / 2.0,
                            0.0,
                            2.0 * PI,
                        );
                        g.clip();
                    }

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
    pub fn load_async(&self) {
        if self.path.starts_with("mxc:") {
            // asyn load
            let (tx, rx): (
                Sender<Result<String, Error>>,
                Receiver<Result<String, Error>>,
            ) = channel();
            let command = if self.thumb {
                BKCommand::GetThumbAsync(self.server_url.clone(), self.path.to_string(), tx)
            } else {
                BKCommand::GetMediaAsync(self.server_url.clone(), self.path.to_string(), tx)
            };
            self.backend.send(command).unwrap();
            let local_path = self.local_path.clone();
            let pix = self.pixbuf.clone();
            let scaled = self.scaled.clone();
            let da = self.widget.clone();

            da.get_style_context().add_class("image-spinner");
            gtk::timeout_add(50, move || match rx.try_recv() {
                Err(TryRecvError::Empty) => Continue(true),
                Err(TryRecvError::Disconnected) => Continue(false),
                Ok(Ok(fname)) => {
                    *local_path.lock().unwrap() = Some(fname.clone());
                    load_pixbuf(pix.clone(), scaled.clone(), da.clone(), &fname);
                    da.get_style_context().remove_class("image-spinner");
                    Continue(false)
                }
                Ok(Err(err)) => {
                    error!("Image path could not be found due to error: {:?}", err);
                    Continue(false)
                }
            });
        } else {
            load_pixbuf(
                self.pixbuf.clone(),
                self.scaled.clone(),
                self.widget.clone(),
                &self.path,
            );
        }
    }
}

pub fn load_pixbuf(
    pix: Arc<Mutex<Option<Pixbuf>>>,
    scaled: Arc<Mutex<Option<Pixbuf>>>,
    widget: DrawingArea,
    fname: &str,
) {
    if is_gif(&fname) {
        load_animation(pix, scaled, widget, &fname);
        return;
    }

    match Pixbuf::new_from_file(fname) {
        Ok(px) => {
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
    fname: &str,
) {
    let res = PixbufAnimation::new_from_file(fname);
    if res.is_err() {
        return;
    }
    let anim = res.unwrap();
    let iter = anim.get_iter(glib::get_current_time());

    gtk::timeout_add(iter.get_delay_time() as u32, move || {
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

pub fn is_gif(fname: &str) -> bool {
    let p = &Path::new(fname);
    if !p.is_file() {
        return false;
    }

    if let Ok(info) = gio::File::new_for_path(&p).query_info(
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
