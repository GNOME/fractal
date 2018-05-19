extern crate gtk;
use self::gtk::prelude::*;

use glib;
//use std::sync::{Arc, Mutex};

use app::App;

impl App {
    pub fn connect_account_settings(&self) {
        let op = &self.op;
        let builder = &self.ui.builder;
        let cancel = self.ui.builder
            .get_object::<gtk::Button>("cancel_account_settings")
            .expect("Can't find cancel_account_settings in ui file.");
        let confirm = self.ui.builder
            .get_object::<gtk::Button>("apply_account_settings")
            .expect("Can't find join_room_button in ui file.");
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("account_settings_dialog")
            .expect("Can't find account_settings_dialog in ui file.");
        let advanced_toggle = self.ui.builder
            .get_object::<gtk::EventBox>("account_settings_advanced_toggle")
            .expect("Can't find account_settings_advanced_toggle in ui file.");
        let delete_toggle = self.ui.builder
            .get_object::<gtk::EventBox>("account_settings_delete_toggle")
            .expect("Can't find account_settings_delete_toggle in ui file.");


        dialog.connect_delete_event(clone!(op => move |_, _| {
            op.lock().unwrap().close_account_settings_dialog();
            glib::signal::Inhibit(true)
        }));
        cancel.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().close_account_settings_dialog();
        }));

        confirm.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().apply_account_settings();
            op.lock().unwrap().close_account_settings_dialog();
        }));

        advanced_toggle.connect_button_press_event(clone!(builder => move |this, _| {
            let widget = builder
                .get_object::<gtk::Revealer>("account_settings_advanced")
                .expect("Can't find account_settings_advanced in ui file.");
            if widget.get_reveal_child() {
                this.get_style_context().unwrap().remove_class("advanced_revealer_divider");
                widget.set_reveal_child(false);
            }
            else {
                this.get_style_context().unwrap().add_class("advanced_revealer_divider");
                widget.set_reveal_child(true);
            }
            glib::signal::Inhibit(false)
        }));

        delete_toggle.connect_button_press_event(clone!(builder => move |this, _| {
            let widget = builder
                .get_object::<gtk::Revealer>("account_settings_delete")
                .expect("Can't find account_settings_delete in ui file.");
            if widget.get_reveal_child() {
                this.get_style_context().unwrap().remove_class("advanced_revealer_divider");
                widget.set_reveal_child(false);
            }
            else {
                this.get_style_context().unwrap().add_class("advanced_revealer_divider");
                widget.set_reveal_child(true);
            }
            glib::signal::Inhibit(false)
        }));
        /*
           invite.set_sensitive(false);
           invite.connect_clicked(clone!(op => move |_| {
           op.lock().unwrap().start_chat();
           }));
           */
    }
}
