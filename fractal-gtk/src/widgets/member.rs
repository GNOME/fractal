extern crate pango;
extern crate gtk;

use self::gtk::prelude::*;

use types::Member;

use appop::AppOp;

use globals;
use cache::download_to_cache;
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

        download_to_cache(backend.clone(), self.member.uid.clone());
        let avatar = widgets::Avatar::avatar_new(Some(globals::USERLIST_ICON_SIZE));
        avatar.circle(self.member.uid.clone(), Some(alias.clone()), globals::USERLIST_ICON_SIZE);
        //get_member_info(backend.clone(), avatar.clone(), username.clone(), self.member.uid.clone(), globals::USERLIST_ICON_SIZE, 10);
        avatar.set_margin_start(3);
        avatar.set_valign(gtk::Align::Center);

        v.set_margin_start(3);
        v.pack_start(&username, true, true, 0);
        if show_uid {
            v.pack_start(&uid, true, true, 0);
        }

        match self.op.member_level(self.member) {
            100 => {
                let overlay = gtk::Overlay::new();
                overlay.add(&avatar);
                overlay.add_overlay(&widgets::admin_badge(widgets::AdminColor::Gold, None));
                w.add(&overlay);
            }
            50 => {
                let overlay = gtk::Overlay::new();
                overlay.add(&avatar);
                overlay.add_overlay(&widgets::admin_badge(widgets::AdminColor::Silver, None));
                w.add(&overlay);
            }
            _ => {
                w.add(&avatar);
            }
        }

        w.add(&v);

        event_box.add(&w);
        event_box.show_all();
        event_box
    }
}
