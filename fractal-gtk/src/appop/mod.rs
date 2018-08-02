extern crate gtk;

use std::sync::mpsc::Sender;
use std::collections::HashMap;

use gio::ApplicationExt;
use self::gtk::prelude::*;

use globals;
use backend::BKCommand;
use backend;

use types::Member;
use types::Message;
use types::Room;
use types::RoomList;
use types::StickerGroup;

use passwd::PasswordStorage;

use widgets;
use cache;
use uibuilder;

use app::InternalCommand;

mod login;
mod sync;
mod user;
mod account;
mod room_settings;
mod notifications;
mod state;
mod room;
mod media_viewer;
mod files;
mod message;
mod directory;
mod notify;
mod attach;
mod member;
mod invite;
mod about;
mod start_chat;
mod stickers;

pub use self::state::AppState;
use self::message::TmpMsg;
pub use self::room::RoomPanel;
use self::member::SearchType;
use self::media_viewer::MediaViewer;


pub struct AppOp {
    pub ui: uibuilder::UI,
    pub gtk_app: gtk::Application,
    pub backend: Sender<backend::BKCommand>,
    pub internal: Sender<InternalCommand>,

    pub syncing: bool,
    pub msg_queue: Vec<TmpMsg>,
    pub sending_message: bool,
    pub last_viewed_messages: HashMap<String, String>,
    pub first_new_messages: HashMap<String, Option<Message>>,

    pub username: Option<String>,
    pub uid: Option<String>,
    pub avatar: Option<String>,
    pub server_url: String,
    pub identity_url: String,

    pub autoscroll: bool,
    pub active_room: Option<String>,
    pub rooms: RoomList,
    pub room_settings: Option<widgets::RoomSettings>,
    pub room_history: Option<widgets::RoomHistory>,
    pub roomlist: widgets::RoomList,
    pub load_more_spn: gtk::Spinner,
    pub unsent_messages: HashMap<String, (String, i32)>,

    pub inhibit_escape: bool,

    pub state: AppState,
    pub since: Option<String>,
    pub member_limit: usize,

    pub logged_in: bool,
    pub loading_more: bool,

    pub invitation_roomid: Option<String>,
    pub md_enabled: bool,
    invite_list: Vec<Member>,
    search_type: SearchType,

    pub stickers: Vec<StickerGroup>,

    pub directory: Vec<Room>,

    pub media_viewer: Option<MediaViewer>,
}

impl PasswordStorage for AppOp {}


impl AppOp {
    pub fn new(app: gtk::Application,
               ui: uibuilder::UI,
               tx: Sender<BKCommand>,
               itx: Sender<InternalCommand>) -> AppOp {
        AppOp {
            ui: ui,
            gtk_app: app,
            load_more_spn: gtk::Spinner::new(),
            backend: tx,
            internal: itx,
            autoscroll: true,
            active_room: None,
            rooms: HashMap::new(),
            room_settings: None,
            room_history: None,
            username: None,
            uid: None,
            avatar: None,
            server_url: String::from(globals::DEFAULT_HOMESERVER),
            identity_url: String::from(globals::DEFAULT_IDENTITYSERVER),
            syncing: false,
            msg_queue: vec![],
            sending_message: false,
            last_viewed_messages: HashMap::new(),
            first_new_messages: HashMap::new(),
            state: AppState::Login,
            roomlist: widgets::RoomList::new(None),
            since: None,
            member_limit: 50,
            unsent_messages: HashMap::new(),

            inhibit_escape: false,

            logged_in: false,
            loading_more: false,

            md_enabled: false,
            invitation_roomid: None,
            invite_list: vec![],
            search_type: SearchType::Invite,
            stickers: vec![],

            directory: vec![],

            media_viewer: None,
        }
    }

    pub fn init(&mut self) {
        self.set_state(AppState::Loading);

        if let Ok(data) = cache::load() {
            let r: Vec<Room> = data.rooms.values().cloned().collect();
            self.set_rooms(&r, None);
            self.since = Some(data.since);
            self.username = Some(data.username);
            self.uid = Some(data.uid);
        }

        if let Ok(pass) = self.get_pass() {
            if let Ok((token, uid)) = self.get_token() {
                self.set_token(Some(token), Some(uid), Some(pass.2));
            } else {
                self.set_login_pass(&pass.0, &pass.1, &pass.2, &pass.3);
                self.connect(Some(pass.0), Some(pass.1), Some(pass.2), Some(pass.3));
            }
        } else {
            self.set_state(AppState::Login);
        }
    }

    pub fn activate(&self) {
        let window: gtk::Window = self.ui.builder
            .get_object("main_window")
            .expect("Couldn't find main_window in ui file.");
        window.show();
        window.present();
    }

    pub fn quit(&self) {
        self.cache_rooms();
        self.disconnect();
        self.gtk_app.quit();
    }
}
