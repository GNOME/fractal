extern crate gtk;
extern crate gettextrs;

use self::gtk::prelude::*;
use self::gettextrs::gettext;

use appop::AppOp;
use appop::member::SearchType;

use app::InternalCommand;
use backend::BKCommand;

use widgets;

use types::Member;
use types::Room;


impl AppOp {
    pub fn add_to_invite(&mut self, u: Member) {
        let listboxid = match self.search_type {
            SearchType::Invite => "to_invite",
            SearchType::DirectChat => "to_chat",
        };

        let to_invite = self.ui.builder
            .get_object::<gtk::ListBox>(listboxid)
            .expect("Can't find to_invite in ui file.");

        if self.invite_list.contains(&u) {
            return;
        }

        if let SearchType::DirectChat = self.search_type {
            self.invite_list = vec![];
            for ch in to_invite.get_children().iter() {
                to_invite.remove(ch);
            }
        }

        self.invite_list.push(u.clone());

        self.ui.builder
            .get_object::<gtk::Button>("direct_chat_button")
            .map(|btn| btn.set_sensitive(true));

        self.ui.builder
            .get_object::<gtk::Button>("invite_button")
            .map(|btn| btn.set_sensitive(true));

        let w;
        {
            let mb = widgets::MemberBox::new(&u, &self);
            w = mb.widget(true);
        }

        let mbox;

        mbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let btn = gtk::Button::new();
        let img = gtk::Image::new_from_icon_name("window-close-symbolic", 2);
        btn.get_style_context().unwrap().add_class("circular");
        btn.set_image(&img);

        mbox.pack_start(&w, true, true, 0);
        mbox.pack_start(&btn, false, false, 0);
        mbox.show_all();

        let tx = self.internal.clone();
        let uid = u.uid.clone();
        btn.connect_clicked(move |_| {
            tx.send(InternalCommand::RmInvite(uid.clone())).unwrap();
        });

        let size = (self.invite_list.len() - 1) as i32;
        to_invite.insert(&mbox, size);
    }

    pub fn rm_from_invite(&mut self, uid: String) {
        let invid;
        let dialogid;

        match self.search_type {
            SearchType::Invite => {
                invid = "to_invite";
                dialogid = "invite_user_dialog";
            }
            SearchType::DirectChat => {
                invid = "to_chat";
                dialogid = "direct_chat_dialog";
            }
        };

        let to_invite = self.ui.builder
            .get_object::<gtk::ListBox>(invid)
            .expect("Can't find to_invite in ui file.");
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>(dialogid)
            .expect("Can't find invite_user_dialog in ui file.");

        let idx = self.invite_list.iter().position(|x| x.uid == uid);
        if let Some(i) = idx {
            self.invite_list.remove(i);
            if let Some(r) = to_invite.get_row_at_index(i as i32) {
                to_invite.remove(&r);
            }
        }

        if self.invite_list.is_empty() {
            self.ui.builder
                .get_object::<gtk::Button>("direct_chat_button")
                .map(|btn| btn.set_sensitive(false));

            self.ui.builder
                .get_object::<gtk::Button>("invite_button")
                .map(|btn| btn.set_sensitive(false));
        }

        dialog.resize(300, 200);
    }

    pub fn show_invite_user_dialog(&mut self) {
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("invite_user_dialog")
            .expect("Can't find invite_user_dialog in ui file.");
        let scroll = self.ui.builder
            .get_object::<gtk::Widget>("user_search_scroll")
            .expect("Can't find user_search_scroll in ui file.");
        let title = self.ui.builder
            .get_object::<gtk::Label>("invite_title")
            .expect("Can't find invite_title in ui file.");
        self.search_type = SearchType::Invite;

        if let Some(aroom) = self.active_room.clone() {
            if let Some(r) = self.rooms.get(&aroom) {
                if let &Some(ref name) = &r.name {
                    let sentence_template = gettext("Invite to {name}");
                    title.set_text(&sentence_template.replace("{name}", name));
                } else {
                    title.set_text(gettext("Invite").as_str());
                }
            }
        }
        dialog.present();
        scroll.hide();
    }

    pub fn invite(&mut self) {
        if let &Some(ref r) = &self.active_room {
            for user in &self.invite_list {
                self.backend.send(BKCommand::Invite(r.clone(), user.uid.clone())).unwrap();
            }
        }
        self.close_invite_dialog();
    }

    pub fn close_invite_dialog(&mut self) {
        let listbox = self.ui.builder
            .get_object::<gtk::ListBox>("user_search_box")
            .expect("Can't find user_search_box in ui file.");
        let scroll = self.ui.builder
            .get_object::<gtk::Widget>("user_search_scroll")
            .expect("Can't find user_search_scroll in ui file.");
        let to_invite = self.ui.builder
            .get_object::<gtk::ListBox>("to_invite")
            .expect("Can't find to_invite in ui file.");
        let entry = self.ui.builder
            .get_object::<gtk::Entry>("invite_entry")
            .expect("Can't find invite_entry in ui file.");
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("invite_user_dialog")
            .expect("Can't find invite_user_dialog in ui file.");

        self.invite_list = vec![];
        for ch in to_invite.get_children().iter() {
            to_invite.remove(ch);
        }
        for ch in listbox.get_children().iter() {
            listbox.remove(ch);
        }
        scroll.hide();
        entry.set_text("");
        dialog.hide();
        dialog.resize(300, 200);
    }

    pub fn remove_inv(&mut self, roomid: String) {
        self.rooms.remove(&roomid);
        self.roomlist.remove_room(roomid);
    }

    pub fn accept_inv(&mut self, accept: bool) {
        if let Some(ref rid) = self.invitation_roomid {
            match accept {
                true => self.backend.send(BKCommand::AcceptInv(rid.clone())).unwrap(),
                false => self.backend.send(BKCommand::RejectInv(rid.clone())).unwrap(),
            }
            self.internal.send(InternalCommand::RemoveInv(rid.clone())).unwrap();
        }
        self.invitation_roomid = None;
    }

    pub fn show_inv_dialog(&mut self, r: &Room) {
        let dialog = self.ui.builder
            .get_object::<gtk::MessageDialog>("invite_dialog")
            .expect("Can't find invite_dialog in ui file.");

        let room_name = r.name.clone().unwrap_or_default();
        let title = format!("{} {}?", gettext("Join"), room_name);
        let secondary;
        if let Some(ref sender) = r.inv_sender {
            let sender_name = sender.get_alias();
            let sentence_template = gettext("You've been invited to join to <b>{room_name}</b> room by <b>{sender_name}</b>");
            secondary = sentence_template.replace("{room_name}", room_name.as_str())
                                         .replace("{sender_name}", sender_name.as_str());
        } else {
            let sentence_template = gettext("You've been invited to join to <b>{room_name}</b>");
            secondary = sentence_template.replace("{room_name}", room_name.as_str());
        }

        dialog.set_property_text(Some(&title));
        dialog.set_property_secondary_use_markup(true);
        dialog.set_property_secondary_text(Some(&secondary));

        self.invitation_roomid = Some(r.id.clone());
        dialog.present();
    }
}
