use cairo;
use gdk;
use gtk;
use gtk::prelude::*;
use gtk::ListBoxRow;
use log::info;
use pango;

use crate::widgets;
use crate::widgets::sidebar::{DndChannelData, DndChannelWeak, RoomCategory};
use crate::widgets::AvatarExt;

use crate::store::SidebarRow;

const ICON_SIZE: i32 = 24;

// Room row for the room sidebar. This widget shows the room avatar, the room name and the unread
// messages in the room
// +-----+--------------------------+------+
// | IMG | Fractal                  |  32  |
// +-----+--------------------------+------+
pub struct RoomRow {}

impl RoomRow {
    pub fn new(
        item: &SidebarRow,
        cat: RoomCategory,
        channel: DndChannelWeak,
        store: glib::WeakRef<gio::ListStore>,
        targets: Option<&Vec<gtk::TargetEntry>>,
    ) -> ListBoxRow {
        let row = gtk::ListBoxRow::new();
        let widget = gtk::EventBox::new();
        row.set_margin_start(2);
        row.set_margin_end(2);
        row.set_margin_top(2);
        row.set_margin_bottom(2);
        let container = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let avatar = widgets::Avatar::avatar_new(Some(ICON_SIZE));
        let direct = gtk::Image::new_from_icon_name("avatar-default-symbolic", 1);
        if let Some(style) = direct.get_style_context() {
            style.add_class("direct-chat");
        }

        let name = gtk::Label::new(None);
        name.set_valign(gtk::Align::Center);
        name.set_halign(gtk::Align::Start);
        name.set_ellipsize(pango::EllipsizeMode::End);

        let number = gtk::Label::new(None);
        number.set_valign(gtk::Align::Center);

        if let Some(style) = container.get_style_context() {
            style.add_class("room-row");
        }

        container.pack_start(&avatar, false, false, 5);
        container.pack_start(&direct, false, false, 0);
        container.pack_start(&name, true, true, 0);
        container.pack_end(&number, false, false, 5);

        widget.add(&container);
        row.add(&widget);
        row.show_all();
        item.bind_property("name", &name, "label")
            .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
            .build();

        number.set_valign(gtk::Align::Center);
        // Bind notification counter
        item.bind_property("notifications", &number, "label")
            .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
            .transform_to(|binding, value| {
                if let Some(target) = binding.get_target() {
                    if let Ok(widget) = target.downcast::<gtk::Widget>() {
                        if let Some(string) = value.get::<String>() {
                            widget.set_visible(!string.is_empty());
                            if let Some(style) = widget.get_style_context() {
                                style.add_class("notify-badge");
                            }
                        }
                    }
                }
                Some(value.clone())
            })
            .build();

        let number_weak: glib::object::SendWeakRef<gtk::Label> = number.downgrade().into();
        item.connect_notify("highlight", move |item, _| {
            let number = upgrade_weak!(number_weak);
            if let Ok(id) = item.get_property("highlight") {
                if let Some(style) = number.get_style_context() {
                    if id.get::<bool>().map_or(false, |v| v) {
                        style.add_class("notify-highlight");
                    } else {
                        style.remove_class("notify-highlight");
                    }
                }
            }
        });

        item.notify("highlight");

        item.bind_property("direct", &direct, "visible")
            .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
            .build();

        item.bind_property("hidden", &row, "visible")
            .flags(
                glib::BindingFlags::DEFAULT
                    | glib::BindingFlags::INVERT_BOOLEAN
                    | glib::BindingFlags::SYNC_CREATE,
            )
            .build();

        item.bind_property("room_id", &row, "action-target")
            .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
            .transform_to(|_, value| {
                // Transform string into a variant
                let variant = value.get::<String>().map(|s| glib::Variant::from(s));
                variant.map(|v| (&v).to_value())
            })
            .build();

        row.set_action_name("app.open-room");

        // We can't use bind_property for the avatar because it's not a real widget, atleast for
        // now
        let avatar_weak: glib::object::SendWeakRef<gtk::Box> = avatar.downgrade().into();
        item.connect_notify("avatar", move |item, _| {
            //TODO: use use the avatar from item
            info!("The avatar changed {:?}", item.get_property("avatar"));
            let avatar = upgrade_weak!(avatar_weak);
            if let Ok(id) = item.get_property("room_id") {
                if let Some(id) = id.get::<String>() {
                    if let Ok(name) = item.get_property("name") {
                        avatar.circle(id, name.get::<String>(), ICON_SIZE);
                    }
                }
            }
        });

        item.notify("avatar");

        let name_weak: glib::object::SendWeakRef<gtk::Label> = name.downgrade().into();
        item.connect_notify("bold", move |item, _| {
            let name = upgrade_weak!(name_weak);
            if let Ok(bold) = item.get_property("bold") {
                if let Some(style) = name.get_style_context() {
                    if let Some(bold) = bold.get() {
                        if bold {
                            style.add_class("notify-bold");
                        } else {
                            style.remove_class("notify-bold");
                        }
                    }
                }
            }
        });

        item.notify("bold");

        if let Some(targets) = targets {
            connect_dnd(item, &widget, cat, channel, store, &targets);
        }
        row
    }
}

fn connect_dnd(
    item: &SidebarRow,
    row: &gtk::EventBox,
    cat: RoomCategory,
    channel: DndChannelWeak,
    store: glib::WeakRef<gio::ListStore>,
    targets: &Vec<gtk::TargetEntry>,
) {
    row.drag_source_set(
        gdk::ModifierType::BUTTON1_MASK,
        targets,
        gdk::DragAction::MOVE,
    );
    row.drag_source_add_text_targets();

    row.connect_drag_begin(move |w, ctx| {
        if let Some(w) = w.get_parent() {
            let ww = w.get_allocated_width();
            let wh = w.get_allocated_height();
            let image = cairo::ImageSurface::create(cairo::Format::ARgb32, ww, wh).unwrap();
            let g = cairo::Context::new(&image);
            //TODO use theme color for background or add a temporarily style class
            g.set_source_rgba(1.0, 1.0, 1.0, 0.8);
            g.rectangle(0.0, 0.0, ww as f64, wh as f64);
            g.fill();

            w.draw(&g);

            //TODO fix positioning
            //https://stackoverflow.com/questions/24844489/how-to-use-gdk-device-get-position
            ctx.drag_set_icon_surface(&image);
            w.hide();
        }
    });

    row.connect_drag_end(move |w, _context| {
        // Show the row again if it wasn't removed
        if let Some(w) = w.get_parent() {
            w.show();
        }
    });

    let item_weak = item.downgrade();
    row.connect_drag_data_get(move |w, _context, data, _info, _time| {
        let item = upgrade_weak!(item_weak);
        if let Ok(id) = item.get_property("room_id") {
            if let Some(id) = id.get::<String>() {
                if let Some(index) = w
                    .get_parent()
                    .and_then(|w| w.downcast::<gtk::ListBoxRow>().ok())
                    .and_then(|w| Some(w.get_index()))
                {
                    let atom = gdk::Atom::intern("CHANGE_POSITION");
                    if data.get_target() == atom {
                        // WORKAORUND: set() needs a mutable reference to data
                        let data: &mut gtk::SelectionData = unsafe {
                            let ptr: *const gtk::SelectionData = data;
                            (ptr as *mut gtk::SelectionData).as_mut().unwrap()
                        };
                        let channel = upgrade_weak!(channel);
                        channel.set(Some(DndChannelData::new(cat, store.clone(), index as u32)));
                        // We need to set data so drag_data_received is fired
                        data.set(&atom, 0, &[]);
                    } else {
                        data.set_text(&id);
                    }
                }
            }
        }
    });
}
