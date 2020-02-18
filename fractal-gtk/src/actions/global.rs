use fractal_api::clone;
use log::{debug, info};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::appop::AppOp;
use crate::i18n::i18n;
use crate::widgets::FileDialog::open;
use crate::App;
use fractal_api::types::Message;
use gio::prelude::*;
use gio::SimpleAction;
use glib;
use gtk::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Login,
    Loading,
    NoRoom,
    Room,
    RoomSettings,
    MediaViewer,
    AccountSettings,
    Directory,
}

impl<'a> From<&'a glib::Variant> for AppState {
    fn from(v: &glib::Variant) -> AppState {
        v.get::<String>().expect("Invalid back state type").into()
    }
}

impl From<String> for AppState {
    fn from(v: String) -> AppState {
        match v.as_str() {
            "login" => AppState::Login,
            "loading" => AppState::Loading,
            "no-room" => AppState::NoRoom,
            "room" => AppState::Room,
            "media-viewer" => AppState::MediaViewer,
            "account-settings" => AppState::AccountSettings,
            "room-settings" => AppState::RoomSettings,
            "directory" => AppState::Directory,
            _ => panic!("Invalid back state type"),
        }
    }
}

impl From<AppState> for glib::Variant {
    fn from(v: AppState) -> glib::Variant {
        match v {
            AppState::Login => "login".to_variant(),
            AppState::Loading => "loading".to_variant(),
            AppState::NoRoom => "no-room".to_variant(),
            AppState::Room => "room".to_variant(),
            AppState::MediaViewer => "media-viewer".to_variant(),
            AppState::AccountSettings => "account-settings".to_variant(),
            AppState::RoomSettings => "room-setting".to_variant(),
            AppState::Directory => "directory".to_variant(),
        }
    }
}

/* This creates globale actions which are connected to the application */
/* TODO: Remove op */
pub fn new(app: &gtk::Application, op: &Arc<Mutex<AppOp>>) {
    let settings = SimpleAction::new("settings", None);
    let chat = SimpleAction::new("start_chat", None);
    let newr = SimpleAction::new("new_room", None);
    let joinr = SimpleAction::new("join_room", None);
    let logout = SimpleAction::new("logout", None);

    let inv = SimpleAction::new("room_invite", None);
    let search = SimpleAction::new("search", None);
    let leave = SimpleAction::new("leave_room", None);

    let shortcuts = SimpleAction::new("shortcuts", None);
    let about = SimpleAction::new("about", None);
    let quit = SimpleAction::new("quit", None);

    let open_room = SimpleAction::new("open-room", glib::VariantTy::new("s").ok());
    let back = SimpleAction::new("back", None);
    let media_viewer = SimpleAction::new("open-media-viewer", glib::VariantTy::new("s").ok());
    let account = SimpleAction::new("open-account-settings", None);
    let directory = SimpleAction::new("directory", None);
    //TODO: use roomid as value
    let room_settings = SimpleAction::new("open-room-settings", None);
    // TODO: send file should be a room_history action
    let send_file = SimpleAction::new("send-file", None);

    let previous_room = SimpleAction::new("previous-room", None);
    let next_room = SimpleAction::new("next-room", None);
    let prev_unread_room = SimpleAction::new("prev-unread-room", None);
    let next_unread_room = SimpleAction::new("next-unread-room", None);
    let first_room = SimpleAction::new("first-room", None);
    let last_room = SimpleAction::new("last-room", None);
    let older_messages = SimpleAction::new("older-messages", None);
    let newer_messages = SimpleAction::new("newer-messages", None);

    app.add_action(&settings);
    app.add_action(&account);
    app.add_action(&chat);
    app.add_action(&newr);
    app.add_action(&joinr);
    app.add_action(&logout);

    app.add_action(&inv);
    app.add_action(&search);
    app.add_action(&leave);

    app.add_action(&quit);
    app.add_action(&shortcuts);
    app.add_action(&about);
    app.add_action(&open_room);
    app.add_action(&back);
    app.add_action(&directory);
    app.add_action(&room_settings);
    app.add_action(&media_viewer);
    app.add_action(&account);

    app.add_action(&send_file);

    app.add_action(&previous_room);
    app.add_action(&next_room);
    app.add_action(&prev_unread_room);
    app.add_action(&next_unread_room);
    app.add_action(&first_room);
    app.add_action(&last_room);
    app.add_action(&older_messages);
    app.add_action(&newer_messages);

    // When activated, shuts down the application
    let app_weak = app.downgrade();
    quit.connect_activate(move |_action, _parameter| {
        let app = upgrade_weak!(app_weak);
        app.quit();
    });

    about.connect_activate(clone!(op => move |_, _| op.lock().unwrap().about_dialog() ));

    settings.connect_activate(move |_, _| {
        info!("SETTINGS");
    });
    settings.set_enabled(false);

    logout.connect_activate(clone!(op => move |_, _| op.lock().unwrap().logout() ));
    inv.connect_activate(clone!(op => move |_, _| op.lock().unwrap().show_invite_user_dialog() ));
    chat.connect_activate(clone!(op => move |_, _| op.lock().unwrap().show_direct_chat_dialog() ));
    leave.connect_activate(clone!(op => move |_, _| op.lock().unwrap().leave_active_room() ));
    newr.connect_activate(clone!(op => move |_, _| op.lock().unwrap().new_room_dialog() ));
    joinr.connect_activate(clone!(op => move |_, _| op.lock().unwrap().join_to_room_dialog() ));

    previous_room.connect_activate(clone!(op => move |_, _| {
        let mut op = op.lock().unwrap();
        if let Some(id) = op.roomlist.prev_id() {
            op.set_active_room_by_id(id);
        }
    }));
    next_room.connect_activate(clone!(op => move |_, _| {
        let mut op = op.lock().unwrap();
        if let Some(id) = op.roomlist.next_id() {
            op.set_active_room_by_id(id);
        }
    }));
    prev_unread_room.connect_activate(clone!(op => move |_, _| {
        let mut op = op.lock().unwrap();
        if let Some(id) = op.roomlist.prev_unread_id() {
            op.set_active_room_by_id(id);
        }
    }));
    next_unread_room.connect_activate(clone!(op => move |_, _| {
        let mut op = op.lock().unwrap();
        if let Some(id) = op.roomlist.next_unread_id() {
            op.set_active_room_by_id(id);
        }
    }));
    first_room.connect_activate(clone!(op => move |_, _| {
        let mut op = op.lock().unwrap();
        if let Some(id) = op.roomlist.first_id() {
            op.set_active_room_by_id(id);
        }
    }));
    last_room.connect_activate(clone!(op => move |_, _| {
        let mut op = op.lock().unwrap();
        if let Some(id) = op.roomlist.last_id() {
            op.set_active_room_by_id(id);
        }
    }));
    older_messages.connect_activate(clone!(op => move |_, _| {
        if let Some(ref mut hist) = op.lock().unwrap().history {
            hist.page_up();
        }
    }));
    newer_messages.connect_activate(clone!(op => move |_, _| {
        if let Some(ref mut hist) = op.lock().unwrap().history {
            hist.page_down();
        }
    }));

    let back_history = op.lock().unwrap().room_back_history.clone();

    let back_weak = Rc::downgrade(&back_history);
    account.connect_activate(clone!(op => move |_, _| {
        op.lock().unwrap().show_account_settings_dialog();

        let back = upgrade_weak!(back_weak);
        back.borrow_mut().push(AppState::AccountSettings);
    }));

    let back_weak = Rc::downgrade(&back_history);
    directory.connect_activate(clone!(op => move |_, _| {
        op.lock().unwrap().set_state(AppState::Directory);

    let back = upgrade_weak!(back_weak);
    back.borrow_mut().push(AppState::Directory);
    }));

    /* TODO: We could pass a message to this to highlight it in the room history, might be
     * handy when opening the room from a notification */
    let back_weak = Rc::downgrade(&back_history);
    open_room.connect_activate(clone!(op => move |_, data| {
        if let Some(id) = get_room_id(data) {
            op.lock().unwrap().set_active_room_by_id(id.to_string());
           /* This does nothing if fractal is already in focus */
            op.lock().unwrap().activate();
        }
        let back = upgrade_weak!(back_weak);
        // Push a new state only if the current state is not already Room
        let push = if let Some(last) = back.borrow().last() {
            last != &AppState::Room
        } else {
            true
        };
        if push {
            back.borrow_mut().push(AppState::Room);
        }
    }));

    let back_weak = Rc::downgrade(&back_history);
    room_settings.connect_activate(clone!(op => move |_, _| {
        op.lock().unwrap().create_room_settings();

        let back = upgrade_weak!(back_weak);
        back.borrow_mut().push(AppState::RoomSettings);
    }));

    let back_weak = Rc::downgrade(&back_history);
    media_viewer.connect_activate(move |_, data| {
        open_viewer(data);

        let back = upgrade_weak!(back_weak);
        back.borrow_mut().push(AppState::MediaViewer);
    });

    let back_weak = Rc::downgrade(&back_history);
    back.connect_activate(move |_, _| {
        back_weak.upgrade().map(|back| {
            // Remove the current state form the store
            back.borrow_mut().pop();
            if let Some(state) = back.borrow().last() {
                debug!("Go back to state {:?}", state);
                if let Some(op) = App::get_op() {
                    let mut op = op.lock().unwrap();
                    op.set_state(state.clone());
                }
            } else {
                // Falback when there is no back history
                debug!("There is no state to go back to. Go back to state NoRoom");
                if let Some(op) = App::get_op() {
                    let mut op = op.lock().unwrap();
                    if op.login_data.is_some() {
                        op.set_state(AppState::NoRoom);
                    }
                }
            }
        });
    });

    let app_weak = app.downgrade();
    send_file.connect_activate(move |_, _| {
        let app = upgrade_weak!(app_weak);
        if let Some(window) = app.get_active_window() {
            if let Some(path) = open(&window, i18n("Select a file").as_str(), &[]) {
                APPOP!(attach_message, (path));
            }
        }
    });

    /* Add Keybindings to actions */
    app.set_accels_for_action("app.quit", &["<Ctrl>Q"]);
    app.set_accels_for_action("app.previous-room", &["<Ctrl>Page_Up"]);
    app.set_accels_for_action("app.next-room", &["<Ctrl>Page_Down"]);
    app.set_accels_for_action("app.prev-unread-room", &["<Ctrl><Shift>Page_Up"]);
    app.set_accels_for_action("app.next-unread-room", &["<Ctrl><Shift>Page_Down"]);
    app.set_accels_for_action("app.first-room", &["<Ctrl>Home"]);
    app.set_accels_for_action("app.last-room", &["<Ctrl>End"]);
    app.set_accels_for_action("app.older-messages", &["Page_Up"]);
    app.set_accels_for_action("app.newer-messages", &["Page_Down"]);
    app.set_accels_for_action("app.back", &["Escape"]);

    // connect mouse back button to app.back action
    let app_weak = app.downgrade();
    if let Some(window) = app.get_active_window() {
        window.connect_button_press_event(move |_, e| {
            if e.get_button() == 8 {
                if let Some(app) = app_weak.upgrade() {
                    app.lookup_action("back")
                        .expect("App did not have back action.")
                        .activate(None);
                    return Inhibit(true);
                }
            }

            Inhibit(false)
        });
    }

    // TODO: Mark active room as read when window gets focus
    //op.lock().unwrap().mark_active_room_messages();
}

fn get_room_id(data: Option<&glib::Variant>) -> Option<&str> {
    data?.get_str()
}

fn get_message(data: Option<&glib::Variant>) -> Option<Message> {
    get_message_by_id(data.as_ref()?.get_str()?)
}

/* TODO: get message from stroage once implemented */
fn get_message_by_id(id: &str) -> Option<Message> {
    let op = App::get_op()?;
    let op = op.lock().unwrap();
    let room_id = op.active_room.as_ref()?;
    op.get_message_by_id(room_id, id)
}

fn open_viewer(data: Option<&glib::Variant>) -> Option<()> {
    let msg = get_message(data)?;
    let op = App::get_op()?;
    let mut op = op.lock().unwrap();
    op.create_media_viewer(msg);
    None
}

pub fn activate_action(action_group_name: &str, action_name: &str) {
    if let Some(op) = App::get_op() {
        let main_window = op
            .lock()
            .unwrap()
            .ui
            .builder
            .get_object::<gtk::Window>("main_window")
            .expect("Can't find main_window in ui file.");
        if let Some(action_group) = main_window.get_action_group(action_group_name) {
            action_group.activate_action(action_name, None);
        }
    }
}
