use gtk;
use gtk::prelude::*;
use gtk::ResponseType;
use i18n::i18n;
use std::path::PathBuf;

pub fn save(parent: &gtk::Window, title: &str) -> Option<PathBuf> {
    let file_chooser = gtk::FileChooserNative::new(
        Some(i18n("Save media as").as_str()),
        Some(parent),
        gtk::FileChooserAction::Save,
        Some(i18n("_Save").as_str()),
        Some(i18n("_Cancel").as_str()),
    );

    file_chooser.set_current_folder(dirs::download_dir().unwrap_or_default());
    file_chooser.set_current_name(title);
    let response = file_chooser.run();
    if ResponseType::from(response) == ResponseType::Accept {
        return file_chooser.get_filename();
    }
    None
}

pub fn open(parent: &gtk::Window, title: &str) -> Option<PathBuf> {
    let file_chooser = gtk::FileChooserNative::new(
        Some(title),
        Some(parent),
        gtk::FileChooserAction::Open,
        Some(i18n("_Select").as_str()),
        Some(i18n("_Cancel").as_str()),
    );

    let response = file_chooser.run();
    if gtk::ResponseType::from(response) == gtk::ResponseType::Accept {
        return file_chooser.get_filename();
    }
    None
}
