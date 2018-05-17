extern crate gtk;
extern crate gettextrs;

use self::gtk::prelude::*;
use self::gettextrs::gettext;

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

        self.ui.builder
            .get_object::<gtk::ComboBox>("directory_combo")
            .expect("Can't find directory_combo in ui file.")
            .set_active(0);
    }

    pub fn search_rooms(&self, more: bool) {
        let combo_store = self.ui.builder
            .get_object::<gtk::ListStore>("protocol_model")
            .expect("Can't find protocol_model in ui file.");
        let combo = self.ui.builder
            .get_object::<gtk::ComboBox>("directory_combo")
            .expect("Can't find directory_combo in ui file.");

        let active = combo.get_active();
        let protocol: String = match combo_store.iter_nth_child(None, active) {
            Some(it) => {
                let v = combo_store.get_value(&it, 1);
                v.get().unwrap()
            }
            None => String::from(""),
        };

        let q = self.ui.builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");

        let btn = self.ui.builder
            .get_object::<gtk::Button>("directory_search_button")
            .expect("Can't find directory_search_button in ui file.");
        btn.set_label(gettext("Searching...").as_str());
        btn.set_sensitive(false);

        if !more {
            let directory = self.ui.builder
                .get_object::<gtk::ListBox>("directory_room_list")
                .expect("Can't find directory_room_list in ui file.");
            for ch in directory.get_children() {
                directory.remove(&ch);
            }
        }

        self.backend
            .send(BKCommand::DirectorySearch(q.get_text().unwrap(), protocol, more))
            .unwrap();
    }

    pub fn load_more_rooms(&self) {
        self.search_rooms(true);
    }

    pub fn set_directory_room(&self, room: Room) {
        let directory = self.ui.builder
            .get_object::<gtk::ListBox>("directory_room_list")
            .expect("Can't find directory_room_list in ui file.");

        let rb = widgets::RoomBox::new(&room, &self);
        let room_widget = rb.widget();
        directory.add(&room_widget);

        self.enable_directory_search();
    }

    pub fn enable_directory_search(&self) {
        let btn = self.ui.builder
            .get_object::<gtk::Button>("directory_search_button")
            .expect("Can't find directory_search_button in ui file.");
        btn.set_label(gettext("Search").as_str());
        btn.set_sensitive(true);
    }
}
