mod autocomplete;
mod direct;
mod directory;
mod headerbar;
mod invite;
mod join_room;
mod leave_room;
mod login;
mod markdown;
mod new_room;
mod roomlist_search;
mod send;

use crate::app::App;

impl App {
    pub fn connect_gtk(&self) {
        self.connect_headerbars();
        self.connect_login_view();

        self.connect_send();
        self.connect_markdown();
        self.connect_autocomplete();

        self.connect_directory();
        self.connect_leave_room_dialog();
        self.connect_new_room_dialog();
        self.connect_join_room_dialog();

        self.connect_invite_dialog();
        self.connect_invite_user();
        self.connect_direct_chat();

        self.connect_roomlist_search();
    }
}
