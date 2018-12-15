use crate::i18n::{i18n, i18n_k};
use log::{error, warn};

use gtk;
use gtk::prelude::*;

use crate::appop::AppOp;

use crate::backend;
use crate::backend::BKCommand;

use crate::actions;
use crate::actions::AppState;
use crate::cache;
use crate::widgets;

use crate::types::{Room, RoomMembership, RoomTag};

use crate::util::markup_text;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

pub struct Force(pub bool);

#[derive(Debug, Clone)]
pub enum RoomPanel {
    Room,
    NoRoom,
}

impl AppOp {
    pub fn remove_room(&mut self, id: String) {
        self.rooms.remove(&id);
        self.unsent_messages.remove(&id);
    }

    pub fn set_rooms(&mut self, mut rooms: Vec<Room>, clear_room_list: bool) {
        if clear_room_list {
            self.rooms.clear();
        }
        while let Some(room) = rooms.pop() {
            if room.membership.is_left() {
                // removing left rooms
                if self.active_room.as_ref().map_or(false, |x| x == &room.id) {
                    self.really_leave_active_room();
                } else {
                    self.remove_room(room.id);
                }
            } else if self.rooms.contains_key(&room.id) {
                // TODO: update the existing rooms
            } else {
                if room.name.is_none() {
                    // This force the room name calculation for 1:1 rooms and for rooms with no name
                    self.backend
                        .send(BKCommand::GetRoomMembers(room.id.clone()))
                        .unwrap();
                }
                // Download the room avatar
                self.backend
                    .send(BKCommand::GetRoomAvatar(room.id.clone()))
                    .unwrap();
                self.rooms.insert(room.id.clone(), room);
                //TODO: add rooms to sidebar
            }
        }
        if clear_room_list {
            self.cache_rooms();
        }
    }

    pub fn reload_rooms(&mut self) {
        self.set_state(AppState::NoRoom);
    }

    pub fn set_active_room_by_id(&mut self, id: String) {
        if let Some(room) = self.rooms.get(&id) {
            if let RoomMembership::Invited(ref sender) = room.membership {
                self.show_inv_dialog(Some(sender), room.name.as_ref());
                self.invitation_roomid = Some(room.id.clone());
                return;
            }
        }
        self.room_panel(RoomPanel::Room);

        let msg_entry = self.ui.sventry.view.clone();
        if let Some(buffer) = msg_entry.get_buffer() {
            let start = buffer.get_start_iter();
            let end = buffer.get_end_iter();

            if let Some(msg) = buffer.get_text(&start, &end, false) {
                if let Some(ref active_room) = self.active_room {
                    if msg.len() > 0 {
                        if let Some(mark) = buffer.get_insert() {
                            let iter = buffer.get_iter_at_mark(&mark);
                            let msg_position = iter.get_offset();

                            self.unsent_messages
                                .insert(active_room.clone(), (msg, msg_position));
                        }
                    } else {
                        self.unsent_messages.remove(active_room);
                    }
                }
            }
        }

        self.clear_tmp_msgs();

        /* Transform id into the active_room */
        let active_room = id;
        // Select new active room in the sidebar
        //self.roomlist.select(&active_room);

        // getting room details
        self.backend
            .send(BKCommand::SetRoom(active_room.clone()))
            .unwrap();

        /* create the intitial list of messages to fill the new room history */
        let mut messages = vec![];
        if let Some(room) = self.rooms.get(&active_room) {
            for msg in room.messages.iter() {
                /* Make sure the message is from this room and not redacted */
                if msg.room == active_room && !msg.redacted {
                    let row = self.create_new_room_message(msg);
                    if let Some(row) = row {
                        messages.push(row);
                    }
                }
            }

            self.set_current_room_detail(String::from("m.room.name"), room.name.clone());
            self.set_current_room_detail(String::from("m.room.topic"), room.topic.clone());
        }

        self.append_tmp_msgs();

        /* make sure we remove the old room history first, because the lazy loading could try to
         * load messages */
        if let Some(history) = self.history.take() {
            history.destroy();
        }

        let actions = actions::RoomHistory::new(self.backend.clone(), self.ui.clone());
        let mut history = widgets::RoomHistory::new(actions, active_room.clone(), self);
        history.create(messages);
        self.history = Some(history);

        self.active_room = Some(active_room);
        /* Mark the new active room as read */
        self.mark_last_message_as_read(Force(false));
    }

    pub fn really_leave_active_room(&mut self) {
        let r = self.active_room.clone().unwrap_or_default();
        self.backend.send(BKCommand::LeaveRoom(r.clone())).unwrap();
        self.rooms.remove(&r);
        self.active_room = None;
        self.clear_tmp_msgs();
        self.room_panel(RoomPanel::NoRoom);
    }

    pub fn leave_active_room(&self) {
        let dialog = self
            .ui
            .builder
            .get_object::<gtk::MessageDialog>("leave_room_dialog")
            .expect("Can't find leave_room_dialog in ui file.");

        if let Some(r) = self
            .rooms
            .get(&self.active_room.clone().unwrap_or_default())
        {
            let text = i18n_k(
                "Leave {room_name}?",
                &[("room_name", &r.name.clone().unwrap_or_default())],
            );
            dialog.set_property_text(Some(&text));
            dialog.present();
        }
    }

    pub fn create_new_room(&mut self) {
        let name = self
            .ui
            .builder
            .get_object::<gtk::Entry>("new_room_name")
            .expect("Can't find new_room_name in ui file.");
        let private = self
            .ui
            .builder
            .get_object::<gtk::ToggleButton>("private_visibility_button")
            .expect("Can't find private_visibility_button in ui file.");

        let n = name.get_text().unwrap_or_default();

        // Since the switcher
        let p = if private.get_active() {
            backend::RoomType::Private
        } else {
            backend::RoomType::Public
        };

        let internal_id: String = thread_rng().sample_iter(&Alphanumeric).take(10).collect();
        self.backend
            .send(BKCommand::NewRoom(n.clone(), p, internal_id.clone()))
            .unwrap();

        let mut fakeroom = Room::new(internal_id.clone(), RoomMembership::Joined(RoomTag::None));
        fakeroom.name = Some(n);
        self.new_room(fakeroom, None);
        self.set_active_room_by_id(internal_id);
        self.room_panel(RoomPanel::Room);
    }

    pub fn room_panel(&self, t: RoomPanel) {
        let s = self
            .ui
            .builder
            .get_object::<gtk::Stack>("room_view_stack")
            .expect("Can't find room_view_stack in ui file.");
        let headerbar = self
            .ui
            .builder
            .get_object::<gtk::HeaderBar>("room_header_bar")
            .expect("Can't find room_header_bar in ui file.");

        let v = match t {
            RoomPanel::Room => "room_view",
            RoomPanel::NoRoom => "noroom",
        };

        s.set_visible_child_name(v);

        match v {
            "noroom" => {
                for ch in headerbar.get_children().iter() {
                    ch.hide();

                    // Select new active room in the sidebar
                    // self.roomlist.unselect();
                }
            }
            "room_view" => {
                for ch in headerbar.get_children().iter() {
                    ch.show();
                }

                self.ui.sventry.view.grab_focus();

                let active_room_id = self.active_room.clone().unwrap_or_default();
                let msg = self
                    .unsent_messages
                    .get(&active_room_id)
                    .cloned()
                    .unwrap_or_default();
                if let Some(buffer) = self.ui.sventry.view.get_buffer() {
                    buffer.set_text(&msg.0);

                    let iter = buffer.get_iter_at_offset(msg.1);
                    buffer.place_cursor(&iter);
                }
            }
            _ => {
                for ch in headerbar.get_children().iter() {
                    ch.show();
                }
            }
        }
    }

    pub fn cache_rooms(&self) {
        // serializing rooms
        let rooms = self.rooms.clone();
        let since = self.since.clone();
        let username = self.username.clone().unwrap_or_default();
        let uid = self.uid.clone().unwrap_or_default();
        let device_id = self.device_id.clone().unwrap_or_default();

        if let Err(_) = cache::store(&rooms, since, username, uid, device_id) {
            error!("Error caching rooms");
        };
    }

    pub fn set_room_detail(&mut self, roomid: String, key: String, value: Option<String>) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            let k: &str = &key;
            match k {
                "m.room.name" => {
                    r.name = value.clone();
                }
                "m.room.topic" => {
                    r.topic = value.clone();
                }
                _ => {}
            };
        }

        if roomid == self.active_room.clone().unwrap_or_default() {
            self.set_current_room_detail(key, value);
        }
    }

    pub fn set_room_avatar(&mut self, roomid: String, avatar: Option<String>) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            r.avatar = avatar;
        }
    }

    pub fn set_current_room_detail(&self, key: String, value: Option<String>) {
        let value = value.unwrap_or_default();
        let k: &str = &key;
        match k {
            "m.room.name" => {
                let name_label = self
                    .ui
                    .builder
                    .get_object::<gtk::Label>("room_name")
                    .expect("Can't find room_name in ui file.");

                name_label.set_text(&value);
            }
            "m.room.topic" => {
                self.set_room_topic_label(Some(value.clone()));
            }
            _ => warn!("no key {}", key),
        };
    }

    pub fn filter_rooms(&self, _term: Option<String>) {}

    pub fn new_room_dialog(&self) {
        let dialog = self
            .ui
            .builder
            .get_object::<gtk::Dialog>("new_room_dialog")
            .expect("Can't find new_room_dialog in ui file.");
        let btn = self
            .ui
            .builder
            .get_object::<gtk::Button>("new_room_button")
            .expect("Can't find new_room_button in ui file.");
        btn.set_sensitive(false);
        dialog.present();
    }

    pub fn join_to_room_dialog(&mut self) {
        let dialog = self
            .ui
            .builder
            .get_object::<gtk::Dialog>("join_room_dialog")
            .expect("Can't find join_room_dialog in ui file.");
        self.ui
            .builder
            .get_object::<gtk::Button>("join_room_button")
            .map(|btn| btn.set_sensitive(false));
        dialog.present();
    }

    pub fn join_to_room(&mut self) {
        let name = self
            .ui
            .builder
            .get_object::<gtk::Entry>("join_room_name")
            .expect("Can't find join_room_name in ui file.");

        let n = name.get_text().unwrap_or_default().trim().to_string();

        self.backend.send(BKCommand::JoinRoom(n.clone())).unwrap();
    }

    pub fn new_room(&mut self, r: Room, internal_id: Option<String>) {
        if let Some(id) = internal_id {
            self.remove_room(id);
        }

        if !self.rooms.contains_key(&r.id) {
            self.rooms.insert(r.id.clone(), r.clone());
        }

        self.set_active_room_by_id(r.id);
    }

    pub fn added_to_fav(&mut self, roomid: String, tofav: bool) {
        if let Some(ref mut r) = self.rooms.get_mut(&roomid) {
            let tag = if tofav {
                RoomTag::Favourite
            } else {
                RoomTag::None
            };
            r.membership = RoomMembership::Joined(tag);
        }
    }

    /// This method calculate the room name when there's no room name event
    /// For this we use the members in the room. If there's only one member we'll return that
    /// member name, if there's more than one we'll return the first one and others
    pub fn recalculate_room_name(&mut self, roomid: String) {
        if !self.rooms.contains_key(&roomid) {
            return;
        }

        let rname;
        {
            let r = self.rooms.get_mut(&roomid).unwrap();
            // we should do nothing it this room has room name
            if let Some(_) = r.name {
                return;
            }

            // removing one because the user should be in the room
            let n = r.members.len() - 1;
            let suid = self.uid.clone().unwrap_or_default();
            let mut members = r.members.iter().filter(|&(uid, _)| uid != &suid);

            let m1 = match members.next() {
                Some((_uid, m)) => m.get_alias(),
                None => String::new(),
            };

            let m2 = match members.next() {
                Some((_uid, m)) => m.get_alias(),
                None => String::new(),
            };

            let name = match n {
                0 => i18n("EMPTY ROOM"),
                1 => String::from(m1),
                2 => i18n_k("{m1} and {m2}", &[("m1", &m1), ("m2", &m2)]),
                _ => i18n_k("{m1} and Others", &[("m1", &m1)]),
            };

            r.name = Some(name);
            rname = r.name.clone();
        }

        self.room_name_change(roomid, rname);
    }

    pub fn room_name_change(&mut self, roomid: String, name: Option<String>) {
        if !self.rooms.contains_key(&roomid) {
            return;
        }

        {
            let r = self.rooms.get_mut(&roomid).unwrap();
            r.name = name.clone();
        }

        if roomid == self.active_room.clone().unwrap_or_default() {
            self.ui
                .builder
                .get_object::<gtk::Label>("room_name")
                .expect("Can't find room_name in ui file.")
                .set_text(&name.clone().unwrap_or_default());
        }
    }

    pub fn room_topic_change(&mut self, roomid: String, topic: Option<String>) {
        if !self.rooms.contains_key(&roomid) {
            return;
        }

        {
            let r = self.rooms.get_mut(&roomid).unwrap();
            r.topic = topic.clone();
        }

        if roomid == self.active_room.clone().unwrap_or_default() {
            self.set_room_topic_label(topic);
        }
    }

    pub fn set_room_topic_label(&self, topic: Option<String>) {
        let t = self
            .ui
            .builder
            .get_object::<gtk::Label>("room_topic")
            .expect("Can't find room_topic in ui file.");
        let n = self
            .ui
            .builder
            .get_object::<gtk::Label>("room_name")
            .expect("Can't find room_name in ui file.");

        match topic {
            None => {
                t.set_tooltip_text("");
                n.set_tooltip_text("");
                t.hide();
            }
            Some(ref topic) if topic.is_empty() => {
                t.set_tooltip_text("");
                n.set_tooltip_text("");
                t.hide();
            }
            Some(ref topic) => {
                n.set_tooltip_text(&topic[..]);
                t.set_markup(&markup_text(&topic.split('\n').next().unwrap_or_default()));
                t.set_tooltip_text(&topic[..]);
                t.show();
            }
        };
    }

    pub fn new_room_avatar(&self, roomid: String) {
        if !self.rooms.contains_key(&roomid) {
            return;
        }

        self.backend.send(BKCommand::GetRoomAvatar(roomid)).unwrap();
    }
}
