use crate::i18n::i18n;

use gtk;
use gtk::prelude::*;

use crate::appop::AppOp;
use crate::config;

impl AppOp {
    pub fn about_dialog(&self) {
        let window: gtk::ApplicationWindow = self
            .ui
            .builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");

        let program_name = format!("Fractal{}", config::NAME_SUFFIX);

        let dialog = gtk::AboutDialog::new();
        dialog.set_logo_icon_name(config::APP_ID);
        dialog.set_comments(i18n("A Matrix.org client for GNOME").as_str());
        dialog.set_copyright(i18n("© 2017–2018 Daniel García Moreno, et al.").as_str());
        dialog.set_license_type(gtk::License::Gpl30);
        dialog.set_modal(true);
        dialog.set_version(config::VERSION);
        dialog.set_program_name(&program_name);
        dialog.set_website("https://wiki.gnome.org/Fractal");
        dialog.set_website_label(i18n("Learn more about Fractal").as_str());
        dialog.set_translator_credits(i18n("translator-credits").as_str());
        dialog.set_transient_for(&window);

        dialog.set_artists(&["Tobias Bernard"]);

        dialog.set_authors(&[
            "Daniel García Moreno",
            "Jordan Petridis",
            "Alexandre Franke",
            "Saurav Sachidanand",
            "Julian Sparber",
            "Eisha Chen-yen-su",
            "Christopher Davis",
        ]);

        dialog.add_credit_section(i18n("Name by").as_str(), &["Regina Bíró"]);
        dialog.connect_response(move |d, _| {
            d.destroy();
        });

        dialog.show();
    }
}
