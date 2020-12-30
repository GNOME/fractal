use super::UI;
use crate::config;
use crate::util::i18n::i18n;
use gtk::prelude::*;

impl UI {
    pub fn about_dialog(&self) {
        let program_name = format!("Fractal{}", config::NAME_SUFFIX);

        let dialog = gtk::AboutDialog::new();
        dialog.set_logo_icon_name(Some(config::APP_ID));
        dialog.set_comments(Some(i18n("A Matrix.org client for GNOME").as_str()));
        dialog.set_copyright(Some(
            i18n("© 2017–2020 Daniel García Moreno, et al.").as_str(),
        ));
        dialog.set_license_type(gtk::License::Gpl30);
        dialog.set_modal(true);
        dialog.set_version(Some(config::VERSION));
        dialog.set_program_name(&program_name);
        dialog.set_website(Some("https://wiki.gnome.org/Fractal"));
        dialog.set_website_label(Some(i18n("Learn more about Fractal").as_str()));
        dialog.set_translator_credits(Some(i18n("translator-credits").as_str()));
        dialog.set_transient_for(Some(&self.main_window));

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
            d.close();
        });

        dialog.show();
    }
}
