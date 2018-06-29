extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;
use appop::AppState;

use backend::BKCommand;
use fractal_api::types::Member;

use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::{Receiver, Sender};

use widgets;
use widgets::members_list::MembersList;
use widgets::AvatarExt;

impl AppOp {
    pub fn show_room_settings(&mut self) {
        //check for type we have to show
        self.init_room_settings();
        self.set_state(AppState::RoomSettings);
    }

    pub fn close_room_settings(&mut self) {
        let scroll = self.ui
            .builder
            .get_object::<gtk::ScrolledWindow>("room_settings_scroll")
            .expect("Can't find room_settings_scroll in ui file.");
        let b = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_members_list")
            .expect("Can't find room_settings_members_list in ui file.");
        for w in b.get_children().iter() {
            b.remove(w);
        }
        if let Some(adj) = scroll.get_vadjustment() {
            adj.set_value(0f64);
        }
        self.set_state(AppState::Chat);
    }

    fn init_room_settings(&mut self) -> Option<()> {
        let room = self.rooms.get(&self.active_room.clone()?)?;
        let avatar = room.avatar.clone();
        let name = room.name.clone();
        let topic = room.topic.clone();
        let mut is_room = true;
        let mut is_group = false;
        let members: Vec<Member> = room.members.values().cloned().collect();
        let power = *room.power_levels.get(&self.uid.clone()?).unwrap_or(&0);

        let edit = power >= 50 && !room.direct;

        let description = if room.direct {
            is_room = false;
            is_group = false;
            self.get_direct_partner_uid(members.clone())
        } else {
            /* we don't have private groups yet
               let description = Some(format!("Private Group - {} members", members.len()));
               */
            //Some(format!("Public Room - {} members", members.len()))
            Some(format!("Room - {} members", members.len()))
        };

        self.room_settings_show_avatar(avatar, edit);
        self.room_settings_show_room_name(name, edit);
        self.room_settings_show_room_topic(topic, is_room, edit);
        self.room_settings_show_room_type(description);
        self.room_settings_show_members(members);

        /* admin parts */
        self.room_settings_show_group_room(is_room || is_group);
        self.room_settings_show_admin_groupe(is_group && edit);
        self.room_settings_show_admin_room(is_room && edit);
        self.room_settings_hide_not_implemented_widgets();

        None
    }

    /* returns the uid of the fisrt member in the room, ignoring the current user */
    fn get_direct_partner_uid(&self, members: Vec<Member>) -> Option<String> {
        let mut uid = None;
        for member in members {
            if member.uid != self.uid.clone()? {
                uid = Some(member.uid);
                break;
            }
        }
        uid
    }

    pub fn room_settings_show_room_name(&self, text: Option<String>, edit: bool) -> Option<()> {
        let label = self.ui
            .builder
            .get_object::<gtk::Label>("room_settings_room_name")
            .expect("Can't find room_settings_room_name in ui file.");
        let b = self.ui
            .builder
            .get_object::<gtk::Box>("room_settings_room_name_box")
            .expect("Can't find room_settings_room_topic_entry in ui file.");
        let entry = self.ui
            .builder
            .get_object::<gtk::Entry>("room_settings_room_name_entry")
            .expect("Can't find room_settings_room_name_entry in ui file.");
        let button = self.ui
            .builder
            .get_object::<gtk::Button>("room_settings_room_name_button")
            .expect("Can't find room_settings_room_name_button in ui file.");

        if edit {
            if let Some(text) = text {
                entry.set_text(&text);
            } else {
                entry.set_text("");
            }
            label.hide();
            entry.set_editable(true);
            self.reset_action_button(button);
            b.show();
        } else {
            if let Some(text) = text {
                label.set_text(&text);
            } else {
                label.set_text("Noname");
            }
            b.hide();
            label.show();
        }
        None
    }

    pub fn reset_action_button(&self, button: gtk::Button) {
        let image = gtk::Image::new_from_icon_name("emblem-ok-symbolic", 1);
        button.set_image(&image);
        button.set_sensitive(true);
    }

    pub fn room_settings_show_room_topic(
        &self,
        text: Option<String>,
        is_room: bool,
        edit: bool,
    ) -> Option<()> {
        let label = self.ui
            .builder
            .get_object::<gtk::Label>("room_settings_room_topic")
            .expect("Can't find room_settings_room_topic in ui file.");
        let b = self.ui
            .builder
            .get_object::<gtk::Box>("room_settings_room_topic_box")
            .expect("Can't find room_settings_room_topic_entry in ui file.");
        let entry = self.ui
            .builder
            .get_object::<gtk::Entry>("room_settings_room_topic_entry")
            .expect("Can't find room_settings_room_topic_entry in ui file.");
        let button = self.ui
            .builder
            .get_object::<gtk::Button>("room_settings_room_topic_button")
            .expect("Can't find room_settings_room_topic_button in ui file.");

        if is_room {
            if edit {
                if let Some(text) = text {
                    entry.set_text(&text);
                } else {
                    entry.set_text("");
                }
                label.hide();
                entry.set_editable(true);
                self.reset_action_button(button);
                b.show();
            } else {
                b.hide();
                if let Some(text) = text {
                    label.set_text(&text);
                    label.show();
                } else {
                    label.hide();
                }
            }
        } else {
            b.hide();
            label.hide();
        }

        None
    }

    pub fn room_settings_show_group_room(&self, show: bool) -> Option<()> {
        let notify = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_notification_sounds")
            .expect("Can't find room_settings_notification_sounds in ui file.");
        let invite = self.ui
            .builder
            .get_object::<gtk::Button>("room_settings_invite")
            .expect("Can't find room_settings_invite in ui file.");

        if show {
            notify.show();
            invite.show();
        } else {
            notify.hide();
            invite.hide();
        }

        None
    }

    pub fn room_settings_show_admin_groupe(&self, show: bool) -> Option<()> {
        let history = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_history_visibility")
            .expect("Can't find room_settings_history_visibility in ui file.");

        if show {
            history.show();
        } else {
            history.hide();
        }

        None
    }

    pub fn room_settings_show_admin_room(&self, show: bool) -> Option<()> {
        let room = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_room_visibility")
            .expect("Can't find room_settings_room_visibility in ui file.");
        let join = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_join")
            .expect("Can't find room_settings_join in ui file.");

        if show {
            room.show();
            join.show();
        } else {
            room.hide();
            join.hide();
        }

        None
    }

    pub fn room_settings_show_room_type(&self, text: Option<String>) -> Option<()> {
        let label = self.ui
            .builder
            .get_object::<gtk::Label>("room_settings_room_description")
            .expect("Can't find room_settings_room_name in ui file.");

        if let Some(text) = text {
            label.set_text(&text);
            label.show();
        } else {
            label.hide();
        }
        None
    }

    fn room_settings_show_avatar(&self, avatar: Option<String>, edit: bool) -> Option<()> {
        let container = self.ui
            .builder
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
        let image = widgets::Avatar::circle_avatar(avatar.clone()?, Some(100));

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

    pub fn update_room_avatar(&mut self, file: String) -> Option<()> {
        let avatar_spinner = self.ui
            .builder
            .get_object::<gtk::Spinner>("room_settings_avatar_spinner")
            .expect("Can't find room_settings_avatar_spinner in ui file.");
        let avatar_btn = self.ui
            .builder
            .get_object::<gtk::Button>("room_settings_avatar_button")
            .expect("Can't find room_settings_avatar_button in ui file.");
        let room = self.rooms.get(&self.active_room.clone()?)?;
        let command = BKCommand::SetRoomAvatar(room.id.clone(), file.clone());
        self.backend.send(command).unwrap();
        self.room_settings_show_avatar(Some(file), true);
        avatar_btn.set_sensitive(false);
        avatar_spinner.show();
        None
    }

    pub fn update_room_name(&mut self) -> Option<()> {
        let entry = self.ui
            .builder
            .get_object::<gtk::Entry>("room_settings_room_name_entry")
            .expect("Can't find room_settings_name_entry in ui file.");
        let button = self.ui
            .builder
            .get_object::<gtk::Button>("room_settings_room_name_button")
            .expect("Can't find room_settings_name_button in ui file.");

        let new_name = entry.get_text()?;
        let room = self.rooms.get(&self.active_room.clone()?)?;

        let spinner = gtk::Spinner::new();
        spinner.start();
        button.set_image(&spinner);
        button.set_sensitive(false);
        entry.set_editable(false);

        let command = BKCommand::SetRoomName(room.id.clone(), new_name.clone());
        self.backend.send(command).unwrap();

        None
    }

    pub fn validate_room_name(&self, new_name: Option<String>) -> Option<String> {
        let room = self.rooms.get(&self.active_room.clone()?)?;
        let old_name = room.name.clone()?;
        let new_name = new_name?;
        if new_name != "" && new_name != old_name {
            return Some(new_name);
        }

        None
    }

    pub fn validate_room_topic(&self, new_name: Option<String>) -> Option<String> {
        let room = self.rooms.get(&self.active_room.clone()?)?;
        let old_name = room.topic.clone()?;
        let new_name = new_name?;
        if new_name != "" && new_name != old_name {
            return Some(new_name);
        }

        None
    }

    pub fn update_room_topic(&mut self) -> Option<()> {
        let name = self.ui
            .builder
            .get_object::<gtk::Entry>("room_settings_room_topic_entry")
            .expect("Can't find room_settings_topic in ui file.");
        let button = self.ui
            .builder
            .get_object::<gtk::Button>("room_settings_room_topic_button")
            .expect("Can't find room_settings_topic_button in ui file.");
        let topic = name.get_text()?;

        let room = self.rooms.get(&self.active_room.clone()?)?;

        let spinner = gtk::Spinner::new();
        spinner.start();
        button.set_image(&spinner);
        button.set_sensitive(false);
        name.set_editable(false);

        let command = BKCommand::SetRoomTopic(room.id.clone(), topic.clone());
        self.backend.send(command).unwrap();

        None
    }

    pub fn show_new_room_avatar(&self) {
        let avatar_spinner = self.ui
            .builder
            .get_object::<gtk::Spinner>("room_settings_avatar_spinner")
            .expect("Can't find room_settings_avatar_spinner in ui file.");
        let avatar_btn = self.ui
            .builder
            .get_object::<gtk::Button>("room_settings_avatar_button")
            .expect("Can't find room_settings_avatar_button in ui file.");

        /* We could update the avatar for this room,
         * but we are waiting for the new avatar event */
        avatar_spinner.hide();
        avatar_btn.set_sensitive(true);
    }

    pub fn show_new_room_name(&self) {
        let entry = self.ui
            .builder
            .get_object::<gtk::Entry>("room_settings_room_name_entry")
            .expect("Can't find room_settings_room_name_entry in ui file.");
        let button = self.ui
            .builder
            .get_object::<gtk::Button>("room_settings_room_name_button")
            .expect("Can't find room_settings_name_button in ui file.");
        button.hide();
        entry.set_editable(true);
        self.reset_action_button(button);
    }

    pub fn show_new_room_topic(&self) {
        let entry = self.ui
            .builder
            .get_object::<gtk::Entry>("room_settings_room_topic_entry")
            .expect("Can't find room_settings_room_topic_entry in ui file.");
        let button = self.ui
            .builder
            .get_object::<gtk::Button>("room_settings_room_topic_button")
            .expect("Can't find room_settings_topic_button in ui file.");
        button.hide();
        entry.set_editable(true);
        self.reset_action_button(button);
    }

    fn room_settings_hide_not_implemented_widgets(&self) -> Option<()> {
        let notification = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_notification_sounds")
            .expect("Can't find room_settings_notification_sounds in ui file.");
        let media = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_media")
            .expect("Can't find room_settings_media in ui file.");
        let switch = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_notification_switch")
            .expect("Can't find room_settings_notification_switch in ui file.");
        let history = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_history_visibility")
            .expect("Can't find room_settings_history_visibility in ui file.");
        let join = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_join")
            .expect("Can't find room_settings_join in ui file.");
        notification.hide();
        media.hide();
        switch.hide();
        history.hide();
        join.hide();

        None
    }

    fn room_settings_show_members(&self, members: Vec<Member>) -> Option<()> {
        let entry = self.ui
            .builder
            .get_object::<gtk::SearchEntry>("room_settings_members_search")
            .expect("Can't find room_settings_members_search in ui file.");
        let b = self.ui
            .builder
            .get_object::<gtk::Frame>("room_settings_members_list")
            .expect("Can't find room_settings_members_list in ui file.");
        let label = self.ui
            .builder
            .get_object::<gtk::Label>("room_settings_member_list_title")
            .expect("Can't find room_settings_member_list_title in ui file.");
        for w in b.get_children().iter() {
            b.remove(w);
        }
        label.set_text(&format!("{} members", members.len()));
        let list = widgets::MembersList::new(members.clone(), entry);
        let w = list.create()?;

        /* ask for all avatars */
        for (i, member) in members.iter().enumerate() {
            account_settings_get_member_info(
                self.backend.clone(),
                member.uid.clone(),
                i,
                list.clone(),
            );
        }
        b.add(&w);
        None
    }
}

/* this funtion should be moved to the backend */
pub fn account_settings_get_member_info(
    backend: Sender<BKCommand>,
    sender: String,
    index: usize,
    list: MembersList,
) {
    let (tx, rx): (Sender<(String, String)>, Receiver<(String, String)>) = channel();
    backend
        .send(BKCommand::GetUserInfoAsync(sender.clone(), tx))
        .unwrap();
    gtk::timeout_add(100, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => gtk::Continue(false),
        Ok((_name, _avatar)) => {
            /* update UI */
            /*println!("Update user {} with {}", sender, avatar); */
            list.update(index);
            gtk::Continue(false)
        }
    });
}
