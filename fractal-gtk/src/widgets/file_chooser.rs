pub fn save(parent: &gtk::widget, title: &str, source: String) {
    let file_chooser = gtk::FileChooserNative::new(
        Some(title),
        Some(parent),
        gtk::FileChooserAction::Save,
        Some(i18n("_Save").as_str()),
        Some(i18n("_Cancel").as_str()),
        );

    file_chooser.set_current_folder(dirs::download_dir().unwrap_or_default());

    file_chooser.connect_response(move |fcd, res| {
        if ResponseType::from(res) == ResponseType::Accept {
            if let Err(_) = fs::copy(source.clone(), fcd.get_filename().unwrap_or_default()) {
                let msg = i18n("Could not save the file");
                ErrorDialog::new(false, &msg);
            }
        }
    });

    file_chooser.run();
}

pub fn open(parent: &gtk::widget, title: &str, source: String) {
    let file_chooser = gtk::FileChooserNative::new(
        Some(&title),
        Some(&window),
        gtk::FileChooserAction::Open,
        Some(i18n("Select").as_str()),
        Some(i18n("_Cancel").as_str()),
        );
    file_chooser.connect_response(move |fcd, res| {
        if ResponseType::from(res) == ResponseType::Accept {
        }
    });

    file_chooser.run();
}
