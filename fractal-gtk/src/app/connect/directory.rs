use glib::clone;

use crate::i18n::i18n;

use gtk::prelude::*;
use libhandy::{Column, ColumnExt};

use crate::app::App;
use crate::appop::RoomSearchPagination;

impl App {
    pub fn connect_directory(&self) {
        let q = self
            .ui
            .builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");

        let directory_stack = self
            .ui
            .builder
            .get_object::<gtk::Stack>("directory_stack")
            .expect("Can't find directory_stack in ui file.");

        let column = Column::new();
        let listbox = gtk::ListBox::new();

        column.set_maximum_width(800);
        /* For some reason the Column is not seen as a gtk::container
         * and therefore we can't call add() without the cast */
        let column = column.upcast::<gtk::Widget>();
        let column = column.downcast::<gtk::Container>().unwrap();
        column.set_hexpand(true);
        column.set_vexpand(true);
        column.set_margin_top(24);
        column.set_margin_start(12);
        column.set_margin_end(12);

        let frame = gtk::Frame::new(None);
        frame.set_shadow_type(gtk::ShadowType::In);
        frame.add(&listbox);
        frame.get_style_context().add_class("room-directory");
        column.add(&frame);
        listbox.show();
        frame.show();
        column.show();
        directory_stack.add_named(&column, "directory_column");

        let column = column.upcast::<gtk::Widget>();
        let column = column.downcast::<Column>().unwrap();
        self.ui
            .builder
            .expose_object::<gtk::ListBox>("directory_room_list", &listbox);
        self.ui
            .builder
            .expose_object::<Column>("directory_column", &column);

        let directory_choice_label = self
            .ui
            .builder
            .get_object::<gtk::Label>("directory_choice_label")
            .expect("Can't find directory_choice_label in ui file.");

        let default_matrix_server_radio = self
            .ui
            .builder
            .get_object::<gtk::RadioButton>("default_matrix_server_radio")
            .expect("Can't find default_matrix_server_radio in ui file.");

        let other_protocol_radio = self
            .ui
            .builder
            .get_object::<gtk::RadioButton>("other_protocol_radio")
            .expect("Can't find other_protocol_radio in ui file.");

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

        let other_homeserver_radio = self
            .ui
            .builder
            .get_object::<gtk::RadioButton>("other_homeserver_radio")
            .expect("Can't find other_homeserver_radio in ui file.");

        let other_homeserver_url_entry = self
            .ui
            .builder
            .get_object::<gtk::Entry>("other_homeserver_url_entry")
            .expect("Can't find other_homeserver_url_entry in ui file.");

        let other_homeserver_url = self
            .ui
            .builder
            .get_object::<gtk::EntryBuffer>("other_homeserver_url")
            .expect("Can't find other_homeserver_url in ui file.");

        let scroll = self
            .ui
            .builder
            .get_object::<gtk::ScrolledWindow>("directory_scroll")
            .expect("Can't find directory_scroll in ui file.");

        let mut op = self.op.clone();
        scroll.connect_edge_reached(move |_, dir| {
            if dir == gtk::PositionType::Bottom {
                op.lock().unwrap().load_more_rooms();
            }
        });

        op = self.op.clone();
        q.connect_activate(move |_| {
            let mut op = op.lock().unwrap();
            op.directory_pagination = RoomSearchPagination::Initial;
            op.search_rooms();
        });

        default_matrix_server_radio.connect_toggled(clone!(@strong directory_choice_label, @strong default_matrix_server_radio, @strong protocol_combo, @strong other_homeserver_url_entry => move |_| {
            if default_matrix_server_radio.get_active() {
                protocol_combo.set_sensitive(false);
                other_homeserver_url_entry.set_sensitive(false);
            }

            directory_choice_label.set_text(&i18n("Default Matrix Server"));
        }));

        other_protocol_radio.connect_toggled(clone!(@strong directory_choice_label, @strong other_protocol_radio, @strong protocol_combo, @strong protocol_model, @strong other_homeserver_url_entry => move |_| {
            if other_protocol_radio.get_active() {
                protocol_combo.set_sensitive(true);
                other_homeserver_url_entry.set_sensitive(false);
            }

            let active = protocol_combo.get_active().map_or(-1, |uint| uint as i32);
            let protocol: String = match protocol_model.iter_nth_child(None, active) {
                Some(it) => {
                    let v = protocol_model.get_value(&it, 0);
                    v.get().unwrap().unwrap()
                }
                None => String::new(),
            };

            directory_choice_label.set_text(&protocol);
        }));

        protocol_combo.connect_changed(
            clone!(@strong directory_choice_label, @strong protocol_combo, @strong protocol_model => move |_| {
                let active = protocol_combo.get_active().map_or(-1, |uint| uint as i32);
                let protocol: String = match protocol_model.iter_nth_child(None, active) {
                    Some(it) => {
                        let v = protocol_model.get_value(&it, 0);
                        v.get().unwrap().unwrap()
                    }
                    None => String::new(),
                };

                directory_choice_label.set_text(&protocol);
            }),
        );

        other_homeserver_radio.connect_toggled(
            clone!(@strong other_homeserver_radio, @strong protocol_combo, @strong other_homeserver_url_entry => move |_| {
                if other_homeserver_radio.get_active() {
                    protocol_combo.set_sensitive(false);
                    other_homeserver_url_entry.set_sensitive(true);
                }
            }),
        );

        other_homeserver_url_entry.connect_changed(
            clone!(@strong directory_choice_label, @strong other_homeserver_url => move |_| {
                directory_choice_label.set_text(&other_homeserver_url.get_text());
            }),
        );
    }
}
