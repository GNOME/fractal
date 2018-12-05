use i18n::i18n;

use std::fs;

use gtk;
use gtk::prelude::*;
use gtk::ResponseType;

use dirs;

use appop::AppOp;
use widgets::ErrorDialog;

impl AppOp {
    pub fn save_file_as(&self, src: String, name: String) {
        let main_window = self
            .ui
            .builder
            .get_object::<gtk::Window>("main_window")
            .expect("Cant find main_window in ui file.");

        let file_chooser = gtk::FileChooserNative::new(
            Some(i18n("Save media as").as_str()),
            Some(&main_window),
            gtk::FileChooserAction::Save,
            Some(i18n("_Save").as_str()),
            Some(i18n("_Cancel").as_str()),
        );

        file_chooser.set_current_folder(dirs::download_dir().unwrap_or_default());
        file_chooser.set_current_name(&name);

        let parent_weak = main_window.downgrade();
        file_chooser.connect_response(move |fcd, res| {
            if ResponseType::from(res) == ResponseType::Accept {
                if let Err(_) = fs::copy(src.clone(), fcd.get_filename().unwrap_or_default()) {
                    let msg = i18n("Could not save the file");
                    let parent = upgrade_weak!(parent_weak);
                    ErrorDialog::new(&parent, &msg);
                }
            }
        });

        file_chooser.run();
    }
}
