extern crate gtk;
use self::gtk::prelude::*;

use app::App;

impl App {
    pub fn connect_directory(&self) {
        let q = self.ui.builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");

        let directory_choice_label = self.ui.builder
            .get_object::<gtk::Label>("directory_choice_label")
            .expect("Can't find directory_choice_label in ui file.");

        let default_servers_radio = self.ui.builder
            .get_object::<gtk::RadioButton>("default_servers_radio")
            .expect("Can't find default_servers_radio in ui file.");

        let specific_remote_server_radio = self.ui.builder
            .get_object::<gtk::RadioButton>("specific_remote_server_radio")
            .expect("Can't find specific_remote_server_radio in ui file.");

        let specific_remote_server_url_entry = self.ui.builder
            .get_object::<gtk::Entry>("specific_remote_server_url_entry")
            .expect("Can't find specific_remote_server_url_entry in ui file.");

        let specific_remote_server_url = self.ui.builder
            .get_object::<gtk::EntryBuffer>("specific_remote_server_url")
            .expect("Can't find specific_remote_server_url in ui file.");

        let scroll = self.ui.builder
            .get_object::<gtk::ScrolledWindow>("directory_scroll")
            .expect("Can't find directory_scroll in ui file.");

        let mut op = self.op.clone();
        scroll.connect_edge_reached(move |_, dir| if dir == gtk::PositionType::Bottom {
            op.lock().unwrap().load_more_rooms();
        });

        op = self.op.clone();
        q.connect_activate(move |_| { op.lock().unwrap().search_rooms(false); });

        default_servers_radio.connect_toggled(clone!(directory_choice_label, default_servers_radio, specific_remote_server_url_entry => move |_| {
            if default_servers_radio.get_active() {
                specific_remote_server_url_entry.set_sensitive(false);
            }

            directory_choice_label.set_text("Default Servers");
        }));

        specific_remote_server_radio.connect_toggled(clone!(specific_remote_server_radio, specific_remote_server_url_entry => move |_| {
            if specific_remote_server_radio.get_active() {
                specific_remote_server_url_entry.set_sensitive(true);
            }
        }));

        specific_remote_server_url_entry.connect_changed(clone!(directory_choice_label, specific_remote_server_url => move |_| {
            directory_choice_label.set_text(&specific_remote_server_url.get_text());
        }));
    }
}
