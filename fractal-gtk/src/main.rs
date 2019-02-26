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
mod widgets;

mod appop;

use std::env::args;
use std::error::Error;

use crate::app::App;
use gio::ApplicationExt;
use gio::ApplicationExtManual;

use log::Level;
use loggerv;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(not(debug_assertions))]
    {
        let clap_args = clap::App::new("app")
            .arg(
                clap::Arg::with_name("v")
                    .short("v")
                    .multiple("true")
                    .help("Sets the level of verbosity"),
            )
            .get_matches();

        loggerv::init_with_level(clap_args.occurrences_of("v"))
            .expect("Failed to initialize logger");
    }

    #[cfg(debug_assertions)]
    loggerv::init_with_level(Level::Info).expect("Failed to initialize logger");

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
