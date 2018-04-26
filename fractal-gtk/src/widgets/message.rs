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
use util::markup_text;

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

    pub fn widget(&self) -> gtk::ListBoxRow {
        // msg
        // +--------+---------+
        // | avatar | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 10);

        let content = self.build_room_msg_content(false);
        let avatar = self.build_room_msg_avatar();

        msg_widget.pack_start(&avatar, false, false, 0);
        msg_widget.pack_start(&content, true, true, 0);

        let row = gtk::ListBoxRow::new();
        self.set_msg_styles(&row);
        row.set_selectable(false);
        row.set_margin_top(12);
        row.add(&msg_widget);
        row.show_all();

        row
    }

    pub fn small_widget(&self) -> gtk::ListBoxRow {
        // msg
        // +--------+---------+
        // |        | content |
        // +--------+---------+
        let msg_widget = gtk::Box::new(gtk::Orientation::Horizontal, 5);

        let content = self.build_room_msg_content(true);

        msg_widget.pack_start(&content, true, true, 50);

        let row = gtk::ListBoxRow::new();
        self.set_msg_styles(&row);
        row.set_selectable(false);
        row.add(&msg_widget);
        row.show_all();

        row
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
            info.set_margin_top(2);
            info.set_margin_bottom(3);
            content.pack_start(&info, false, false, 0);
        }

        let body: gtk::Box;

        match msg.mtype.as_ref() {
            "m.image" => {
                body = self.build_room_msg_image();
            }
            "m.emote" => {
                body = self.build_room_msg_emote(&msg);
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
                self.username.set_text(&member.get_alias());
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
            None => String::from(sender),
        };

        self.username.set_text(&uname);
        self.username.set_justify(gtk::Justification::Left);
        self.username.set_halign(gtk::Align::Start);
        if let Some(style) = self.username.get_style_context() {
            style.add_class("username");
        }

        self.username.clone()
    }

    /// Add classes to the widget depending on the properties:
    ///
    ///  * msg-tmp: if the message doesn't have id
    ///  * msg-mention: if the message contains the username in the body and
    ///                    sender does not contain username
    ///  * msg-emote: if the message is an emote
    fn set_msg_styles(&self, w: &gtk::ListBoxRow) {
        let uname = &self.op.username.clone().unwrap_or_default();
        let uid = self.op.uid.clone().unwrap_or_default();
        let msg = self.msg;
        let body: &str = &msg.body;

        if let Some(style) = w.get_style_context() {
            // temp msg, not sent yet
            if msg.id.is_none() || msg.id.clone().unwrap_or_default().is_empty() {
                style.add_class("msg-tmp");
            }
            // mentions
            if String::from(body).contains(uname) && msg.sender != uid {
                style.add_class("msg-mention");
            }
            // emotes
            if msg.mtype == "m.emote" {
                style.add_class("msg-emote");
            }
        }
    }

    fn set_label_styles(&self, w: &gtk::Label) {
        w.set_line_wrap(true);
        w.set_line_wrap_mode(pango::WrapMode::WordChar);
        w.set_justify(gtk::Justification::Left);
        w.set_xalign(0.0);
        w.set_valign(gtk::Align::Start);
        w.set_halign(gtk::Align::Start);
        w.set_selectable(true);
    }

    fn build_room_msg_body(&self, body: &str) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let msg = gtk::Label::new("");

        msg.set_markup(&markup_text(body));
        self.set_label_styles(&msg);

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
        let now = Local::now();

        let d = if (now.year() == dt.year()) && (now.ordinal() == dt.ordinal()) {
            dt.format("%H:%M").to_string()
        } else if now.year() == dt.year() {
            dt.format("%e %b %H:%M").to_string()
        } else {
            dt.format("%e %b %Y %H:%M").to_string()
        };

        let date = gtk::Label::new("");
        date.set_markup(&format!("<span alpha=\"60%\">{}</span>", d.trim()));
        date.set_line_wrap(true);
        date.set_justify(gtk::Justification::Right);
        date.set_valign(gtk::Align::Start);
        date.set_halign(gtk::Align::End);
        if let Some(style) = date.get_style_context() {
            style.add_class("timestamp");
        }

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

    fn build_room_msg_emote(&self, msg: &Message) -> gtk::Box {
        let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let member = self.room.members.get(&msg.sender);
        let sender: &str = &msg.sender;

        let sname = match member {
            Some(m) => m.get_alias(),
            None => String::from(sender),
        };

        let msg_label = gtk::Label::new("");
        let body: &str = &msg.body;

        msg_label.set_markup(&format!("<b>{}</b> {}", sname, markup_text(body)));

        self.set_label_styles(&msg_label);

        bx.add(&msg_label);
        bx
    }
}
