extern crate gtk;
extern crate gettextrs;

use self::gtk::prelude::*;
use self::gettextrs::gettext;

use appop::AppOp;
use globals;


impl AppOp {
    pub fn about_dialog(&self) {
        let window: gtk::ApplicationWindow = self.ui.builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");

        let dialog = gtk::AboutDialog::new();
        dialog.set_logo_icon_name(globals::APP_ID);
        dialog.set_comments(gettext("A Matrix.org client for GNOME").as_str());
        dialog.set_copyright(gettext("© 2017–2018 Daniel García Moreno, et al.").as_str());
        dialog.set_license_type(gtk::License::Gpl30);
        dialog.set_modal(true);
        dialog.set_version(env!("CARGO_PKG_VERSION"));
        dialog.set_program_name("Fractal");
        dialog.set_website("https://wiki.gnome.org/Fractal");
        dialog.set_website_label(gettext("Learn more about Fractal").as_str());
        dialog.set_transient_for(&window);

        dialog.set_artists(&[
            "Tobias Bernard",
        ]);

        dialog.set_authors(&[
            "Daniel García Moreno",
            "Jordan Petridis",
            "Alexandre Franke",
            "Saurav Sachidanand",
            "Julian Sparber",
        ]);

        dialog.add_credit_section(gettext("Name by").as_str(), &["Regina Bíró"]);

        dialog.show();
    }
}
