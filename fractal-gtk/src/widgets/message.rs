extern crate gtk;
extern crate gdk_pixbuf;
extern crate chrono;
extern crate pango;

use self::gdk_pixbuf::Pixbuf;
use self::gtk::prelude::*;

use types::Message;
use types::Member;
use types::Room;

use self::chrono::prelude::*;

use backend::BKCommand;

use fractal_api as api;
use util;

use std::path::Path;

use app::AppOp;
use globals;
use widgets;
use widgets::AvatarExt;
use widgets::member::get_member_info;

// Room Message item
pub struct MessageBox<'a> {
    room: &'a Room,
    msg: &'a Message,
    op: &'a AppOp,
    username: gtk::Label,
    pub username_event_box: gtk::EventBox,
}

impl<'a> MessageBox<'a> {
    pub fn new(room: &'a Room, msg: &'a Message, op: &'a AppOp) -> MessageBox<'a> {
        let username = gtk::Label::new("");
        let eb = gtk::EventBox::new();

        MessageBox {
            msg: msg,
            room: room,
            op: op,
            username: username,
            username_event_box: eb,
        }
    }

    pub fn widget(&self) -> gtk::Box {
        // msg
        // +--------+---------+
        // | avatar | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        if let Some(style) = msg_widget.get_style_context() {
            style.add_class("message-entry");
        }

        let content = self.build_room_msg_content(false);
        let avatar = self.build_room_msg_avatar();

        msg_widget.pack_start(&avatar, false, false, 5);
        msg_widget.pack_start(&content, true, true, 0);

        msg_widget.show_all();

        msg_widget
    }

    pub fn small_widget(&self) -> gtk::Box {
        // msg
        // +--------+---------+
        // |        | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 5);

        let content = self.build_room_msg_content(true);
        msg_widget.pack_start(&content, true, true, 55);

        msg_widget.show_all();

        msg_widget
    }

    fn build_room_msg_content(&self, small: bool) -> gtk::Box {
        // content
        // +------+
        // | info |
        // +------+
        // | body |
        // +------+
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let msg = self.msg;

        if !small {
            let info = self.build_room_msg_info(self.msg);
            content.pack_start(&info, false, false, 0);
        }

        let body: gtk::Box;

        match msg.mtype.as_ref() {
            "m.image" => {
                body = self.build_room_msg_image();
            }
            "m.video" | "m.audio" | "m.file" => {
                body = self.build_room_msg_file();
            }
            _ => {
                body = self.build_room_msg_body(&msg.body);
            }
        }

        content.pack_start(&body, true, true, 0);

        content
    }

    fn build_room_msg_avatar(&self) -> widgets::Avatar {
        let sender = self.msg.sender.clone();
        let backend = self.op.backend.clone();
        let avatar = widgets::Avatar::avatar_new(Some(globals::MSG_ICON_SIZE));

        let fname = api::util::cache_path(&sender).unwrap_or(strn!(""));

        let pathname = fname.clone();
        let p = Path::new(&pathname);
        if p.is_file() {
            avatar.circle(fname, Some(globals::MSG_ICON_SIZE));
        } else {
            avatar.default(String::from("avatar-default-symbolic"),
                           Some(globals::MSG_ICON_SIZE));
        }

        let m = self.room.members.get(&sender);

        match m {
            Some(member) => {
                self.username.set_text(&member.get_alias().unwrap_or_default());
                get_member_info(backend.clone(), avatar.clone(), self.username.clone(), sender.clone(), globals::MSG_ICON_SIZE, 10);
            }
            None => {
                self.username.set_text(&sender);
                get_member_info(backend.clone(), avatar.clone(), self.username.clone(), sender.clone(), globals::MSG_ICON_SIZE, 10);
            }
        };

        avatar
    }

    fn build_room_msg_username(&self, sender: &str, member: Option<&Member>) -> gtk::Label {
        let uname = match member {
            Some(m) => m.get_alias(),
            None => Some(String::from(sender)),
        };

        self.username.set_text(&uname.unwrap_or_default());
        self.username.set_justify(gtk::Justification::Left);
        self.username.set_halign(gtk::Align::Start);
        if let Some(style) = self.username.get_style_context() {
            style.add_class("username");
        }

        self.username.clone()
    }

    fn build_room_msg_body(&self, body: &str) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let msg = gtk::Label::new("");

        let uname = &self.op.username.clone().unwrap_or_default();

        if self.msg.id.is_none() || self.msg.id.clone().unwrap_or_default().is_empty() {
            msg.set_markup(&format!("<span color=\"#aaaaaa\">{}</span>", util::markup(body)));
        } else if String::from(body).contains(uname) {
            msg.set_markup(&format!("<span color=\"#ff888e\">{}</span>", util::markup(body)));
        } else {
            msg.set_markup(&util::markup(body));
        }

        msg.set_line_wrap(true);
        msg.set_line_wrap_mode(pango::WrapMode::WordChar);
        msg.set_justify(gtk::Justification::Left);
        msg.set_halign(gtk::Align::Start);
        msg.set_alignment(0.0, 0.0);
        msg.set_selectable(true);

        bx.add(&msg);
        bx
    }

    fn build_room_msg_image(&self) -> gtk::Box {
        let msg = self.msg;
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let image = gtk::Image::new();

        if let Ok(pixbuf) = Pixbuf::new_from_file_at_scale(&msg.thumb.clone().unwrap_or_default(), 200, 200, true) {
            image.set_from_pixbuf(&pixbuf);
        } else {
            image.set_from_file(&msg.thumb.clone().unwrap_or_default());
        }

        let viewbtn = gtk::Button::new();
        let url = msg.url.clone().unwrap_or_default();
        let backend = self.op.backend.clone();
        //let img = image.clone();
        viewbtn.connect_clicked(move |_| {
            //let spin = gtk::Spinner::new();
            //spin.start();
            //btn.add(&spin);
            backend.send(BKCommand::GetMedia(url.clone())).unwrap();
        });

        viewbtn.set_image(&image);

        bx.add(&viewbtn);
        bx
    }

    fn build_room_msg_file(&self) -> gtk::Box {
        let msg = self.msg;
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let viewbtn = gtk::Button::new();
        let url = msg.url.clone().unwrap_or_default();
        let backend = self.op.backend.clone();
        viewbtn.connect_clicked(move |_| {
            backend.send(BKCommand::GetMedia(url.clone())).unwrap();
        });

        viewbtn.set_label(&msg.body);

        bx.add(&viewbtn);
        bx
    }

    fn build_room_msg_date(&self, dt: &DateTime<Local>) -> gtk::Label {
        let d = dt.format("%d/%b/%y %H:%M").to_string();

        let date = gtk::Label::new("");
        date.set_markup(&format!("<span alpha=\"60%\">{}</span>", d));
        date.set_line_wrap(true);
        date.set_justify(gtk::Justification::Right);
        date.set_halign(gtk::Align::End);
        date.set_alignment(1.0, 0.0);

        date
    }

    fn build_room_msg_info(&self, msg: &Message) -> gtk::Box {
        // info
        // +----------+------+
        // | username | date |
        // +----------+------+
        let info = gtk::Box::new(gtk::Orientation::Horizontal, 0);

        let member = self.room.members.get(&msg.sender);
        let username = self.build_room_msg_username(&msg.sender, member);
        let date = self.build_room_msg_date(&msg.date);

        self.username_event_box.add(&username);

        info.pack_start(&self.username_event_box, true, true, 0);
        info.pack_start(&date, false, false, 0);

        info
    }
}
