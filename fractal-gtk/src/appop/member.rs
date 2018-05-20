extern crate gtk;
extern crate gettextrs;

use self::gtk::prelude::*;
use self::gettextrs::{gettext, ngettext};

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
    pub fn clean_member_list(&self) {
        let mlist: gtk::ListBox = self.ui.builder
            .get_object("member_list")
            .expect("Couldn't find member_list in ui file.");

        let childs = mlist.get_children();
        let n = childs.len() - 1;
        for ch in childs.iter().take(n) {
            mlist.remove(ch);
        }
    }

    pub fn member_level(&self, member: &Member) -> i32 {
        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            if let Some(level) = r.power_levels.get(&member.uid) {
                return *level;
            }
        }

        0
    }

    pub fn show_members(&self, members: Vec<Member>) {
        self.clean_member_list();

        let mlist: gtk::ListBox = self.ui.builder
            .get_object("member_list")
            .expect("Couldn't find member_list in ui file.");

        let msg_entry: gtk::Entry = self.ui.builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");

        // limiting the number of members to show in the list
        for member in members.iter().take(self.member_limit) {
            let w;
            let m = member.clone();

            {
                let mb = widgets::MemberBox::new(&m, &self);
                w = mb.widget(false);
            }

            let msg = msg_entry.clone();
            w.connect_button_press_event(move |_, _| {
                if let Some(ref a) = m.alias {
                    let mut pos = msg.get_position();
                    msg.insert_text(&a.clone(), &mut pos);
                    pos = msg.get_text_length() as i32;
                    msg.grab_focus_without_selecting();
                    msg.set_position(pos);
                }
                glib::signal::Inhibit(true)
            });

            let p = mlist.get_children().len() - 1;
            mlist.insert(&w, p as i32);
        }

        if members.len() > self.member_limit {
            let sentence_template = ngettext("and one more", "and {member_count} more",
                                             (members.len() - self.member_limit) as u32);
            let newlabel = sentence_template.replace("{member_count}",
                                                     &(members.len() - self.member_limit).to_string());
            self.more_members_btn.set_label(&newlabel);
            self.more_members_btn.show();
        } else {
            self.more_members_btn.hide();
        }

        let members_count = self.ui.builder
            .get_object::<gtk::Label>("members_count")
            .expect("Can't find member_count in ui file.");
        members_count.set_text(&format!("{}", members.len()));
    }

    pub fn show_all_members(&self) {
        let inp: gtk::SearchEntry = self.ui.builder
            .get_object("members_search")
            .expect("Couldn't find members_searcn in ui file.");
        let text = inp.get_text();
        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            let members = match text {
                // all members if no search text
                None => r.members.values().cloned().collect(),
                Some(t) => {
                    // members with the text in the alias
                    r.members.values().filter(move |x| {
                        match x.alias {
                            None => false,
                            Some(ref a) => a.to_lowercase().contains(&t.to_lowercase())
                        }
                    }).cloned().collect()
                }
            };
            self.show_members(members);
        }
    }

    pub fn set_room_members(&mut self, roomid: String, members: Vec<Member>) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            r.members = HashMap::new();
            for m in members {
                r.members.insert(m.uid.clone(), m);
            }
        }

        self.recalculate_room_name(roomid.clone());

        if let Some(aroom) = self.active_room.clone() {
            if aroom == roomid {
                self.reload_members();
            }
        }
    }

    pub fn reload_members(&mut self) {
        self.clean_member_list();
        self.show_all_members();
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
                    avatar: Some(strn!(ev.content["avatar_url"].as_str().unwrap_or(""))),
                    alias: Some(strn!(ev.content["displayname"].as_str().unwrap_or(""))),
                    uid: sender.clone(),
                };
                if let Some(r) = self.rooms.get_mut(&ev.room.clone()) {
                    r.members.insert(m.uid.clone(), m.clone());
                }
            }
            // ignoring other memberships
            _ => {}
        }

        if ev.room != self.active_room.clone().unwrap_or_default() {
            // if it isn't the current room, this event we don't need to update the UI
            return;
        }

        match ev.content["membership"].as_str() {
            Some("leave") => {
                self.show_all_members();
            }
            Some("join") => {
                self.show_all_members();
            }
            // ignoring other memberships
            _ => {}
        }
    }

    pub fn user_search_finished(&self, users: Vec<Member>) {
        match self.search_type {
            SearchType::Invite => {
                let listbox = self.ui.builder
                    .get_object::<gtk::ListBox>("user_search_box")
                    .expect("Can't find user_search_box in ui file.");
                let scroll = self.ui.builder
                    .get_object::<gtk::Widget>("user_search_scroll")
                    .expect("Can't find user_search_scroll in ui file.");
                self.search_finished(users, listbox, scroll);
            },
            SearchType::DirectChat => {
                let listbox = self.ui.builder
                    .get_object::<gtk::ListBox>("direct_chat_search_box")
                    .expect("Can't find direct_chat_search_box in ui file.");
                let scroll = self.ui.builder
                    .get_object::<gtk::Widget>("direct_chat_search_scroll")
                    .expect("Can't find direct_chat_search_scroll in ui file.");
                self.search_finished(users, listbox, scroll);
            }
        }
    }

    pub fn search_finished(&self, users: Vec<Member>,
                           listbox: gtk::ListBox,
                           scroll: gtk::Widget) {
        for ch in listbox.get_children().iter() {
            listbox.remove(ch);
        }
        scroll.hide();

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
