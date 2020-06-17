use crate::i18n::i18n;
use gtk::prelude::*;
use gtk::ResponseType;
use std::path::PathBuf;

pub fn save(parent: &gtk::Window, title: &str, filter: &[gtk::FileFilter]) -> Option<PathBuf> {
    let file_chooser = gtk::FileChooserNative::new(
        Some(i18n("Save media as").as_str()),
        Some(parent),
        gtk::FileChooserAction::Save,
        Some(i18n("_Save").as_str()),
        Some(i18n("_Cancel").as_str()),
    );
    for f in filter {
        file_chooser.add_filter(f);
    }

    file_chooser.set_current_folder(dirs::download_dir().unwrap_or_default());
    file_chooser.set_current_name(title);
    let response = file_chooser.run();
    if response == ResponseType::Accept {
        return file_chooser.get_filename();
    }
    None
}

pub fn open(parent: &gtk::Window, title: &str, filter: &[gtk::FileFilter]) -> Option<PathBuf> {
    let file_chooser = gtk::FileChooserNative::new(
        Some(title),
        Some(parent),
        gtk::FileChooserAction::Open,
        Some(i18n("_Select").as_str()),
        Some(i18n("_Cancel").as_str()),
    );

    for f in filter {
        file_chooser.add_filter(f);
    }

    let response = file_chooser.run();
    if response == gtk::ResponseType::Accept {
        return file_chooser.get_filename();
    }
    None
}
