use crate::i18n::{i18n, i18n_k};

use fractal_api::backend::room;
use fractal_api::identifiers::{RoomId, UserId};
use fractal_api::util::ResultExpectLog;
use gtk::prelude::*;
use std::thread;

use crate::app::App;
use crate::appop::member::SearchType;
use crate::appop::AppOp;

use crate::backend::BKResponse;

use crate::globals;

use crate::widgets;

use crate::types::Member;

impl AppOp {
    pub fn add_to_invite(&mut self, u: Member) {
        if self.invite_list.iter().any(|(mem, _)| *mem == u) {
            return;
        }

        let textviewid = match self.search_type {
            SearchType::Invite => "invite_entry",
            SearchType::DirectChat => "to_chat_entry",
        };

        let invite_entry = self
            .ui
            .builder
            .get_object::<gtk::TextView>(textviewid)
            .expect("Can't find invite_entry in ui file.");

        if let SearchType::DirectChat = self.search_type {
            self.invite_list = vec![];

            if let Some(buffer) = invite_entry.get_buffer() {
                let mut start = buffer.get_start_iter();
                let mut end = buffer.get_end_iter();

                buffer.delete(&mut start, &mut end);
            }
        }

        if let Some(btn) = self
            .ui
            .builder
            .get_object::<gtk::Button>("direct_chat_button")
        {
            btn.set_sensitive(true)
        }

        if let Some(btn) = self.ui.builder.get_object::<gtk::Button>("invite_button") {
            btn.set_sensitive(true)
        }

        if let Some(buffer) = invite_entry.get_buffer() {
            let mut start_word = buffer.get_iter_at_offset(buffer.get_property_cursor_position());
            let mut end_word = buffer.get_iter_at_offset(buffer.get_property_cursor_position());

            // Remove the search input in the entry before inserting the member's pill
            if !start_word.starts_word() {
                start_word.backward_word_start();
            }
            if !end_word.ends_word() {
                end_word.forward_word_end();
            }
            buffer.delete(&mut start_word, &mut end_word);

            if let Some(anchor) = buffer.create_child_anchor(&mut end_word) {
                let w;
                {
                    let mb = widgets::MemberBox::new(&u, &self);
                    w = mb.pill();
                }

                invite_entry.add_child_at_anchor(&w, &anchor);

                self.invite_list.push((u, anchor));
            }
        }
    }

    pub fn rm_from_invite(&mut self, uid: UserId) {
        let idx = self.invite_list.iter().position(|x| x.0.uid == uid);
        if let Some(i) = idx {
            self.invite_list.remove(i);
        }

        if self.invite_list.is_empty() {
            if let Some(btn) = self
                .ui
                .builder
                .get_object::<gtk::Button>("direct_chat_button")
            {
                btn.set_sensitive(false)
            }

            if let Some(btn) = self.ui.builder.get_object::<gtk::Button>("invite_button") {
                btn.set_sensitive(false)
            }
        }

        let dialogid = match self.search_type {
            SearchType::Invite => "invite_user_dialog",
            SearchType::DirectChat => "direct_chat_dialog",
        };

        let dialog = self
            .ui
            .builder
            .get_object::<libhandy::Dialog>(dialogid)
            .expect("Can’t find invite_user_dialog in ui file.");

        dialog.resize(300, 200);
    }

    pub fn detect_removed_invite(&mut self) {
        let invite_list = self.invite_list.clone();
        for (member, anchor) in invite_list {
            if anchor.get_deleted() {
                self.rm_from_invite(member.uid);
            }
        }
    }

    pub fn show_invite_user_dialog(&mut self) {
        let dialog = self
            .ui
            .builder
            .get_object::<libhandy::Dialog>("invite_user_dialog")
            .expect("Can't find invite_user_dialog in ui file.");
        let scroll = self
            .ui
            .builder
            .get_object::<gtk::Widget>("user_search_scroll")
            .expect("Can't find user_search_scroll in ui file.");
        let headerbar = self
            .ui
            .builder
            .get_object::<gtk::HeaderBar>("invite_headerbar")
            .expect("Can't find invite_headerbar in ui file.");
        self.search_type = SearchType::Invite;

        if let Some(aroom) = self.active_room.clone() {
            if let Some(r) = self.rooms.get(&aroom) {
                if let Some(ref name) = r.name {
                    headerbar
                        .set_title(Some(i18n_k("Invite to {name}", &[("name", name)]).as_str()));
                } else {
                    headerbar.set_title(Some(i18n("Invite").as_str()));
                }
            }
        }

        self.set_invite_user_dialog_placeholder();

        dialog.present();
        scroll.hide();
    }

    pub fn invite(&mut self) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        if let Some(ref r) = self.active_room {
            for user in &self.invite_list {
                let server = login_data.server_url.clone();
                let access_token = login_data.access_token.clone();
                let room_id = r.clone();
                let user_id = user.0.uid.clone();
                let tx = self.backend.clone();
                thread::spawn(move || {
                    let query = room::invite(server, access_token, room_id, user_id);
                    if let Err(err) = query {
                        tx.send(BKResponse::InviteError(err))
                            .expect_log("Connection closed");
                    }
                });
            }
        }
        self.close_invite_dialog();
    }

    pub fn close_invite_dialog(&mut self) {
        let listbox = self
            .ui
            .builder
            .get_object::<gtk::ListBox>("user_search_box")
            .expect("Can't find user_search_box in ui file.");
        let scroll = self
            .ui
            .builder
            .get_object::<gtk::Widget>("user_search_scroll")
            .expect("Can't find user_search_scroll in ui file.");
        let invite_entry = self
            .ui
            .builder
            .get_object::<gtk::TextView>("invite_entry")
            .expect("Can't find invite_entry in ui file.");
        let dialog = self
            .ui
            .builder
            .get_object::<libhandy::Dialog>("invite_user_dialog")
            .expect("Can't find invite_user_dialog in ui file.");

        self.invite_list = vec![];
        if let Some(buffer) = invite_entry.get_buffer() {
            let mut start = buffer.get_start_iter();
            let mut end = buffer.get_end_iter();

            buffer.delete(&mut start, &mut end);
        }
        for ch in listbox.get_children().iter() {
            listbox.remove(ch);
        }
        scroll.hide();
        dialog.hide();
        dialog.resize(300, 200);
    }

    pub fn remove_inv(&mut self, room_id: RoomId) {
        self.rooms.remove(&room_id);
        self.roomlist.remove_room(room_id);
    }

    pub fn accept_inv(&mut self, accept: bool) {
        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        if let Some(rid) = self.invitation_roomid.take() {
            let room_id = rid.clone();
            if accept {
                let tx = self.backend.clone();
                thread::spawn(move || {
                    match room::join_room(login_data.server_url, login_data.access_token, room_id) {
                        Ok(jtr) => {
                            let jtr = Some(jtr);
                            APPOP!(set_join_to_room, (jtr));
                            APPOP!(reload_rooms);
                        }
                        Err(err) => {
                            tx.send(BKResponse::JoinRoomError(err))
                                .expect_log("Connection closed");
                        }
                    }
                });
            } else {
                let tx = self.backend.clone();
                thread::spawn(move || {
                    let query =
                        room::leave_room(login_data.server_url, login_data.access_token, room_id);
                    if let Err(err) = query {
                        tx.send(BKResponse::LeaveRoomError(err))
                            .expect_log("Connection closed");
                    }
                });
            }
            self.remove_inv(rid);
        }
    }

    /* FIXME: move to a widget */
    pub fn show_inv_dialog(&self, sender: Option<&Member>, room_name: Option<&String>) {
        let dialog = self
            .ui
            .builder
            .get_object::<gtk::MessageDialog>("invite_dialog")
            .expect("Can't find invite_dialog in ui file.");

        let empty = String::new();
        let room_name = room_name.unwrap_or(&empty);
        let title = i18n_k("Join {room_name}?", &[("room_name", &room_name)]);
        let secondary;
        if let Some(ref sender) = sender {
            let sender_name = sender.get_alias();
            secondary = i18n_k(
                "You’ve been invited to join <b>{room_name}</b> room by <b>{sender_name}</b>",
                &[("room_name", &room_name), ("sender_name", &sender_name)],
            );
        } else {
            secondary = i18n_k(
                "You’ve been invited to join <b>{room_name}</b>",
                &[("room_name", &room_name)],
            );
        }

        dialog.set_property_text(Some(title.as_str()));
        dialog.set_property_secondary_use_markup(true);
        dialog.set_property_secondary_text(Some(secondary.as_str()));

        dialog.present();
    }

    pub fn set_invite_user_dialog_placeholder(&mut self) {
        let textviewid = match self.search_type {
            SearchType::Invite => "invite_entry",
            SearchType::DirectChat => "to_chat_entry",
        };

        let invite_entry = self
            .ui
            .builder
            .get_object::<gtk::TextView>(textviewid)
            .expect("Can't find invite_entry in ui file.");

        if let Some(buffer) = invite_entry.get_buffer() {
            let start = buffer.get_start_iter();
            let end = buffer.get_end_iter();

            if let Some(text) = buffer.get_text(&start, &end, true) {
                if text.is_empty() && self.invite_list.is_empty() {
                    buffer.set_text(globals::PLACEHOLDER_TEXT);

                    let start = buffer.get_start_iter();
                    let end = buffer.get_end_iter();

                    buffer.apply_tag_by_name("placeholder", &start, &end);
                }
            }
        }
    }

    pub fn remove_invite_user_dialog_placeholder(&mut self) {
        let textviewid = match self.search_type {
            SearchType::Invite => "invite_entry",
            SearchType::DirectChat => "to_chat_entry",
        };

        let invite_entry = self
            .ui
            .builder
            .get_object::<gtk::TextView>(textviewid)
            .expect("Can't find invite_entry in ui file.");

        if let Some(buffer) = invite_entry.get_buffer() {
            let start = buffer.get_start_iter();
            let end = buffer.get_end_iter();

            if let Some(text) = buffer.get_text(&start, &end, true) {
                if text == globals::PLACEHOLDER_TEXT && self.invite_list.is_empty() {
                    buffer.set_text("");

                    let start = buffer.get_start_iter();
                    let end = buffer.get_end_iter();

                    buffer.remove_tag_by_name("placeholder", &start, &end);
                }
            }
        }
    }
}
