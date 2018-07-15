use gtk::prelude::*;
use gtk;
use gdk;

mod attach;
mod autocomplete;
mod direct;
mod account;
mod directory;
mod emoji;
mod headerbar;
mod invite;
mod join_room;
mod leave_room;
mod load_more;
mod login;
mod markdown;
mod media_viewer;
mod more_members;
mod new_room;
mod room_config;
mod roomlist_search;
mod scroll;
mod search;
mod send;
mod spellcheck;
mod stickers;

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
        let main_window = self.ui.builder
            .get_object::<gtk::ApplicationWindow>("main_window")
            .expect("Cant find main_window in ui file.");
        main_window.connect_key_press_event(move |w, k| {
            match k.get_keyval() {
                gdk::enums::key::Escape => {
                    Inhibit(op.lock().unwrap().escape(w))
                },
                gdk::enums::key::Left => {
                    Inhibit(op.lock().unwrap().left())
                },
                gdk::enums::key::Right => {
                    Inhibit(op.lock().unwrap().right())
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
        self.connect_media_viewer_headerbar();
        self.connect_media_viewer_box();
        self.connect_emoji();
        //self.connect_stickers();
        self.connect_autocomplete();
        self.connect_spellcheck();

        self.connect_directory();
        self.connect_room_config();
        self.connect_leave_room_dialog();
        self.connect_new_room_dialog();
        self.connect_join_room_dialog();
        self.connect_account_settings();

        self.connect_search();

        self.connect_member_search();
        self.connect_invite_dialog();
        self.connect_invite_user();
        self.connect_direct_chat();

        self.connect_roomlist_search();
    }
}
