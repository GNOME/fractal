use cairo;
use gdk;
use gtk;
use gtk::prelude::*;
use pango;

use crate::types::Room;

use crate::widgets;
use crate::widgets::AvatarExt;

const ICON_SIZE: i32 = 24;

// Room row for the room sidebar. This widget shows the room avatar, the room name and the unread
// messages in the room
// +-----+--------------------------+------+
// | IMG | Fractal                  |  32  |
// +-----+--------------------------+------+
pub struct RoomRow {
    pub room: Room,
    pub icon: widgets::Avatar,
    pub direct: gtk::Image,
    pub text: gtk::Label,
    pub notifications: gtk::Label,
    pub widget: gtk::EventBox,
}

impl RoomRow {
    pub fn new(room: Room) -> RoomRow {
        let widget = gtk::EventBox::new();
        let name = room.name.clone().unwrap_or("...".to_string());

        let icon = widgets::Avatar::avatar_new(Some(ICON_SIZE));
        let direct =
            gtk::Image::new_from_icon_name(Some("avatar-default-symbolic"), gtk::IconSize::Menu);
        direct.get_style_context().add_class("direct-chat");

        let text = gtk::Label::new(Some(name.clone().as_str()));
        text.set_valign(gtk::Align::Start);
        text.set_halign(gtk::Align::Start);
        text.set_ellipsize(pango::EllipsizeMode::End);

        let n = room.notifications;
        let h = room.highlight;
        let ntext = if room.membership.is_invited() {
            String::from("•")
        } else {
            format!("{}", n)
        };
        let notifications = gtk::Label::new(Some(&ntext[..]));
        let style = notifications.get_style_context();
        style.add_class("notify-badge");

        if h > 0 || room.membership.is_invited() {
            style.add_class("notify-highlight");
        } else {
            style.remove_class("notify-highlight");
        }

        if n > 0 || room.membership.is_invited() {
            notifications.show();
        } else {
            notifications.hide();
        }

        icon.circle(room.id.to_string(), Some(name), ICON_SIZE, None, None);

        let rr = RoomRow {
            room,
            icon,
            text,
            notifications,
            widget,
            direct,
        };

        rr.connect_dnd();

        rr
    }

    pub fn set_notifications(&mut self, n: i32, h: i32) {
        self.room.notifications = n;
        self.room.highlight = h;
        self.notifications.set_text(&format!("{}", n));
        if n > 0 || self.room.membership.is_invited() {
            self.notifications.show();
        } else {
            self.notifications.hide();
        }

        let style = self.notifications.get_style_context();
        if h > 0 || self.room.membership.is_invited() {
            style.add_class("notify-highlight");
        } else {
            style.remove_class("notify-highlight");
        }
    }

    pub fn set_bold(&self, bold: bool) {
        let style = self.text.get_style_context();
        if bold {
            style.add_class("notify-bold");
        } else {
            style.remove_class("notify-bold");
        }
    }

    pub fn render_notifies(&self) {
        let n = self.room.notifications;
        if n > 0 || self.room.membership.is_invited() {
            self.notifications.show();
        } else {
            self.notifications.hide();
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.room.name = Some(name.clone());
        self.text.set_text(&name);
    }

    pub fn set_avatar(&mut self, avatar: Option<String>) {
        self.room.avatar = avatar.clone();

        let name = self.room.name.clone().unwrap_or("...".to_string());

        self.icon
            .circle(self.room.id.to_string(), Some(name), ICON_SIZE, None, None);
    }

    pub fn widget(&self) -> gtk::ListBoxRow {
        let b = gtk::Box::new(gtk::Orientation::Horizontal, 5);

        for ch in self.widget.get_children() {
            self.widget.remove(&ch);
        }
        self.widget.add(&b);

        b.get_style_context().add_class("room-row");

        b.pack_start(&self.icon, false, false, 5);
        if self.room.direct {
            b.pack_start(&self.direct, false, false, 0);
        }
        self.text.set_valign(gtk::Align::Center);
        self.notifications.set_valign(gtk::Align::Center);
        b.pack_start(&self.text, true, true, 0);
        b.pack_start(&self.notifications, false, false, 5);
        self.widget.show_all();

        if self.room.notifications == 0 {
            self.notifications.hide();
        }

        let row = gtk::ListBoxRow::new();
        row.add(&self.widget);
        let data = glib::Variant::from(&self.room.id.to_string());
        row.set_action_target_value(Some(&data));
        row.set_action_name(Some("app.open-room"));

        row
    }

    pub fn connect_dnd(&self) {
        if self.room.membership.is_invited() {
            return;
        }

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

        let id = self.room.id.to_string();
        self.widget
            .connect_drag_data_get(move |_w, _, data, _x, _y| {
                data.set_text(&id);
            });
    }
}
