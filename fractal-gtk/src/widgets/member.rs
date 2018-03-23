extern crate pango;
extern crate gtk;
extern crate gdk_pixbuf;

use self::gdk_pixbuf::Pixbuf;
use self::gtk::prelude::*;

use types::Member;

use backend::BKCommand;

use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::TryRecvError;

use app::AppOp;

use globals;
use widgets;
use widgets::AvatarExt;

// Room Search item
pub struct MemberBox<'a> {
    member: &'a Member,
    op: &'a AppOp,
}

impl<'a> MemberBox<'a> {
    pub fn new(member: &'a Member, op: &'a AppOp) -> MemberBox<'a> {
        MemberBox {
            member: member,
            op: op,
        }
    }

    pub fn widget(&self, show_uid: bool) -> gtk::EventBox {
        let backend = self.op.backend.clone();
        let username = gtk::Label::new("");
        let uid = gtk::Label::new("");
        let event_box = gtk::EventBox::new();
        let w = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        let v = gtk::Box::new(gtk::Orientation::Vertical, 0);

        uid.set_text(&self.member.uid);
        uid.set_valign(gtk::Align::Start);
        uid.set_halign(gtk::Align::Start);
        if let Some(style) = uid.get_style_context() {
            style.add_class("member-uid");
        }

        username.set_text(&self.member.get_alias().unwrap_or_default());
        username.set_tooltip_text(&self.member.get_alias().unwrap_or_default()[..]);
        username.set_margin_end(5);
        username.set_ellipsize(pango::EllipsizeMode::End);
        username.set_valign(gtk::Align::Center);
        username.set_halign(gtk::Align::Start);
        if let Some(style) = username.get_style_context() {
            style.add_class("member");
        }

        let avatar = widgets::Avatar::avatar_new(Some(globals::USERLIST_ICON_SIZE));
        avatar.default(String::from("avatar-default-symbolic"),
                       Some(globals::USERLIST_ICON_SIZE));
        get_member_info(backend.clone(), avatar.clone(), username.clone(), self.member.uid.clone(), globals::USERLIST_ICON_SIZE, 10);
        avatar.set_margin_start(3);
        avatar.set_valign(gtk::Align::Center);

        v.set_margin_start(3);
        v.pack_start(&username, true, true, 0);
        if show_uid {
            v.pack_start(&uid, true, true, 0);
        }

        w.add(&avatar);
        w.add(&v);

        event_box.add(&w);
        event_box.show_all();
        event_box
    }
}

#[allow(dead_code)]
pub fn get_member_avatar(backend: Sender<BKCommand>,
                         img: widgets::Avatar,
                         m: Option<Member>,
                         size: i32, tries: i32) {
    if tries <= 0 {
        return;
    }

    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    backend.send(BKCommand::GetAvatarAsync(m.clone(), tx)).unwrap();
    gtk::timeout_add(100, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => gtk::Continue(false),
        Ok(avatar) => {
            if let Ok(_) = Pixbuf::new_from_file_at_scale(&avatar, size, size, false) {
                img.circle(avatar, Some(size));
            } else {
                // trying again if fail
                img.default(String::from("avatar-default-symbolic"), Some(size));
                get_member_avatar(backend.clone(), img.clone(), m.clone(), size, tries - 1);
            }

            gtk::Continue(false)
        }
    });
}



pub fn get_member_info(backend: Sender<BKCommand>,
                       img: widgets::Avatar,
                       username: gtk::Label,
                       sender: String,
                       size: i32, tries: i32) {

    if tries <= 0 {
        return;
    }

    let (tx, rx): (Sender<(String, String)>, Receiver<(String, String)>) = channel();
    backend.send(BKCommand::GetUserInfoAsync(sender.clone(), tx)).unwrap();
    gtk::timeout_add(100, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => gtk::Continue(false),
        Ok((name, avatar)) => {
            if let Ok(_) = Pixbuf::new_from_file_at_scale(&avatar, size, size, false) {
                img.circle(avatar, Some(size));
            } else {
                // trying again if fail
                img.default(String::from("avatar-default-symbolic"), Some(size));
                get_member_info(backend.clone(), img.clone(), username.clone(), sender.clone(), size, tries - 1);
                return gtk::Continue(false);
            }

            if !name.is_empty() {
                username.set_text(&name);
            } else {
                get_member_info(backend.clone(), img.clone(), username.clone(), sender.clone(), size, tries - 1);
            }

            gtk::Continue(false)
        }
    });
}
