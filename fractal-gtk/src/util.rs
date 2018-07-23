extern crate cairo;
extern crate gdk;
extern crate gdk_pixbuf;

use self::gdk_pixbuf::Pixbuf;
use self::gdk_pixbuf::PixbufExt;
use failure::Error;
use self::gdk::ContextExt;
use gio::{SettingsExt, Settings, SettingsSchemaSource};

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


pub fn get_markdown_schema() -> bool {
    if let Some(source) = SettingsSchemaSource::get_default() {
        if let Some(_schema) = source.lookup("org.gnome.Fractal", true) {
            let settings: Settings = Settings::new("org.gnome.Fractal");

            settings.get_boolean("markdown-active")
        } else {
            false
        }
    } else {
        false
    }
}

pub fn set_markdown_schema(md: bool) {
    if let Some(source) = SettingsSchemaSource::get_default() {
        if let Some(_schema) = source.lookup("org.gnome.Fractal", true) {
            let settings: Settings = Settings::new("org.gnome.Fractal");

            settings.set_boolean("markdown-active", md);
        }
    }
}
