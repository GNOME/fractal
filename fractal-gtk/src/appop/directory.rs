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

    pub fn set_protocols(&mut self, protocols: Vec<Protocol>) {
        self.protocols = protocols;
        self.search_rooms(false);
    }

    pub fn search_rooms(&mut self, more: bool) {
        let protocols: Vec<String> = self.protocols.clone().into_iter()
                                         .map(|p| p.id).collect();

        let q = self.ui.builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");

        let specific_remote_server_radio = self.ui.builder
            .get_object::<gtk::RadioButton>("specific_remote_server_radio")
            .expect("Can't find specific_remote_server_radio in ui file.");

        let specific_remote_server_url = self.ui.builder
            .get_object::<gtk::EntryBuffer>("specific_remote_server_url")
            .expect("Can't find specific_remote_server_url in ui file.");

        let homeserver = if specific_remote_server_radio.get_active() {
            specific_remote_server_url.get_text()
        } else {
            String::from("")
        };

        let requested_protocols = if specific_remote_server_radio.get_active() {
            None
        } else {
            Some(protocols)
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
        }

        q.set_sensitive(false);

        self.backend
            .send(BKCommand::DirectorySearch(homeserver, q.get_text().unwrap_or_default(), requested_protocols, more))
            .unwrap();
    }

    pub fn load_more_rooms(&mut self) {
        self.search_rooms(true);
    }

    pub fn finish_directory_search(&self) {
        let directory = self.ui.builder
            .get_object::<gtk::ListBox>("directory_room_list")
            .expect("Can't find directory_room_list in ui file.");
        let q = self.ui.builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");

        for ch in directory.get_children().iter().take(1) {
            directory.remove(ch);
        }
        q.set_sensitive(true);
    }

    pub fn set_directory_rooms(&mut self, rooms: Vec<Room>) {
        for r in rooms.iter() {
            if self.directory.contains(r) {
                continue;
            }
            self.directory.push(r.clone());
        }

        self.directory.sort_by_key(|a| -a.n_members);
        self.redraw_directory_rooms();
    }

    pub fn redraw_directory_rooms(&self) {
        let directory = self.ui.builder
            .get_object::<gtk::ListBox>("directory_room_list")
            .expect("Can't find directory_room_list in ui file.");
        directory.get_style_context().map(|c| c.add_class("room-directory"));

        for ch in directory.get_children().iter().skip(1) {
            directory.remove(ch);
        }

        for r in self.directory.iter() {
            let rb = widgets::RoomBox::new(&r, &self);
            let room_widget = rb.widget();
            directory.add(&room_widget);
        }
    }
}
