extern crate gtk;
extern crate gdk_pixbuf;
extern crate pango;

use self::gdk_pixbuf::Pixbuf;
use self::gtk::prelude::*;

use types::Room;

use backend::BKCommand;

use util;

use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};

use app::AppOp;

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
        let r = self.room;

        let h = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let w = gtk::Box::new(gtk::Orientation::Horizontal, 5);

        let mname = match r.name {
            ref n if n.is_empty() => r.alias.clone(),
            ref n => n.clone(),
        };

        let avatar = gtk::Image::new_from_icon_name("image-missing", 5);
        let a = avatar.clone();
        let id = r.id.clone();
        let name = mname.clone();
        let (tx, rx): (Sender<String>, Receiver<String>) = channel();
        self.op.backend.send(BKCommand::GetThumbAsync(r.avatar.clone(), tx)).unwrap();
        gtk::timeout_add(50, move || match rx.try_recv() {
            Err(_) => gtk::Continue(true),
            Ok(fname) => {
                let mut f = fname.clone();
                if f.is_empty() {
                    f = util::draw_identicon(&id, name.clone(), util::AvatarMode::Circle).unwrap();
                }
                if let Ok(pixbuf) = Pixbuf::new_from_file_at_scale(&f, 32, 32, false) {
                    a.set_from_pixbuf(&pixbuf);
                }
                gtk::Continue(false)
            }
        });
        w.pack_start(&avatar, false, false, 0);

        let b = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let msg = gtk::Label::new("");
        msg.set_line_wrap(true);
        msg.set_markup(&format!("<b>{}</b>", mname));
        msg.set_line_wrap_mode(pango::WrapMode::WordChar);
        msg.set_justify(gtk::Justification::Left);
        msg.set_halign(gtk::Align::Start);
        msg.set_alignment(0 as f32, 0 as f32);

        let topic = gtk::Label::new("");
        topic.set_line_wrap(true);
        msg.set_line_wrap_mode(pango::WrapMode::WordChar);
        topic.set_markup(&util::markup(&r.topic));
        topic.set_justify(gtk::Justification::Left);
        topic.set_halign(gtk::Align::Start);
        topic.set_alignment(0 as f32, 0 as f32);

        let idw = gtk::Label::new("");
        idw.set_markup(&format!("<span alpha=\"60%\">{}</span>", r.alias));
        idw.set_justify(gtk::Justification::Left);
        idw.set_halign(gtk::Align::Start);
        idw.set_alignment(0 as f32, 0 as f32);

        let joinbtn = gtk::Button::new_with_label("Join");
        let rid = r.id.clone();
        let backend = self.op.backend.clone();
        joinbtn.connect_clicked(move |_| {
            backend.send(BKCommand::JoinRoom(rid.clone())).unwrap();
        });
        joinbtn.get_style_context().unwrap().add_class("suggested-action");

        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        buttons.pack_start(&joinbtn, false, false, 0);

        b.add(&msg);
        b.add(&topic);
        b.add(&idw);
        b.add(&buttons);

        w.pack_start(&b, true, true, 0);

        let members = gtk::Label::new(&format!("{}", r.n_members)[..]);
        w.pack_start(&members, false, false, 5);

        h.add(&w);
        h.add(&gtk::Separator::new(gtk::Orientation::Horizontal));
        h.show_all();
        h
    }
}
