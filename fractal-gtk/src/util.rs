extern crate cairo;
extern crate gtk;
extern crate gdk;
extern crate gdk_pixbuf;

use self::gtk::ImageExt;
use self::gtk::IconThemeExt;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::TryRecvError;
use backend::BKCommand;
use self::gdk_pixbuf::Pixbuf;
use self::gdk_pixbuf::PixbufExt;
use failure::Error;
use self::gdk::ContextExt;

use html2pango::{html_escape, markup_links};

pub mod glib_thread_prelude {
    pub use std::thread;
    pub use std::sync::mpsc::channel;
    pub use std::sync::mpsc::{Sender, Receiver};
    pub use std::sync::mpsc::TryRecvError;
    pub use error::Error;
}


#[macro_export]
macro_rules! glib_thread {
    ($type: ty, $thread: expr, $glib_code: expr) => {{
        let (tx, rx): (Sender<$type>, Receiver<$type>) = channel();
        thread::spawn(move || {
            let output = $thread();
            tx.send(output).unwrap();
        });

        gtk::timeout_add(50, move || match rx.try_recv() {
            Err(TryRecvError::Empty) => gtk::Continue(true),
            Err(TryRecvError::Disconnected) => {
                eprintln!("glib_thread error");
                gtk::Continue(false)
            }
            Ok(output) => {
                $glib_code(output);
                gtk::Continue(false)
            }
        });
    }}
}

pub fn get_pixbuf_data(pb: &Pixbuf) -> Result<Vec<u8>, Error> {
    let image = cairo::ImageSurface::create(cairo::Format::ARgb32,
                                            pb.get_width(),
                                            pb.get_height())
        .or(Err(format_err!("Cairo Error")))?;

    let g = cairo::Context::new(&image);
    g.set_source_pixbuf(pb, 0.0, 0.0);
    g.paint();

    let mut buf: Vec<u8> = Vec::new();
    image.write_to_png(&mut buf)?;
    Ok(buf)
}

pub fn markup_text(s: &str) -> String {
    markup_links(&html_escape(s))
}

/// If `path` starts with mxc this func download the img async, in other case the image is loaded
/// in the `image` widget scaled to size
pub fn load_thumb(backend: &Sender<BKCommand>, path: &str, img: &gtk::Image, size: (i32, i32)) {
    let pixbuf: Option<Pixbuf>;

    if path.starts_with("mxc:") {
        pixbuf = match gtk::IconTheme::get_default() {
            None => None,
            Some(i1) => match i1.load_icon("image-loading-symbolic", size.0, gtk::IconLookupFlags::empty()) {
                Err(_) => None,
                Ok(i2) => i2,
            }
        };

        // asyn load
        let (tx, rx): (Sender<String>, Receiver<String>) = channel();
        backend.send(BKCommand::GetThumbAsync(path.to_string(), tx)).unwrap();
        let im = img.clone();
        gtk::timeout_add(50, move || match rx.try_recv() {
            Err(TryRecvError::Empty) => gtk::Continue(true),
            Err(TryRecvError::Disconnected) => gtk::Continue(false),
            Ok(fname) => {
                let mut f = fname.clone();
                if let Ok(pix) = Pixbuf::new_from_file_at_scale(&f, size.0, size.1, true) {
                    im.set_from_pixbuf(&pix);
                } else {
                    im.set_from_file(f);
                }
                gtk::Continue(false)
            }
        });
    } else {
        pixbuf = Pixbuf::new_from_file_at_scale(path, size.0, size.1, true).ok();
    }

    if let Some(pix) = pixbuf {
        img.set_from_pixbuf(&pix);
    } else {
        img.set_from_file(path);
    }
}
