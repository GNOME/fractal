use crate::error::Error;
use crate::globals::CACHE_PATH;
use failure::format_err;
use failure::Error as FailError;
use gdk::prelude::*;
use gdk_pixbuf::Pixbuf;
use gio::{Settings, SettingsExt, SettingsSchemaSource};
use html2pango::{html_escape, markup_links};
use log::error;
use std::fs::create_dir_all;
use std::sync::mpsc::SendError;

pub mod glib_thread_prelude {
    pub use crate::error::Error;
    pub use std::sync::mpsc::channel;
    pub use std::sync::mpsc::TryRecvError;
    pub use std::sync::mpsc::{Receiver, Sender};
    pub use std::thread;
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
            Err(TryRecvError::Empty) => Continue(true),
            Err(TryRecvError::Disconnected) => {
                error!("glib_thread error");
                Continue(false)
            }
            Ok(output) => {
                $glib_code(output);
                Continue(false)
            }
        });
    }};
}

pub fn cache_dir_path(dir: Option<&str>, name: &str) -> Result<String, Error> {
    let path = CACHE_PATH.join(dir.unwrap_or_default());

    if !path.is_dir() {
        create_dir_all(&path)?;
    }

    path.join(name)
        .to_str()
        .map(Into::into)
        .ok_or(Error::CacheError)
}

pub fn get_pixbuf_data(pb: &Pixbuf) -> Result<Vec<u8>, FailError> {
    let image = cairo::ImageSurface::create(cairo::Format::ARgb32, pb.get_width(), pb.get_height())
        .or_else(|_| Err(format_err!("Cairo Error")))?;

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
    SettingsSchemaSource::get_default()
        .and_then(|s| s.lookup("org.gnome.Fractal", true))
        .and_then(|_| {
            let settings: Settings = Settings::new("org.gnome.Fractal");
            Some(settings.get_boolean("markdown-active"))
        })
        .unwrap_or_default()
}

pub fn set_markdown_schema(md: bool) {
    if SettingsSchemaSource::get_default()
        .and_then(|s| s.lookup("org.gnome.Fractal", true))
        .is_some()
    {
        let settings: Settings = Settings::new("org.gnome.Fractal");
        if let Err(err) = settings.set_boolean("markdown-active", md) {
            error!("Can't save markdown active state: {:?}", err);
        }
    }
}

/* Macro for upgrading a weak reference or returning the given value
 *
 * This works for glib/gtk objects as well as anything else providing an upgrade method */
macro_rules! upgrade_weak {
    ($x:expr, $r:expr) => {{
        match $x.upgrade() {
            Some(o) => o,
            None => return $r,
        }
    }};
    ($x:expr) => {
        upgrade_weak!($x, ())
    };
}

macro_rules! unwrap_or_unit_return {
    ($x:expr) => {
        match $x {
            Some(a) => a,
            None => return,
        }
    };
}

pub trait ResultExpectLog {
    fn expect_log(&self, log: &str);
}

impl<T> ResultExpectLog for Result<(), SendError<T>> {
    fn expect_log(&self, log: &str) {
        if self.is_err() {
            error!("{}", log);
        }
    }
}
