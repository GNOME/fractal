use crate::i18n::{i18n, i18n_k, ni18n_f};
use fractal_api::backend::room;
use fractal_api::identifiers::RoomId;
use fractal_api::url::Url;
use fractal_api::util::ResultExpectLog;
use log::{error, warn};
use std::convert::TryFrom;
use std::fs::remove_file;
use std::os::unix::fs;
use std::thread;

use gtk::prelude::*;

use crate::app::App;
use crate::appop::AppOp;

use crate::backend;
use crate::backend::BKCommand;
use crate::backend::BKResponse;
use fractal_api::util::cache_dir_path;

use crate::actions;
use crate::actions::AppState;
use crate::cache;
use crate::widgets;

use crate::types::{Member, Reason, Room, RoomMembership, RoomTag};

use crate::util::markup_text;

use glib::markup_escape_text;

// The TextBufferExt alias is necessary to avoid conflict with gtk's TextBufferExt
use gspell::{CheckerExt, TextBuffer, TextBufferExt as GspellTextBufferExt};

use std::time::Instant;

pub struct Force(pub bool);

impl AppOp {
    pub fn remove_room(&mut self, id: RoomId) {
        self.rooms.remove(&id);
        self.unsent_messages.remove(&id);
        self.roomlist.remove_room(id);
    }

    pub fn set_rooms(&mut self, rooms: Vec<Room>, clear_room_list: bool) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        if clear_room_list {
            self.rooms.clear();
        }
        let mut roomlist = vec![];
        for room in rooms {
            // removing left rooms
            if let RoomMembership::Left(kicked) = room.membership.clone() {
                if let Reason::Kicked(reason, kicker) = kicked {
                    if let Some(r) = self.rooms.get(&room.id) {
                        let room_name = r.name.clone().unwrap_or_default();
                        self.kicked_room(room_name, reason, kicker.alias.unwrap_or_default());
                    }
                }
                if self.active_room.as_ref().map_or(false, |x| x == &room.id) {
                    self.really_leave_active_room();
                } else {
                    self.remove_room(room.id);
                }
            } else if let Some(update_room) = self.rooms.get_mut(&room.id) {
                // TODO: update the existing rooms
                if room.language.is_some() {
                    update_room.language = room.language.clone();
                };

                let typing_users: Vec<Member> = room
                    .typing_users
                    .iter()
                    .map(|u| update_room.members.get(&u.uid).unwrap_or(&u).to_owned())
                    .collect();
                update_room.typing_users = typing_users;
                self.update_typing_notification();
            } else {
                // Request all joined members for each new room
                let server = login_data.server_url.clone();
                let access_token = login_data.access_token.clone();
                let room_id = room.id.clone();
                let tx = self.backend.clone();
                thread::spawn(move || {
                    match room::get_room_members(server, access_token, room_id) {
                        Ok((room, members)) => {
                            APPOP!(set_room_members, (room, members));
                        }
                        Err(err) => {
                            tx.send(BKCommand::SendBKResponse(BKResponse::RoomMembersError(err)))
                                .expect_log("Connection closed");
                        }
                    }
                });
                // Download the room avatar
                // TODO: Use the avatar url returned by sync
                let server = login_data.server_url.clone();
                let access_token = login_data.access_token.clone();
                let room_id = room.id.clone();
                let tx = self.backend.clone();
                thread::spawn(
                    move || match room::get_room_avatar(server, access_token, room_id) {
                        Ok((room, avatar)) => {
                            APPOP!(set_room_avatar, (room, avatar));
                        }
                        Err(err) => {
                            tx.send(BKCommand::SendBKResponse(BKResponse::RoomAvatar(Err(err))))
                                .expect_log("Connection closed");
                        }
                    },
                );
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

            self.roomlist = widgets::RoomList::new(adj, Some(login_data.server_url.clone()));
            self.roomlist.add_rooms(roomlist);
            container.add(self.roomlist.widget());

            let bk = self.backend.clone();
            self.roomlist.connect_fav(move |room, tofav| {
                let tx = bk.clone();
                let server = login_data.server_url.clone();
                let access_token = login_data.access_token.clone();
                let uid = login_data.uid.clone();
                thread::spawn(move || {
                    match room::add_to_fav(server, access_token, uid, room.id, tofav) {
                        Ok((r, tofav)) => {
                            APPOP!(added_to_fav, (r, tofav));
                        }
                        Err(err) => {
                            tx.send(BKCommand::SendBKResponse(BKResponse::AddedToFavError(err)))
                                .expect_log("Connection closed");
                        }
                    }
                });
            });

            // Select active room in the sidebar
            if let Some(active_room) = self.active_room.clone() {
                self.set_active_room_by_id(active_room);
            }
            self.cache_rooms();
        }
    }

    pub fn reload_rooms(&mut self) {
        self.set_state(AppState::NoRoom);
    }

    pub fn set_active_room_by_id(&mut self, id: RoomId) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        if let Some(room) = self.rooms.get(&id) {
            if let Some(language) = room.language.clone() {
                self.set_language(language);
            }
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

            let user_power = room
                .admins
                .get(&login_data.uid)
                .copied()
                .unwrap_or(room.default_power_level);

            // No room admin information, assuming normal
            if user_power >= 0 || room.admins.is_empty() {
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
            .send(BKCommand::SetRoom(
                login_data.server_url.clone(),
                login_data.access_token.clone(),
                active_room.clone(),
            ))
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

        let back_history = self.room_back_history.clone();
        let actions = actions::Message::new(
            self.backend.clone(),
            login_data.server_url,
            login_data.access_token,
            self.ui.clone(),
            back_history,
        );
        let history = widgets::RoomHistory::new(actions, active_room.clone(), self);
        self.history = if let Some(mut history) = history {
            history.create(messages);
            Some(history)
        } else {
            None
        };

        self.active_room = Some(active_room);
        self.set_state(AppState::Room);
        /* Mark the new active room as read */
        self.mark_last_message_as_read(Force(false));
        self.update_typing_notification();
    }

    // FIXME: This should be a special case in a generic
    //        function that leaves any room in any state.
    pub fn really_leave_active_room(&mut self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let r = unwrap_or_unit_return!(self.active_room.clone());
        let room_id = r.clone();
        let tx = self.backend.clone();
        thread::spawn(move || {
            let query = room::leave_room(login_data.server_url, login_data.access_token, room_id);
            if let Err(err) = query {
                tx.send(BKCommand::SendBKResponse(BKResponse::LeaveRoomError(err)))
                    .expect_log("Connection closed");
            }
        });
        self.rooms.remove(&r);
        self.active_room = None;
        self.clear_tmp_msgs();
        self.set_state(AppState::NoRoom);

        self.roomlist.remove_room(r);
    }

    pub fn leave_active_room(&self) {
        let active_room = unwrap_or_unit_return!(self.active_room.clone());
        let r = unwrap_or_unit_return!(self.rooms.get(&active_room));

        let dialog = self
            .ui
            .builder
            .get_object::<gtk::MessageDialog>("leave_room_dialog")
            .expect("Can't find leave_room_dialog in ui file.");

        let text = i18n_k(
            "Leave {room_name}?",
            &[("room_name", &r.name.clone().unwrap_or_default())],
        );
        dialog.set_property_text(Some(text.as_str()));
        dialog.present();
    }

    pub fn kicked_room(&self, room_name: String, reason: String, kicker: String) {
        let parent: gtk::Window = self
            .ui
            .builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");
        let parent_weak = parent.downgrade();
        let parent = upgrade_weak!(parent_weak);
        let viewer = widgets::KickedDialog::new();
        viewer.set_parent_window(&parent);
        viewer.show(&room_name, &reason, &kicker);
    }

    pub fn create_new_room(&mut self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
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
        let privacy = if private.get_active() {
            backend::RoomType::Private
        } else {
            backend::RoomType::Public
        };

        let internal_id = RoomId::new(&login_data.server_url.to_string())
            .expect("The server domain should have been validated");
        let int_id = internal_id.clone();
        let name = n.clone();
        let tx = self.backend.clone();
        thread::spawn(move || {
            match room::new_room(
                login_data.server_url,
                login_data.access_token,
                name,
                privacy,
            ) {
                Ok(r) => {
                    let id = Some(int_id);
                    APPOP!(new_room, (r, id));
                }
                Err(err) => {
                    tx.send(BKCommand::SendBKResponse(BKResponse::NewRoom(
                        Err(err),
                        int_id,
                    )))
                    .expect_log("Connection closed");
                }
            }
        });

        let fakeroom = Room {
            name: Some(n),
            ..Room::new(internal_id.clone(), RoomMembership::Joined(RoomTag::None))
        };

        self.new_room(fakeroom, None);
        self.set_active_room_by_id(internal_id);
        self.set_state(AppState::Room);
    }

    pub fn cache_rooms(&self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        // serializing rooms
        let rooms = self.rooms.clone();
        let since = self.since.clone();
        let username = login_data.username.unwrap_or_default();
        let uid = login_data.uid;
        let device_id = self.device_id.clone().unwrap_or_default();

        if let Err(_) = cache::store(&rooms, since, username, uid, device_id) {
            error!("Error caching rooms");
        };
    }

    pub fn set_room_detail(&mut self, room_id: RoomId, key: String, value: Option<String>) {
        if let Some(r) = self.rooms.get_mut(&room_id) {
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

        if self
            .active_room
            .as_ref()
            .map_or(false, |a_room| *a_room == room_id)
        {
            self.set_current_room_detail(key, value);
        }
    }

    pub fn set_room_avatar(&mut self, room_id: RoomId, avatar: Option<Url>) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        if avatar.is_none() {
            if let Ok(dest) = cache_dir_path(None, &room_id.to_string()) {
                let _ = remove_file(dest);
            }
        }
        if let Some(r) = self.rooms.get_mut(&room_id) {
            if avatar.is_none() && r.members.len() == 2 {
                for m in r.members.keys() {
                    if *m != login_data.uid {
                        //FIXME: Find a better solution
                        // create a symlink from user avatar to room avatar (works only on unix)
                        if let Ok(source) = cache_dir_path(None, &m.to_string()) {
                            if let Ok(dest) = cache_dir_path(None, &room_id.to_string()) {
                                let _ = fs::symlink(source, dest);
                            }
                        }
                    }
                }
            }
            r.avatar = avatar.map(|s| s.into_string());
            self.roomlist
                .set_room_avatar(room_id.clone(), r.avatar.clone());
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
                self.set_room_topic_label(Some(value));
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
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let name = self
            .ui
            .builder
            .get_object::<gtk::Entry>("join_room_name")
            .expect("Can't find join_room_name in ui file.")
            .get_text()
            .map_or(String::new(), |gstr| gstr.to_string());

        match RoomId::try_from(name.trim()) {
            Ok(room_id) => {
                self.backend
                    .send(BKCommand::JoinRoom(
                        login_data.server_url,
                        login_data.access_token,
                        room_id,
                    ))
                    .unwrap();
            }
            Err(err) => {
                self.backend
                    .send(BKCommand::SendBKResponse(BKResponse::JoinRoom(Err(
                        err.into()
                    ))))
                    .unwrap();
            }
        }
    }

    pub fn new_room(&mut self, r: Room, internal_id: Option<RoomId>) {
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

    pub fn added_to_fav(&mut self, room_id: RoomId, tofav: bool) {
        if let Some(ref mut r) = self.rooms.get_mut(&room_id) {
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
    pub fn recalculate_room_name(&mut self, room_id: RoomId) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let r = unwrap_or_unit_return!(self.rooms.get_mut(&room_id));

        // we should do nothing if this room has room name
        if r.name.is_some() {
            return;
        }

        // removing one because the user should be in the room
        let n = r.members.len() - 1;
        let suid = login_data.uid;
        let mut members = r
            .members
            .iter()
            .filter(|&(uid, _)| uid != &suid)
            .map(|(_uid, m)| m.get_alias());

        let m1 = members.next().unwrap_or_default();
        let m2 = members.next().unwrap_or_default();

        let name = match n {
            0 => i18n("EMPTY ROOM"),
            1 => String::from(m1),
            2 => i18n_k("{m1} and {m2}", &[("m1", &m1), ("m2", &m2)]),
            _ => i18n_k("{m1} and Others", &[("m1", &m1)]),
        };

        r.name = Some(name.clone());

        self.room_name_change(room_id, Some(name));
    }

    pub fn room_name_change(&mut self, room_id: RoomId, name: Option<String>) {
        let r = unwrap_or_unit_return!(self.rooms.get_mut(&room_id));
        r.name = name.clone();

        if self
            .active_room
            .as_ref()
            .map_or(false, |a_room| a_room == &room_id)
        {
            self.ui
                .builder
                .get_object::<gtk::Label>("room_name")
                .expect("Can't find room_name in ui file.")
                .set_text(&name.clone().unwrap_or_default());
        }

        self.roomlist.rename_room(room_id, name);
    }

    pub fn room_topic_change(&mut self, room_id: RoomId, topic: Option<String>) {
        let r = unwrap_or_unit_return!(self.rooms.get_mut(&room_id));
        r.topic = topic.clone();

        if self
            .active_room
            .as_ref()
            .map_or(false, |a_room| *a_room == room_id)
        {
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

    pub fn new_room_avatar(&self, room_id: RoomId) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        if !self.rooms.contains_key(&room_id) {
            return;
        }

        let tx = self.backend.clone();
        thread::spawn(move || {
            match room::get_room_avatar(login_data.server_url, login_data.access_token, room_id) {
                Ok((room, avatar)) => {
                    APPOP!(set_room_avatar, (room, avatar));
                }
                Err(err) => {
                    tx.send(BKCommand::SendBKResponse(BKResponse::RoomAvatar(Err(err))))
                        .expect_log("Connection closed");
                }
            }
        });
    }

    pub fn update_typing_notification(&mut self) {
        let active_room_id = unwrap_or_unit_return!(self.active_room.clone());
        let active_room = unwrap_or_unit_return!(self.rooms.get(&active_room_id));
        let history = unwrap_or_unit_return!(self.history.as_mut());

        let typing_users = &active_room.typing_users;
        if typing_users.is_empty() {
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

    pub fn send_typing(&mut self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let active_room = unwrap_or_unit_return!(self.active_room.clone());

        let now = Instant::now();
        if let Some(last_typing) = self.typing.get(&active_room) {
            let time_passed = now.duration_since(*last_typing);
            if time_passed.as_secs() < 3 {
                return;
            }
        }
        self.typing.insert(active_room.clone(), now);
        let tx = self.backend.clone();
        thread::spawn(move || {
            let query = room::send_typing(
                login_data.server_url,
                login_data.access_token,
                login_data.uid,
                active_room,
            );
            if let Err(err) = query {
                tx.send(BKCommand::SendBKResponse(BKResponse::SendTypingError(err)))
                    .expect_log("Connection closed");
            }
        });
    }

    pub fn set_language(&self, lang_code: String) {
        if let Some(language) = &gspell::Language::lookup(&lang_code) {
            let textview = self.ui.sventry.view.upcast_ref::<gtk::TextView>();
            if let Some(gs_checker) = textview
                .get_buffer()
                .and_then(|gtk_buffer| TextBuffer::get_from_gtk_text_buffer(&gtk_buffer))
                .and_then(|gs_buffer| GspellTextBufferExt::get_spell_checker(&gs_buffer))
            {
                CheckerExt::set_language(&gs_checker, Some(language))
            }
        }
    }
}
