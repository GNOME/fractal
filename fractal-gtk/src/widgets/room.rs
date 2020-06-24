use crate::i18n::i18n;

use fractal_api::backend::room;
use gtk;
use gtk::prelude::*;
use pango;
use std::thread;

use crate::types::Room;

use crate::backend::BKResponse;

use crate::util::markup_text;

use crate::app::dispatch_error;
use crate::app::App;
use crate::appop::AppOp;

use crate::widgets;
use crate::widgets::AvatarExt;
use gtk::WidgetExt;

const AVATAR_SIZE: i32 = 60;
const JOIN_BUTTON_WIDTH: i32 = 84;

// Room Search item
pub struct RoomBox<'a> {
    room: &'a Room,
    op: &'a AppOp,
}

impl<'a> RoomBox<'a> {
    pub fn new(room: &'a Room, op: &'a AppOp) -> RoomBox<'a> {
        RoomBox { room: room, op: op }
    }

    pub fn widget(&self) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        let room_box = self.build_room_box();

        row.set_selectable(false);
        row.add(&room_box);
        row.show_all();

        row
    }

    fn build_room_box(&self) -> gtk::Box {
        let widget_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        if let Some(login_data) = self.op.login_data.clone() {
            let room = self.room;

            let avatar = widgets::Avatar::avatar_new(Some(AVATAR_SIZE));
            avatar.circle(
                room.id.to_string(),
                room.name.clone(),
                AVATAR_SIZE,
                None,
                None,
            );
            widget_box.pack_start(&avatar, false, false, 18);

            let details_box = gtk::Box::new(gtk::Orientation::Vertical, 6);

            let name = match room.name {
                ref n if n.is_none() || n.clone().unwrap().is_empty() => room.alias.clone(),
                ref n => n.clone(),
            };

            let name_label = gtk::Label::new(None);
            name_label.set_line_wrap(true);
            name_label.set_line_wrap_mode(pango::WrapMode::WordChar);
            name_label.set_markup(&format!(
                "<b>{}</b>",
                markup_text(&name.unwrap_or_default())
            ));
            name_label.set_justify(gtk::Justification::Left);
            name_label.set_halign(gtk::Align::Start);
            name_label.set_valign(gtk::Align::Start);
            name_label.set_xalign(0.0);

            let topic_label = gtk::Label::new(None);
            if !room.topic.clone().unwrap_or_default().is_empty() {
                topic_label.set_line_wrap(true);
                topic_label.set_line_wrap_mode(pango::WrapMode::WordChar);
                topic_label.set_lines(2);
                topic_label.set_ellipsize(pango::EllipsizeMode::End);
                topic_label.set_markup(&markup_text(&room.topic.clone().unwrap_or_default()));
                topic_label.set_justify(gtk::Justification::Left);
                topic_label.set_halign(gtk::Align::Start);
                topic_label.set_valign(gtk::Align::Start);
                topic_label.set_xalign(0.0);
            }

            let alias_label = gtk::Label::new(None);
            alias_label.set_line_wrap(true);
            alias_label.set_line_wrap_mode(pango::WrapMode::WordChar);
            alias_label.set_markup(&format!(
                "<span alpha=\"60%\">{}</span>",
                room.alias.clone().unwrap_or_default()
            ));
            alias_label.set_justify(gtk::Justification::Left);
            alias_label.set_halign(gtk::Align::Start);
            alias_label.set_valign(gtk::Align::Start);
            alias_label.set_xalign(0.0);

            details_box.add(&name_label);
            if !topic_label
                .get_text()
                .map_or(String::new(), |gstr| gstr.to_string())
                .is_empty()
            {
                details_box.add(&topic_label);
            }
            details_box.add(&alias_label);

            let membership_grid = gtk::Grid::new();
            membership_grid.set_column_spacing(6);

            let members_icon = gtk::Image::new_from_icon_name(
                Some("system-users-symbolic"),
                gtk::IconSize::Menu.into(),
            );
            members_icon.get_style_context().add_class("dim-label");

            let members_count = gtk::Label::new(Some(&format!("{}", room.n_members)[..]));
            members_count.get_style_context().add_class("dim-label");

            let join_button = gtk::Button::new_with_label(i18n("Join").as_str());
            let room_id = room.id.clone();
            join_button.connect_clicked(move |_| {
                let server_url = login_data.server_url.clone();
                let access_token = login_data.access_token.clone();
                let room_id = room_id.clone();
                thread::spawn(move || {
                    match room::join_room(server_url, access_token, room_id.clone()) {
                        Ok(jtr) => {
                            let jtr = Some(jtr);
                            APPOP!(set_join_to_room, (jtr));
                            APPOP!(reload_rooms);
                        }
                        Err(err) => dispatch_error(BKResponse::JoinRoomError(err.into())),
                    }
                });
            });
            join_button.set_property_width_request(JOIN_BUTTON_WIDTH);

            membership_grid.attach(&join_button, 0, 0, 4, 1);
            membership_grid.attach(&members_icon, 5, 0, 1, 1);
            membership_grid.attach(&members_count, 6, 0, 1, 1);

            details_box.add(&membership_grid);

            widget_box.pack_start(&details_box, true, true, 0);

            widget_box.show_all();
        }
        widget_box
    }
}
