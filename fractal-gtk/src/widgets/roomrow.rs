use cairo;
use gdk;
use gtk;
use gtk::prelude::*;
use gtk::ListBoxRow;
use pango;

use std::sync::Arc;
use types::Room;

use widgets;
use widgets::AvatarExt;

use row_data::row_data::RowData;

const ICON_SIZE: i32 = 24;

// Room row for the room sidebar. This widget shows the room avatar, the room name and the unread
// messages in the room
// +-----+--------------------------+------+
// | IMG | Fractal                  |  32  |
// +-----+--------------------------+------+
pub struct RoomRow {}

impl RoomRow {
    pub fn new(item: &RowData) -> ListBoxRow {
        let row = gtk::ListBoxRow::new();
        let widget = gtk::EventBox::new();
        widget.set_margin_start(2);
        widget.set_margin_end(2);
        widget.set_margin_top(2);
        widget.set_margin_bottom(2);
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
            //TODO: use use the avatar from item
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

        item.bind_property("room_id", &row, "action-target")
            .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE)
            .transform_to(|_, value| {
                // Transform string into a variant
                let variant = value.get::<String>().map(|s| glib::Variant::from(s));
                variant.map(|v| (&v).to_value())
            })
            .build();

        //FALLBACK
        //row.set_action_name("app.open-room");

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

        //rr.connect_dnd();
        //
        row
    }

    /*
       pub fn set_notifications(&mut self, n: i32, h: i32) {
       self.notifications.set_text(&format!("{}", n));
        //TODO highlight and show the notifications invitations
        if n > 0 {
            self.notifications.show();
        } else {
            self.notifications.hide();
        }

        if let Some(style) = self.notifications.get_style_context() {
            if h > 0 {
                style.add_class("notify-highlight");
            } else {
                style.remove_class("notify-highlight");
            }
        }
    }

    pub fn set_bold(&self, bold: bool) {
        if let Some(style) = self.text.get_style_context() {
            if bold {
                style.add_class("notify-bold");
            } else {
                style.remove_class("notify-bold");
            }
        }
    }

    pub fn set_avatar(&mut self, avatar: Option<String>) {
        let name = self.text.get_text();
        self.icon
            .circle(self.room_id.clone(), name, ICON_SIZE);
    }
    */

    /*
    pub fn connect_dnd(&self) {
        //TODO: block drag and drop for inv
        //if self.room.inv {
        //    return;
        //}

        let mask = gdk::ModifierType::BUTTON1_MASK;
        let actions = gdk::DragAction::MOVE;
        self.widget.drag_source_set(mask, &[], actions);
        self.widget.drag_source_add_text_targets();

        self.widget.connect_drag_begin(move |w, ctx| {
            let ww = w.get_allocated_width();
            let wh = w.get_allocated_height();
            let image = cairo::ImageSurface::create(cairo::Format::ARgb32, ww, wh).unwrap();
            let g = cairo::Context::new(&image);
            g.set_source_rgba(1.0, 1.0, 1.0, 0.8);
            g.rectangle(0.0, 0.0, ww as f64, wh as f64);
            g.fill();

            w.draw(&g);

            ctx.drag_set_icon_surface(&image);
        });

        let id = self.room_id.clone();
        self.widget
            .connect_drag_data_get(move |_w, _, data, _x, _y| {
                data.set_text(&id);
            });
    }
    */
}
