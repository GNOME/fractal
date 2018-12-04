#![deny(unused_extern_crates)]
extern crate gdk;
extern crate gio;
extern crate glib;
extern crate gtk;
extern crate sourceview;

extern crate dirs;
extern crate gdk_pixbuf;
extern crate itertools;
extern crate rand;
extern crate regex;

extern crate gstreamer as gst;
extern crate gstreamer_player as gst_player;

#[macro_use]
extern crate log;

#[macro_use]
extern crate failure;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate fractal_matrix_api as fractal_api;

extern crate html2pango;

extern crate libhandy;

extern crate gettextrs;

extern crate cairo;
extern crate chrono;
extern crate comrak;
extern crate letter_avatar;
extern crate notify_rust;
extern crate pango;
extern crate secret_service;
extern crate tree_magic;
extern crate url;

extern crate fragile;

extern crate mdl;
#[macro_use]
extern crate lazy_static;

use fractal_api::backend;
use fractal_api::error;
use fractal_api::types;

mod globals;
mod i18n;
#[macro_use]
mod util;
mod cache;
mod passwd;
mod static_resources;
mod uibuilder;
mod uitypes;
#[macro_use]
mod app;
mod widgets;
mod actions;

mod appop;

use app::App;

fn main() {
    static_resources::init().expect("GResource initialization failed.");
    gst::init().expect("Error initializing gstreamer");
    App::new();
}
