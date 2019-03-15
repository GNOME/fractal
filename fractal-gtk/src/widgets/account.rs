use fractal_api::clone;
use fractal_api::types::Medium;
use fractal_api::types::ThirdPartyIdentifier;
use gio::ActionMapExt;
use glib;
use gtk;
use gtk::prelude::*;
use log::info;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use crate::actions::{AccountSettings, StateExt};
use crate::backend::BKCommand;
use crate::cache::download_to_cache;
use crate::i18n::i18n;
use crate::widgets;
use crate::widgets::AvatarExt;

#[derive(Clone, Debug)]
pub struct AccountInfo {
    pub username: Option<String>,
    pub uid: Option<String>,
    pub device_id: Option<String>,
    pub avatar: Option<String>,
    pub server_url: String,
    pub identity_url: String,
}

#[derive(Clone, Debug)]
pub struct Account {
    info: Rc<RefCell<AccountInfo>>,
    backend: Sender<BKCommand>,
    builder: gtk::Builder,
    main_window: gtk::Window,
}

impl Account {
    pub fn new(
        account_info: AccountInfo,
        backend: Sender<BKCommand>,
        main_window: gtk::Window,
    ) -> Account {
        let builder = gtk::Builder::new();

        let this = Account {
            info: Rc::new(RefCell::new(account_info)),
            backend,
            builder: builder.clone(),
            main_window,
        };

        builder
            .add_from_resource("/org/gnome/Fractal/ui/account.ui")
            .expect("Can't load ui file: account_settings.ui");
        builder
            .add_from_resource("/org/gnome/Fractal/ui/password_dialog.ui")
            .expect("Can't load ui file: password_dialog.ui");

        let cancel_password = builder
            .get_object::<gtk::Button>("password-dialog-cancel")
            .expect("Can't find password-dialog-cancel in ui file.");
        let confirm_password = builder
            .get_object::<gtk::Button>("password-dialog-apply")
            .expect("Can't find password-dialog-apply in ui file.");
        let password_dialog = builder
            .get_object::<gtk::Dialog>("password_dialog")
            .expect("Can't find password_dialog in ui file.");
        let avatar_btn = builder
            .get_object::<gtk::Button>("account_settings_avatar_button")
            .expect("Can't find account_settings_avatar_button in ui file.");
        let name_entry = builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let name_btn = builder
            .get_object::<gtk::Button>("account_settings_name_button")
            .expect("Can't find account_settings_name_button in ui file.");
        let password_btn = builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let old_password = builder
            .get_object::<gtk::Entry>("password-dialog-old-entry")
            .expect("Can't find password-dialog-old-entry in ui file.");
        let new_password = builder
            .get_object::<gtk::Entry>("password-dialog-entry")
            .expect("Can't find password-dialog-entry in ui file.");
        let verify_password = builder
            .get_object::<gtk::Entry>("password-dialog-verify-entry")
            .expect("Can't find password-dialog-verify-entry in ui file.");
        let destruction_entry = builder
            .get_object::<gtk::Entry>("account_settings_delete_password_confirm")
            .expect("Can't find account_settings_delete_password_confirm in ui file.");
        let destruction_btn = builder
            .get_object::<gtk::Button>("account_settings_delete_btn")
            .expect("Can't find account_settings_delete_btn in ui file.");

        let actions = AccountSettings::new(&this.main_window, &this.backend);
        let container = builder
            .get_object::<gtk::Box>("account_settings_box")
            .expect("Can't find account_settings_box in ui file.");
        container.insert_action_group("user-settings", &actions);

        /* Body */
        if let Some(action) = actions.lookup_action("change-avatar") {
            action.bind_button_state(&avatar_btn);
            avatar_btn.set_action_name("user-settings.change-avatar");
            let avatar_spinner = builder
                .get_object::<gtk::Spinner>("account_settings_avatar_spinner")
                .expect("Can't find account_settings_avatar_spinner in ui file.");
            let spinner = avatar_spinner.downgrade();
            avatar_btn.connect_property_sensitive_notify(move |w| {
                let spinner = upgrade_weak!(spinner);
                if !w.get_sensitive() {
                    spinner.start();
                    spinner.show();
                } else {
                    spinner.hide();
                    spinner.stop();
                }
            });
        }

        let button = name_btn.clone();
        let info = this.info.clone();
        name_entry.connect_property_text_notify(move |w| {
            if let Some(text) = w.get_text() {
                if text != "" {
                    if let Some(ref username) = info.borrow().username {
                        if *username == text {
                            button.hide();
                            return;
                        }
                    }
                    button.show();
                    return;
                }
            }
            button.hide();
        });

        let button = name_btn.clone();
        name_entry.connect_activate(move |_w| {
            let _ = button.emit("clicked", &[]);
        });

        name_btn.connect_clicked(clone!(this => move |_w| {
            this.update_username_account_settings();
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

        /* Password dialog */
        password_btn.connect_clicked(clone!(this => move |_| {
            this.show_password_dialog();
        }));

        password_dialog.set_transient_for(Some(&this.main_window));
        password_dialog.connect_delete_event(clone!(this => move |_, _| {
            this.close_password_dialog();
            glib::signal::Inhibit(true)
        }));

        /* Headerbar */
        cancel_password.connect_clicked(clone!(this => move |_| {
            this.close_password_dialog();
        }));

        confirm_password.connect_clicked(clone!(this => move |_| {
            this.set_new_password();
            this.close_password_dialog();
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

        destruction_entry.connect_property_text_notify(clone!(destruction_btn => move |w| {
            if let Some(text) = w.get_text() {
                if text != "" {
                    destruction_btn.set_sensitive(true);
                    return;
                }
            }
            destruction_btn.set_sensitive(false);
        }));

        destruction_btn.connect_clicked(clone!(this => move |_| {
            this.account_destruction();
        }));

        this.show_account_settings_dialog();

        this
    }

    pub fn get_widget(&self) -> gtk::Box {
        self.builder
            .get_object::<gtk::Box>("account_settings_box")
            .expect("Can't find account_settings_delete_box in ui file.")
    }

    pub fn set_three_pid(&self, data: Option<Vec<ThirdPartyIdentifier>>) {
        self.update_address(data);
    }

    pub fn get_three_pid(&self) {
        self.backend.send(BKCommand::GetThreePID).unwrap();
    }

    pub fn added_three_pid(&self, _l: Option<String>) {
        self.get_three_pid();
    }

    pub fn valid_phone_token(&self, sid: Option<String>, secret: Option<String>) {
        if let Some(sid) = sid {
            if let Some(secret) = secret {
                let _ = self.backend.send(BKCommand::AddThreePID(
                    self.info.borrow().identity_url.clone(),
                    secret.clone(),
                    sid.clone(),
                ));
            }
        } else {
            self.show_error_dialog(i18n("The validation code is not correct."));
            self.get_three_pid();
        }
    }

    pub fn show_three_pid_error_dialog(&self, error: String) {
        self.show_error_dialog(error);
    }

    pub fn show_error_dialog(&self, error: String) {
        let msg = error;
        let flags = gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT;
        let dialog = gtk::MessageDialog::new(
            Some(&self.main_window),
            flags,
            gtk::MessageType::Error,
            gtk::ButtonsType::None,
            &msg,
        );

        dialog.add_button(&i18n("OK"), gtk::ResponseType::Ok.into());

        let backend = self.backend.clone();
        dialog.connect_response(move |w, _| {
            backend.send(BKCommand::GetThreePID).unwrap();
            w.destroy();
        });
        dialog.show_all();
    }

    pub fn get_token_email(&self, sid: Option<String>, secret: Option<String>) {
        if let Some(sid) = sid {
            if let Some(secret) = secret {
                self.show_email_dialog(sid, secret);
            }
        }
    }

    pub fn get_token_phone(&self, sid: Option<String>, secret: Option<String>) {
        if let Some(sid) = sid {
            if let Some(secret) = secret {
                self.show_phone_dialog(sid, secret);
            }
        }
    }

    pub fn show_new_avatar(&self, path: Option<String>) {
        self.info.borrow_mut().avatar = path;

        let avatar_spinner = self
            .builder
            .get_object::<gtk::Spinner>("account_settings_avatar_spinner")
            .expect("Can't find account_settings_avatar_spinner in ui file.");
        let avatar_btn = self
            .builder
            .get_object::<gtk::Button>("account_settings_avatar_button")
            .expect("Can't find account_settings_avatar_button in ui file.");

        info!("Request finished");
        avatar_spinner.hide();
        avatar_btn.set_sensitive(true);
        self.show_avatar();
    }

    pub fn show_new_username(&self, name: Option<String>) {
        let entry = self
            .builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let button = self
            .builder
            .get_object::<gtk::Button>("account_settings_name_button")
            .expect("Can't find account_settings_name_button in ui file.");
        if let Some(name) = name.clone() {
            button.hide();
            let image = gtk::Image::new_from_icon_name("emblem-ok-symbolic", 1);
            button.set_image(&image);
            button.set_sensitive(true);
            entry.set_editable(true);
            entry.set_text(&name);
        }
        self.info.borrow_mut().username = name;
    }

    pub fn password_changed(&self) {
        let password_btn = self
            .builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let password_btn_stack = self
            .builder
            .get_object::<gtk::Stack>("account_settings_password_stack")
            .expect("Can't find account_settings_password_stack in ui file.");
        password_btn.set_sensitive(true);
        password_btn_stack.set_visible_child_name("label");
    }

    pub fn show_password_error_dialog(&self, error: String) {
        let password_btn = self
            .builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let password_btn_stack = self
            .builder
            .get_object::<gtk::Stack>("account_settings_password_stack")
            .expect("Can't find account_settings_password_stack in ui file.");
        self.show_error_dialog(error);
        password_btn.set_sensitive(true);
        password_btn_stack.set_visible_child_name("label");
    }

    pub fn account_destruction_logoff(&self) {
        /* Do logout */
    }

    fn show_account_settings_dialog(&self) {
        // Reset view before displaying it
        self.close_account_settings_dialog();
        let avatar_spinner = self
            .builder
            .get_object::<gtk::Spinner>("account_settings_avatar_spinner")
            .expect("Can't find account_settings_avatar_spinner in ui file.");
        let avatar_btn = self
            .builder
            .get_object::<gtk::Button>("account_settings_avatar_button")
            .expect("Can't find account_settings_avatar_button in ui file.");
        let name = self
            .builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let name_btn = self
            .builder
            .get_object::<gtk::Button>("account_settings_name_button")
            .expect("Can't find account_settings_name_button in ui file.");
        let uid = self
            .builder
            .get_object::<gtk::Label>("account_settings_uid")
            .expect("Can't find account_settings_uid in ui file.");
        let device_id = self
            .builder
            .get_object::<gtk::Label>("account_settings_device_id")
            .expect("Can't find account_settings_device_id in ui file.");
        let homeserver = self
            .builder
            .get_object::<gtk::Label>("account_settings_homeserver")
            .expect("Can't find account_settings_homeserver in ui file.");
        let advanced_box = self
            .builder
            .get_object::<gtk::Box>("account_settings_advanced_box")
            .expect("Can't find account_settings_advanced_box in ui file.");
        let delete_box = self
            .builder
            .get_object::<gtk::Box>("account_settings_delete_box")
            .expect("Can't find account_settings_delete_box in ui file.");
        let stack = self
            .builder
            .get_object::<gtk::Stack>("account_settings_stack")
            .expect("Can't find account_settings_delete_box in ui file.");
        let destruction_btn = self
            .builder
            .get_object::<gtk::Button>("account_settings_delete_btn")
            .expect("Can't find account_settings_delete_btn in ui file.");
        let destruction_entry = self
            .builder
            .get_object::<gtk::Entry>("account_settings_delete_password_confirm")
            .expect("Can't find account_settings_delete_password_confirm in ui file.");
        let password_btn = self
            .builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let password_btn_stack = self
            .builder
            .get_object::<gtk::Stack>("account_settings_password_stack")
            .expect("Can't find account_settings_password_stack in ui file.");
        let destruction_flag = self
            .builder
            .get_object::<gtk::CheckButton>("account_settings_delete_check")
            .expect("Can't find account_settings_delete_check in ui file.");

        stack.set_visible_child_name("loading");
        self.get_three_pid();
        uid.set_text(&self.info.borrow().uid.clone().unwrap_or_default());
        device_id.set_text(&self.info.borrow().device_id.clone().unwrap_or_default());
        homeserver.set_text(&self.info.borrow().server_url);
        name.set_text(&self.info.borrow().username.clone().unwrap_or_default());
        name.grab_focus_without_selecting();
        name.set_position(-1);

        avatar_spinner.hide();
        avatar_btn.set_sensitive(true);
        self.show_avatar();

        name_btn.hide();
        name.set_editable(true);
        let image = gtk::Image::new_from_icon_name("emblem-ok-symbolic", 1);
        name_btn.set_image(&image);
        name_btn.set_sensitive(true);

        /* reset the password button */
        password_btn_stack.set_visible_child_name("label");
        password_btn.set_sensitive(true);

        destruction_flag.set_active(false);
        destruction_btn.set_sensitive(false);
        destruction_entry.set_text("");
        advanced_box.set_redraw_on_allocate(true);
        delete_box.set_redraw_on_allocate(true);
    }

    fn update_username_account_settings(&self) {
        let name = self
            .builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let button = self
            .builder
            .get_object::<gtk::Button>("account_settings_name_button")
            .expect("Can't find account_settings_name_button in ui file.");

        let old_username = self.info.borrow().username.clone().unwrap_or_default();
        let username = name.get_text().unwrap_or_default();

        if old_username != username {
            let spinner = gtk::Spinner::new();
            spinner.start();
            button.set_image(&spinner);
            button.set_sensitive(false);
            name.set_editable(false);
            self.backend.send(BKCommand::SetUserName(username)).unwrap();
        } else {
            button.hide();
        }
    }

    fn show_phone_dialog(&self, sid: String, secret: String) {
        let entry = gtk::Entry::new();
        let msg = i18n("Enter the code received via SMS");
        let flags = gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT;
        let dialog = gtk::MessageDialog::new(
            Some(&self.main_window),
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
        dialog.add_button(&i18n("Cancel"), gtk::ResponseType::Cancel.into());
        let button = dialog.add_button(&i18n("Continue"), gtk::ResponseType::Ok.into());
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

        let value = entry.clone();
        let id_server = self.info.borrow().identity_url.clone();
        dialog.connect_response(move |w, r| {
            match gtk::ResponseType::from(r) {
                gtk::ResponseType::Ok => {
                    if let Some(token) = value.get_text() {
                        let _ = backend.send(BKCommand::SubmitPhoneToken(
                            id_server.clone(),
                            secret.clone(),
                            sid.clone(),
                            token,
                        ));
                    }
                }
                _ => {}
            }
            w.destroy();
        });
        dialog.show_all();
    }

    fn show_email_dialog(&self, sid: String, secret: String) {
        let msg = i18n("In order to add this email address, go to your inbox and follow the link you received. Once you’ve done that, click Continue.");
        let flags = gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT;
        let dialog = gtk::MessageDialog::new(
            Some(&self.main_window),
            flags,
            gtk::MessageType::Error,
            gtk::ButtonsType::None,
            &msg,
        );
        let backend = self.backend.clone();
        let id_server = self.info.borrow().identity_url.clone();
        dialog.add_button(&i18n("Cancel"), gtk::ResponseType::Cancel.into());
        dialog.add_button(&i18n("Continue"), gtk::ResponseType::Ok.into());
        dialog.connect_response(move |w, r| {
            match gtk::ResponseType::from(r) {
                gtk::ResponseType::Ok => {
                    let _ = backend.send(BKCommand::AddThreePID(
                        id_server.clone(),
                        secret.clone(),
                        sid.clone(),
                    ));
                }
                _ => {}
            }
            w.destroy();
        });
        dialog.show_all();
    }

    fn update_address(&self, data: Option<Vec<ThirdPartyIdentifier>>) {
        let grid = self
            .builder
            .get_object::<gtk::Grid>("account_settings_grid")
            .expect("Can't find account_settings_grid in ui file.");
        let email = self
            .builder
            .get_object::<gtk::Box>("account_settings_email")
            .expect("Can't find account_settings_box_email in ui file.");
        let phone = self
            .builder
            .get_object::<gtk::Box>("account_settings_phone")
            .expect("Can't find account_settings_box_phone in ui file.");
        let stack = self
            .builder
            .get_object::<gtk::Stack>("account_settings_stack")
            .expect("Can't find account_settings_delete_box in ui file.");
        let password = self
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
                    i = i + 1;
                }
            }
            child = grid.get_child_at(1, i);
        }

        /* Make sure we have at least one empty entry for email and phone */
        let mut empty_email = widgets::Address::new(
            widgets::AddressType::Email,
            self.info.borrow().identity_url.clone(),
            self.backend.clone(),
        );
        let mut empty_phone = widgets::Address::new(
            widgets::AddressType::Phone,
            self.info.borrow().identity_url.clone(),
            self.backend.clone(),
        );
        email.pack_start(&empty_email.create(None), true, true, 0);
        phone.pack_start(&empty_phone.create(None), true, true, 0);
        if let Some(data) = data {
            for item in data {
                match item.medium {
                    Medium::Email => {
                        if first_email {
                            empty_email.update(Some(item.address));
                            let entry = widgets::Address::new(
                                widgets::AddressType::Email,
                                self.info.borrow().identity_url.clone(),
                                self.backend.clone(),
                            )
                            .create(None);
                            grid.insert_next_to(&email, gtk::PositionType::Bottom);
                            grid.attach_next_to(&entry, &email, gtk::PositionType::Bottom, 1, 1);
                            first_email = false;
                        } else {
                            let entry = widgets::Address::new(
                                widgets::AddressType::Email,
                                self.info.borrow().identity_url.clone(),
                                self.backend.clone(),
                            )
                            .create(Some(item.address));
                            grid.insert_next_to(&email, gtk::PositionType::Bottom);
                            grid.attach_next_to(&entry, &email, gtk::PositionType::Bottom, 1, 1);
                        }
                    }
                    Medium::MsIsdn => {
                        if first_phone {
                            empty_phone.update(Some(item.address));
                            let entry = widgets::Address::new(
                                widgets::AddressType::Phone,
                                self.info.borrow().identity_url.clone(),
                                self.backend.clone(),
                            )
                            .create(None);
                            grid.insert_next_to(&phone, gtk::PositionType::Bottom);
                            grid.attach_next_to(&entry, &phone, gtk::PositionType::Bottom, 1, 1);
                            first_phone = false;
                        } else {
                            let s = String::from("+") + &item.address;
                            let entry = widgets::Address::new(
                                widgets::AddressType::Phone,
                                self.info.borrow().identity_url.clone(),
                                self.backend.clone(),
                            )
                            .create(Some(s));
                            grid.insert_next_to(&phone, gtk::PositionType::Bottom);
                            grid.attach_next_to(&entry, &phone, gtk::PositionType::Bottom, 1, 1);
                        }
                    }
                }
            }
        }
        stack.set_visible_child_name("info");
    }

    fn show_password_dialog(&self) {
        let dialog = self
            .builder
            .get_object::<gtk::Dialog>("password_dialog")
            .expect("Can't find password_dialog in ui file.");
        let confirm_password = self
            .builder
            .get_object::<gtk::Button>("password-dialog-apply")
            .expect("Can't find password-dialog-apply in ui file.");
        confirm_password.set_sensitive(false);
        dialog.present();
    }

    fn show_avatar(&self) {
        let stack = self
            .builder
            .get_object::<gtk::Stack>("account_settings_stack")
            .expect("Can't find account_settings_delete_box in ui file.");
        let avatar = self
            .builder
            .get_object::<gtk::Overlay>("account_settings_avatar")
            .expect("Can't find account_settings_avatar in ui file.");
        let avatar_spinner = self
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

        let uid = self.info.borrow().uid.clone().unwrap_or_default();
        let data = w.circle(uid.clone(), self.info.borrow().username.clone(), 100);
        download_to_cache(self.backend.clone(), uid, data.clone());

        /* FIXME: hack to make the avatar drawing area clickable*/
        let current = stack.get_visible_child_name();
        stack.set_visible_child_name("loading");
        if let Some(current) = current {
            stack.set_visible_child_name(&current);
        }
    }

    fn close_account_settings_dialog(&self) {
        let advanced_box = self
            .builder
            .get_object::<gtk::Box>("account_settings_advanced_box")
            .expect("Can't find account_settings_advanced_box in ui file.");
        let delete_box = self
            .builder
            .get_object::<gtk::Box>("account_settings_delete_box")
            .expect("Can't find account_settings_delete_box in ui file.");
        let b = self
            .builder
            .get_object::<gtk::Box>("account_settings_box")
            .expect("Can't find account_settings_delete_box in ui file.");

        advanced_box.queue_draw();
        delete_box.queue_draw();
        b.queue_draw();
    }

    fn set_new_password(&self) {
        let old_password = self
            .builder
            .get_object::<gtk::Entry>("password-dialog-old-entry")
            .expect("Can't find password-dialog-old-entry in ui file.");
        let new_password = self
            .builder
            .get_object::<gtk::Entry>("password-dialog-entry")
            .expect("Can't find password-dialog-entry in ui file.");
        let password_btn = self
            .builder
            .get_object::<gtk::Button>("account_settings_password")
            .expect("Can't find account_settings_password in ui file.");
        let password_btn_stack = self
            .builder
            .get_object::<gtk::Stack>("account_settings_password_stack")
            .expect("Can't find account_settings_password_stack in ui file.");

        if let Some(old) = old_password.get_text() {
            if let Some(new) = new_password.get_text() {
                if let Some(mxid) = self.info.borrow().uid.clone() {
                    if old != "" && new != "" {
                        password_btn.set_sensitive(false);
                        password_btn_stack.set_visible_child_name("spinner");
                        let _ = self.backend.send(BKCommand::ChangePassword(mxid, old, new));
                    }
                }
            }
        }
    }

    fn close_password_dialog(&self) {
        let dialog = self
            .builder
            .get_object::<gtk::Dialog>("password_dialog")
            .expect("Can't find password_dialog in ui file.");
        let old_password = self
            .builder
            .get_object::<gtk::Entry>("password-dialog-old-entry")
            .expect("Can't find password-dialog-old-entry in ui file.");
        let new_password = self
            .builder
            .get_object::<gtk::Entry>("password-dialog-entry")
            .expect("Can't find password-dialog-entry in ui file.");
        let verify_password = self
            .builder
            .get_object::<gtk::Entry>("password-dialog-verify-entry")
            .expect("Can't find password-dialog-verify-entry in ui file.");
        /* Clear all user input */
        old_password.set_text("");
        new_password.set_text("");
        verify_password.set_text("");
        dialog.hide();
    }

    fn account_destruction(&self) {
        let entry = self
            .builder
            .get_object::<gtk::Entry>("account_settings_delete_password_confirm")
            .expect("Can't find account_settings_delete_password_confirm in ui file.");
        let mark = self
            .builder
            .get_object::<gtk::CheckButton>("account_settings_delete_check")
            .expect("Can't find account_settings_delete_check in ui file.");

        let msg = i18n("Are you sure you want to delete your account?");
        let flags = gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT;
        let dialog = gtk::MessageDialog::new(
            Some(&self.main_window),
            flags,
            gtk::MessageType::Warning,
            gtk::ButtonsType::None,
            &msg,
        );

        dialog.add_button("Confirm", gtk::ResponseType::Ok.into());
        dialog.add_button("Cancel", gtk::ResponseType::Cancel.into());

        let flag = mark.get_active();
        if let Some(password) = entry.get_text() {
            if let Some(mxid) = self.info.borrow().uid.clone() {
                let backend = self.backend.clone();
                dialog.connect_response(clone!(mxid, password, flag => move |w, r| {
                    match gtk::ResponseType::from(r) {
                        gtk::ResponseType::Ok => {
                            let _ = backend.send(BKCommand::AccountDestruction(mxid.clone(), password.clone(), flag));
                        },
                        _ => {}
                    }
                    w.destroy();
                }));
                dialog.show_all();
            }
        }
    }
}
