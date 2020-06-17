use fractal_api::backend::user;
use fractal_api::util::ResultExpectLog;
use gtk::prelude::*;
use log::info;
use std::path::PathBuf;
use std::thread;

use crate::app::App;
use crate::appop::AppOp;
use crate::appop::AppState;

use crate::backend::{BKCommand, BKResponse};
use crate::i18n::i18n;
use crate::widgets;
use crate::widgets::AvatarExt;

use crate::cache::download_to_cache;
use fractal_api::r0::contact::get_identifiers::ThirdPartyIdentifier;
use fractal_api::r0::Medium;

use super::LoginData;

impl AppOp {
    pub fn set_three_pid(&self, data: Option<Vec<ThirdPartyIdentifier>>) {
        self.update_address(data);
    }

    pub fn get_three_pid(&self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let tx = self.backend.clone();
        thread::spawn(move || {
            match user::get_threepid(login_data.server_url, login_data.access_token) {
                Ok(list) => {
                    let l = Some(list);
                    APPOP!(set_three_pid, (l));
                }
                Err(err) => {
                    tx.send(BKCommand::SendBKResponse(BKResponse::GetThreePIDError(err)))
                        .expect_log("Connection closed");
                }
            }
        });
    }

    pub fn added_three_pid(&self) {
        self.get_three_pid();
    }

    pub fn valid_phone_token(&self, sid: Option<String>, secret: Option<String>) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        if let Some(sid) = sid {
            if let Some(secret) = secret {
                let tx = self.backend.clone();
                thread::spawn(move || {
                    match user::add_threepid(
                        login_data.server_url,
                        login_data.access_token,
                        login_data.identity_url,
                        secret,
                        sid,
                    ) {
                        Ok(_) => {
                            APPOP!(added_three_pid);
                        }
                        Err(err) => {
                            tx.send(BKCommand::SendBKResponse(BKResponse::AddThreePIDError(err)))
                                .expect_log("Connection closed");
                        }
                    }
                });
            }
        } else {
            self.show_error_dialog_in_settings(i18n("The validation code is not correct."));
        }
    }

    pub fn show_phone_dialog(&self, sid: String, secret: String) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let parent = self
            .ui
            .builder
            .get_object::<gtk::Window>("main_window")
            .expect("Can't find main_window in ui file.");

        let entry = gtk::Entry::new();
        let msg = i18n("Enter the code received via SMS");
        let flags = gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT;
        let dialog = gtk::MessageDialog::new(
            Some(&parent),
            flags,
            gtk::MessageType::Error,
            gtk::ButtonsType::None,
            &msg,
        );
        if let Some(area) = dialog.get_message_area() {
            if let Ok(area) = area.downcast::<gtk::Box>() {
                area.add(&entry);
            }
        }
        let backend = self.backend.clone();
        dialog.add_button(&i18n("Cancel"), gtk::ResponseType::Cancel);
        let button = dialog.add_button(&i18n("Continue"), gtk::ResponseType::Ok);
        button.set_sensitive(false);
        let ok = button.clone();
        entry.connect_activate(move |_| {
            if ok.get_sensitive() {
                let _ = ok.emit("clicked", &[]);
            }
        });

        entry.connect_property_text_notify(move |w| {
            if let Some(text) = w.get_text() {
                if text != "" {
                    button.set_sensitive(true);
                    return;
                }
            }
            button.set_sensitive(false);
        });

        let value = entry;
        dialog.connect_response(move |w, r| {
            if let gtk::ResponseType::Ok = r {
                if let Some(token) = value.get_text().map(|gstr| gstr.to_string()) {
                    let server_url = login_data.server_url.clone();
                    let secret = secret.clone();
                    let sid = sid.clone();
                    let tx = backend.clone();
                    thread::spawn(move || {
                        match user::submit_phone_token(server_url, secret, sid, token) {
                            Ok((sid, secret)) => {
                                let secret = Some(secret);
                                APPOP!(valid_phone_token, (sid, secret));
                            }
                            Err(err) => {
                                tx.send(BKCommand::SendBKResponse(
                                    BKResponse::SubmitPhoneTokenError(err),
                                ))
                                .expect_log("Connection closed");
                            }
                        }
                    });
                }
            }
            w.destroy();
        });
        self.get_three_pid();
        dialog.show_all();
    }

    pub fn show_email_dialog(&self, sid: String, secret: String) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let parent = self
            .ui
            .builder
            .get_object::<gtk::Window>("main_window")
            .expect("Can't find main_window in ui file.");

        let msg = i18n("In order to add this email address, go to your inbox and follow the link you received. Once you’ve done that, click Continue.");
        let flags = gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT;
        let dialog = gtk::MessageDialog::new(
            Some(&parent),
            flags,
            gtk::MessageType::Error,
            gtk::ButtonsType::None,
            &msg,
        );
        let backend = self.backend.clone();
        dialog.add_button(&i18n("Cancel"), gtk::ResponseType::Cancel);
        dialog.add_button(&i18n("Continue"), gtk::ResponseType::Ok);
        dialog.connect_response(move |w, r| {
            if let gtk::ResponseType::Ok = r {
                let login_data = login_data.clone();
                let secret = secret.clone();
                let sid = sid.clone();
                let tx = backend.clone();
                thread::spawn(move || {
                    match user::add_threepid(
                        login_data.server_url,
                        login_data.access_token,
                        login_data.identity_url,
                        secret,
                        sid,
                    ) {
                        Ok(_) => {
                            APPOP!(added_three_pid);
                        }
                        Err(err) => {
                            tx.send(BKCommand::SendBKResponse(BKResponse::AddThreePIDError(err)))
                                .expect_log("Connection closed");
                        }
                    }
                });
            }
            w.destroy();
        });
        self.get_three_pid();
        dialog.show_all();
    }

    pub fn show_error_dialog_in_settings(&self, error: String) {
        let dialog = self.create_error_dialog(error);
        dialog.connect_response(move |w, _| w.destroy());
        self.get_three_pid();
        dialog.show_all();
    }

    pub fn show_load_settings_error_dialog(&self, error: String) {
        let dialog = self.create_error_dialog(error);
        dialog.connect_response(move |w, _| w.destroy());
        dialog.show_all();
    }

    pub fn create_error_dialog(&self, error: String) -> gtk::MessageDialog {
        let parent = self
            .ui
            .builder
            .get_object::<gtk::Window>("main_window")
            .expect("Can't find main_window in ui file.");

        let msg = error;
        let flags = gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT;
        let dialog = gtk::MessageDialog::new(
            Some(&parent),
            flags,
            gtk::MessageType::Error,
            gtk::ButtonsType::None,
            &msg,
        );

        dialog.add_button(&i18n("OK"), gtk::ResponseType::Ok);

        dialog
    }

    pub fn get_token_email(&mut self, sid: Option<String>, secret: Option<String>) {
        if let Some(sid) = sid {
            if let Some(secret) = secret {
                self.show_email_dialog(sid, secret);
            }
        }
    }

    pub fn get_token_phone(&mut self, sid: Option<String>, secret: Option<String>) {
        if let Some(sid) = sid {
            if let Some(secret) = secret {
                self.show_phone_dialog(sid, secret);
            }
        }
    }

    pub fn show_account_settings_dialog(&mut self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        // Reset view before displaying it
        self.close_account_settings_dialog();
        let avatar_spinner = self
            .ui
            .builder
            .get_object::<gtk::Spinner>("account_settings_avatar_spinner")
            .expect("Can't find account_settings_avatar_spinner in ui file.");
        let avatar_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_avatar_button")
            .expect("Can't find account_settings_avatar_button in ui file.");
        let name = self
            .ui
            .builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let name_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_name_button")
            .expect("Can't find account_settings_name_button in ui file.");
        let uid = self
            .ui
            .builder
            .get_object::<gtk::Label>("account_settings_uid")
            .expect("Can't find account_settings_uid in ui file.");
        let device_id = self
            .ui
            .builder
            .get_object::<gtk::Label>("account_settings_device_id")
            .expect("Can't find account_settings_device_id in ui file.");
        let homeserver = self
            .ui
            .builder
            .get_object::<gtk::Label>("account_settings_homeserver")
            .expect("Can't find account_settings_homeserver in ui file.");
        let advanced_box = self
            .ui
            .builder
            .get_object::<gtk::Box>("account_settings_advanced_box")
            .expect("Can't find account_settings_advanced_box in ui file.");
        let delete_box = self
            .ui
            .builder
            .get_object::<gtk::Box>("account_settings_delete_box")
            .expect("Can't find account_settings_delete_box in ui file.");
        let stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("account_settings_stack")
            .expect("Can't find account_settings_delete_box in ui file.");
        let destruction_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_delete_btn")
            .expect("Can't find account_settings_delete_btn in ui file.");
        let destruction_entry = self
            .ui
            .builder
            .get_object::<gtk::Entry>("account_settings_delete_password_confirm")
            .expect("Can't find account_settings_delete_password_confirm in ui file.");
        let password_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let password_btn_stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("account_settings_password_stack")
            .expect("Can't find account_settings_password_stack in ui file.");
        let destruction_flag = self
            .ui
            .builder
            .get_object::<gtk::CheckButton>("account_settings_delete_check")
            .expect("Can't find account_settings_delete_check in ui file.");

        stack.set_visible_child_name("loading");
        self.get_three_pid();
        uid.set_text(&login_data.uid.to_string());
        device_id.set_text(&self.device_id.clone().unwrap_or_default());
        homeserver.set_text(login_data.server_url.as_str());
        name.set_text(&login_data.username.unwrap_or_default());
        name.grab_focus_without_selecting();
        name.set_position(-1);

        avatar_spinner.hide();
        avatar_btn.set_sensitive(true);
        self.show_avatar();

        name_btn.hide();
        name.set_editable(true);
        let image = gtk::Image::new_from_icon_name(Some("emblem-ok-symbolic"), gtk::IconSize::Menu);
        name_btn.set_image(Some(&image));
        name_btn.set_sensitive(true);

        /* reset the password button */
        password_btn_stack.set_visible_child_name("label");
        password_btn.set_sensitive(true);

        destruction_flag.set_active(false);
        destruction_btn.set_sensitive(false);
        destruction_entry.set_text("");
        advanced_box.set_redraw_on_allocate(true);
        delete_box.set_redraw_on_allocate(true);

        self.set_state(AppState::AccountSettings);
    }

    pub fn update_address(&self, data: Option<Vec<ThirdPartyIdentifier>>) {
        let grid = self
            .ui
            .builder
            .get_object::<gtk::Grid>("account_settings_grid")
            .expect("Can't find account_settings_grid in ui file.");
        let email = self
            .ui
            .builder
            .get_object::<gtk::Box>("account_settings_email")
            .expect("Can't find account_settings_box_email in ui file.");
        let phone = self
            .ui
            .builder
            .get_object::<gtk::Box>("account_settings_phone")
            .expect("Can't find account_settings_box_phone in ui file.");
        let stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("account_settings_stack")
            .expect("Can't find account_settings_delete_box in ui file.");
        let password = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");

        let mut first_email = true;
        let mut first_phone = true;

        let mut i = 1;
        let mut child = grid.get_child_at(1, i);
        while child.is_some() {
            if let Some(child) = child.clone() {
                if child != phone && child != email && child != password {
                    grid.remove_row(i);
                } else {
                    for w in email.get_children().iter() {
                        email.remove(w);
                    }
                    for w in phone.get_children().iter() {
                        phone.remove(w);
                    }
                    i += 1;
                }
            }
            child = grid.get_child_at(1, i);
        }

        /* Make sure we have at least one empty entry for email and phone */
        let mut empty_email = widgets::Address::new(widgets::AddressType::Email, &self);
        let mut empty_phone = widgets::Address::new(widgets::AddressType::Phone, &self);
        email.pack_start(&empty_email.create(None), true, true, 0);
        phone.pack_start(&empty_phone.create(None), true, true, 0);
        if let Some(data) = data {
            for item in data {
                match item.medium {
                    Medium::Email => {
                        if first_email {
                            empty_email.update(Some(item.address));
                            let entry = widgets::Address::new(widgets::AddressType::Email, &self)
                                .create(None);
                            grid.insert_next_to(&email, gtk::PositionType::Bottom);
                            grid.attach_next_to(
                                &entry,
                                Some(&email),
                                gtk::PositionType::Bottom,
                                1,
                                1,
                            );
                            first_email = false;
                        } else {
                            let entry = widgets::Address::new(widgets::AddressType::Email, &self)
                                .create(Some(item.address));
                            grid.insert_next_to(&email, gtk::PositionType::Bottom);
                            grid.attach_next_to(
                                &entry,
                                Some(&email),
                                gtk::PositionType::Bottom,
                                1,
                                1,
                            );
                        }
                    }
                    Medium::MsIsdn => {
                        if first_phone {
                            empty_phone.update(Some(item.address));
                            let entry = widgets::Address::new(widgets::AddressType::Phone, &self)
                                .create(None);
                            grid.insert_next_to(&phone, gtk::PositionType::Bottom);
                            grid.attach_next_to(
                                &entry,
                                Some(&phone),
                                gtk::PositionType::Bottom,
                                1,
                                1,
                            );
                            first_phone = false;
                        } else {
                            let s = String::from("+") + &item.address;
                            let entry = widgets::Address::new(widgets::AddressType::Phone, &self)
                                .create(Some(s));
                            grid.insert_next_to(&phone, gtk::PositionType::Bottom);
                            grid.attach_next_to(
                                &entry,
                                Some(&phone),
                                gtk::PositionType::Bottom,
                                1,
                                1,
                            );
                        }
                    }
                }
            }
        }
        stack.set_visible_child_name("info");
    }

    pub fn show_password_dialog(&self) {
        let dialog = self
            .ui
            .builder
            .get_object::<gtk::Dialog>("password_dialog")
            .expect("Can't find password_dialog in ui file.");
        let confirm_password = self
            .ui
            .builder
            .get_object::<gtk::Button>("password-dialog-apply")
            .expect("Can't find password-dialog-apply in ui file.");
        confirm_password.set_sensitive(false);
        dialog.present();
    }

    pub fn show_new_avatar(&mut self, path: PathBuf) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let avatar_spinner = self
            .ui
            .builder
            .get_object::<gtk::Spinner>("account_settings_avatar_spinner")
            .expect("Can't find account_settings_avatar_spinner in ui file.");
        let avatar_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_avatar_button")
            .expect("Can't find account_settings_avatar_button in ui file.");

        info!("Request finished");
        self.set_login_data(LoginData {
            avatar: Some(path),
            ..login_data
        });
        avatar_spinner.hide();
        avatar_btn.set_sensitive(true);
        self.show_avatar();
    }

    pub fn show_avatar(&self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("account_settings_stack")
            .expect("Can't find account_settings_delete_box in ui file.");
        let avatar = self
            .ui
            .builder
            .get_object::<gtk::Overlay>("account_settings_avatar")
            .expect("Can't find account_settings_avatar in ui file.");
        let avatar_spinner = self
            .ui
            .builder
            .get_object::<gtk::Spinner>("account_settings_avatar_spinner")
            .expect("Can't find account_settings_avatar_spinner in ui file.");
        /* remove all old avatar */
        for w in avatar.get_children().iter() {
            if w != &avatar_spinner {
                avatar.remove(w);
            }
        }

        let w = widgets::Avatar::avatar_new(Some(100));
        avatar.add(&w);

        let data = w.circle(
            login_data.uid.to_string(),
            login_data.username,
            100,
            None,
            None,
        );
        download_to_cache(
            self.backend.clone(),
            login_data.server_url,
            login_data.uid,
            data,
        );

        /* FIXME: hack to make the avatar drawing area clickable*/
        let current = stack.get_visible_child_name();
        stack.set_visible_child_name("loading");
        if let Some(current) = current {
            stack.set_visible_child_name(&current);
        }
    }

    pub fn show_new_username(&mut self, name: Option<String>) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let entry = self
            .ui
            .builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let button = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_name_button")
            .expect("Can't find account_settings_name_button in ui file.");
        if let Some(name) = name.clone() {
            button.hide();
            let image =
                gtk::Image::new_from_icon_name(Some("emblem-ok-symbolic"), gtk::IconSize::Menu);
            button.set_image(Some(&image));
            button.set_sensitive(true);
            entry.set_editable(true);
            entry.set_text(&name);
        }
        self.set_login_data(LoginData {
            username: name,
            ..login_data
        });
    }

    pub fn update_username_account_settings(&self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let tx = self.backend.clone();
        let name = self
            .ui
            .builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let button = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_name_button")
            .expect("Can't find account_settings_name_button in ui file.");

        let old_username = login_data.username.clone().unwrap_or_default();
        let username = name
            .get_text()
            .map_or(String::new(), |gstr| gstr.to_string());

        if old_username != username {
            let spinner = gtk::Spinner::new();
            spinner.start();
            button.set_image(Some(&spinner));
            button.set_sensitive(false);
            name.set_editable(false);
            thread::spawn(move || {
                match user::set_username(
                    login_data.server_url,
                    login_data.access_token,
                    login_data.uid,
                    username,
                ) {
                    Ok(username) => {
                        let u = Some(username);
                        APPOP!(show_new_username, (u));
                    }
                    Err(err) => {
                        tx.send(BKCommand::SendBKResponse(BKResponse::SetUserNameError(err)))
                            .expect_log("Connection closed");
                    }
                }
            });
        } else {
            button.hide();
        }
    }

    pub fn close_account_settings_dialog(&self) {
        let advanced_box = self
            .ui
            .builder
            .get_object::<gtk::Box>("account_settings_advanced_box")
            .expect("Can't find account_settings_advanced_box in ui file.");
        let delete_box = self
            .ui
            .builder
            .get_object::<gtk::Box>("account_settings_delete_box")
            .expect("Can't find account_settings_delete_box in ui file.");
        let b = self
            .ui
            .builder
            .get_object::<gtk::Box>("account_settings_box")
            .expect("Can't find account_settings_delete_box in ui file.");

        advanced_box.queue_draw();
        delete_box.queue_draw();
        b.queue_draw();
    }

    pub fn set_new_password(&mut self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
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
        let password_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let password_btn_stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("account_settings_password_stack")
            .expect("Can't find account_settings_password_stack in ui file.");

        if let Some(old) = old_password.get_text() {
            if let Some(new) = new_password.get_text() {
                if old != "" && new != "" {
                    password_btn.set_sensitive(false);
                    password_btn_stack.set_visible_child_name("spinner");
                    let tx = self.backend.clone();
                    thread::spawn(move || {
                        match user::change_password(
                            login_data.server_url,
                            login_data.access_token,
                            login_data.uid.localpart().into(),
                            old.to_string(),
                            new.to_string(),
                        ) {
                            Ok(_) => {
                                APPOP!(password_changed);
                            }
                            Err(err) => {
                                tx.send(BKCommand::SendBKResponse(
                                    BKResponse::ChangePasswordError(err),
                                ))
                                .expect_log("Connection closed");
                            }
                        }
                    });
                }
            }
        }
    }

    pub fn password_changed(&self) {
        let password_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let password_btn_stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("account_settings_password_stack")
            .expect("Can't find account_settings_password_stack in ui file.");
        password_btn.set_sensitive(true);
        password_btn_stack.set_visible_child_name("label");
    }

    pub fn show_password_error_dialog(&self, error: String) {
        let password_btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let password_btn_stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("account_settings_password_stack")
            .expect("Can't find account_settings_password_stack in ui file.");
        self.show_error_dialog_in_settings(error);
        password_btn.set_sensitive(true);
        password_btn_stack.set_visible_child_name("label");
    }

    pub fn close_password_dialog(&mut self) {
        let dialog = self
            .ui
            .builder
            .get_object::<gtk::Dialog>("password_dialog")
            .expect("Can't find password_dialog in ui file.");
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
        /* Clear all user input */
        old_password.set_text("");
        new_password.set_text("");
        verify_password.set_text("");
        dialog.hide();
    }

    pub fn account_destruction(&self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let entry = self
            .ui
            .builder
            .get_object::<gtk::Entry>("account_settings_delete_password_confirm")
            .expect("Can't find account_settings_delete_password_confirm in ui file.");
        let mark = self
            .ui
            .builder
            .get_object::<gtk::CheckButton>("account_settings_delete_check")
            .expect("Can't find account_settings_delete_check in ui file.");
        let parent = self
            .ui
            .builder
            .get_object::<gtk::Window>("main_window")
            .expect("Can't find main_window in ui file.");

        let msg = i18n("Are you sure you want to delete your account?");
        let flags = gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT;
        let dialog = gtk::MessageDialog::new(
            Some(&parent),
            flags,
            gtk::MessageType::Warning,
            gtk::ButtonsType::None,
            &msg,
        );

        dialog.add_button("Confirm", gtk::ResponseType::Ok);
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);

        let _flag = mark.get_active(); // TODO: This is not used, remove from UI?
        if let Some(password) = entry.get_text().map(|gstr| gstr.to_string()) {
            let backend = self.backend.clone();
            dialog.connect_response(move |w, r| {
                if let gtk::ResponseType::Ok = r {
                    let tx = backend.clone();
                    let password = password.clone();
                    let login_data = login_data.clone();
                    thread::spawn(move || {
                        match user::account_destruction(
                            login_data.server_url.clone(),
                            login_data.access_token.clone(),
                            login_data.uid.localpart().into(),
                            password,
                        ) {
                            Ok(_) => {
                                APPOP!(account_destruction_logoff);
                            }
                            Err(err) => {
                                tx.send(BKCommand::SendBKResponse(
                                    BKResponse::AccountDestructionError(err),
                                ))
                                .expect_log("Connection closed");
                            }
                        }
                    });
                }
                w.destroy();
            });
            dialog.show_all();
        }
    }
    pub fn account_destruction_logoff(&self) {
        /* Do logout */
    }
}
