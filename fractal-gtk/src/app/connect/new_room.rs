use crate::clone;
use gtk::prelude::*;

use crate::app::App;

impl App {
    pub fn connect_new_room_dialog(&self) {
        let dialog = self
            .ui
            .builder
            .get_object::<libhandy::Dialog>("new_room_dialog")
            .expect("Can't find new_room_dialog in ui file.");
        let cancel = self
            .ui
            .builder
            .get_object::<gtk::Button>("cancel_new_room")
            .expect("Can't find cancel_new_room in ui file.");
        let confirm = self
            .ui
            .builder
            .get_object::<gtk::Button>("new_room_button")
            .expect("Can't find new_room_button in ui file.");
        let entry = self
            .ui
            .builder
            .get_object::<gtk::Entry>("new_room_name")
            .expect("Can't find new_room_name in ui file.");
        let private = self
            .ui
            .builder
            .get_object::<gtk::ToggleButton>("private_visibility_button")
            .expect("Can't find private_visibility_button in ui file.");

        private.set_active(true);
        cancel.connect_clicked(clone!(entry, dialog, private => move |_| {
            dialog.hide();
            entry.set_text("");
            private.set_active(true);
        }));
        dialog.connect_delete_event(clone!(entry, dialog, private => move |_, _| {
            dialog.hide();
            entry.set_text("");
            private.set_active(true);
            glib::signal::Inhibit(true)
        }));

        let op = self.op.clone();
        confirm.connect_clicked(clone!(entry, dialog, private => move |_| {
            dialog.hide();
            op.lock().unwrap().create_new_room();
            entry.set_text("");
            private.set_active(true);
        }));

        let op = self.op.clone();
        entry.connect_activate(clone!(dialog => move |entry| {
            dialog.hide();
            op.lock().unwrap().create_new_room();
            entry.set_text("");
            private.set_active(true);
        }));
        entry.connect_changed(clone!(confirm => move |entry| {
                confirm.set_sensitive(entry.get_buffer().get_length() > 0);
        }));
    }
}
