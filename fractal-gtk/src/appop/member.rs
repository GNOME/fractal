extern crate gtk;

use self::gtk::prelude::*;

use std::collections::HashMap;

use appop::AppOp;
use app::InternalCommand;
use glib;
use widgets;
use backend::BKCommand;

use types::Member;
use types::Event;


#[derive(Debug, Clone)]
pub enum SearchType {
    Invite,
    DirectChat,
}


impl AppOp {
    pub fn member_level(&self, member: &Member) -> i32 {
        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            if let Some(level) = r.power_levels.get(&member.uid) {
                return *level;
            }
        }
        0
    }

    pub fn set_room_members(&mut self, roomid: String, members: Vec<Member>) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            r.members = HashMap::new();
            for m in members {
                r.members.insert(m.uid.clone(), m);
            }
        }

        self.recalculate_room_name(roomid.clone());

        /* FIXME: update the current room settings insteat of creating a new one */
        if self.room_settings.is_some() {
            self.create_room_settings();
        }
    }

    pub fn room_member_event(&mut self, ev: Event) {
        // NOTE: maybe we should show this events in the message list to notify enters and leaves
        // to the user

        let sender = ev.sender.clone();
        match ev.content["membership"].as_str() {
            Some("leave") => {
                if let Some(r) = self.rooms.get_mut(&ev.room.clone()) {
                    r.members.remove(&sender);
                }
            }
            Some("join") => {
                let m = Member {
                    avatar: Some(String::from(ev.content["avatar_url"].as_str().unwrap_or(""))),
                    alias: Some(String::from(ev.content["displayname"].as_str().unwrap_or(""))),
                    uid: sender.clone(),
                };
                if let Some(r) = self.rooms.get_mut(&ev.room.clone()) {
                    r.members.insert(m.uid.clone(), m.clone());
                }
            }
            // ignoring other memberships
            _ => {}
        }
    }

    pub fn user_search_finished(&self, users: Vec<Member>) {
        match self.search_type {
            SearchType::Invite => {
                let entry = self.ui.builder
                    .get_object::<gtk::TextView>("invite_textview")
                    .expect("Can't find invite_textview in ui file.");
                let listbox = self.ui.builder
                    .get_object::<gtk::ListBox>("user_search_box")
                    .expect("Can't find user_search_box in ui file.");
                let scroll = self.ui.builder
                    .get_object::<gtk::Widget>("user_search_scroll")
                    .expect("Can't find user_search_scroll in ui file.");

                if let Some(buffer) = entry.get_buffer() {
                    let start = buffer.get_start_iter();
                    let end = buffer.get_end_iter();

                    self.search_finished(users, listbox, scroll, buffer.get_text(&start, &end, false));
                }
            },
            SearchType::DirectChat => {
                let entry = self.ui.builder
                    .get_object::<gtk::TextView>("to_chat_textview")
                    .expect("Can't find to_chat_textview in ui file.");
                let listbox = self.ui.builder
                    .get_object::<gtk::ListBox>("direct_chat_search_box")
                    .expect("Can't find direct_chat_search_box in ui file.");
                let scroll = self.ui.builder
                    .get_object::<gtk::Widget>("direct_chat_search_scroll")
                    .expect("Can't find direct_chat_search_scroll in ui file.");

                if let Some(buffer) = entry.get_buffer() {
                    let start = buffer.get_start_iter();
                    let end = buffer.get_end_iter();

                    self.search_finished(users, listbox, scroll, buffer.get_text(&start, &end, false));
                }
            }
        }
    }

    pub fn search_finished(&self, mut users: Vec<Member>,
                           listbox: gtk::ListBox,
                           scroll: gtk::Widget,
                           term: Option<String>) {
        for ch in listbox.get_children().iter() {
            listbox.remove(ch);
        }
        scroll.hide();

        let t = term.unwrap_or_default();
        let uid_in_term = t.contains("@") && t.contains(":");
        // Adding a new user if the user
        if uid_in_term && !users.iter().find(|u| u.uid == t).is_some() {
            let member = Member{ avatar: None, alias: None, uid: t };
            users.insert(0, member);
        }

        for (i, u) in users.iter().enumerate() {
            let w;
            {
                let mb = widgets::MemberBox::new(u, &self);
                w = mb.widget(true);
            }

            let tx = self.internal.clone();
            w.connect_button_press_event(clone!(u => move |_, _| {
                tx.send(InternalCommand::ToInvite(u.clone())).unwrap();
                glib::signal::Inhibit(true)
            }));

            listbox.insert(&w, i as i32);
            scroll.show();
        }
    }

    pub fn search_invite_user(&self, term: Option<String>) {
        if let Some(t) = term {
            self.backend.send(BKCommand::UserSearch(t)).unwrap();
        }
    }
}
