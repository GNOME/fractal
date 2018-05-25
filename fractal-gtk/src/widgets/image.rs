extern crate cairo;
extern crate gtk;
extern crate glib;
extern crate gdk;
extern crate gdk_pixbuf;
extern crate tree_magic;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use self::gtk::prelude::*;
use self::gtk::DrawingArea;
use self::gdk_pixbuf::Pixbuf;
use self::gdk_pixbuf::PixbufExt;
use self::gdk::ContextExt;
use self::gdk_pixbuf::PixbufAnimation;
use self::gdk_pixbuf::PixbufAnimationExt;

use backend::BKCommand;
use std::sync::mpsc::TryRecvError;

pub struct Thumb(pub bool);

pub struct Circle(pub bool);

#[derive(Clone, Debug)]
pub struct Image {
    pub path: String,
    pub max_size: (i32, i32),
    pub widget: DrawingArea,
    pub backend: Sender<BKCommand>,
    pub pixbuf: Arc<Mutex<Option<Pixbuf>>>,
    pub thumb: bool,
    pub circle: bool,
}

impl Image {
    pub fn new(backend: &Sender<BKCommand>, path: &str, size: (i32, i32), Thumb(thumb): Thumb, Circle(circle): Circle) -> Image {
        let da = DrawingArea::new();
        let pixbuf = match gtk::IconTheme::get_default() {
            None => None,
            Some(i1) => match i1.load_icon("image-loading-symbolic", size.1, gtk::IconLookupFlags::empty()) {
                Err(_) => None,
                Ok(i2) => i2,
            }
        };

        let img = Image {
            path: path.to_string(),
            max_size: size,
            widget: da,
            pixbuf: Arc::new(Mutex::new(pixbuf)),
            thumb: thumb,
            circle: circle,
            backend: backend.clone(),
        };
        img.draw();
        img.load_async();

        img
    }

    pub fn draw(&self) {
        let da = &self.widget;

        let w = self.max_size.0;
        let h = self.max_size.1;

        da.set_hexpand(true);
        da.set_vexpand(false);

        if let Some(ref pb) = *self.pixbuf.lock().unwrap() {
            let w = pb.get_width();
            let h = pb.get_height();
            da.set_size_request(w, h);
        } else {
            da.set_size_request(w, h);
        }

        let pix = self.pixbuf.clone();

        if self.circle {
            da.connect_draw(move |da, g| {
                use std::f64::consts::PI;
                g.set_antialias(cairo::Antialias::Best);

                let s = if w > h {
                    w
                } else {
                    h
                };
                let width = s as f64;
                let height = s as f64;

                let context = da.get_style_context().unwrap();

                gtk::render_background(&context, g, 0.0, 0.0, width, height);

                if let Some(ref pb) = *pix.lock().unwrap() {
                    let hpos: f64 = (width - (pb.get_height()) as f64) / 2.0;

                    g.arc(width / 2.0, height / 2.0, width.min(height) / 2.0, 0.0, 2.0 * PI);
                    g.clip();

                    g.set_source_pixbuf(&pb, 0.0, hpos);
                    g.rectangle(0.0, 0.0, width, height);
                    g.fill();
                }

                Inhibit(false)
            });
        } else {
            da.connect_draw(move |da, g| {
                let width = w as f64;
                let height = h as f64;

                let mut rw = w;

                if let Some(p) = da.get_parent() {
                    let parent_width = p.get_allocated_width();
                    let max = parent_width - 50;
                    if max < w {
                        rw = max;
                    }
                }

                let context = da.get_style_context().unwrap();

                gtk::render_background(&context, g, 0.0, 0.0, width, height);

                if let Some(ref pb) = *pix.lock().unwrap() {
                    let mut pw = pb.get_width();
                    let mut ph = pb.get_height();

                    if pw > ph && pw > rw {
                        ph = rw * ph / pw;
                        pw = rw;
                    } else if ph >= pw && ph > h {
                        pw = h * pw / ph;
                        ph = h;
                    }
                    da.set_size_request(pw, ph);

                    if let Some(scaled) = pb.scale_simple(pw, ph, gdk_pixbuf::InterpType::Bilinear) {
                        g.set_source_pixbuf(&scaled, 0.0, 0.0);
                        g.rectangle(0.0, 0.0, pw as f64, ph as f64);
                        g.fill();
                    }
                }

                Inhibit(false)
            });
        }
    }

    /// If `path` starts with mxc this func download the img async, in other case the image is loaded
    /// in the `image` widget scaled to size
    pub fn load_async(&self) {
        if self.path.starts_with("mxc:") {
            // asyn load
            let (tx, rx): (Sender<String>, Receiver<String>) = channel();
            let command = match self.thumb {
                false => BKCommand::GetMediaAsync(self.path.to_string(), tx),
                true => BKCommand::GetThumbAsync(self.path.to_string(), tx),
            };
            self.backend.send(command).unwrap();
            let pix = self.pixbuf.clone();
            let da = self.widget.clone();
            gtk::timeout_add(50, move || match rx.try_recv() {
                Err(TryRecvError::Empty) => gtk::Continue(true),
                Err(TryRecvError::Disconnected) => gtk::Continue(false),
                Ok(fname) => {
                    load_pixbuf(pix.clone(), da.clone(), &fname);
                    gtk::Continue(false)
                }
            });
        } else {
            load_pixbuf(self.pixbuf.clone(), self.widget.clone(), &self.path);
        }
    }
}

pub fn load_pixbuf(pix: Arc<Mutex<Option<Pixbuf>>>, widget: DrawingArea, fname: &str) {
    if is_gif(&fname) {
        load_animation(pix.clone(), widget, &fname);
        return;
    }

    match Pixbuf::new_from_file(fname) {
        Ok(px) => { *pix.lock().unwrap() = Some(px); }
        _ => { *pix.lock().unwrap() = None; }
    };
}

pub fn load_animation(pix: Arc<Mutex<Option<Pixbuf>>>, widget: DrawingArea, fname: &str) {
    let res = PixbufAnimation::new_from_file(fname);
    if res.is_err() {
        return;
    }
    let anim = res.unwrap();
    let iter = anim.get_iter(&glib::get_current_time());

    gtk::timeout_add(iter.get_delay_time() as u32, move || {
        iter.advance(&glib::get_current_time());

        if widget.is_drawable() {
            let px = iter.get_pixbuf();
            *pix.lock().unwrap() = Some(px);
            widget.queue_draw();
        } else {
            return gtk::Continue(false);
        }
        gtk::Continue(true)
    });
}

pub fn is_gif(fname: &str) -> bool {
    let p = &Path::new(fname);
    if !p.is_file() {
        return false;
    }
    let result = tree_magic::from_filepath(p);
    result == "image/gif"
}
