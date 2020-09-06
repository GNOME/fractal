use crate::backend::room;
use gtk::prelude::*;
use std::thread;

use crate::app::App;
use crate::appop::AppOp;
use crate::appop::SearchType;
use crate::backend::HandleError;

impl AppOp {
    pub fn start_chat(&mut self) {
        if self.invite_list.len() != 1 {
            return;
        }

        let login_data = unwrap_or_unit_return!(self.login_data.clone());
        let user = self.invite_list[0].clone();

        let member = user.0.clone();
        thread::spawn(move || {
            match room::direct_chat(
                login_data.server_url,
                login_data.access_token,
                login_data.uid,
                member,
            ) {
                Ok(r) => {
                    APPOP!(new_room, (r));
                }
                Err(err) => {
                    err.handle_error();
                }
            }
        });

        self.close_direct_chat_dialog();
    }

    pub fn show_direct_chat_dialog(&mut self) {
        let dialog = self
            .ui
            .builder
            .get_object::<libhandy::Dialog>("direct_chat_dialog")
            .expect("Can't find direct_chat_dialog in ui file.");
        let scroll = self
            .ui
            .builder
            .get_object::<gtk::Widget>("direct_chat_search_scroll")
            .expect("Can't find direct_chat_search_scroll in ui file.");
        self.search_type = SearchType::DirectChat;
        if let Some(btn) = self
            .ui
            .builder
            .get_object::<gtk::Button>("direct_chat_button")
        {
            btn.set_sensitive(false)
        }
        dialog.present();
        scroll.hide();
    }

    pub fn close_direct_chat_dialog(&mut self) {
        let listbox = self
            .ui
            .builder
            .get_object::<gtk::ListBox>("direct_chat_search_box")
            .expect("Can't find direct_chat_search_box in ui file.");
        let scroll = self
            .ui
            .builder
            .get_object::<gtk::Widget>("direct_chat_search_scroll")
            .expect("Can't find direct_chat_search_scroll in ui file.");
        let to_chat_entry = self
            .ui
            .builder
            .get_object::<gtk::TextView>("to_chat_entry")
            .expect("Can't find to_chat_entry in ui file.");
        let entry = self
            .ui
            .builder
            .get_object::<gtk::TextView>("to_chat_entry")
            .expect("Can't find to_chat_entry in ui file.");
        let dialog = self
            .ui
            .builder
            .get_object::<libhandy::Dialog>("direct_chat_dialog")
            .expect("Can't find direct_chat_dialog in ui file.");

        self.invite_list = vec![];
        if let Some(buffer) = to_chat_entry.get_buffer() {
            let mut start = buffer.get_start_iter();
            let mut end = buffer.get_end_iter();

            buffer.delete(&mut start, &mut end);
        }
        for ch in listbox.get_children().iter() {
            listbox.remove(ch);
        }
        scroll.hide();
        if let Some(buffer) = entry.get_buffer() {
            buffer.set_text("");
        }
        dialog.hide();
        dialog.resize(300, 200);
    }
}
