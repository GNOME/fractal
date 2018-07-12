extern crate rand;
extern crate gtk;

use self::gtk::prelude::*;

use appop::AppOp;
use appop::RoomPanel;
use appop::SearchType;

use backend::BKCommand;
use types::Room;

use self::rand::{thread_rng, Rng};


impl AppOp {
    pub fn start_chat(&mut self) {
        if self.invite_list.len() != 1 {
            return;
        }

        let user = self.invite_list[0].clone();

        let internal_id: String = thread_rng().gen_ascii_chars().take(10).collect();
        self.backend.send(BKCommand::DirectChat(user.clone(), internal_id.clone())).unwrap();
        self.close_direct_chat_dialog();

        let mut fakeroom = Room::new(internal_id.clone(), user.alias.clone());
        fakeroom.direct = true;

        self.new_room(fakeroom, None);
        self.roomlist.set_selected(Some(internal_id.clone()));
        self.set_active_room_by_id(internal_id);
        self.room_panel(RoomPanel::Loading);
    }

    pub fn show_direct_chat_dialog(&mut self) {
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("direct_chat_dialog")
            .expect("Can't find direct_chat_dialog in ui file.");
        let scroll = self.ui.builder
            .get_object::<gtk::Widget>("direct_chat_search_scroll")
            .expect("Can't find direct_chat_search_scroll in ui file.");
        self.search_type = SearchType::DirectChat;
        self.ui.builder
            .get_object::<gtk::Button>("direct_chat_button")
            .map(|btn| btn.set_sensitive(false));
        dialog.present();
        scroll.hide();
    }

    pub fn close_direct_chat_dialog(&mut self) {
        let listbox = self.ui.builder
            .get_object::<gtk::ListBox>("direct_chat_search_box")
            .expect("Can't find direct_chat_search_box in ui file.");
        let scroll = self.ui.builder
            .get_object::<gtk::Widget>("direct_chat_search_scroll")
            .expect("Can't find direct_chat_search_scroll in ui file.");
        let to_invite = self.ui.builder
            .get_object::<gtk::ListBox>("to_chat")
            .expect("Can't find to_chat in ui file.");
        let entry = self.ui.builder
            .get_object::<gtk::Entry>("to_chat_entry")
            .expect("Can't find to_chat_entry in ui file.");
        let dialog = self.ui.builder
            .get_object::<gtk::Dialog>("direct_chat_dialog")
            .expect("Can't find direct_chat_dialog in ui file.");

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
}
