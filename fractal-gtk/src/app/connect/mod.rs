extern crate gdk;
extern crate gtk;
use self::gtk::prelude::*;

mod attach;
mod autocomplete;
mod direct;
mod directory;
mod headerbar;
mod invite;
mod join_room;
mod leave_room;
mod load_more;
mod login;
mod markdown;
mod more_members;
mod new_room;
mod room_config;
mod roomlist_search;
mod scroll;
mod search;
mod send;
mod spellcheck;

use app::App;

impl App {
    pub fn connect_gtk(&self) {
        // Set up shutdown callback
        let window: gtk::Window = self.ui.builder
            .get_object("main_window")
            .expect("Couldn't find main_window in ui file.");

        window.set_title("Fractal");
        window.show_all();

        let op = self.op.clone();
        window.connect_delete_event(move |_, _| {
            op.lock().unwrap().quit();
            Inhibit(false)
        });

        let op = self.op.clone();
        let chat: gtk::Widget = self.ui.builder
            .get_object("room_view_stack")
            .expect("Couldn't find room_view_stack in ui file.");
        chat.connect_key_release_event(move |_, k| {
            match k.get_keyval() {
                gdk::enums::key::Escape => {
                    op.lock().unwrap().escape();
                    Inhibit(true)
                },
                _ => Inhibit(false)
            }
        });

        let op = self.op.clone();
        window.connect_property_has_toplevel_focus_notify(move |w| {
            if !w.is_active() {
                op.lock().unwrap().mark_active_room_messages();
            }
        });

        self.create_load_more_spn();
        self.connect_more_members_btn();
        self.create_actions();

        self.connect_headerbars();
        self.connect_login_view();

        self.connect_msg_scroll();

        self.connect_send();
        self.connect_attach();
        self.connect_markdown();
        self.connect_autocomplete();
        self.connect_spellcheck();

        self.connect_directory();
        self.connect_room_config();
        self.connect_leave_room_dialog();
        self.connect_new_room_dialog();
        self.connect_join_room_dialog();

        self.connect_search();

        self.connect_member_search();
        self.connect_invite_dialog();
        self.connect_invite_user();
        self.connect_direct_chat();

        self.connect_roomlist_search();
    }
}
