extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;
use appop::AppState;

use backend::BKCommand;
use fractal_api::types::Member;
use cache::download_to_cache;

use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};

use widgets;
use widgets::room_settings::RoomSettings;
use widgets::AvatarExt;

impl AppOp {
    pub fn show_room_settings(&mut self) {
        self.create_room_settings();
    }

    pub fn create_room_settings(&mut self) -> Option<()> {
        let stack = self.ui.builder
            .get_object::<gtk::Stack>("main_content_stack")
            .expect("Can't find main_content_stack in ui file.");
        let stack_header = self.ui.builder
            .get_object::<gtk::Stack>("headerbar_stack")
            .expect("Can't find headerbar_stack in ui file.");

        {
            let room = self.rooms.get(&self.active_room.clone()?)?;
            let mut panel = widgets::RoomSettings::new(self.backend.clone(), self.uid.clone(), room.clone());
            let (body, header) = panel.create()?;

            /* remove old panel */
            if let Some(widget) = stack.get_child_by_name("room-settings") {
                stack.remove(&widget);
            }
            if let Some(widget) = stack_header.get_child_by_name("room-settings") {
                stack_header.remove(&widget);
            }

            stack.add_named(&body, "room-settings");
            stack_header.add_named(&header, "room-settings");

            /* Headerbar */
            panel.get_back_button()?.connect_clicked(clone!(stack_header, stack => move |_| {
                /* we should use self.set_state(AppState::Chat); 
                 * we could maybe create an internal command */
                stack.set_visible_child_name("chat");
                stack_header.set_visible_child_name("normal");
            }));

            self.room_settings = Some(panel);
        }

        self.set_state(AppState::RoomSettings);

        None
    }

    fn room_settings_show_avatar(&self, _avatar: Option<String>, edit: bool) -> Option<()> {
        let container = self.ui.builder
            .get_object::<gtk::Box>("room_settings_avatar_box")
            .expect("Can't find room_settings_avatar_box in ui file.");
        let avatar_btn = self.ui
            .builder
            .get_object::<gtk::Button>("room_settings_avatar_button")
            .expect("Can't find room_settings_avatar_button in ui file.");

        for w in container.get_children().iter() {
            if w != &avatar_btn {
                container.remove(w);
            }
        }

        download_to_cache(self.backend.clone(), self.uid.clone().unwrap_or_default());
        let image = widgets::Avatar::avatar_new(Some(100));
        image.circle(self.uid.clone().unwrap_or_default(), self.username.clone(), 100);

        if edit {
            let overlay = self.ui
                .builder
                .get_object::<gtk::Overlay>("room_settings_avatar_overlay")
                .expect("Can't find room_settings_avatar_overlay in ui file.");
            let overlay_box = self.ui
                .builder
                .get_object::<gtk::Box>("room_settings_avatar")
                .expect("Can't find room_settings_avatar in ui file.");
            let avatar_spinner = self.ui
                .builder
                .get_object::<gtk::Spinner>("room_settings_avatar_spinner")
                .expect("Can't find room_settings_avatar_spinner in ui file.");
            /* remove all old avatar */
            for w in overlay_box.get_children().iter() {
                overlay_box.remove(w);
            }
            overlay_box.add(&image);
            overlay.show();
            avatar_spinner.hide();
            avatar_btn.set_sensitive(true);
            /*Hack for button bug */
            avatar_btn.hide();
            avatar_btn.show();
        } else {
            avatar_btn.hide();
            container.add(&image);
        }

        return None;
    }

    pub fn close_room_settings(&mut self) {
        self.set_state(AppState::Chat);
    }

    pub fn show_new_room_avatar(&self) -> Option<()> {
        let panel = self.room_settings.clone()?;
        panel.show_new_room_avatar();
        None
    }

    pub fn show_new_room_name(&self) -> Option<()> {
        let panel = self.room_settings.clone()?;
        panel.show_new_room_name();
        None
    }

    pub fn show_new_room_topic(&self) -> Option<()> {
        let panel = self.room_settings.clone()?;
        panel.show_new_room_topic();
        None
    }

    pub fn update_members_list(&self, uid: String) -> Option<()> {
        self.room_settings.clone()?.update_members_list(uid);
        None
    }

    pub fn request_members_list(&self) -> Option<()> {
        let room = self.rooms.get(&self.active_room.clone()?)?;
        let members: Vec<Member> = room.members.values().cloned().collect();
        for (i, member) in members.iter().enumerate() {
            account_settings_get_member_info(
                self.backend.clone(),
                member.uid.clone());
        }

        None
    }
}

/* this funtion should be moved to the backend */
pub fn account_settings_get_member_info(
    backend: Sender<BKCommand>,
    sender: String,
    ) {
    let (tx, rx): (Sender<(String, String)>, Receiver<(String, String)>) = channel();
    backend
        .send(BKCommand::GetUserInfoAsync(sender.clone(), Some(tx)))
        .unwrap();
    gtk::timeout_add(100, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => gtk::Continue(false),
        Ok((_name, _avatar)) => {
            /* update UI */
            //view.update_members_list(index);
            //APPOP!(update_members_list, (sender));
            gtk::Continue(false)
        }
    });
}
