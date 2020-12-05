use glib::clone;
use gtk::prelude::*;

use crate::app::{self, App};

impl App {
    pub fn connect_leave_room_dialog(&self) {
        let dialog = self
            .ui
            .builder
            .get_object::<gtk::Dialog>("leave_room_dialog")
            .expect("Can't find leave_room_dialog in ui file.");
        let cancel = self
            .ui
            .builder
            .get_object::<gtk::Button>("leave_room_cancel")
            .expect("Can't find leave_room_cancel in ui file.");
        let confirm = self
            .ui
            .builder
            .get_object::<gtk::Button>("leave_room_confirm")
            .expect("Can't find leave_room_confirm in ui file.");

        cancel.connect_clicked(clone!(@strong dialog => move |_| {
            dialog.hide();
        }));
        dialog.connect_delete_event(clone!(@strong dialog => move |_, _| {
            dialog.hide();
            glib::signal::Inhibit(true)
        }));

        confirm.connect_clicked(clone!(@strong dialog => move |_| {
            dialog.hide();
            let _ = app::get_app_tx().send(Box::new(|op| op.really_leave_active_room()));
        }));
    }
}
