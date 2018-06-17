extern crate gtk;
extern crate gettextrs;

use std::env;
use std::fs;

use self::gtk::prelude::*;
use self::gtk::ResponseType;
use self::gettextrs::gettext;

use glib;

use app::App;
use appop::AppOp;

impl AppOp {
    pub fn save_file_as(&self, src: String, name: String) {
        let main_window = self.ui.builder
            .get_object::<gtk::ApplicationWindow>("main_window")
            .expect("Cant find main_window in ui file.");

        let file_chooser = gtk::FileChooserDialog::new(
            Some(&gettext("Save media as")),
            Some(&main_window),
            gtk::FileChooserAction::Save,
        );

        file_chooser.set_modal(true);
        file_chooser.add_buttons(&[
            (&gettext("_Cancel"), ResponseType::Cancel.into()),
            (&gettext("_Save"), ResponseType::Accept.into()),
        ]);
        file_chooser.set_current_folder(env::home_dir().unwrap_or_default());
        file_chooser.set_current_name(&name);

        file_chooser.connect_response(move |fcd, res| {
            if ResponseType::from(res) == ResponseType::Accept {
                if fcd.get_filename().unwrap_or_default().exists() {
                    let confirm_dialog = gtk::MessageDialog::new(
                        Some(fcd),
                        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
                        gtk::MessageType::Question,
                        gtk::ButtonsType::YesNo,
                        &gettext("Do you want to overwrite the file?")
                    );

                    confirm_dialog.connect_response(clone!(fcd, src => move |cd, res| {
                        if ResponseType::from(res) == ResponseType::Yes {
                            if let Err(_) = fs::copy(src.clone(), fcd.get_filename().unwrap_or_default()) {
                                let msg = gettext("Could not save the file");
                                APPOP!(show_error, (msg));
                            }
                            cd.destroy();
                            fcd.destroy();
                        } else {
                            cd.destroy();
                        }
                    }));

                    confirm_dialog.show_all();
                } else {
                    if let Err(_) = fs::copy(src.clone(), fcd.get_filename().unwrap_or_default()) {
                        let msg = gettext("Could not save the file");
                        APPOP!(show_error, (msg));
                    }
                    fcd.destroy();
                }
            } else {
                fcd.destroy();
            }
        });

        file_chooser.show_all();
    }
}
