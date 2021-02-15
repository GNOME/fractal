mod application;
#[rustfmt::skip]
mod config;
mod window;

use application::ExampleApplication;
use config::{GETTEXT_PACKAGE, LOCALEDIR, RESOURCES_FILE};
use gettextrs::*;
use gtk::gdk::Display;
use gtk::gio;
use gtk::IconTheme;

fn main() {
    // Initialize logger, debug is carried out via debug!, info!, and warn!.
    pretty_env_logger::init();

    // Prepare i18n
    setlocale(LocaleCategory::LcAll, "");
    bindtextdomain(GETTEXT_PACKAGE, LOCALEDIR);
    textdomain(GETTEXT_PACKAGE);

    gtk::glib::set_application_name("Fractal");
    gtk::glib::set_prgname(Some("fractal"));

    gtk::init().expect("Unable to start GTK4");

    let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
    gio::resources_register(&res);

    IconTheme::get_for_display(&Display::get_default().unwrap())
        .unwrap()
        .add_resource_path("/org/gnome/FractalNext/icons");

    let app = ExampleApplication::new();
    app.run();
}
