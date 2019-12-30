use gtk;
use gtk::prelude::*;

use crate::appop::AppOp;

use crate::cache::download_to_cache;

use crate::backend::BKCommand;
use crate::widgets;
use crate::widgets::AvatarExt;

use super::LoginData;

impl AppOp {
    pub fn get_username(&self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        self.backend
            .send(BKCommand::GetUsername(
                login_data.server_url.clone(),
                login_data.uid.clone(),
            ))
            .unwrap();
        self.backend
            .send(BKCommand::GetAvatar(login_data.server_url, login_data.uid))
            .unwrap();
    }

    pub fn show_user_info(&self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("user_info")
            .expect("Can't find user_info_avatar in ui file.");

        /* Show user infos inside the popover but wait for all data to arrive */
        if login_data.avatar.is_some() && login_data.username.is_some() {
            let avatar = self
                .ui
                .builder
                .get_object::<gtk::Container>("user_info_avatar")
                .expect("Can't find user_info_avatar in ui file.");

            let name = self
                .ui
                .builder
                .get_object::<gtk::Label>("user_info_username")
                .expect("Can't find user_info_avatar in ui file.");

            let uid = self
                .ui
                .builder
                .get_object::<gtk::Label>("user_info_uid")
                .expect("Can't find user_info_avatar in ui file.");

            uid.set_text(&login_data.uid);
            name.set_text(&login_data.username.clone().unwrap_or_default());

            /* remove all old avatar from the popover */
            for w in avatar.get_children().iter() {
                avatar.remove(w);
            }

            let w = widgets::Avatar::avatar_new(Some(40));
            let data = w.circle(
                login_data.uid.clone(),
                login_data.username.clone(),
                40,
                None,
                None,
            );
            download_to_cache(
                self.backend.clone(),
                login_data.server_url.clone(),
                login_data.uid.clone(),
                data.clone(),
            );

            avatar.add(&w);
            stack.set_visible_child_name("info");
        } else {
            stack.set_visible_child_name("spinner");
        }

        let eb = gtk::EventBox::new();
        match login_data.avatar.clone() {
            Some(_) => {
                let w = widgets::Avatar::avatar_new(Some(24));
                let data = w.circle(login_data.uid.clone(), login_data.username, 24, None, None);
                download_to_cache(
                    self.backend.clone(),
                    login_data.server_url.clone(),
                    login_data.uid.clone(),
                    data.clone(),
                );

                eb.add(&w);
            }
            None => {
                let w = gtk::Spinner::new();
                w.show();
                w.start();
                eb.add(&w);
            }
        };

        eb.connect_button_press_event(move |_, _| Inhibit(false));
    }

    pub fn set_login_data(&mut self, login_data: LoginData) {
        self.login_data = Some(login_data);
        self.show_user_info();
    }

    pub fn set_username(&mut self, username: Option<String>) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        self.set_login_data(LoginData {
            username,
            ..login_data
        });
    }

    pub fn set_avatar(&mut self, fname: Option<String>) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        self.set_login_data(LoginData {
            avatar: fname,
            ..login_data
        });
    }
}
