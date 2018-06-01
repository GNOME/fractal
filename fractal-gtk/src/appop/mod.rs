extern crate gtk;
extern crate gettextrs;

use std::sync::mpsc::Sender;
use std::collections::HashMap;

use gio::ApplicationExt;
use self::gtk::prelude::*;
use self::gettextrs::gettext;

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
mod notifications;
mod state;
mod room;
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
pub use self::message::MsgPos;
pub use self::message::LastViewed;
pub use self::room::RoomPanel;
use self::member::SearchType;


pub struct AppOp {
    pub ui: uibuilder::UI,
    pub gtk_app: gtk::Application,
    pub backend: Sender<backend::BKCommand>,
    pub internal: Sender<InternalCommand>,

    pub syncing: bool,
    tmp_msgs: Vec<TmpMsg>,
    shown_messages: usize,
    pub last_viewed_messages: HashMap<String, Message>,

    pub username: Option<String>,
    pub uid: Option<String>,
    pub avatar: Option<String>,
    pub server_url: String,
    pub identity_url: String,

    pub autoscroll: bool,
    pub active_room: Option<String>,
    pub rooms: RoomList,
    pub roomlist: widgets::RoomList,
    pub load_more_spn: gtk::Spinner,
    pub more_members_btn: gtk::Button,
    pub unsent_messages: HashMap<String, (String, i32)>,

    pub highlighted_entry: Vec<String>,
    pub popover_position: Option<i32>,
    pub popover_search: Option<String>,
    pub popover_closing: bool,

    pub tmp_avatar: Option<String>,
    pub tmp_sid: Option<String>,

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
            more_members_btn: gtk::Button::new_with_label(gettext("Load more members").as_str()),
            backend: tx,
            internal: itx,
            autoscroll: true,
            active_room: None,
            rooms: HashMap::new(),
            username: None,
            uid: None,
            avatar: None,
            server_url: String::from(globals::DEFAULT_HOMESERVER),
            identity_url: String::from(globals::DEFAULT_IDENTITYSERVER),
            syncing: false,
            tmp_msgs: vec![],
            shown_messages: 0,
            last_viewed_messages: HashMap::new(),
            state: AppState::Login,
            roomlist: widgets::RoomList::new(None),
            since: None,
            member_limit: 50,
            unsent_messages: HashMap::new(),

            tmp_avatar: None,
            tmp_sid: None,

            highlighted_entry: vec![],
            popover_position: None,
            popover_search: None,
            popover_closing: false,

            logged_in: false,
            loading_more: false,

            md_enabled: false,
            invitation_roomid: None,
            invite_list: vec![],
            search_type: SearchType::Invite,
            stickers: vec![],

            directory: vec![],
        }
    }

    pub fn init(&mut self) {
        self.set_state(AppState::Loading);

        if let Ok(data) = cache::load() {
            let r: Vec<Room> = data.rooms.values().cloned().collect();
            self.set_rooms(&r, None);
            self.last_viewed_messages = data.last_viewed_messages;
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
