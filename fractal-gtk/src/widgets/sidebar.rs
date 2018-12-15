use i18n::i18n;

use gdk;
use gdk::DragContextExtManual;
use gio::ListModelExt;
use gio::ListStore;
use glib;
use pango;

use gtk;
use gtk::prelude::*;
use std::collections::HashMap;
use url::Url;

use globals;
use std::sync::{Arc, Mutex, MutexGuard};
use types::Message;
use types::Room;

use chrono::prelude::*;
use gio::ListStoreExt;
use row_data::row_data::RowData;
use widgets;

pub struct RoomListGroup {
    list: gtk::ListBox,
    header: gtk::Box,
    body: gtk::Revealer,

    widget: gtk::Box,
}

impl RoomListGroup {
    pub fn new(store: &ListStore, name: String, empty_text: Option<String>) -> RoomListGroup {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        // construct the header
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        if let Some(style) = header.get_style_context() {
            style.add_class("room-title");
        }
        let header_event = gtk::EventBox::new();
        let title = gtk::Label::new(Some(name.as_str()));
        title.set_halign(gtk::Align::Start);
        title.set_valign(gtk::Align::Start);
        let arrow = gtk::Image::new_from_icon_name("pan-down-symbolic", 2);

        header.pack_start(&title, true, true, 0);
        header.pack_end(&arrow, false, false, 0);
        header.show_all();
        header_event.add(&header);

        // construct the body
        let body = gtk::Revealer::new();
        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        // Create a label for the text displayed when there are nor room in the list only when
        // there is a empty_text
        let empty = if empty_text.is_some() {
            let empty = gtk::Label::new(empty_text.as_ref().map(|s| s.as_str()));
            empty.set_line_wrap_mode(pango::WrapMode::WordChar);
            empty.set_line_wrap(true);
            empty.set_justify(gtk::Justification::Center);
            if let Some(style) = empty.get_style_context() {
                style.add_class("room-empty-text");
            }

            container.add(&empty);
            Some(empty)
        } else {
            None
        };

        body.add(&container);
        body.set_reveal_child(true);

        // The model need to be sorted via g_list_store_sort() maybe based on last activty field
        let list = gtk::ListBox::new();
        container.add(&list);

        let body_weak = body.downgrade();
        let arrow_weak = arrow.downgrade();
        let list_weak = list.downgrade();
        header_event.connect_button_press_event(move |_, _| {
            let revealer = upgrade_weak!(body_weak, glib::signal::Inhibit(true));
            let arrow = upgrade_weak!(arrow_weak, glib::signal::Inhibit(true));
            let list = upgrade_weak!(list_weak, glib::signal::Inhibit(true));
            if revealer.get_child_revealed() {
                arrow.set_from_icon_name("pan-end-symbolic", 2);
                revealer.set_reveal_child(false);
                if let Some(style) = list.get_style_context() {
                    style.add_class("collapsed");
                }
            } else {
                arrow.set_from_icon_name("pan-down-symbolic", 2);
                revealer.set_reveal_child(true);
                if let Some(style) = list.get_style_context() {
                    style.remove_class("collapsed");
                }
            }
            glib::signal::Inhibit(true)
        });

        widget.add(&header_event);
        widget.add(&body);
        widget.show_all();

        list.bind_model(store, move |item| {
            let item = item.downcast_ref::<RowData>().unwrap();
            widgets::RoomRow::new(item)
        });

        let widget_weak = widget.downgrade();
        let empty_weak = empty.map(|x| x.downgrade());
        store.connect_items_changed(move |m, _, _, _| {
            let widget = upgrade_weak!(widget_weak);
            if let Some(ref empty_weak) = empty_weak {
                let empty = upgrade_weak!(empty_weak);
                empty.set_visible(m.get_n_items() == 0);
                widget.set_visible(true);
            } else {
                widget.set_visible(m.get_n_items() != 0);
            }
        });
        store.items_changed(0, 0, 0);

        RoomListGroup {
            list,
            header,
            body,
            widget,
        }
    }

    // This deselectes all row of other lisboxes
    // TODO pass array of other lists so we don't need to update this list when we add more
    // catergories
    pub fn connect_selecting(&self, other1: &RoomListGroup, other2: &RoomListGroup) {
        let other1 = other1.get_listbox().downgrade();
        let other2 = other2.get_listbox().downgrade();
        self.get_listbox().connect_row_selected(move |_, row| {
            if row.is_some() {
                upgrade_weak!(other1).unselect_all();
                upgrade_weak!(other2).unselect_all();
            }
        });
    }

    pub fn get_listbox(&self) -> &gtk::ListBox {
        &self.list
    }

    pub fn get_widget(&self) -> &gtk::Box {
        &self.widget
    }

    pub fn select(&self, room_id: &str) {
        println!("Select row {}", room_id);
    }

    pub fn unselect(&self) {
        self.list.unselect_all();
    }
}

pub struct Sidebar {
    widget: gtk::Box,

    inv: RoomListGroup,
    fav: RoomListGroup,
    rooms: RoomListGroup,
}

impl Sidebar {
    pub fn new(container: &gtk::Box) -> Self {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 6);

        // create listbox and bind a model
        let model = gio::ListStore::new(RowData::static_type());
        let data = RowData::new("room_id", &"Hello world 2", "", "999+", false, false, false);
        model.append(&data);
        let data = RowData::new("room_id", &"Hello world 2", "", "2", true, true, true);
        model.append(&data);
        model.append(&RowData::new(
            "room_id",
            &"Hello world",
            "",
            "",
            false,
            true,
            false,
        ));
        data.set_property("name", &"string".to_value());
        data.set_property("avatar", &"new_avatar".to_value());

        let inv = RoomListGroup::new(&model, i18n("Invites"), None);
        let fav = RoomListGroup::new(
            &model,
            i18n("Favorites"),
            Some(i18n(
                "Drag and drop rooms here to add them to your favorites",
            )),
        );
        let rooms = RoomListGroup::new(
            &model,
            i18n("Rooms"),
            Some(i18n("You don’t have any rooms yet")),
        );

        widget.add(inv.get_widget());
        widget.add(fav.get_widget());
        widget.add(rooms.get_widget());

        inv.connect_selecting(&rooms, &fav);
        fav.connect_selecting(&inv, &rooms);
        rooms.connect_selecting(&inv, &fav);

        container.add(&widget);
        widget.show();

        Sidebar {
            widget,
            inv,
            fav,
            rooms,
        }
    }

    pub fn get_widget(&self) -> &gtk::Box {
        &self.widget
    }

    pub fn select(&self, room_id: &str) {
        // The room exsits only once so we can try to set it in every group
        self.inv.select(room_id);
        self.fav.select(room_id);
        self.rooms.select(room_id);
    }

    pub fn unselect(&self) {
        self.inv.unselect();
        self.fav.unselect();
        self.rooms.unselect();
    }

    /*
    pub fn connect_fav<F: Fn(Room, bool) + 'static>(&self, cb: F) {
        let acb = Arc::new(cb);

        let favw = self.fav.get().widget.clone();
        let r = self.rooms.clone();
        let f = self.fav.clone();
        let cb = acb.clone();
        self.connect_drop(favw, move |roomid| {
            if let Some(room) = r.get().remove_room(roomid) {
                cb(room.room.clone(), true);
                f.get().add_room_up(room);
            }
        });

        let rw = self.rooms.get().widget.clone();
        let r = self.rooms.clone();
        let f = self.fav.clone();
        let cb = acb.clone();
        self.connect_drop(rw, move |roomid| {
            if let Some(room) = f.get().remove_room(roomid) {
                cb(room.room.clone(), false);
                r.get().add_room_up(room);
            }
        });
    }

    pub fn connect_drop<F: Fn(String) + 'static>(&self, widget: gtk::EventBox, cb: F) {
        let flags = gtk::DestDefaults::empty();
        let action = gdk::DragAction::all();
        widget.drag_dest_set(flags, &[], action);
        widget.drag_dest_add_text_targets();
        widget.connect_drag_motion(move |_w, ctx, _x, _y, time| {
            ctx.drag_status(gdk::DragAction::MOVE, time);
            glib::signal::Inhibit(true)
        });
        widget.connect_drag_drop(move |w, ctx, _x, _y, time| {
            if let Some(target) = w.drag_dest_find_target(ctx, None) {
                w.drag_get_data(ctx, &target, time);
            }
            glib::signal::Inhibit(true)
        });
        widget.connect_drag_data_received(move |_w, _ctx, _x, _y, data, _info, _time| {
            if let Some(roomid) = data.get_text() {
                cb(roomid);
            }
        });
    }
    */
}
