use fractal_api::backend;
use fractal_api::error;
use fractal_api::types;

mod config;
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
use gio::prelude::*;
use gio::ApplicationExt;

#[cfg(debug_assertions)]
use log::Level;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(not(debug_assertions))]
    {
        let clap_args = clap::App::new("fractal")
            .version(env!("CARGO_PKG_VERSION"))
            .about("Matrix group messaging app")
            .arg(
                clap::Arg::with_name("v")
                    .short("v")
                    .multiple(true)
                    .help("Sets the level of verbosity"),
            )
            .get_matches();

        loggerv::init_with_verbosity(clap_args.occurrences_of("v"))
            .expect("Failed to initialize logger");
    }

    #[cfg(debug_assertions)]
    loggerv::init_with_level(Level::Info).expect("Failed to initialize logger");

    static_resources::init().expect("GResource initialization failed.");

    // Initialize GStreamer. This checks, among other things, what plugins are available
    gst::init()?;

    // Create a Application with default flags
    let application = gtk::Application::new(
        Some(config::APP_ID),
        gio::ApplicationFlags::HANDLES_COMMAND_LINE,
    )?;

    application.set_resource_base_path(Some("/org/gnome/Fractal"));

    application.connect_startup(|application| {
        App::on_startup(application);
    });

    application.connect_command_line(|app, command| {
        for arg in command.get_arguments() {
            match arg.to_str() {
                Some("-V") | Some("--version") => {
                    println!("{}", config::VERSION);
                    app.quit();
                }
                _ => {}
            };
        }

        0
    });

    application.run(&args().collect::<Vec<_>>());

    Ok(())
}
