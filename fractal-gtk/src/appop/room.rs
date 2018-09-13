extern crate gtk;
extern crate sourceview;

use i18n::{i18n, i18n_k};

use self::gtk::prelude::*;

use appop::AppOp;
use appop::AppState;
use app::InternalCommand;

use backend;
use backend::BKCommand;

use globals;
use cache;
use widgets;

use types::Room;
use types::Message;

use util::markup_text;

use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

pub struct Force(pub bool);


#[derive(Debug, Clone)]
pub enum RoomPanel {
    Room,
    NoRoom,
    Loading,
}


impl AppOp {
    pub fn update_rooms(&mut self, rooms: Vec<Room>, default: Option<Room>) {
        let rs: Vec<Room> = rooms.iter().filter(|x| !x.left).cloned().collect();
        self.set_rooms(&rs, default);

        // uploading each room avatar
        for r in rooms.iter() {
            self.backend.send(BKCommand::GetRoomAvatar(r.id.clone())).unwrap();
        }

        self.set_last_viewed_messages();
    }

    pub fn new_rooms(&mut self, rooms: Vec<Room>) {
        // ignoring existing rooms
        let rs: Vec<&Room> = rooms.iter().filter(|x| !self.rooms.contains_key(&x.id) && !x.left).collect();

        for r in rs {
            self.rooms.insert(r.id.clone(), r.clone());
            self.roomlist.add_room(r.clone());
            self.roomlist.moveup(r.id.clone());
        }

        // removing left rooms
        let rs: Vec<&Room> = rooms.iter().filter(|x| x.left).collect();
        for r in rs {
            if r.id == self.active_room.clone().unwrap_or_default() {
                self.really_leave_active_room();
            } else {
                self.remove_room(r.id.clone());
            }
        }

        self.set_last_viewed_messages();
    }

    pub fn remove_room(&mut self, id: String) {
        self.rooms.remove(&id);
        self.roomlist.remove_room(id.clone());
        self.unsent_messages.remove(&id);
    }

    pub fn set_rooms(&mut self, rooms: &Vec<Room>, def: Option<Room>) {
        let container: gtk::Box = self.ui.builder
            .get_object("room_container")
            .expect("Couldn't find room_container in ui file.");

        let selected_room = self.roomlist.get_selected();

        self.rooms.clear();
        for ch in container.get_children().iter() {
            container.remove(ch);
        }

        for r in rooms.iter() {
            if let None = r.name {
                // This will force the room name calculation for 1:1 rooms and other rooms with no
                // name
                self.backend.send(BKCommand::GetRoomMembers(r.id.clone())).unwrap();
            }

            self.rooms.insert(r.id.clone(), r.clone());
        }

        self.roomlist = widgets::RoomList::new(Some(self.server_url.clone()));
        self.roomlist.add_rooms(rooms.iter().cloned().collect());
        container.add(&self.roomlist.widget());
        self.roomlist.set_selected(selected_room);

        let bk = self.internal.clone();
        self.roomlist.connect(move |room| {
            bk.send(InternalCommand::SelectRoom(room)).unwrap();
        });
        let bk = self.backend.clone();
        self.roomlist.connect_fav(move |room, tofav| {
            bk.send(BKCommand::AddToFav(room.id.clone(), tofav)).unwrap();
        });

        let mut godef = def;
        if let Some(aroom) = self.active_room.clone() {
            if let Some(r) = self.rooms.get(&aroom) {
                godef = Some(r.clone());
            }
        }

        if let Some(d) = godef {
            self.set_active_room_by_id(d.id.clone());
        } else {
            self.set_state(AppState::Chat);
            self.room_panel(RoomPanel::NoRoom);
            self.active_room = None;
            self.clear_tmp_msgs();
        }

        self.cache_rooms();
    }

    pub fn reload_rooms(&mut self) {
        self.set_state(AppState::Chat);
    }

    pub fn set_active_room_by_id(&mut self, roomid: String) {
        let mut room = None;
        if let Some(r) = self.rooms.get(&roomid) {
            room = Some(r.clone());
        }

        if let Some(r) = room {
            if r.inv {
                self.show_inv_dialog(&r);
                return;
            }

            self.set_active_room(&r);
        }
    }

    pub fn set_active_room(&mut self, room: &Room) {
        if let Some(lvm_id) = self.last_viewed_messages.clone().get(&room.id) {
            if let Some(lvm) = self.get_msg_from_id(&room.id, &lvm_id) {
                let new_msg = self.get_first_new_from_last(&lvm);
                self.first_new_messages.insert(room.id.clone(), new_msg);
            }
        }

        self.member_limit = 50;
        self.room_panel(RoomPanel::Loading);

        let msg_entry: sourceview::View = self.ui.builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");
        if let Some(buffer) = msg_entry.get_buffer() {
            let start = buffer.get_start_iter();
            let end = buffer.get_end_iter();

            if let Some(msg) = buffer.get_text(&start, &end, false) {
                let active_room_id = self.active_room.clone().unwrap_or_default();
                if msg.len() > 0 {
                    if let Some(mark) = buffer.get_insert() {
                        let iter = buffer.get_iter_at_mark(&mark);
                        let msg_position = iter.get_offset();

                        self.unsent_messages.insert(active_room_id, (msg, msg_position));
                    }
                } else {
                    self.unsent_messages.remove(&active_room_id);
                }
            }
        }

        self.active_room = Some(room.id.clone());
        self.clear_tmp_msgs();
        self.autoscroll = true;

        let mut getmessages = true;

        let list = self.ui.builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        /* create the intitial list of messages to fill the new room history */
        let active_room = self.active_room.clone().unwrap_or_default();
        let mut messages = vec![];
        for msg in room.messages.iter() {
            /* Make sure the message is from this room and not redacted */
            if msg.room == active_room && !msg.redacted {
                let row = self.create_new_room_message(msg);
                if let Some(row) = row {
                    messages.push(row);
                }
            }
        }

        /* FIXME: We show all messages always therefore we don't need to track how many messages we show */
        self.shown_messages = messages.len();

        /* make sure we remove the old room history first, because the lazy loading could try to
         * load messages */
        if let Some(history) = self.history.take() {
            history.destroy();
        }

        let mut history = widgets::RoomHistory::new(list, self);
        history.create(messages);
        self.history = Some(history);

        let l = room.messages.len();
        if l > 0 && l < globals::INITIAL_MESSAGES {
            self.internal.send(InternalCommand::LoadMore).unwrap();
        }

        self.internal.send(InternalCommand::AppendTmpMessages).unwrap();
        self.internal.send(InternalCommand::SetPanel(RoomPanel::Room)).unwrap();

        if !room.messages.is_empty() {
            getmessages = false;
            if let Some(msg) = room.messages.iter().last() {
                self.mark_as_read(msg, Force(false));
            }
        }

        // getting room details
        self.backend.send(BKCommand::SetRoom(room.clone())).unwrap();

        self.set_room_topic_label(room.topic.clone());

        let name_label = self.ui.builder
            .get_object::<gtk::Label>("room_name")
            .expect("Can't find room_name in ui file.");

        name_label.set_text(&room.name.clone().unwrap_or_default());

        let mut size = 24;
        if let Some(r) = room.topic.clone() {
            if !r.is_empty() {
                size = 16;
            }
        }

        self.set_current_room_avatar(room.avatar.clone(), size);
        self.set_current_room_detail(String::from("m.room.name"), room.name.clone());
        self.set_current_room_detail(String::from("m.room.topic"), room.topic.clone());

        if getmessages {
            self.backend.send(BKCommand::GetRoomMessages(self.active_room.clone().unwrap_or_default())).unwrap();
        }
    }

    pub fn really_leave_active_room(&mut self) {
        let r = self.active_room.clone().unwrap_or_default();
        self.backend.send(BKCommand::LeaveRoom(r.clone())).unwrap();
        self.rooms.remove(&r);
        self.active_room = None;
        self.clear_tmp_msgs();
        self.room_panel(RoomPanel::NoRoom);

        self.roomlist.remove_room(r);
    }

    pub fn leave_active_room(&self) {
        let dialog = self.ui.builder
            .get_object::<gtk::MessageDialog>("leave_room_dialog")
            .expect("Can't find leave_room_dialog in ui file.");

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            let text = i18n_k("Leave {room_name}?", &[("room_name", &r.name.clone().unwrap_or_default())]);
            dialog.set_property_text(Some(&text));
            dialog.present();
        }
    }

    pub fn create_new_room(&mut self) {
        let name = self.ui.builder
            .get_object::<gtk::Entry>("new_room_name")
            .expect("Can't find new_room_name in ui file.");
        let private = self.ui.builder
            .get_object::<gtk::ToggleButton>("private_visibility_button")
            .expect("Can't find private_visibility_button in ui file.");

        let n = name.get_text().unwrap_or(String::from(""));

        // Since the switcher
        let p = match private.get_active() {
            true => backend::RoomType::Private,
            false => backend::RoomType::Public,
        };

        let internal_id: String = thread_rng().sample_iter(&Alphanumeric).take(10).collect();
        self.backend.send(BKCommand::NewRoom(n.clone(), p, internal_id.clone())).unwrap();

        let fakeroom = Room::new(internal_id.clone(), Some(n));
        self.new_room(fakeroom, None);
        self.roomlist.set_selected(Some(internal_id.clone()));
        self.set_active_room_by_id(internal_id);
        self.room_panel(RoomPanel::Loading);
    }

    pub fn room_panel(&self, t: RoomPanel) {
        let s = self.ui.builder
            .get_object::<gtk::Stack>("room_view_stack")
            .expect("Can't find room_view_stack in ui file.");
        let headerbar = self.ui.builder
            .get_object::<gtk::HeaderBar>("room_header_bar")
            .expect("Can't find room_header_bar in ui file.");

        let v = match t {
            RoomPanel::Loading => "loading",
            RoomPanel::Room => "room_view",
            RoomPanel::NoRoom => "noroom",
        };

        s.set_visible_child_name(v);

        match v {
            "noroom" => {
                for ch in headerbar.get_children().iter() {
                    ch.hide();
                }
                self.roomlist.set_selected(None);
            },
            "room_view" => {
                for ch in headerbar.get_children().iter() {
                    ch.show();
                }

                let msg_entry: sourceview::View = self.ui.builder
                    .get_object("msg_entry")
                    .expect("Couldn't find msg_entry in ui file.");
                msg_entry.grab_focus();

                let active_room_id = self.active_room.clone().unwrap_or_default();
                let msg = self.unsent_messages
                    .get(&active_room_id).cloned()
                    .unwrap_or((String::new(), 0));
                if let Some(buffer) = msg_entry.get_buffer() {
                    buffer.set_text(&msg.0);

                    let iter = buffer.get_iter_at_offset(msg.1);
                    buffer.place_cursor(&iter);
                }
            },
            _ => {
                for ch in headerbar.get_children().iter() {
                    ch.show();
                }
            }
        }
    }

    pub fn cache_rooms(&self) {
        // serializing rooms
        if let Err(_) = cache::store(&self.rooms, self.since.clone().unwrap_or_default(), self.username.clone().unwrap_or_default(), self.uid.clone().unwrap_or_default()) {
            println!("Error caching rooms");
        };
    }

    pub fn set_room_detail(&mut self, roomid: String, key: String, value: Option<String>) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            let k: &str = &key;
            match k {
                "m.room.name" => { r.name = value.clone(); }
                "m.room.topic" => { r.topic = value.clone(); }
                _ => {}
            };
        }

        if roomid == self.active_room.clone().unwrap_or_default() {
            self.set_current_room_detail(key, value);
        }
    }

    pub fn set_room_avatar(&mut self, roomid: String, avatar: Option<String>) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            r.avatar = avatar.clone();
            self.roomlist.set_room_avatar(roomid.clone(), r.avatar.clone());
        }

        if roomid == self.active_room.clone().unwrap_or_default() {
            let mut size = 24;
            if let Some(r) = self.rooms.get_mut(&roomid) {
                if !r.clone().topic.unwrap_or_default().is_empty() {
                    size = 16;
                }
            }
            self.set_current_room_avatar(avatar, size);
        }
    }

    pub fn set_current_room_detail(&self, key: String, value: Option<String>) {
        let value = value.unwrap_or_default();
        let k: &str = &key;
        match k {
            "m.room.name" => {
                let name_label = self.ui.builder
                    .get_object::<gtk::Label>("room_name")
                    .expect("Can't find room_name in ui file.");

                name_label.set_text(&value);
            }
            "m.room.topic" => {
                self.set_room_topic_label(Some(value.clone()));
            }
            _ => println!("no key {}", key),
        };
    }

    pub fn set_current_room_avatar(&self, _avatar: Option<String>, _size: i32) {
    }

    pub fn filter_rooms(&self, term: Option<String>) {
        self.roomlist.filter_rooms(term);
    }

    pub fn new_room_dialog(&self) {
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("new_room_dialog")
            .expect("Can't find new_room_dialog in ui file.");
        let btn = self.ui.builder
            .get_object::<gtk::Button>("new_room_button")
            .expect("Can't find new_room_button in ui file.");
        btn.set_sensitive(false);
        dialog.present();
    }

    pub fn join_to_room_dialog(&mut self) {
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("join_room_dialog")
            .expect("Can't find join_room_dialog in ui file.");
        self.ui.builder
            .get_object::<gtk::Button>("join_room_button")
            .map(|btn| btn.set_sensitive(false));
        dialog.present();
    }

    pub fn join_to_room(&mut self) {
        let name = self.ui.builder
            .get_object::<gtk::Entry>("join_room_name")
            .expect("Can't find join_room_name in ui file.");

        let n = name.get_text().unwrap_or(String::from(""));

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
        self.roomlist.set_selected(Some(r.id.clone()));

        self.set_active_room_by_id(r.id);
    }

    pub fn added_to_fav(&mut self, roomid: String, tofav: bool) {
        if let Some(ref mut r) = self.rooms.get_mut(&roomid) {
            r.fav = tofav;
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
                None => "".to_string(),
            };

            let m2 = match members.next() {
                Some((_uid, m)) => m.get_alias(),
                None => "".to_string(),
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
            self.ui.builder
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
        let t = self.ui.builder
            .get_object::<gtk::Label>("room_topic")
            .expect("Can't find room_topic in ui file.");
        let n = self.ui.builder
            .get_object::<gtk::Label>("room_name")
            .expect("Can't find room_name in ui file.");

        match topic {
            None => {
                t.set_tooltip_text("");
                n.set_tooltip_text("");
                t.hide();
            },
            Some(ref topic) if topic.is_empty() => {
                t.set_tooltip_text("");
                n.set_tooltip_text("");
                t.hide();
            },
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
