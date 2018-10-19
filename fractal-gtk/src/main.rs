#![deny(unused_extern_crates)]
extern crate glib;
extern crate gio;
extern crate gtk;
extern crate gdk;
extern crate sourceview;

extern crate regex;
extern crate gdk_pixbuf;
extern crate rand;
extern crate itertools;
extern crate dirs;

extern crate gstreamer as gst;
extern crate gstreamer_player as gst_player;

#[macro_use]
extern crate log;

#[macro_use]
extern crate failure;

#[macro_use]
extern crate serde_derive;
extern crate serde_json;

#[macro_use]
extern crate fractal_matrix_api as fractal_api;

extern crate html2pango;

extern crate libhandy;

extern crate gettextrs;

extern crate secret_service;
extern crate cairo;
extern crate pango;
extern crate url;
extern crate tree_magic;
extern crate chrono;
extern crate comrak;
extern crate notify_rust;

extern crate fragile;

extern crate mdl;
#[macro_use]
extern crate lazy_static;

use fractal_api::backend;
use fractal_api::types;
use fractal_api::error;

mod i18n;
mod globals;
#[macro_use]
mod util;
mod cache;
mod uitypes;
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
    gst::init().expect("Error initializing gstreamer");
    App::new();
}
