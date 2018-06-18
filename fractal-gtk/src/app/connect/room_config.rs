extern crate gtk;
use self::gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_room_config(&self) {
        let op = &self.op;
        let back = self.ui.builder
            .get_object::<gtk::Button>("room_settings_back_button")
            .expect("Can't find room_settings_back_button in ui file.");
        let name_btn = self.ui.builder
            .get_object::<gtk::Button>("room_settings_room_name_button")
            .expect("Can't find room_settings_room_name_button in ui file.");
        let name_entry = self.ui.builder
            .get_object::<gtk::Entry>("room_settings_room_name_entry")
            .expect("Can't find room_settings_room_name_entry in ui file.");
        let topic_btn = self.ui.builder
            .get_object::<gtk::Button>("room_settings_room_topic_button")
            .expect("Can't find room_settings_room_topic_button in ui file.");
        let topic_entry = self.ui.builder
            .get_object::<gtk::Entry>("room_settings_room_topic_entry")
            .expect("Can't find room_settings_room_topic_entry in ui file.");

        /* Headerbar */
        back.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().close_room_settings();
        }));

        let button = name_btn.clone();
        name_entry.connect_property_text_notify(clone!(op => move |w| {
            let lock = op.try_lock();
            if let Ok(guard) = lock {
                let result = guard.validate_room_name(w.get_text());
                if result.is_some() {
                    button.show();
                    return;
                }
            }
            button.hide();
        }));

        let button = topic_btn.clone();
        topic_entry.connect_property_text_notify(clone!(op => move |w| {
            let lock = op.try_lock();
            if let Ok(guard) = lock {
                let result = guard.validate_room_topic(w.get_text());
                if result.is_some() {
                    button.show();
                    return;
                }
            }
            button.hide();
        }));

        let button = name_btn.clone();
        name_entry.connect_activate(move |_w| {
            let _ = button.emit("clicked", &[]);
        });

        name_btn.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().update_room_name();
        }));

        let button = topic_btn.clone();
        topic_entry.connect_activate(move |_w| {
            let _ = button.emit("clicked", &[]);
        });

        topic_btn.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().update_room_topic();
        }));

        /* Connect avatar button */
        let avatar_btn = self.ui.builder
            .get_object::<gtk::Button>("room_settings_avatar_button")
            .expect("Can't find room_settings_avatar_button in ui file.");

        let builder = &self.ui.builder;
        avatar_btn.connect_clicked(clone!(op, builder => move |_| {
            let window = builder
                .get_object::<gtk::Window>("main_window")
                .expect("Can't find main_window in ui file.");
            let file_chooser = gtk::FileChooserNative::new("Pick a new room avatar", Some(&window), gtk::FileChooserAction::Open, Some("Select"), None);
            /* http://gtk-rs.org/docs/gtk/struct.FileChooser.html */
            let result = gtk::NativeDialog::run(&file_chooser.clone().upcast::<gtk::NativeDialog>());
            if gtk::ResponseType::from(result) == gtk::ResponseType::Accept {
                if let Some(file) = file_chooser.get_filename() {
                    if let Some(path) = file.to_str() {
                        op.lock().unwrap().update_room_avatar(String::from(path));
                    }
                }
            }
        }));
    }
}
