extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;

use backend::BKCommand;
use widgets;
use widgets::AvatarExt;

use fractal_api::types::UserInfo;

impl AppOp {
    pub fn set_three_pid(&self, data: Option<Vec<UserInfo>>) {
        self.update_address(data);
    }

    pub fn get_three_pid(&self) {
        self.backend.send(BKCommand::GetThreePID).unwrap();
    }

    pub fn show_account_settings_dialog(&self) {
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("account_settings_dialog")
            .expect("Can't find account_settings_dialog in ui file.");
        let avatar = self.ui.builder
            .get_object::<gtk::Container>("account_settings_avatar")
            .expect("Can't find account_settings_avatar in ui file.");
        let name = self.ui.builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");
        let uid = self.ui.builder
            .get_object::<gtk::Label>("account_settings_uid")
            .expect("Can't find account_settings_uid in ui file.");
        let homeserver = self.ui.builder
            .get_object::<gtk::Label>("account_settings_homeserver")
            .expect("Can't find account_settings_homeserver in ui file.");
        let advanced_box = self.ui.builder
            .get_object::<gtk::Box>("account_settings_advanced_box")
            .expect("Can't find account_settings_advanced_box in ui file.");
        let delete_box = self.ui.builder
            .get_object::<gtk::Box>("account_settings_delete_box")
            .expect("Can't find account_settings_delete_box in ui file.");
        let stack = self.ui.builder
            .get_object::<gtk::Stack>("account_settings_stack")
            .expect("Can't find account_settings_delete_box in ui file.");

        stack.set_visible_child_name("loading");
        self.get_three_pid();
        /* remove all old avatar from the popover */
        for w in avatar.get_children().iter() {
            avatar.remove(w);
        }

        uid.set_text(&self.uid.clone().unwrap_or_default());
        homeserver.set_text(&self.server_url);
        name.set_text(&self.username.clone().unwrap_or_default());
        name.grab_focus_without_selecting();
        name.set_position(-1);

        let w = widgets::Avatar::circle_avatar(self.avatar.clone().unwrap_or_default(), Some(100));
        avatar.add(&w);
        avatar.show();

        dialog.set_redraw_on_allocate(true);
        advanced_box.set_redraw_on_allocate(true);
        delete_box.set_redraw_on_allocate(true);
        dialog.present();
    }

    pub fn update_address(&self, data: Option<Vec<UserInfo>>) {
        let grid = self.ui.builder
            .get_object::<gtk::Grid>("account_settings_grid")
            .expect("Can't find account_settings_grid in ui file.");
        let email = self.ui.builder
            .get_object::<gtk::Box>("account_settings_box_email")
            .expect("Can't find account_settings_box_email in ui file.");
        let phone = self.ui.builder
            .get_object::<gtk::Box>("account_settings_box_phone")
            .expect("Can't find account_settings_box_phone in ui file.");
        let email_entry = self.ui.builder
            .get_object::<gtk::Entry>("account_settings_email")
            .expect("Can't find account_settings_email in ui file.");
        let phone_entry = self.ui.builder
            .get_object::<gtk::Entry>("account_settings_phone")
            .expect("Can't find account_settings_phone in ui file.");

        let stack = self.ui.builder
            .get_object::<gtk::Stack>("account_settings_stack")
            .expect("Can't find account_settings_delete_box in ui file.");

        let mut first_email = true;
        let mut first_phone = true;

        let mut i = 1;
        let mut child = grid.get_child_at(1, i);
        while child.is_some() {
            if let Some(child) = child.clone() {
                if child != phone && child != email {
                    grid.remove_row(i);
                }
                else {
                    i = i + 1;
                }
            }
            child = grid.get_child_at(1, i);
        }

        if let Some(data) = data {
            for item in data {
                if item.medium == "email" {
                    if first_email {
                        email_entry.set_text(&item.address);
                        let entry = gtk::Entry::new();
                        entry.show();
                        grid.insert_next_to(&email, gtk::PositionType::Bottom);
                        grid.attach_next_to(&entry, &email, gtk::PositionType::Bottom, 1, 1);
                        first_email = false;
                    }
                    else {
                        let entry = gtk::Entry::new();
                        entry.set_text(&item.address);
                        entry.show();
                        grid.insert_next_to(&email, gtk::PositionType::Bottom);
                        grid.attach_next_to(&entry, &email, gtk::PositionType::Bottom, 1, 1);
                   }
                }
                else if item.medium == "msisdn" {
                    if first_phone {
                       let s = String::from("+") + &String::from(item.address);
                        phone_entry.set_text(&s);

                        let entry = gtk::Entry::new();
                        entry.show();
                        grid.insert_next_to(&phone, gtk::PositionType::Bottom);
                        grid.attach_next_to(&entry, &phone, gtk::PositionType::Bottom, 1, 1);
                        first_phone = false;
                    }
                    else {
                        let entry = gtk::Entry::new();
                        let s = String::from("+") + &String::from(item.address);
                        entry.set_text(&s);
                        entry.show();
                        grid.insert_next_to(&phone, gtk::PositionType::Bottom);
                        grid.attach_next_to(&entry, &phone, gtk::PositionType::Bottom, 1, 1);
                    }
                }
            }
        }
        stack.set_visible_child_name("info");
    }

    pub fn show_password_dialog(&self) {
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("password_dialog")
            .expect("Can't find password_dialog in ui file.");
        let confirm_password = self.ui.builder
            .get_object::<gtk::Button>("password-dialog-apply")
            .expect("Can't find password-dialog-apply in ui file.");
        confirm_password.set_sensitive(false);
        dialog.present();
    }

    pub fn save_tmp_avatar_account_settings(&mut self, file: String) {
        self.tmp_avatar = Some(file);
    }

    pub fn apply_account_settings(&self) {
        let name = self.ui.builder
            .get_object::<gtk::Entry>("account_settings_name")
            .expect("Can't find account_settings_name in ui file.");

        let old_username = self.username.clone().unwrap_or_default();
        let username = name.get_text().unwrap_or_default();

        if old_username !=  username {
            self.backend.send(BKCommand::SetUserName(username)).unwrap();
        }

        if let Some(ref user) = self.tmp_avatar {
            let command = BKCommand::SetUserAvatar(user.to_string());
            self.backend.send(command).unwrap();
        }

    }

    pub fn close_account_settings_dialog(&mut self) {
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("account_settings_dialog")
            .expect("Can't find account_settings_dialog in ui file.");
        /*
           let avatar = self.ui.builder
           .get_object::<gtk::Container>("account_settings_avatar")
           .expect("Can't find account_settings_avatar in ui file.");
           let name = self.ui.builder
           .get_object::<gtk::Entry>("account_settings_name")
           .expect("Can't find account_settings_name in ui file.");
           */
        let advanced = self.ui.builder
            .get_object::<gtk::Revealer>("account_settings_advanced")
            .expect("Can't find account_settings_advanced in ui file.");
        let delete = self.ui.builder
            .get_object::<gtk::Revealer>("account_settings_delete")
            .expect("Can't find account_settings_delete in ui file.");

        let advanced_toggle = self.ui.builder
            .get_object::<gtk::EventBox>("account_settings_advanced_toggle")
            .expect("Can't find account_settings_advanced_toggle in ui file.");
        let delete_toggle = self.ui.builder
            .get_object::<gtk::EventBox>("account_settings_delete_toggle")
            .expect("Can't find account_settings_delete_toggle in ui file.");

        self.tmp_avatar = None;
        advanced_toggle.get_style_context().unwrap().remove_class("advanced_revealer_divider");
        delete_toggle.get_style_context().unwrap().remove_class("advanced_revealer_divider");
        advanced.set_reveal_child(false);
        delete.set_reveal_child(false);
        dialog.hide();
        dialog.resize(700, 200);
    }

    pub fn close_password_dialog(&mut self) {
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("password_dialog")
            .expect("Can't find password_dialog in ui file.");
        let old_password = self.ui.builder
            .get_object::<gtk::Entry>("password-dialog-old-entry")
            .expect("Can't find password-dialog-old-entry in ui file.");
        let new_password = self.ui.builder
            .get_object::<gtk::Entry>("password-dialog-entry")
            .expect("Can't find password-dialog-entry in ui file.");
        let verify_password = self.ui.builder
            .get_object::<gtk::Entry>("password-dialog-verify-entry")
            .expect("Can't find password-dialog-verify-entry in ui file.");
        /* Clear all user input */
        old_password.set_text("");
        new_password.set_text("");
        verify_password.set_text("");
        dialog.hide();
    }
}
