use std::collections::HashMap;
use std::sync::mpsc::Sender;

use gtk;
use gtk::prelude::*;

use url::Url;

use crate::backend;
use crate::backend::BKCommand;
use crate::globals;

use crate::types::Member;
use crate::types::Room;
use crate::types::RoomList;

use crate::passwd::PasswordStorage;

use crate::actions::AppState;
use crate::cache;
use crate::uibuilder;
use crate::widgets;

mod about;
mod account;
pub mod attach;
mod directory;
mod invite;
mod login;
mod media_viewer;
mod member;
mod message;
mod notifications;
mod notify;
mod room;
mod room_settings;
mod start_chat;
pub mod state;
mod sync;
mod user;

use self::member::SearchType;
use self::message::TmpMsg;

pub struct AppOp {
    pub ui: uibuilder::UI,
    pub backend: Sender<backend::BKCommand>,

    pub syncing: bool,
    pub msg_queue: Vec<TmpMsg>,
    pub sending_message: bool,

    pub username: Option<String>,
    pub uid: Option<String>,
    pub device_id: Option<String>,
    pub avatar: Option<String>,
    pub server_url: Url,
    pub identity_url: Url,

    pub active_room: Option<String>,
    pub rooms: RoomList,
    pub room_settings: Option<widgets::RoomSettings>,
    pub history: Option<widgets::RoomHistory>,
    pub roomlist: widgets::RoomList,
    pub unsent_messages: HashMap<String, (String, i32)>,

    pub state: AppState,
    pub since: Option<String>,

    pub logged_in: bool,

    pub invitation_roomid: Option<String>,
    pub md_enabled: bool,
    pub invite_list: Vec<(Member, gtk::TextChildAnchor)>,
    search_type: SearchType,

    pub directory: Vec<Room>,
    pub leaflet: libhandy::Leaflet,
}

impl PasswordStorage for AppOp {}

impl AppOp {
    pub fn new(ui: uibuilder::UI, tx: Sender<BKCommand>) -> AppOp {
        let leaflet = ui
            .builder
            .get_object::<libhandy::Leaflet>("header_leaflet")
            .expect("Couldn't find header_leaflet in ui file");

        AppOp {
            ui: ui,
            backend: tx,
            active_room: None,
            rooms: HashMap::new(),
            room_settings: None,
            history: None,
            username: None,
            uid: None,
            device_id: None,
            avatar: None,
            server_url: globals::DEFAULT_HOMESERVER.clone(),
            identity_url: globals::DEFAULT_IDENTITYSERVER.clone(),
            syncing: false,
            msg_queue: vec![],
            sending_message: false,
            state: AppState::Login,
            roomlist: widgets::RoomList::new(None, None),
            since: None,
            unsent_messages: HashMap::new(),

            logged_in: false,

            md_enabled: false,
            invitation_roomid: None,
            invite_list: vec![],
            search_type: SearchType::Invite,

            directory: vec![],
            leaflet: leaflet,
        }
    }

    pub fn init(&mut self) {
        self.set_state(AppState::Loading);

        if let Ok(data) = cache::load() {
            let r: Vec<Room> = data.rooms.values().cloned().collect();
            self.set_rooms(r, true);
            /* Make sure that since is never an empty string */
            self.since = data.since.filter(|s| !s.is_empty());
            self.username = Some(data.username);
            self.uid = Some(data.uid);
            self.device_id = Some(data.device_id);
        }

        if let Ok(pass) = self.get_pass() {
            if let Ok((token, uid)) = self.get_token() {
                self.set_token(Some(token), Some(uid), pass.2);
            } else {
                self.connect(Some(pass.0), Some(pass.1), pass.2, pass.3);
            }
        } else {
            self.set_state(AppState::Login);
        }
    }

    pub fn activate(&self) {
        let window: gtk::Window = self
            .ui
            .builder
            .get_object("main_window")
            .expect("Couldn't find main_window in ui file.");
        window.show();
        window.present();
    }

    pub fn quit(&self) {
        self.cache_rooms();
        self.disconnect();
    }
}
