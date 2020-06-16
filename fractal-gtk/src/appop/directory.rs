use gtk;
use gtk::prelude::*;
use libhandy::Column;
use std::thread;

use fractal_api::backend::directory;
use fractal_api::util::ResultExpectLog;

use crate::app::App;
use crate::appop::AppOp;

use crate::backend::{BKCommand, BKResponse};
use crate::widgets;

use crate::types::Room;
use fractal_api::r0::thirdparty::get_supported_protocols::ProtocolInstance;

impl AppOp {
    pub fn init_protocols(&self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let tx = self.backend.clone();
        thread::spawn(move || {
            match directory::protocols(login_data.server_url, login_data.access_token) {
                Ok(protocols) => {
                    APPOP!(set_protocols, (protocols));
                }
                Err(err) => {
                    tx.send(BKCommand::SendBKResponse(
                        BKResponse::DirectoryProtocolsError(err),
                    ))
                    .expect_log("Connection closed");
                }
            }
        });
    }

    pub fn set_protocols(&self, protocols: Vec<ProtocolInstance>) {
        let combo = self
            .ui
            .builder
            .get_object::<gtk::ListStore>("protocol_model")
            .expect("Can't find protocol_model in ui file.");
        combo.clear();

        for p in protocols {
            combo.insert_with_values(None, &[0, 1], &[&p.desc, &p.id]);
        }
    }

    pub fn search_rooms(&mut self, more: bool) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let other_protocol_radio = self
            .ui
            .builder
            .get_object::<gtk::RadioButton>("other_protocol_radio")
            .expect("Can't find other_protocol_radio in ui file.");

        let mut protocol = if other_protocol_radio.get_active() {
            let protocol_combo = self
                .ui
                .builder
                .get_object::<gtk::ComboBox>("protocol_combo")
                .expect("Can't find protocol_combo in ui file.");

            let protocol_model = self
                .ui
                .builder
                .get_object::<gtk::ListStore>("protocol_model")
                .expect("Can't find protocol_model in ui file.");

            let active = protocol_combo.get_active().map_or(-1, |uint| uint as i32);
            match protocol_model.iter_nth_child(None, active) {
                Some(it) => {
                    let v = protocol_model.get_value(&it, 1);
                    v.get().unwrap().unwrap()
                }
                None => String::new(),
            }
        } else {
            String::new()
        };

        let q = self
            .ui
            .builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");

        let other_homeserver_radio = self
            .ui
            .builder
            .get_object::<gtk::RadioButton>("other_homeserver_radio")
            .expect("Can't find other_homeserver_radio in ui file.");

        let other_homeserver_url = self
            .ui
            .builder
            .get_object::<gtk::EntryBuffer>("other_homeserver_url")
            .expect("Can't find other_homeserver_url in ui file.");

        let homeserver = if other_homeserver_radio.get_active() {
            other_homeserver_url.get_text()
        } else if protocol == "matrix.org" {
            protocol = String::new();
            String::from("matrix.org")
        } else {
            String::new()
        };

        if !more {
            let directory = self
                .ui
                .builder
                .get_object::<gtk::ListBox>("directory_room_list")
                .expect("Can't find directory_room_list in ui file.");
            for ch in directory.get_children() {
                directory.remove(&ch);
            }

            let directory_stack = self
                .ui
                .builder
                .get_object::<gtk::Stack>("directory_stack")
                .expect("Can't find directory_stack in ui file.");
            let directory_spinner = self
                .ui
                .builder
                .get_object::<gtk::Box>("directory_spinner")
                .expect("Can't find directory_spinner in ui file.");
            directory_stack.set_visible_child(&directory_spinner);

            self.directory.clear();

            q.set_sensitive(false);
        }

        self.backend
            .send(BKCommand::DirectorySearch(
                login_data.server_url,
                login_data.access_token,
                homeserver,
                q.get_text().unwrap().to_string(),
                protocol,
                more,
            ))
            .unwrap();
    }

    pub fn load_more_rooms(&mut self) {
        self.search_rooms(true);
    }

    pub fn append_directory_rooms(&mut self, rooms: Vec<Room>) {
        let directory = self
            .ui
            .builder
            .get_object::<gtk::ListBox>("directory_room_list")
            .expect("Can't find directory_room_list in ui file.");
        directory.get_style_context().add_class("room-directory");

        let directory_stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("directory_stack")
            .expect("Can't find directory_stack in ui file.");
        let directory_column = self
            .ui
            .builder
            .get_object::<Column>("directory_column")
            .expect("Can't find directory_column in ui file.");
        directory_stack.set_visible_child(&directory_column);

        let mut sorted_rooms = rooms;
        sorted_rooms.sort_by_key(|a| -a.n_members);

        for r in sorted_rooms.iter() {
            self.directory.push(r.clone());
            let rb = widgets::RoomBox::new(&r, &self);
            let room_widget = rb.widget();
            directory.add(&room_widget);
        }

        let q = self
            .ui
            .builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");
        q.set_sensitive(true);
    }

    pub fn reset_directory_state(&self) {
        let q = self
            .ui
            .builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");
        q.set_sensitive(true);

        let directory_stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("directory_stack")
            .expect("Can't find directory_stack in ui file.");
        let directory_column = self
            .ui
            .builder
            .get_object::<Column>("directory_column")
            .expect("Can't find directory_column in ui file.");
        directory_stack.set_visible_child(&directory_column);
    }
}
