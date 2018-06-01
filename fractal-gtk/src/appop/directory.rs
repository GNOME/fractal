extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;

use widgets;
use backend::BKCommand;

use types::Protocol;
use types::Room;

impl AppOp {
    pub fn init_protocols(&self) {
        self.backend.send(BKCommand::DirectoryProtocols).unwrap();
    }

    pub fn set_protocols(&self, protocols: Vec<Protocol>) {
        let combo = self.ui.builder
            .get_object::<gtk::ListStore>("protocol_model")
            .expect("Can't find protocol_model in ui file.");
        combo.clear();

        for p in protocols {
            combo.insert_with_values(None, &[0, 1], &[&p.desc, &p.id]);
        }
    }

    pub fn search_rooms(&mut self, more: bool) {
        let other_protocol_radio = self.ui.builder
            .get_object::<gtk::RadioButton>("other_protocol_radio")
            .expect("Can't find other_protocol_radio in ui file.");

        let mut protocol =
            if other_protocol_radio.get_active() {
                let protocol_combo = self.ui.builder
                    .get_object::<gtk::ComboBox>("protocol_combo")
                    .expect("Can't find protocol_combo in ui file.");

                let protocol_model = self.ui.builder
                    .get_object::<gtk::ListStore>("protocol_model")
                    .expect("Can't find protocol_model in ui file.");

                let active = protocol_combo.get_active();
                match protocol_model.iter_nth_child(None, active) {
                    Some(it) => {
                        let v = protocol_model.get_value(&it, 1);
                        v.get().unwrap()
                    },
                    None => String::from("")
                }
            } else {
                String::from("")
            };

        let q = self.ui.builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");

        let other_homeserver_radio = self.ui.builder
            .get_object::<gtk::RadioButton>("other_homeserver_radio")
            .expect("Can't find other_homeserver_radio in ui file.");

        let other_homeserver_url = self.ui.builder
            .get_object::<gtk::EntryBuffer>("other_homeserver_url")
            .expect("Can't find other_homeserver_url in ui file.");

        let homeserver = if other_homeserver_radio.get_active() {
            other_homeserver_url.get_text()
        } else if protocol == "matrix.org" {
            protocol = String::from("");
            String::from("matrix.org")
        } else {
            String::from("")
        };

        if !more {
            let directory = self.ui.builder
                .get_object::<gtk::ListBox>("directory_room_list")
                .expect("Can't find directory_room_list in ui file.");
            for ch in directory.get_children() {
                directory.remove(&ch);
            }

            let spinner = gtk::Spinner::new();
            spinner.start();
            spinner.show();
            directory.add(&spinner);

            self.directory.clear();

            q.set_sensitive(false);
        }

        self.backend
            .send(BKCommand::DirectorySearch(homeserver, q.get_text().unwrap(), protocol, more))
            .unwrap();
    }

    pub fn load_more_rooms(&mut self) {
        self.search_rooms(true);
    }

    pub fn set_directory_rooms(&mut self, rooms: Vec<Room>) {
        for r in rooms.iter() {
            if self.directory.contains(r) {
                continue;
            }
            self.directory.push(r.clone());
        }

        self.directory.sort_by_key(|a| -a.n_members);

        let directory = self.ui.builder
            .get_object::<gtk::ListBox>("directory_room_list")
            .expect("Can't find directory_room_list in ui file.");
        directory.get_style_context().map(|c| c.add_class("room-directory"));

        if directory.get_children().len() == 1 {
            for ch in directory.get_children().iter().take(1) {
                directory.remove(ch);
            }
        }

        for r in self.directory.iter() {
            let rb = widgets::RoomBox::new(&r, &self);
            let room_widget = rb.widget();
            directory.add(&room_widget);
        }

        let q = self.ui.builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");
        q.set_sensitive(true);
    }

    pub fn reset_directory_state(&self) {
        let q = self.ui.builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");
        q.set_sensitive(true);

        let directory = self.ui.builder
            .get_object::<gtk::ListBox>("directory_room_list")
            .expect("Can't find directory_room_list in ui file.");
        if directory.get_children().len() == 1 {
            for ch in directory.get_children().iter().take(1) {
                directory.remove(ch);
            }
        }
    }
}
