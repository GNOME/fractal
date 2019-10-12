use crate::i18n::{i18n, i18n_k, ni18n_f};
use log::{error, warn};
use std::fs::remove_file;
use std::os::unix::fs;
use url::Url;

use gtk;
use gtk::prelude::*;

use crate::appop::AppOp;

use crate::backend;
use crate::backend::BKCommand;
use fractal_api::util::cache_dir_path;

use crate::actions;
use crate::actions::AppState;
use crate::cache;
use crate::widgets;

use crate::types::{Member, Reason, Room, RoomMembership, RoomTag};

use crate::util::markup_text;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use glib::functions::markup_escape_text;

pub struct Force(pub bool);

impl AppOp {
    pub fn remove_room(&mut self, id: String) {
        self.rooms.remove(&id);
        self.unsent_messages.remove(&id);
        self.roomlist.remove_room(id);
    }

    pub fn set_rooms(&mut self, mut rooms: Vec<Room>, clear_room_list: bool) {
        if clear_room_list {
            self.rooms.clear();
        }
        let mut roomlist = vec![];
        while let Some(room) = rooms.pop() {
            if room.membership.is_left() {
                // removing left rooms
                if let RoomMembership::Left(kicked) = room.membership.clone() {
                    if let Reason::Kicked(reason, kicker) = kicked {
                        if let Some(r) = self.rooms.get(&room.id) {
                            let room_name = r.name.clone().unwrap_or_default();
                            self.kicked_room(room_name, reason, kicker.alias.unwrap_or_default());
                        }
                    }
                }
                if self.active_room.as_ref().map_or(false, |x| x == &room.id) {
                    self.really_leave_active_room();
                } else {
                    self.remove_room(room.id);
                }
            } else if self.rooms.contains_key(&room.id) {
                // TODO: update the existing rooms
                let update_room = self.rooms.get_mut(&room.id).unwrap();
                let typing_users: Vec<Member> = room
                    .typing_users
                    .iter()
                    .map(|u| update_room.members.get(&u.uid).unwrap_or(&u).to_owned())
                    .collect();
                update_room.typing_users = typing_users;
                self.update_typing_notification();
            } else {
                // Request all joined members for each new room
                self.backend
                    .send(BKCommand::GetRoomMembers(room.id.clone()))
                    .unwrap();
                // Download the room avatar
                // TODO: Use the avatar url returned by sync
                self.backend
                    .send(BKCommand::GetRoomAvatar(room.id.clone()))
                    .unwrap();
                if clear_room_list {
                    roomlist.push(room.clone());
                } else {
                    self.roomlist.add_room(room.clone());
                    self.roomlist.moveup(room.id.clone());
                }
                self.rooms.insert(room.id.clone(), room);
            }
        }

        if clear_room_list {
            let container: gtk::Box = self
                .ui
                .builder
                .get_object("room_container")
                .expect("Couldn't find room_container in ui file.");

            for ch in container.get_children().iter() {
                container.remove(ch);
            }

            let scrolledwindow: gtk::ScrolledWindow = self
                .ui
                .builder
                .get_object("roomlist_scroll")
                .expect("Couldn't find room_container in ui file.");
            let adj = scrolledwindow.get_vadjustment();
            scrolledwindow.get_child().map(|child| {
                child.downcast_ref::<gtk::Container>().map(|container| {
                    adj.clone().map(|a| container.set_focus_vadjustment(&a));
                });
            });

            self.roomlist = widgets::RoomList::new(adj, Some(self.server_url.to_string()));
            self.roomlist.add_rooms(roomlist);
            container.add(self.roomlist.widget());

            let bk = self.backend.clone();
            self.roomlist.connect_fav(move |room, tofav| {
                bk.send(BKCommand::AddToFav(room.id.clone(), tofav))
                    .unwrap();
            });
            // Select active room in the sidebar
            if let Some(ref active_room) = self.active_room {
                self.set_active_room_by_id(active_room.clone());
            }
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

            let msg_entry = self.ui.sventry.view.clone();
            let msg_entry_stack = self
                .ui
                .sventry_box
                .clone()
                .downcast::<gtk::Stack>()
                .unwrap();

            let user_power = match room.admins.get(&self.uid.clone().unwrap_or_default()) {
                Some(p) => *p,
                None => room
                    .power_levels
                    .get("users_default")
                    .map(|x| *x)
                    .unwrap_or(-1),
            };

            // No room admin information, assuming normal
            if user_power >= 0 || room.admins.len() == 0 {
                msg_entry.set_editable(true);
                msg_entry_stack.set_visible_child_name("Text Entry");

                if let Some(buffer) = msg_entry.get_buffer() {
                    let start = buffer.get_start_iter();
                    let end = buffer.get_end_iter();

                    if let Some(msg) = buffer.get_text(&start, &end, false) {
                        if let Some(ref active_room) = self.active_room {
                            if msg.len() > 0 {
                                if let Some(mark) = buffer.get_insert() {
                                    let iter = buffer.get_iter_at_mark(&mark);
                                    let msg_position = iter.get_offset();

                                    self.unsent_messages.insert(
                                        active_room.clone(),
                                        (msg.to_string(), msg_position),
                                    );
                                }
                            } else {
                                self.unsent_messages.remove(active_room);
                            }
                        }
                    }
                }
            } else {
                msg_entry.set_editable(false);
                msg_entry_stack.set_visible_child_name("Disabled Entry");
            }
        }

        self.clear_tmp_msgs();

        /* Transform id into the active_room */
        let active_room = id;
        // Select new active room in the sidebar
        self.roomlist.select(&active_room);

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
        self.set_state(AppState::Room);
        /* Mark the new active room as read */
        self.mark_last_message_as_read(Force(false));
        self.update_typing_notification();
    }

    pub fn really_leave_active_room(&mut self) {
        let r = self.active_room.clone().unwrap_or_default();
        self.backend.send(BKCommand::LeaveRoom(r.clone())).unwrap();
        self.rooms.remove(&r);
        self.active_room = None;
        self.clear_tmp_msgs();
        self.set_state(AppState::NoRoom);

        self.roomlist.remove_room(r);
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
            dialog.set_property_text(Some(text.as_str()));
            dialog.present();
        }
    }

    pub fn kicked_room(&self, roomid: String, reason: String, kicker: String) {
        let parent: gtk::Window = self
            .ui
            .builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");
        let parent_weak = parent.downgrade();
        let parent = upgrade_weak!(parent_weak);
        let viewer = widgets::KickedDialog::new();
        viewer.set_parent_window(&parent);
        viewer.show(&roomid, &reason, &kicker);
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

        let n = name
            .get_text()
            .map_or(String::new(), |gstr| gstr.to_string());
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
        self.set_state(AppState::Room);
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

    pub fn set_room_avatar(&mut self, roomid: String, avatar: Option<Url>) {
        if avatar.is_none() {
            if let Ok(dest) = cache_dir_path(None, &roomid) {
                let _ = remove_file(dest);
            }
        }
        if let Some(r) = self.rooms.get_mut(&roomid) {
            if avatar.is_none() && r.members.len() == 2 {
                if let Some(ref uid) = self.uid {
                    for m in r.members.keys() {
                        if m != uid {
                            //FIXME: Find a better solution
                            // create a symlink from user avatar to room avatar (works only on unix)
                            if let Ok(source) = cache_dir_path(None, m) {
                                if let Ok(dest) = cache_dir_path(None, &roomid) {
                                    let _ = fs::symlink(source, dest);
                                }
                            }
                        }
                    }
                }
            }
            r.avatar = avatar.map(|s| s.into_string());
            self.roomlist
                .set_room_avatar(roomid.clone(), r.avatar.clone());
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

    pub fn filter_rooms(&self, term: Option<String>) {
        self.roomlist.filter_rooms(term);
    }

    pub fn new_room_dialog(&self) {
        let dialog = self
            .ui
            .builder
            .get_object::<libhandy::Dialog>("new_room_dialog")
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
            .get_object::<libhandy::Dialog>("join_room_dialog")
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

        let n = name
            .get_text()
            .map_or(String::new(), |gstr| gstr.to_string())
            .trim()
            .to_string();

        self.backend.send(BKCommand::JoinRoom(n.clone())).unwrap();
    }

    pub fn new_room(&mut self, r: Room, internal_id: Option<String>) {
        if let Some(id) = internal_id {
            self.remove_room(id);
        }

        if !self.rooms.contains_key(&r.id) {
            self.rooms.insert(r.id.clone(), r.clone());
        }

        self.roomlist.add_room(r.clone());
        self.roomlist.moveup(r.id.clone());

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

        self.roomlist.rename_room(roomid.clone(), name);
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
                t.set_tooltip_text(None);
                n.set_tooltip_text(None);
                t.hide();
            }
            Some(ref topic) if topic.is_empty() => {
                t.set_tooltip_text(None);
                n.set_tooltip_text(None);
                t.hide();
            }
            Some(ref topic) => {
                n.set_tooltip_text(Some(&topic[..]));
                t.set_markup(&markup_text(&topic.split('\n').next().unwrap_or_default()));
                t.set_tooltip_text(Some(&topic[..]));
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

    pub fn update_typing_notification(&mut self) {
        if let Some(active_room) = &self
            .rooms
            .get(&self.active_room.clone().unwrap_or_default())
        {
            if let Some(ref mut history) = self.history {
                let typing_users = &active_room.typing_users;
                if typing_users.len() == 0 {
                    history.typing_notification("");
                } else if typing_users.len() > 2 {
                    history.typing_notification(&i18n("Several users are typing…"));
                } else {
                    let typing_string = ni18n_f(
                        "<b>{}</b> is typing…",
                        "<b>{}</b> and <b>{}</b> are typing…",
                        typing_users.len() as u32,
                        typing_users
                            .iter()
                            .map(|user| markup_escape_text(&user.get_alias()).to_string())
                            .collect::<Vec<String>>()
                            .iter()
                            .map(std::ops::Deref::deref)
                            .collect::<Vec<&str>>()
                            .as_slice(),
                    );
                    history.typing_notification(&typing_string);
                }
            }
        }
    }

    pub fn send_typing(&self) {
        if let Some(ref active_room) = self.active_room {
            self.backend
                .send(BKCommand::SendTyping(active_room.clone()))
                .unwrap();
        }
    }
}
