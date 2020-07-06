use gio::ActionMapExt;
use glib::clone;
use gtk::prelude::*;

use crate::app::App;

use crate::actions::{AccountSettings, StateExt};

impl App {
    pub fn connect_account_settings(&self) {
        let op = &self.op;
        let login_data = unwrap_or_unit_return!(op.lock().unwrap().login_data.clone());
        let builder = &self.ui.builder;
        let cancel_password = self
            .ui
            .builder
            .get_object::<gtk::Button>("password-dialog-cancel")
            .expect("Can't find password-dialog-cancel in ui file.");
        let confirm_password = self
            .ui
            .builder
            .get_object::<gtk::Button>("password-dialog-apply")
            .expect("Can't find password-dialog-apply in ui file.");
        let password_dialog = self
            .ui
            .builder
            .get_object::<gtk::Dialog>("password_dialog")
            .expect("Can't find password_dialog in ui file.");
        let avatar_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_avatar_button")
            .expect("Can't find account_settings_avatar_button in ui file.");
        let name_entry = self
            .ui
            .builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let name_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_name_button")
            .expect("Can't find account_settings_name_button in ui file.");
        let password_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let old_password = self
            .ui
            .builder
            .get_object::<gtk::Entry>("password-dialog-old-entry")
            .expect("Can't find password-dialog-old-entry in ui file.");
        let new_password = self
            .ui
            .builder
            .get_object::<gtk::Entry>("password-dialog-entry")
            .expect("Can't find password-dialog-entry in ui file.");
        let verify_password = self
            .ui
            .builder
            .get_object::<gtk::Entry>("password-dialog-verify-entry")
            .expect("Can't find password-dialog-verify-entry in ui file.");
        let destruction_entry = self
            .ui
            .builder
            .get_object::<gtk::Entry>("account_settings_delete_password_confirm")
            .expect("Can't find account_settings_delete_password_confirm in ui file.");
        let destruction_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_delete_btn")
            .expect("Can't find account_settings_delete_btn in ui file.");

        let window = self.main_window.upcast_ref::<gtk::Window>();
        let actions = AccountSettings::new(
            &window,
            login_data.server_url,
            login_data.access_token,
            login_data.uid,
        );
        let container = self
            .ui
            .builder
            .get_object::<gtk::Box>("account_settings_box")
            .expect("Can't find account_settings_box in ui file.");
        container.insert_action_group("user-settings", Some(&actions));

        /* Body */
        if let Some(action) = actions.lookup_action("change-avatar") {
            action.bind_button_state(&avatar_btn);
            avatar_btn.set_action_name(Some("user-settings.change-avatar"));
            let avatar_spinner = self
                .ui
                .builder
                .get_object::<gtk::Spinner>("account_settings_avatar_spinner")
                .expect("Can't find account_settings_avatar_spinner in ui file.");
            avatar_btn.connect_property_sensitive_notify(
                clone!(@weak avatar_spinner as spinner => move |w| {
                    if w.get_sensitive() {
                        spinner.hide();
                        spinner.stop();
                    } else {
                        spinner.start();
                        spinner.show();
                    }
                }),
            );
        }

        let button = name_btn.clone();
        name_entry.connect_property_text_notify(clone!(@strong op => move |w| {
            if let Some(text) = w.get_text().filter(|text| !text.is_empty()) {
                if op.try_lock()
                    .ok()
                    .and_then(|guard| guard.login_data.clone())
                    .and_then(|login_data| login_data.username)
                    .filter(|username| *username != text)
                    .is_some()
                {
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

        name_btn.connect_clicked(clone!(@strong op => move |_w| {
            op.lock().unwrap().update_username_account_settings();
        }));

        /*
        fn update_password_strength(builder: &gtk::Builder) {
        let bar = builder
        .get_object::<gtk::LevelBar>("password-dialog-strength-indicator")
        .expect("Can't find password-dialog-strength-indicator in ui file.");
        let label = builder
        .get_object::<gtk::Label>("password-dialog-hint")
        .expect("Can't find password-dialog-hint in ui file.");
        let strength_level = 10f64;
        bar.set_value(strength_level);
        label.set_label("text");
        }
        */

        fn validate_password_input(builder: &gtk::Builder) {
            let hint = builder
                .get_object::<gtk::Label>("password-dialog-verify-hint")
                .expect("Can't find password-dialog-verify-hint in ui file.");
            let confirm_password = builder
                .get_object::<gtk::Button>("password-dialog-apply")
                .expect("Can't find password-dialog-apply in ui file.");
            let old = builder
                .get_object::<gtk::Entry>("password-dialog-old-entry")
                .expect("Can't find password-dialog-old-entry in ui file.");
            let new = builder
                .get_object::<gtk::Entry>("password-dialog-entry")
                .expect("Can't find password-dialog-entry in ui file.");
            let verify = builder
                .get_object::<gtk::Entry>("password-dialog-verify-entry")
                .expect("Can't find password-dialog-verify-entry in ui file.");

            let mut empty = true;
            let mut matching = true;
            if let Some(new) = new.get_text() {
                if let Some(verify) = verify.get_text() {
                    if let Some(old) = old.get_text() {
                        if new != verify {
                            matching = false;
                        }
                        if new != "" && verify != "" && old != "" {
                            empty = false;
                        }
                    }
                }
            }
            if matching {
                hint.hide();
            } else {
                hint.show();
            }

            confirm_password.set_sensitive(matching && !empty);
        }

        /* Passsword dialog */
        password_btn.connect_clicked(clone!(@strong op => move |_| {
            op.lock().unwrap().show_password_dialog();
        }));

        password_dialog.connect_delete_event(clone!(@strong op => move |_, _| {
            op.lock().unwrap().close_password_dialog();
            glib::signal::Inhibit(true)
        }));

        /* Headerbar */
        cancel_password.connect_clicked(clone!(@strong op => move |_| {
            op.lock().unwrap().close_password_dialog();
        }));

        confirm_password.connect_clicked(clone!(@strong op => move |_| {
            op.lock().unwrap().set_new_password();
            op.lock().unwrap().close_password_dialog();
        }));

        /* Body */
        verify_password.connect_property_text_notify(clone!(@strong builder => move |_| {
            validate_password_input(&builder.clone());
        }));
        new_password.connect_property_text_notify(clone!(@strong builder => move |_| {
            validate_password_input(&builder.clone());
        }));
        old_password.connect_property_text_notify(clone!(@strong builder => move |_| {
            validate_password_input(&builder)
        }));

        destruction_entry.connect_property_text_notify(
            clone!(@strong destruction_btn => move |w| {
                if let Some(text) = w.get_text() {
                    if text != "" {
                        destruction_btn.set_sensitive(true);
                        return;
                    }
                }
                destruction_btn.set_sensitive(false);
            }),
        );

        destruction_btn.connect_clicked(clone!(@strong op => move |_| {
            op.lock().unwrap().account_destruction();
        }));
    }
}
