use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use fractal_api::identifiers::{RoomId, UserId};
use fractal_api::r0::AccessToken;

use gtk;
use gtk::prelude::*;

use fractal_api::url::Url;

use crate::backend;
use crate::backend::BKCommand;

use crate::i18n;

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
pub mod room;
mod room_settings;
mod start_chat;
pub mod state;
mod sync;
mod user;

use self::member::SearchType;
use self::message::TmpMsg;

#[derive(Clone, Debug)]
pub struct LoginData {
    pub access_token: AccessToken,
    pub uid: UserId,
    pub username: Option<String>,
    pub avatar: Option<PathBuf>,
    pub server_url: Url,
    pub identity_url: Url,
}

pub struct AppOp {
    pub ui: uibuilder::UI,
    pub backend: Sender<backend::BKCommand>,

    pub syncing: bool, // TODO: Replace with a Mutex
    pub msg_queue: Vec<TmpMsg>,
    pub sending_message: bool,

    pub login_data: Option<LoginData>,
    pub device_id: Option<String>,

    pub active_room: Option<RoomId>,
    pub join_to_room: Option<RoomId>,
    pub rooms: RoomList,
    pub room_settings: Option<widgets::RoomSettings>,
    pub history: Option<widgets::RoomHistory>,
    pub roomlist: widgets::RoomList,
    unread_rooms: usize,
    pub unsent_messages: HashMap<RoomId, (String, i32)>,
    pub typing: HashMap<RoomId, std::time::Instant>,

    pub media_viewer: Rc<RefCell<Option<widgets::MediaViewer>>>,

    pub state: AppState,
    pub since: Option<String>,
    pub room_back_history: Rc<RefCell<Vec<AppState>>>,

    pub invitation_roomid: Option<RoomId>,
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
            join_to_room: None,
            rooms: HashMap::new(),
            room_settings: None,
            history: None,
            login_data: None,
            device_id: None,
            syncing: false,
            msg_queue: vec![],
            sending_message: false,
            state: AppState::Login,
            room_back_history: Rc::new(RefCell::new(vec![])),
            roomlist: widgets::RoomList::new(None, None),
            unread_rooms: 0,
            since: None,
            unsent_messages: HashMap::new(),
            typing: HashMap::new(),
            media_viewer: Rc::new(RefCell::new(None)),

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

        // FIXME: Username and uid should not be duplicated in cache.
        if let Ok(data) = cache::load() {
            let r: Vec<Room> = data.rooms.values().cloned().collect();
            self.set_rooms(r, true);
            /* Make sure that since is never an empty string */
            self.since = data.since.filter(|s| !s.is_empty());
            self.device_id = Some(data.device_id);
        }

        // FIXME: Storing and getting the password is insecure.
        //        Only the access token should be used.
        if let Ok((username, password, server, id_url)) = self.get_pass() {
            if let Ok((Some(access_token), uid)) = self.get_token() {
                self.bk_login(uid, access_token, self.device_id.clone(), server, id_url);
            } else {
                self.connect(username, password, server, id_url);
            }
        } else {
            self.set_state(AppState::Login);
        }
    }

    fn get_window(&self) -> gtk::Window {
        self.ui
            .builder
            .get_object("main_window")
            .expect("Couldn't find main_window in ui file.")
    }

    pub fn activate(&self) {
        let window = self.get_window();
        window.show();
        window.present();
    }

    pub fn update_title(&mut self) {
        let unread = self.roomlist.rooms_with_notifications();
        if self.unread_rooms != unread {
            let window = self.get_window();
            if unread == 0 {
                window.set_title(&i18n::i18n("Fractal"));
            } else {
                // Translators: The placeholder is for the number of unread messages in the
                // application
                window.set_title(&i18n::i18n_f("Fractal [{}]", &[&unread.to_string()]));
            }
            self.unread_rooms = unread;
        }
    }

    pub fn quit(&self) {
        self.cache_rooms();
        self.disconnect();
    }
}
