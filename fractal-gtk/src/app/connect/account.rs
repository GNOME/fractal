extern crate gtk;
use self::gtk::prelude::*;

use glib;

use app::App;

impl App {
    pub fn connect_account_settings(&self) {
        let op = &self.op;
        let builder = &self.ui.builder;
        let back = self.ui.builder
            .get_object::<gtk::Button>("account_settings_back_button")
            .expect("Can't find account_settings_back_button in ui file.");
        let cancel_password = self.ui.builder
            .get_object::<gtk::Button>("password-dialog-cancel")
            .expect("Can't find password-dialog-cancel in ui file.");
        let confirm_password = self.ui.builder
            .get_object::<gtk::Button>("password-dialog-apply")
            .expect("Can't find password-dialog-apply in ui file.");
        let password_dialog = self.ui.builder
            .get_object::<gtk::Dialog>("password_dialog")
            .expect("Can't find password_dialog in ui file.");
        let advanced_toggle = self.ui.builder
            .get_object::<gtk::EventBox>("account_settings_advanced_toggle")
            .expect("Can't find account_settings_advanced_toggle in ui file.");
        let delete_toggle = self.ui.builder
            .get_object::<gtk::EventBox>("account_settings_delete_toggle")
            .expect("Can't find account_settings_delete_toggle in ui file.");
        let delete_revealer = self.ui.builder
            .get_object::<gtk::Revealer>("account_settings_delete")
            .expect("Can't find account_settings_advanced_delete in ui file.");
        let advanced_revealer = self.ui.builder
            .get_object::<gtk::Revealer>("account_settings_advanced")
            .expect("Can't find account_settings_advanced_advanced in ui file.");
        let avatar_btn = self.ui.builder
            .get_object::<gtk::Button>("account_settings_avatar_button")
            .expect("Can't find account_settings_avatar_button in ui file.");
        let name_entry = self.ui.builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let name_btn = self.ui.builder
            .get_object::<gtk::Button>("account_settings_name_button")
            .expect("Can't find account_settings_name_button in ui file.");
        let password_btn = self.ui.builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let old_password = self.ui.builder
            .get_object::<gtk::Entry>("password-dialog-old-entry")
            .expect("Can't find password-dialog-old-entry in ui file.");
        let new_password = self.ui.builder
            .get_object::<gtk::Entry>("password-dialog-entry")
            .expect("Can't find password-dialog-entry in ui file.");
        let verify_password = self.ui.builder
            .get_object::<gtk::Entry>("password-dialog-verify-entry")
            .expect("Can't find password-dialog-verify-entry in ui file.");
        let destruction_entry = self.ui.builder
            .get_object::<gtk::Entry>("account_settings_delete_password_confirm")
            .expect("Can't find account_settings_delete_password_confirm in ui file.");
        let destruction_btn = self.ui.builder
            .get_object::<gtk::Button>("account_settings_delete_btn")
            .expect("Can't find account_settings_delete_btn in ui file.");

        /* Headerbar */
        back.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().close_account_settings_dialog();
        }));

        /* Body */
        avatar_btn.connect_clicked(clone!(op, builder => move |_| {
            let window = builder
                .get_object::<gtk::Window>("main_window")
                .expect("Can't find main_window in ui file.");
            let file_chooser = gtk::FileChooserNative::new("Pick a new avatar", Some(&window), gtk::FileChooserAction::Open, Some("Select"), None);
            /* http://gtk-rs.org/docs/gtk/struct.FileChooser.html */
            let result = gtk::NativeDialog::run(&file_chooser.clone().upcast::<gtk::NativeDialog>());
            if gtk::ResponseType::from(result) == gtk::ResponseType::Accept {
                if let Some(file) = file_chooser.get_filename() {
                    if let Some(path) = file.to_str() {
                        op.lock().unwrap().update_avatar_account_settings(String::from(path));
                    }
                }
            }
        }));

        let button = name_btn.clone();
        name_entry.connect_property_text_notify(clone!(op => move |w| {
            if let Some(text) = w.get_text() {
                if text != "" {
                    let lock = op.try_lock();
                    let username = if let Ok(guard) = lock {
                        guard.username.clone()
                    }
                    else {
                        None
                    };
                    if let Some(username) = username {
                        if username == text {
                            button.hide();
                            return;
                        }
                    }
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

        name_btn.connect_clicked(clone!(op => move |_w| {
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
            }
            else {
                hint.show();
            }

            confirm_password.set_sensitive(matching && !empty);
        }

        /* Passsword dialog */
        password_btn.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().show_password_dialog();
        }));

        password_dialog.connect_delete_event(clone!(op => move |_, _| {
            op.lock().unwrap().close_password_dialog();
            glib::signal::Inhibit(true)
        }));

        /* Headerbar */
        cancel_password.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().close_password_dialog();
        }));

        confirm_password.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().set_new_password();
            op.lock().unwrap().close_password_dialog();
        }));

        /* Body */
        verify_password.connect_property_text_notify(clone!(builder => move |_| {
            validate_password_input(&builder.clone());
        }));
        new_password.connect_property_text_notify(clone!(builder => move |_| {
            validate_password_input(&builder.clone());
        }));
        old_password.connect_property_text_notify(clone!(builder => move |_| {
            validate_password_input(&builder)
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

        destruction_entry.connect_property_text_notify(clone!(destruction_btn => move |w| {
            if let Some(text) = w.get_text() {
                if text != "" {
                    destruction_btn.set_sensitive(true);
                    return;
                }
            }
            destruction_btn.set_sensitive(false);
        }));

        destruction_btn.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().account_destruction();
        }));

        let scroll = builder
            .get_object::<gtk::ScrolledWindow>("account_settings_scroll")
            .expect("Can't find account_settings_scroll in ui file.");
        delete_revealer.connect_size_allocate(clone!(scroll => move |_, _| {
            if let Some(adj) = scroll.get_vadjustment() {
                let bottom = adj.get_upper() - adj.get_page_size();
                adj.set_value(bottom);
            }
        }));
        advanced_revealer.connect_size_allocate(clone!(scroll => move |_, _| {
            if let Some(adj) = scroll.get_vadjustment() {
                let bottom = adj.get_upper() - adj.get_page_size();
                adj.set_value(bottom);
            }
        }));
    }
}
