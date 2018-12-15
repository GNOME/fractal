#![deny(unused_extern_crates)]
extern crate gdk;
extern crate gio;
#[macro_use]
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
mod actions;
mod row_data;
mod widgets;

mod appop;

use std::env::args;
use std::error::Error;

use app::App;
use gio::ApplicationExt;
use gio::ApplicationExtManual;

fn main() -> Result<(), Box<dyn Error>> {
    static_resources::init().expect("GResource initialization failed.");

    // Initialize GStreamer. This checks, among other things, what plugins are available
    gst::init()?;

    // Create a Application with default flags
    let appid = globals::APP_ID.unwrap_or("org.gnome.FractalDevel");
    let application = gtk::Application::new(appid, gio::ApplicationFlags::empty())?;

    application.set_property_resource_base_path(Some("/org/gnome/Fractal"));

    application.connect_startup(|application| {
        App::on_startup(application);
    });

    application.run(&args().collect::<Vec<_>>());

    Ok(())
}
