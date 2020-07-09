use gtk::prelude::*;

use crate::backend::{user, HandleError};
use glib::clone;

use std::path::PathBuf;
use std::thread;

use crate::app::App;
use crate::appop::AppOp;

use crate::cache::download_to_cache;

use crate::widgets;
use crate::widgets::AvatarExt;

use super::LoginData;

impl AppOp {
    pub fn get_username(&self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());

        thread::spawn(clone!(@strong login_data => move || {
            match user::get_username(login_data.server_url, login_data.access_token, login_data.uid) {
                Ok(username) => {
                    APPOP!(set_username, (username));
                }
                Err(err) => {
                    err.handle_error();
                }
            }
        }));

        thread::spawn(clone!(@strong login_data => move || {
            match user::get_avatar(login_data.server_url, login_data.uid) {
                Ok(path) => {
                    APPOP!(set_avatar, (path));
                }
                Err(err) => {
                    err.handle_error();
                }
            }
        }));
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

            uid.set_text(&login_data.uid.to_string());
            name.set_text(&login_data.username.clone().unwrap_or_default());

            /* remove all old avatar from the popover */
            for w in avatar.get_children().iter() {
                avatar.remove(w);
            }

            let w = widgets::Avatar::avatar_new(Some(40));
            let data = w.circle(
                login_data.uid.to_string(),
                login_data.username.clone(),
                40,
                None,
                None,
            );
            download_to_cache(
                self.thread_pool.clone(),
                self.user_info_cache.clone(),
                login_data.server_url.clone(),
                login_data.uid.clone(),
                data,
            );

            avatar.add(&w);
            stack.set_visible_child_name("info");
        } else {
            stack.set_visible_child_name("spinner");
        }

        let eb = gtk::EventBox::new();
        match login_data.avatar {
            Some(_) => {
                let w = widgets::Avatar::avatar_new(Some(24));
                let data = w.circle(
                    login_data.uid.to_string(),
                    login_data.username,
                    24,
                    None,
                    None,
                );
                download_to_cache(
                    self.thread_pool.clone(),
                    self.user_info_cache.clone(),
                    login_data.server_url.clone(),
                    login_data.uid,
                    data,
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

    pub fn set_avatar(&mut self, path: PathBuf) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        self.set_login_data(LoginData {
            avatar: Some(path),
            ..login_data
        });
    }
}
