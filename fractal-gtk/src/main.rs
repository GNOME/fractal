use fractal_matrix_api::backend;
use fractal_matrix_api::error;
use fractal_matrix_api::types;
use gstreamer as gst;
use gstreamer_player as gst_player;

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
mod widgets;

mod appop;

use std::env::args;
use std::error::Error;

use crate::app::App;
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
