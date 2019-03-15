use gtk;
use gtk::prelude::*;

use crate::appop::AppOp;
use crate::appop::AppState;

use crate::widgets;

use fractal_api::types::ThirdPartyIdentifier;

impl AppOp {
    pub fn set_three_pid(&self, data: Option<Vec<ThirdPartyIdentifier>>) {
        self.account
            .as_ref()
            .map(|account| account.set_three_pid(data));
    }

    pub fn get_three_pid(&self) {
        self.account.as_ref().map(|account| account.get_three_pid());
    }

    pub fn added_three_pid(&self, l: Option<String>) {
        self.account
            .as_ref()
            .map(|account| account.added_three_pid(l));
    }

    pub fn valid_phone_token(&self, sid: Option<String>, secret: Option<String>) {
        self.account
            .as_ref()
            .map(|account| account.valid_phone_token(sid, secret));
    }

    pub fn show_three_pid_error_dialog(&self, error: String) {
        self.account
            .as_ref()
            .map(|account| account.show_three_pid_error_dialog(error));
    }

    pub fn show_error_dialog(&self, error: String) {
        self.account
            .as_ref()
            .map(|account| account.show_error_dialog(error));
    }

    pub fn get_token_email(&mut self, sid: Option<String>, secret: Option<String>) {
        self.account
            .as_ref()
            .map(|account| account.get_token_email(sid, secret));
    }

    pub fn get_token_phone(&mut self, sid: Option<String>, secret: Option<String>) {
        self.account
            .as_ref()
            .map(|account| account.get_token_phone(sid, secret));
    }

    pub fn show_account_settings_dialog(&mut self) {
        let main_window = self
            .ui
            .builder
            .get_object::<gtk::Window>("main_window")
            .expect("Can't find main_window in ui file.");
        let account_settings_box = self
            .ui
            .builder
            .get_object::<gtk::Box>("account_settings_box")
            .expect("Can't find account_settings_box in ui file.");
        let info = widgets::account::AccountInfo {
            username: self.username.clone(),
            uid: self.uid.clone(),
            device_id: self.device_id.clone(),
            avatar: self.avatar.clone(),
            server_url: self.server_url.clone(),
            identity_url: self.identity_url.clone(),
        };

        self.close_account_settings_dialog();

        let account = widgets::Account::new(info, self.backend.clone(), main_window);
        account_settings_box.pack_start(&account.get_widget(), true, true, 0);
        self.account = Some(account);

        self.set_state(AppState::AccountSettings);
    }

    pub fn show_new_avatar(&mut self, path: Option<String>) {
        self.set_avatar(path.clone());
        self.account
            .as_ref()
            .map(|account| account.show_new_avatar(path));
    }

    pub fn show_new_username(&mut self, name: Option<String>) {
        self.account
            .as_ref()
            .map(|account| account.show_new_username(name.clone()));
        self.set_username(name);
    }

    pub fn close_account_settings_dialog(&mut self) {
        let account_settings_box = self
            .ui
            .builder
            .get_object::<gtk::Box>("account_settings_box")
            .expect("Can't find account_settings_box in ui file.");

        account_settings_box
            .get_children()
            .iter()
            .for_each(|child| account_settings_box.remove(child));
        self.account = None;
    }

    pub fn password_changed(&self) {
        self.account
            .as_ref()
            .map(|account| account.password_changed());
    }

    pub fn show_password_error_dialog(&self, error: String) {
        self.account
            .as_ref()
            .map(|account| account.show_password_error_dialog(error));
    }

    pub fn account_destruction_logoff(&self) {
        self.account
            .as_ref()
            .map(|account| account.account_destruction_logoff());
    }
}
