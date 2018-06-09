extern crate gtk;
extern crate gdk;
extern crate gdk_pixbuf;
extern crate cairo;
extern crate pango;
extern crate gettextrs;

use self::gtk::prelude::*;
use self::gdk_pixbuf::Pixbuf;
use self::gdk_pixbuf::PixbufExt;
use self::gdk::ContextExt;
use self::gettextrs::gettext;

use fractal_api::util::AvatarMode;
use fractal_api::util::draw_identicon;

use types::Room;

use backend::BKCommand;

use util::markup_text;
use util::glib_thread_prelude::*;

use appop::AppOp;

use widgets::image::{Image, Thumb, Circle, Fixed, Centered};
use self::gtk::WidgetExt;

const AVATAR_SIZE: i32 = 60;
const JOIN_BUTTON_WIDTH: i32 = 84;

// Room Search item
pub struct RoomBox<'a> {
    room: &'a Room,
    op: &'a AppOp,
}

impl<'a> RoomBox<'a> {
    pub fn new(room: &'a Room, op: &'a AppOp) -> RoomBox<'a> {
        RoomBox {
            room: room,
            op: op,
        }
    }

    pub fn widget(&self) -> gtk::Box {
        let room = self.room;

        let widget_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let mut avatar = gtk::DrawingArea::new();
        if room.avatar.clone().unwrap_or_default().is_empty() {
            make_identicon(&avatar, AVATAR_SIZE, room.id.clone(), room.name.clone().unwrap_or_default());
        } else {
            let mut avatar_widget = Image::new(&self.op.backend, &room.avatar.clone().unwrap_or_default(),
                                               Some((AVATAR_SIZE, AVATAR_SIZE)), Thumb(true),
                                               Circle(true), Fixed(true), Centered(false));
            avatar_widget.fixed_size = true;
            avatar = avatar_widget.widget;
        }

        widget_box.pack_start(&avatar, false, false, 18);

        let details_box = gtk::Box::new(gtk::Orientation::Vertical, 6);

        let name = match room.name {
            ref n if n.is_none() || n.clone().unwrap().is_empty() => room.alias.clone(),
            ref n => n.clone(),
        };

        let name_label = gtk::Label::new("");
        name_label.set_line_wrap(true);
        name_label.set_line_wrap_mode(pango::WrapMode::WordChar);
        name_label.set_markup(&format!("<b>{}</b>", markup_text(&name.unwrap_or_default())));
        name_label.set_justify(gtk::Justification::Left);
        name_label.set_halign(gtk::Align::Start);
        name_label.set_valign(gtk::Align::Start);
        name_label.set_xalign(0.0);

        let topic_label = gtk::Label::new("");
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

        let alias_label = gtk::Label::new("");
        alias_label.set_markup(&format!("<span alpha=\"60%\">{}</span>",
                                        room.alias.clone().unwrap_or_default()));
        alias_label.set_justify(gtk::Justification::Left);
        alias_label.set_halign(gtk::Align::Start);
        alias_label.set_valign(gtk::Align::Start);
        alias_label.set_xalign(0.0);

        details_box.add(&name_label);
        if !topic_label.get_text().unwrap_or_default().is_empty() {
            details_box.add(&topic_label);
        }
        details_box.add(&alias_label);

        widget_box.pack_start(&details_box, true, true, 0);

        let membership_grid = gtk::Grid::new();
        membership_grid.set_row_spacing(6);

        let members_icon = gtk::Image::new_from_icon_name("system-users-symbolic", gtk::IconSize::Menu.into());
        members_icon.get_style_context().map(|c| c.add_class("dim-label"));

        let members_count = gtk::Label::new(&format!("{}", room.n_members)[..]);
        members_count.get_style_context().map(|c| c.add_class("dim-label"));

        let join_button = gtk::Button::new_with_label(gettext("Join").as_str());
        let room_id = room.id.clone();
        let backend = self.op.backend.clone();
        join_button.connect_clicked(move |_| {
            backend.send(BKCommand::JoinRoom(room_id.clone())).unwrap();
        });
        join_button.set_property_width_request(JOIN_BUTTON_WIDTH);

        membership_grid.attach(&members_icon, 1, 0, 1, 1);
        membership_grid.attach(&members_count, 2, 0, 1, 1);
        membership_grid.attach(&join_button, 0, 1, 4, 1);

        widget_box.pack_start(&membership_grid, false, false, 18);

        widget_box.show_all();
        widget_box
    }
}

fn make_identicon(da: &gtk::DrawingArea, size: i32, rid: String, name: String) {
    let da = da.clone();
    glib_thread!(Result<String, Error>,
        || {
            identicon!(&rid, name)
        },
        |rc: Result<String, Error>| {
            if let Ok(path) = rc {
                da.set_size_request(size, size);

                let pixbuf = Pixbuf::new_from_file_at_scale(&path, size, size, true);

                da.connect_draw(move |da, g| {
                    use std::f64::consts::PI;
                    g.set_antialias(cairo::Antialias::Best);

                    let width = size as f64;
                    let height = size as f64;

                    let context = da.get_style_context().unwrap();

                    gtk::render_background(&context, g, 0.0, 0.0, width, height);

                    if let Ok(ref pb) = pixbuf {
                        let hpos: f64 = (width - (pb.get_height()) as f64) / 2.0;

                        g.arc(width / 2.0, height / 2.0, width.min(height) / 2.0, 0.0, 2.0 * PI);
                        g.clip();

                        g.set_source_pixbuf(&pb, 0.0, hpos);
                        g.rectangle(0.0, 0.0, width, height);
                        g.fill();
                    }

                    Inhibit(false)
                });
            }
        }
    );
}
