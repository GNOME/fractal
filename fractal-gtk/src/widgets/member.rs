use gtk;
use gtk::prelude::*;
use pango;

use crate::types::Member;

use crate::appop::AppOp;

use crate::cache::download_to_cache;
use crate::globals;
use crate::widgets;
use crate::widgets::AvatarExt;

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

        username.set_text(&self.member.get_alias());
        let mut alias = self.member.get_alias();
        alias.push_str("\n");
        alias.push_str(&self.member.uid);
        username.set_tooltip_text(&alias[..]);
        username.set_margin_end(5);
        username.set_ellipsize(pango::EllipsizeMode::End);
        username.set_valign(gtk::Align::Center);
        username.set_halign(gtk::Align::Start);
        if let Some(style) = username.get_style_context() {
            style.add_class("member");
        }

        let avatar = widgets::Avatar::avatar_new(Some(globals::USERLIST_ICON_SIZE));
        let badge = match self.op.member_level(self.member) {
            100 => Some(widgets::AvatarBadgeColor::Gold),
            50...100 => Some(widgets::AvatarBadgeColor::Silver),
            _ => None,
        };
        let data = avatar.circle(
            self.member.uid.clone(),
            Some(alias.clone()),
            globals::USERLIST_ICON_SIZE,
            badge,
            None,
        );
        let member_id = self.member.uid.clone();
        download_to_cache(backend.clone(), member_id.clone(), data.clone());

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

    pub fn pill(&self) -> gtk::Box {
        let backend = self.op.backend.clone();
        let pill = gtk::Box::new(gtk::Orientation::Horizontal, 3);

        let username = gtk::Label::new("");

        username.set_text(&self.member.get_alias());
        username.set_margin_end(3);
        if let Some(style) = username.get_style_context() {
            style.add_class("msg-highlighted");
        }

        let avatar = widgets::Avatar::avatar_new(Some(globals::PILL_ICON_SIZE));
        let data = avatar.circle(
            self.member.uid.clone(),
            Some(self.member.get_alias()),
            globals::PILL_ICON_SIZE,
            None,
            None,
        );
        let member_id = self.member.uid.clone();
        download_to_cache(backend.clone(), member_id.clone(), data.clone());

        avatar.set_margin_start(3);

        pill.pack_start(&avatar, true, true, 0);
        pill.pack_start(&username, true, true, 0);
        pill.show_all();
        pill
    }
}
