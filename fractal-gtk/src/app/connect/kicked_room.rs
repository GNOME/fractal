use fractal_api::clone;
use gtk;
use gtk::prelude::*;

use glib;

use crate::app::App;

impl App {
    pub fn connect_kicked_room_dialog(&self) {
        let dialog = self
            .ui
            .builder
            .get_object::<gtk::Dialog>("kicked_room_dialog")
            .expect("Can't find kicked_room_dialog in ui file.");
        let confirm = self
            .ui
            .builder
            .get_object::<gtk::Button>("kicked_room_confirm")
            .expect("Can't find kicked_room_confirm in ui file EEK.");

        dialog.connect_delete_event(clone!(dialog => move |_, _| {
            dialog.hide();
            glib::signal::Inhibit(true)
        }));

        confirm.connect_clicked(clone!(dialog => move |_| {
            dialog.hide();
        }));
    }
}
