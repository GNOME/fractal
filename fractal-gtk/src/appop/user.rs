extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;

use cache::download_to_cache;

use backend::BKCommand;
use widgets;
use widgets::AvatarExt;

impl AppOp {
    pub fn get_username(&self) {
        self.backend.send(BKCommand::GetUsername).unwrap();
        self.backend.send(BKCommand::GetAvatar).unwrap();
    }

    pub fn show_user_info (&self) {
        let stack = self.ui.builder
            .get_object::<gtk::Stack>("user_info")
            .expect("Can't find user_info_avatar in ui file.");

        /* Show user infos inside the popover but wait for all data to arrive */
        if self.avatar.is_some() && self.username.is_some() && self.uid.is_some() {
            let avatar = self.ui.builder
                .get_object::<gtk::Container>("user_info_avatar")
                .expect("Can't find user_info_avatar in ui file.");

            let name = self.ui.builder
                .get_object::<gtk::Label>("user_info_username")
                .expect("Can't find user_info_avatar in ui file.");

            let uid = self.ui.builder
                .get_object::<gtk::Label>("user_info_uid")
                .expect("Can't find user_info_avatar in ui file.");

            uid.set_text(&self.uid.clone().unwrap_or_default());
            name.set_text(&self.username.clone().unwrap_or_default());

            /* remove all old avatar from the popover */
            for w in avatar.get_children().iter() {
                avatar.remove(w);
            }

            download_to_cache(self.backend.clone(), self.uid.clone().unwrap_or_default());
            let w = widgets::Avatar::avatar_new(Some(40));
            w.circle(self.uid.clone().unwrap_or_default(), self.username.clone(), 40);
            avatar.add(&w);
            stack.set_visible_child_name("info");
        }
        else {
            stack.set_visible_child_name("spinner");
        }

        /* update user menu button avatar */
        let button = self.ui.builder
            .get_object::<gtk::MenuButton>("user_menu_button")
            .expect("Can't find user_menu_button in ui file.");

        let eb = gtk::EventBox::new();
        match self.avatar.clone() {
            Some(_) => {
                download_to_cache(self.backend.clone(), self.uid.clone().unwrap_or_default());
                let w = widgets::Avatar::avatar_new(Some(24));
                w.circle(self.uid.clone().unwrap_or_default(), self.username.clone(), 24);
                eb.add(&w);
            }
            None => {
                let w = gtk::Spinner::new();
                w.show();
                w.start();
                eb.add(&w);
            }
        };

        eb.connect_button_press_event(move |_, _| { Inhibit(false) });
        button.set_image(&eb);
    }

    pub fn set_username(&mut self, username: Option<String>) {
        self.username = username;
        self.show_user_info();
    }

    pub fn set_uid(&mut self, uid: Option<String>) {
        self.uid = uid;
        self.show_user_info();
    }

    pub fn set_avatar(&mut self, fname: Option<String>) {
        self.avatar = fname;
        self.show_user_info();
    }
}
