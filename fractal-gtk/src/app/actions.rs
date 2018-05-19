use gio;
use gio::SimpleActionExt;
use gio::ActionMapExt;

use appop::AppState;

use app::App;

impl App {
    pub fn create_actions(&self) {
        let settings = gio::SimpleAction::new("settings", None);
        let account = gio::SimpleAction::new("account_settings", None);
        let dir = gio::SimpleAction::new("directory", None);
        let chat = gio::SimpleAction::new("start_chat", None);
        let newr = gio::SimpleAction::new("new_room", None);
        let joinr = gio::SimpleAction::new("join_room", None);
        let logout = gio::SimpleAction::new("logout", None);

        let room = gio::SimpleAction::new("room_details", None);
        let inv = gio::SimpleAction::new("room_invite", None);
        let search = gio::SimpleAction::new("search", None);
        let leave = gio::SimpleAction::new("leave_room", None);

        let quit = gio::SimpleAction::new("quit", None);
        let shortcuts = gio::SimpleAction::new("shortcuts", None);
        let about = gio::SimpleAction::new("about", None);

        let op = &self.op;

        op.lock().unwrap().gtk_app.add_action(&settings);
        op.lock().unwrap().gtk_app.add_action(&account);
        op.lock().unwrap().gtk_app.add_action(&dir);
        op.lock().unwrap().gtk_app.add_action(&chat);
        op.lock().unwrap().gtk_app.add_action(&newr);
        op.lock().unwrap().gtk_app.add_action(&joinr);
        op.lock().unwrap().gtk_app.add_action(&logout);

        op.lock().unwrap().gtk_app.add_action(&room);
        op.lock().unwrap().gtk_app.add_action(&inv);
        op.lock().unwrap().gtk_app.add_action(&search);
        op.lock().unwrap().gtk_app.add_action(&leave);

        op.lock().unwrap().gtk_app.add_action(&quit);
        op.lock().unwrap().gtk_app.add_action(&shortcuts);
        op.lock().unwrap().gtk_app.add_action(&about);

        quit.connect_activate(clone!(op => move |_, _| op.lock().unwrap().quit() ));
        about.connect_activate(clone!(op => move |_, _| op.lock().unwrap().about_dialog() ));

        settings.connect_activate(move |_, _| { println!("SETTINGS"); });
        settings.set_enabled(false);

        account.connect_activate(clone!(op => move |_, _| op.lock().unwrap().show_account_settings_dialog()) );

        dir.connect_activate(clone!(op => move |_, _| op.lock().unwrap().set_state(AppState::Directory) ));
        logout.connect_activate(clone!(op => move |_, _| op.lock().unwrap().logout() ));
        room.connect_activate(clone!(op => move |_, _| op.lock().unwrap().show_room_dialog() ));
        inv.connect_activate(clone!(op => move |_, _| op.lock().unwrap().show_invite_user_dialog() ));
        chat.connect_activate(clone!(op => move |_, _| op.lock().unwrap().show_direct_chat_dialog() ));
        search.connect_activate(clone!(op => move |_, _| op.lock().unwrap().toggle_search() ));
        leave.connect_activate(clone!(op => move |_, _| op.lock().unwrap().leave_active_room() ));
        newr.connect_activate(clone!(op => move |_, _| op.lock().unwrap().new_room_dialog() ));
        joinr.connect_activate(clone!(op => move |_, _| op.lock().unwrap().join_to_room_dialog() ));
    }
}
