use i18n::i18n;
use std::cell::Cell;
use std::rc::{Rc, Weak};

use gdk;
use gio::ListModelExt;
use gio::ListStore;
use glib;
use pango;

use gtk;
use gtk::prelude::*;

use gio::ListStoreExt;
use store;
use widgets;

pub type DndChannel = Rc<Cell<Option<DndChannelData>>>;
pub type DndChannelWeak = Weak<Cell<Option<DndChannelData>>>;

pub struct DndChannelData {
    category: RoomCategory,
    store: glib::WeakRef<gio::ListStore>,
    position: u32,
}

impl DndChannelData {
    pub fn new(
        category: RoomCategory,
        store: glib::WeakRef<gio::ListStore>,
        position: u32,
    ) -> Self {
        DndChannelData {
            category,
            store,
            position,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoomCategory {
    Invites,
    Favorites,
    Rooms,
    LowPriority,
}

pub struct RoomListGroup {
    listbox: gtk::ListBox,
    widget: gtk::Box,
}

impl RoomListGroup {
    pub fn new(
        store: &ListStore,
        drag_area: bool,
        name: String,
        empty_text: Option<String>,
        category: RoomCategory,
        channel: DndChannel,
    ) -> Self {
        // We have to keep a weak reference so we can move it into each row, but the
        // drag_data_received handler gets a strong reference
        let channel_weak = Rc::downgrade(&channel);
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let list = gtk::ListBox::new();
        // Create target for drag and drop if the listbox is a drag_area
        let targets = if drag_area {
            let targets = vec![gtk::TargetEntry::new(
                "CHANGE_POSITION",
                // This seams to have no effect
                gtk::TargetFlags::SAME_APP,
                0,
            )];
            widget.drag_dest_set(gtk::DestDefaults::ALL, &targets, gdk::DragAction::MOVE);
            let store_weak = store.downgrade();
            widget.connect_drag_data_received(
                move |_widget, _context, _x, _y, _s, _info, _time| {
                    if let Some(data) = channel.take() {
                        if category == data.category {
                            //TODO: implement sorting via dnd
                            info!("Sorting via DnD isn't implemented yet");
                        } else {
                            let store = upgrade_weak!(data.store);
                            let this_store = upgrade_weak!(store_weak);
                            let row = store.get_object(data.position);
                            if let Some(row) = row {
                                // remove the row from the source store and add it to our store
                                store.remove(data.position);
                                this_store.insert(0, &row);
                            }
                        }
                    }
                },
            );

            Some(targets)
        } else {
            None
        };

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

        container.add(&list);
        body.add(&container);
        body.set_reveal_child(true);

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

        let store_weak = store.downgrade();
        //moving the target list into the clousure
        list.bind_model(store, move |item| {
            let item = item.downcast_ref::<store::SidebarRow>().unwrap();
            widgets::RoomRow::new(
                item,
                category,
                channel_weak.clone(),
                store_weak.clone(),
                targets.as_ref(),
            )
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
            listbox: list,
            widget,
        }
    }

    pub fn get_widget(&self) -> &gtk::Box {
        &self.widget
    }

    pub fn get_listbox(&self) -> &gtk::ListBox {
        &self.listbox
    }
}

pub struct Sidebar(gtk::Box);

impl Sidebar {
    pub fn new(store: &store::Sidebar) -> Self {
        // Channel to communicate between dnd source and destination
        let channel = Rc::new(Cell::new(None));
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 6);

        let invites_widget = RoomListGroup::new(
            store.get_invites_store(),
            false,
            i18n("Invites"),
            None,
            RoomCategory::Invites,
            channel.clone(),
        );
        let favorites_widget = RoomListGroup::new(
            store.get_favorites_store(),
            true,
            i18n("Favorites"),
            Some(i18n(
                "Drag and drop rooms here to add them to your favorites",
            )),
            RoomCategory::Favorites,
            channel.clone(),
        );
        let rooms_widget = RoomListGroup::new(
            store.get_rooms_store(),
            true,
            i18n("Rooms"),
            Some(i18n("You don’t have any rooms yet")),
            RoomCategory::Rooms,
            channel.clone(),
        );
        let low_priority_widget = RoomListGroup::new(
            store.get_low_priority_store(),
            true,
            i18n("Low Priority"),
            Some(i18n("Drag and drop rooms here to add them to low priority")),
            RoomCategory::LowPriority,
            channel.clone(),
        );

        widget.add(invites_widget.get_widget());
        widget.add(favorites_widget.get_widget());
        widget.add(rooms_widget.get_widget());
        widget.add(low_priority_widget.get_widget());

        connect_selecting(
            &invites_widget,
            &rooms_widget,
            &favorites_widget,
            &low_priority_widget,
        );
        connect_selecting(
            &favorites_widget,
            &invites_widget,
            &rooms_widget,
            &low_priority_widget,
        );
        connect_selecting(
            &rooms_widget,
            &invites_widget,
            &favorites_widget,
            &low_priority_widget,
        );
        connect_selecting(
            &low_priority_widget,
            &rooms_widget,
            &invites_widget,
            &favorites_widget,
        );

        widget.show();

        Sidebar(widget)
    }

    pub fn get_widget(&self) -> &gtk::Box {
        &self.0
    }
}

// This deselectes all row of other lisboxes
// TODO replace this function with a macro
pub fn connect_selecting(
    list: &RoomListGroup,
    other1: &RoomListGroup,
    other2: &RoomListGroup,
    other3: &RoomListGroup,
) {
    let other1 = other1.get_listbox().downgrade();
    let other2 = other2.get_listbox().downgrade();
    let other3 = other3.get_listbox().downgrade();
    list.get_listbox().connect_row_selected(move |_, row| {
        if row.is_some() {
            upgrade_weak!(other1).unselect_all();
            upgrade_weak!(other2).unselect_all();
            upgrade_weak!(other3).unselect_all();
        }
    });
}
