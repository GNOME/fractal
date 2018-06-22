#![deny(unused_extern_crates)]
extern crate glib;
extern crate gio;
extern crate gtk;
extern crate gdk;

extern crate gstreamer as gst;
extern crate gstreamer_player as gst_player;

#[macro_use]
extern crate log;

#[macro_use]
extern crate failure;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate fractal_matrix_api as fractal_api;

extern crate html2pango;

extern crate gspell;

extern crate gettextrs;

extern crate chrono;

extern crate fragile;

use fractal_api::backend;
use fractal_api::types;
use fractal_api::error;

mod i18n;
mod globals;
#[macro_use]
mod util;
mod cache;
mod uibuilder;
mod static_resources;
mod passwd;
#[macro_use]
mod app;
mod widgets;

mod appop;

use app::App;


fn main() {
    static_resources::init().expect("GResource initialization failed.");
    App::new();
}
