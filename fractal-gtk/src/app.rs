extern crate gtk;
extern crate gdk_pixbuf;
extern crate gdk_pixbuf_sys;
extern crate secret_service;
extern crate chrono;
extern crate gdk;
extern crate notify_rust;
extern crate rand;

use self::notify_rust::Notification;

use util::get_pixbuf_data;
use util::markup;

use self::chrono::prelude::*;

use self::secret_service::SecretService;
use self::secret_service::EncryptionType;

use self::rand::{thread_rng, Rng};

use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::RecvError;
use std::collections::HashMap;
use std::process::Command;
use std::thread;

use gio::ApplicationExt;
use gio::SimpleActionExt;
use gio::ActionMapExt;
use glib;
use gio;
use self::gdk_pixbuf::Pixbuf;
use self::gio::prelude::*;
use self::gtk::prelude::*;

use globals;

use backend::Backend;
use backend::BKCommand;
use backend::BKResponse;
use backend;

use types::Member;
use types::Message;
use types::Protocol;
use types::Room;
use types::RoomList;
use types::Event;

use widgets;
use widgets::AvatarExt;
use cache;


#[derive(Debug)]
pub enum Error {
    SecretServiceError,
}

derror!(secret_service::SsError, Error::SecretServiceError);


const APP_ID: &'static str = "org.gnome.Fractal";


struct TmpMsg {
    pub msg: Message,
    pub widget: gtk::Widget,
}


pub struct AppOp {
    pub gtk_builder: gtk::Builder,
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

    pub autoscroll: bool,
    pub active_room: Option<String>,
    pub rooms: RoomList,
    pub roomlist: widgets::RoomList,
    pub load_more_btn: gtk::Button,
    pub more_members_btn: gtk::Button,
    pub unsent_messages: HashMap<String, (String, i32)>,

    pub state: AppState,
    pub since: Option<String>,
    pub member_limit: usize,

    pub logged_in: bool,
    pub loading_more: bool,

    pub invitation_roomid: Option<String>,
    invite_list: Vec<Member>,
    search_type: SearchType,
}

#[derive(Debug, Clone)]
pub enum SearchType {
    Invite,
    DirectChat,
}

#[derive(Debug, Clone)]
pub enum MsgPos {
    Top,
    Bottom,
}

#[derive(Debug, Clone)]
pub enum RoomPanel {
    Room,
    NoRoom,
    Loading,
}


#[derive(Debug, Clone)]
pub enum AppState {
    Login,
    Chat,
    Directory,
    Loading,
}

static mut OP: Option<Arc<Mutex<AppOp>>> = None;

macro_rules! APPOP {
    ($fn: ident, ($($x:ident),*) ) => {{
        if let Some(ctx) = glib::MainContext::default() {
            ctx.invoke(move || {
                $( let $x = $x.clone(); )*
                if let Some(op) = AppOp::def() {
                    op.lock().unwrap().$fn($($x),*);
                }
            });
        }
    }};
    ($fn: ident) => {{
        APPOP!($fn, ( ) );
    }}
}

impl AppOp {
    pub fn def() -> Option<Arc<Mutex<AppOp>>> {
        unsafe {
            match OP {
                Some(ref m) => Some(m.clone()),
                None => None,
            }
        }
    }

    pub fn new(app: gtk::Application,
               builder: gtk::Builder,
               tx: Sender<BKCommand>,
               itx: Sender<InternalCommand>) -> AppOp {
        AppOp {
            gtk_builder: builder,
            gtk_app: app,
            load_more_btn: gtk::Button::new_with_label("Load more messages"),
            more_members_btn: gtk::Button::new_with_label("Load more members"),
            backend: tx,
            internal: itx,
            autoscroll: true,
            active_room: None,
            rooms: HashMap::new(),
            username: None,
            uid: None,
            avatar: None,
            server_url: String::from("https://matrix.org"),
            syncing: false,
            tmp_msgs: vec![],
            shown_messages: 0,
            last_viewed_messages: HashMap::new(),
            state: AppState::Login,
            roomlist: widgets::RoomList::new(None),
            since: None,
            member_limit: 50,
            unsent_messages: HashMap::new(),

            logged_in: false,
            loading_more: false,

            invitation_roomid: None,
            invite_list: vec![],
            search_type: SearchType::Invite,
        }
    }

    pub fn initial_sync(&self, show: bool) {
        if show {
            self.inapp_notify("Initial sync, this can take some time");
        } else {
            self.hide_inapp_notify();
        }
    }

    pub fn bk_login(&mut self, uid: String, token: String) {
        self.logged_in = true;
        self.clean_login();
        if let Err(_) = self.store_token(uid.clone(), token) {
            println!("Error: Can't store the token using libsecret");
        }

        self.set_state(AppState::Chat);
        self.set_uid(Some(uid.clone()));
        /* Do we need to set the username to uid
        self.set_username(Some(uid));*/
        self.get_username();

        // initial sync, we're shoing some feedback to the user
        self.initial_sync(true);

        self.sync();

        self.init_protocols();
    }

    pub fn bk_logout(&mut self) {
        self.set_rooms(&vec![], None);
        if let Err(_) = cache::destroy() {
            println!("Error removing cache file");
        }

        self.logged_in = false;
        self.syncing = false;

        self.set_state(AppState::Login);
        self.set_uid(None);
        self.set_username(None);
        self.set_avatar(None);

        // stoping the backend and starting again, we don't want to receive more messages from
        // backend
        self.backend.send(BKCommand::ShutDown).unwrap();

        let (tx, rx): (Sender<BKResponse>, Receiver<BKResponse>) = channel();
        let bk = Backend::new(tx);
        self.backend = bk.run();
        backend_loop(rx);
    }

    pub fn update_rooms(&mut self, rooms: Vec<Room>, default: Option<Room>) {
        let rs: Vec<Room> = rooms.iter().filter(|x| !x.left).cloned().collect();
        self.set_rooms(&rs, default);

        // uploading each room avatar
        for r in rooms.iter() {
            self.backend.send(BKCommand::GetRoomAvatar(r.id.clone())).unwrap();
        }
    }

    pub fn new_rooms(&mut self, rooms: Vec<Room>) {
        // ignoring existing rooms
        let rs: Vec<&Room> = rooms.iter().filter(|x| !self.rooms.contains_key(&x.id) && !x.left).collect();

        for r in rs {
            self.rooms.insert(r.id.clone(), r.clone());
            self.roomlist.add_room(r.clone());
            self.roomlist.moveup(r.id.clone());
        }

        // removing left rooms
        let rs: Vec<&Room> = rooms.iter().filter(|x| x.left).collect();
        for r in rs {
            if r.id == self.active_room.clone().unwrap_or_default() {
                self.really_leave_active_room();
            } else {
                self.remove_room(r.id.clone());
            }
        }
    }

    pub fn remove_room(&mut self, id: String) {
        self.rooms.remove(&id);
        self.roomlist.remove_room(id.clone());
        self.unsent_messages.remove(&id);
    }

    pub fn clear_room_notifications(&mut self, r: String) {
        self.set_room_notifications(r.clone(), 0, 0);
        self.roomlist.set_bold(r, false);
    }

    pub fn set_state(&mut self, state: AppState) {
        self.state = state;

        let widget_name = match self.state {
            AppState::Login => "login",
            AppState::Chat => "chat",
            AppState::Directory => "directory",
            AppState::Loading => "loading",
        };

        self.gtk_builder
            .get_object::<gtk::Stack>("main_content_stack")
            .expect("Can't find main_content_stack in ui file.")
            .set_visible_child_name(widget_name);

        //setting headerbar
        let bar_name = match self.state {
            AppState::Login => "login",
            AppState::Directory => "back",
            AppState::Loading => "login",
            _ => "normal",
        };

        self.gtk_builder
            .get_object::<gtk::Stack>("headerbar_stack")
            .expect("Can't find headerbar_stack in ui file.")
            .set_visible_child_name(bar_name);

        //set focus for views
        let widget_focus = match self.state {
            AppState::Login => "login_username",
            AppState::Directory => "directory_search_entry",
            _ => "",
        };

        if widget_focus != "" {
            self.gtk_builder
                .get_object::<gtk::Widget>(widget_focus)
                .expect("Can't find widget to set focus in ui file.")
                .grab_focus();
        }
    }

    pub fn escape(&mut self) {
        if let AppState::Chat = self.state {
            self.room_panel(RoomPanel::NoRoom);
            self.active_room = None;
            self.clear_tmp_msgs();
        }
    }

    pub fn clean_login(&self) {
        let user_entry: gtk::Entry = self.gtk_builder
            .get_object("login_username")
            .expect("Can't find login_username in ui file.");
        let pass_entry: gtk::Entry = self.gtk_builder
            .get_object("login_password")
            .expect("Can't find login_password in ui file.");
        let server_entry: gtk::Entry = self.gtk_builder
            .get_object("login_server")
            .expect("Can't find login_server in ui file.");
        let idp_entry: gtk::Entry = self.gtk_builder
            .get_object("login_idp")
            .expect("Can't find login_idp in ui file.");

        user_entry.set_text("");
        pass_entry.set_text("");
        server_entry.set_text("https://matrix.org");
        idp_entry.set_text("https://vector.im");
    }

    pub fn login(&mut self) {
        let user_entry: gtk::Entry = self.gtk_builder
            .get_object("login_username")
            .expect("Can't find login_username in ui file.");
        let pass_entry: gtk::Entry = self.gtk_builder
            .get_object("login_password")
            .expect("Can't find login_password in ui file.");
        let server_entry: gtk::Entry = self.gtk_builder
            .get_object("login_server")
            .expect("Can't find login_server in ui file.");
        let login_error: gtk::Label = self.gtk_builder
            .get_object("login_error_msg")
            .expect("Can't find login_error_msg in ui file.");

        let username = user_entry.get_text();
        let password = pass_entry.get_text();

        if username.clone().unwrap_or_default().is_empty() ||
           password.clone().unwrap_or_default().is_empty() {
            login_error.set_text("Invalid username or password");
            login_error.show();
            return;
        } else {
            login_error.set_text("Unknown Error");
            login_error.hide();
        }

        self.set_state(AppState::Loading);
        self.since = None;
        self.connect(username, password, server_entry.get_text());
    }

    pub fn set_login_pass(&self, username: &str, password: &str, server: &str) {
        let user_entry: gtk::Entry = self.gtk_builder
            .get_object("login_username")
            .expect("Can't find login_username in ui file.");
        let pass_entry: gtk::Entry = self.gtk_builder
            .get_object("login_password")
            .expect("Can't find login_password in ui file.");
        let server_entry: gtk::Entry = self.gtk_builder
            .get_object("login_server")
            .expect("Can't find login_server in ui file.");

        user_entry.set_text(username);
        pass_entry.set_text(password);
        server_entry.set_text(server);
    }

    #[allow(dead_code)]
    pub fn register(&mut self) {
        let user_entry: gtk::Entry = self.gtk_builder
            .get_object("register_username")
            .expect("Can't find register_username in ui file.");
        let pass_entry: gtk::Entry = self.gtk_builder
            .get_object("register_password")
            .expect("Can't find register_password in ui file.");
        let pass_conf: gtk::Entry = self.gtk_builder
            .get_object("register_password_confirm")
            .expect("Can't find register_password_confirm in ui file.");
        let server_entry: gtk::Entry = self.gtk_builder
            .get_object("register_server")
            .expect("Can't find register_server in ui file.");

        let username = match user_entry.get_text() {
            Some(s) => s,
            None => String::from(""),
        };
        let password = match pass_entry.get_text() {
            Some(s) => s,
            None => String::from(""),
        };
        let passconf = match pass_conf.get_text() {
            Some(s) => s,
            None => String::from(""),
        };

        if password != passconf {
            self.show_error("Passwords didn't match, try again".to_string());
            return;
        }

        self.server_url = match server_entry.get_text() {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        //self.store_pass(username.clone(), password.clone(), server_url.clone())
        //    .unwrap_or_else(|_| {
        //        // TODO: show an error
        //        println!("Error: Can't store the password using libsecret");
        //    });

        let uname = username.clone();
        let pass = password.clone();
        let ser = self.server_url.clone();
        self.backend.send(BKCommand::Register(uname, pass, ser)).unwrap();
    }

    pub fn connect(&mut self, username: Option<String>, password: Option<String>, server: Option<String>) -> Option<()> {
        self.server_url = match server {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        self.store_pass(username.clone()?, password.clone()?, self.server_url.clone())
            .unwrap_or_else(|_| {
                // TODO: show an error
                println!("Error: Can't store the password using libsecret");
            });

        let uname = username?;
        let pass = password?;
        let ser = self.server_url.clone();
        self.backend.send(BKCommand::Login(uname, pass, ser)).unwrap();
        Some(())
    }

    pub fn set_token(&mut self, token: Option<String>, uid: Option<String>, server: Option<String>) -> Option<()> {
        self.server_url = match server {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        let ser = self.server_url.clone();
        self.backend.send(BKCommand::SetToken(token?, uid?, ser)).unwrap();
        Some(())
    }

    #[allow(dead_code)]
    pub fn connect_guest(&mut self, server: Option<String>) {
        self.server_url = match server {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        self.backend.send(BKCommand::Guest(self.server_url.clone())).unwrap();
    }

    pub fn get_username(&self) {
        self.backend.send(BKCommand::GetUsername).unwrap();
        self.backend.send(BKCommand::GetAvatar).unwrap();
    }

    pub fn show_user_info (&self) {
        let stack = self.gtk_builder
            .get_object::<gtk::Stack>("user_info")
            .expect("Can't find user_info_avatar in ui file.");

        /* Show user infos inside the popover but wait for all data to arrive */
        if self.avatar.is_some() && self.username.is_some() && self.uid.is_some() {
            let avatar = self.gtk_builder
                .get_object::<gtk::Container>("user_info_avatar")
                .expect("Can't find user_info_avatar in ui file.");

            let name = self.gtk_builder
                .get_object::<gtk::Label>("user_info_username")
                .expect("Can't find user_info_avatar in ui file.");

            let uid = self.gtk_builder
                .get_object::<gtk::Label>("user_info_uid")
                .expect("Can't find user_info_avatar in ui file.");

            uid.set_text(&self.uid.clone().unwrap_or_default());
            name.set_text(&self.username.clone().unwrap_or_default());

            /* remove all old avatar from the popover */
            for w in avatar.get_children().iter() {
                avatar.remove(w);
            }

            let w = widgets::Avatar::circle_avatar(self.avatar.clone().unwrap_or_default(), Some(40));
            avatar.add(&w);
            stack.set_visible_child_name("info");
        }
        else {
            stack.set_visible_child_name("spinner");
        }

        /* update user menu button avatar */
        let button = self.gtk_builder
            .get_object::<gtk::MenuButton>("user_menu_button")
            .expect("Can't find user_menu_button in ui file.");

        let eb = gtk::EventBox::new();
            match self.avatar.clone() {
                Some(s) => {
                    let w = widgets::Avatar::circle_avatar(s.clone(), Some(24));
                    eb.add(&w);
                }
            None => {
                let w = gtk::Spinner::new();
                w.show();
                w.start();
                eb.add(&w);
            }
        };

        eb.connect_button_press_event(move |_, _| { Inhibit(false) });
        button.set_image(&eb);
    }

    pub fn set_username(&mut self, username: Option<String>) {
        self.username = username;
        self.show_user_info();
    }

    pub fn set_uid(&mut self, uid: Option<String>) {
        self.uid = uid;
        self.show_user_info();
    }

    pub fn set_avatar(&mut self, fname: Option<String>) {
        self.avatar = fname;
        self.show_user_info();
    }

    pub fn disconnect(&self) {
        self.backend.send(BKCommand::ShutDown).unwrap();
    }

    pub fn logout(&mut self) {
        let _ = self.delete_pass("fractal");
        self.backend.send(BKCommand::Logout).unwrap();
        self.bk_logout();
    }

    pub fn delete_pass(&self, key: &str) -> Result<(), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;

        // deleting previous items
        let allpass = collection.get_all_items()?;
        let passwds = allpass.iter()
            .filter(|x| x.get_label().unwrap_or_default() == key);
        for p in passwds {
            p.delete()?;
        }

        Ok(())
    }

    pub fn store_token(&self, uid: String, token: String) -> Result<(), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        let key = "fractal-token";

        // deleting previous items
        self.delete_pass(key)?;

        // create new item
        collection.create_item(
            key, // label
            vec![
                ("uid", &uid),
            ], // properties
            token.as_bytes(), //secret
            true, // replace item with same attributes
            "text/plain" // secret content type
        )?;

        Ok(())
    }

    pub fn get_token(&self) -> Result<(String, String), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        let allpass = collection.get_all_items()?;
        let key = "fractal-token";

        let passwd = allpass.iter()
            .find(|x| x.get_label().unwrap_or_default() == key);

        if passwd.is_none() {
            return Err(Error::SecretServiceError);
        }

        let p = passwd.unwrap();
        let attrs = p.get_attributes()?;
        let secret = p.get_secret()?;
        let token = String::from_utf8(secret).unwrap();

        let attr = attrs.iter()
            .find(|&ref x| x.0 == "uid")
            .ok_or(Error::SecretServiceError)?;
        let uid = attr.1.clone();

        Ok((token, uid))
    }

    pub fn store_pass(&self,
                      username: String,
                      password: String,
                      server: String)
                      -> Result<(), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        let key = "fractal";

        // deleting previous items
        self.delete_pass(key)?;

        // create new item
        collection.create_item(
            key, // label
            vec![
                ("username", &username),
                ("server", &server),
            ], // properties
            password.as_bytes(), //secret
            true, // replace item with same attributes
            "text/plain" // secret content type
        )?;

        Ok(())
    }

    pub fn migrate_old_passwd(&self) -> Result<(), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        let allpass = collection.get_all_items()?;

        // old name password
        let passwd = allpass.iter()
            .find(|x| x.get_label().unwrap_or(strn!("")) == "guillotine");

        if passwd.is_none() {
            return Ok(());
        }

        let p = passwd.unwrap();
        let attrs = p.get_attributes()?;
        let secret = p.get_secret()?;

        let mut attr = attrs.iter()
            .find(|&ref x| x.0 == "username")
            .ok_or(Error::SecretServiceError)?;
        let username = attr.1.clone();
        attr = attrs.iter()
            .find(|&ref x| x.0 == "server")
            .ok_or(Error::SecretServiceError)?;
        let server = attr.1.clone();
        let pwd = String::from_utf8(secret).unwrap();

        // removing old
        for p in passwd {
            p.delete()?;
        }

        self.store_pass(username, pwd, server)?;

        Ok(())
    }

    pub fn get_pass(&self) -> Result<(String, String, String), Error> {
        self.migrate_old_passwd()?;

        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;
        let allpass = collection.get_all_items()?;
        let key = "fractal";

        let passwd = allpass.iter()
            .find(|x| x.get_label().unwrap_or_default() == key);

        if passwd.is_none() {
            return Err(Error::SecretServiceError);
        }

        let p = passwd.unwrap();
        let attrs = p.get_attributes()?;
        let secret = p.get_secret()?;

        let mut attr = attrs.iter()
            .find(|&ref x| x.0 == "username")
            .ok_or(Error::SecretServiceError)?;
        let username = attr.1.clone();
        attr = attrs.iter()
            .find(|&ref x| x.0 == "server")
            .ok_or(Error::SecretServiceError)?;
        let server = attr.1.clone();

        let tup = (username, String::from_utf8(secret).unwrap(), server);

        Ok(tup)
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
                self.set_login_pass(&pass.0, &pass.1, &pass.2);
                self.connect(Some(pass.0), Some(pass.1), Some(pass.2));
            }
        } else {
            self.set_state(AppState::Login);
        }
    }

    pub fn room_panel(&self, t: RoomPanel) {
        let s = self.gtk_builder
            .get_object::<gtk::Stack>("room_view_stack")
            .expect("Can't find room_view_stack in ui file.");
        let headerbar = self.gtk_builder
            .get_object::<gtk::HeaderBar>("room_header_bar")
            .expect("Can't find room_header_bar in ui file.");

        let v = match t {
            RoomPanel::Loading => "loading",
            RoomPanel::Room => "room_view",
            RoomPanel::NoRoom => "noroom",
        };

        s.set_visible_child_name(v);

        match v {
            "noroom" => {
                for ch in headerbar.get_children().iter() {
                    ch.hide();
                }
                self.roomlist.set_selected(None);
            },
            "room_view" => {
                for ch in headerbar.get_children().iter() {
                    ch.show();
                }

                let msg_entry: gtk::Entry = self.gtk_builder
                    .get_object("msg_entry")
                    .expect("Couldn't find msg_entry in ui file.");
                msg_entry.grab_focus();

                let active_room_id = self.active_room.clone().unwrap_or_default();
                let msg = self.unsent_messages
                    .get(&active_room_id).cloned()
                    .unwrap_or((String::new(), 0));
                msg_entry.set_text(&msg.0);
                msg_entry.set_position(msg.1);
            },
            _ => {
                for ch in headerbar.get_children().iter() {
                    ch.show();
                }
            }
        }
    }

    pub fn sync(&mut self) {
        if !self.syncing && self.logged_in {
            self.syncing = true;
            self.backend.send(BKCommand::Sync).unwrap();
        }
    }

    pub fn synced(&mut self, since: Option<String>) {
        self.syncing = false;
        self.since = since;
        self.sync();
        self.initial_sync(false);
    }

    pub fn sync_error(&mut self) {
        self.syncing = false;
        self.sync();
    }

    pub fn set_rooms(&mut self, rooms: &Vec<Room>, def: Option<Room>) {
        let container: gtk::Box = self.gtk_builder
            .get_object("room_container")
            .expect("Couldn't find room_container in ui file.");

        let selected_room = self.roomlist.get_selected();

        self.rooms.clear();
        for ch in container.get_children().iter() {
            container.remove(ch);
        }

        for r in rooms.iter() {
            self.rooms.insert(r.id.clone(), r.clone());
        }

        self.roomlist = widgets::RoomList::new(Some(self.server_url.clone()));
        self.roomlist.add_rooms(rooms.iter().cloned().collect());
        container.add(&self.roomlist.widget());
        self.roomlist.set_selected(selected_room);

        let bk = self.internal.clone();
        self.roomlist.connect(move |room| {
            bk.send(InternalCommand::SelectRoom(room)).unwrap();
        });
        let bk = self.backend.clone();
        self.roomlist.connect_fav(move |room, tofav| {
            bk.send(BKCommand::AddToFav(room.id.clone(), tofav)).unwrap();
        });

        let mut godef = def;
        if let Some(aroom) = self.active_room.clone() {
            if let Some(r) = self.rooms.get(&aroom) {
                godef = Some(r.clone());
            }
        }

        if let Some(d) = godef {
            self.set_active_room_by_id(d.id.clone());
        } else {
            self.set_state(AppState::Chat);
            self.room_panel(RoomPanel::NoRoom);
            self.active_room = None;
            self.clear_tmp_msgs();
        }

        self.cache_rooms();
    }

    pub fn cache_rooms(&self) {
        // serializing rooms
        if let Err(_) = cache::store(&self.rooms, self.last_viewed_messages.clone(), self.since.clone().unwrap_or_default(), self.username.clone().unwrap_or_default(), self.uid.clone().unwrap_or_default()) {
            println!("Error caching rooms");
        };
    }

    pub fn reload_rooms(&mut self) {
        self.set_state(AppState::Chat);
    }

    pub fn remove_messages(&mut self) {
        let messages = self.gtk_builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");
        for ch in messages.get_children().iter().skip(1) {
            messages.remove(ch);
        }
    }

    pub fn set_active_room_by_id(&mut self, roomid: String) {
        let mut room = None;
        if let Some(r) = self.rooms.get(&roomid) {
            room = Some(r.clone());
        }

        if let Some(r) = room {
            if r.inv {
                self.show_inv_dialog(&r);
                return;
            }

            self.set_active_room(&r);
        }
    }

    pub fn show_inv_dialog(&mut self, r: &Room) {
        let dialog = self.gtk_builder
            .get_object::<gtk::MessageDialog>("invite_dialog")
            .expect("Can't find invite_dialog in ui file.");

        let room_name = r.name.clone().unwrap_or_default();
        let title = format!("Join {}?", room_name);
        let secondary;
        if let Some(ref sender) = r.inv_sender {
            let sender_name = sender.get_alias().unwrap_or(sender.uid.clone());
            secondary = format!("You've been invited to join to <b>{}</b> room by <b>{}</b>",
                                     room_name, sender_name);
        } else {
            secondary = format!("You've been invited to join to <b>{}</b>", room_name);
        }

        dialog.set_property_text(Some(&title));
        dialog.set_property_secondary_use_markup(true);
        dialog.set_property_secondary_text(Some(&secondary));

        self.invitation_roomid = Some(r.id.clone());
        dialog.present();
    }

    pub fn accept_inv(&mut self, accept: bool) {
        if let Some(ref rid) = self.invitation_roomid {
            match accept {
                true => self.backend.send(BKCommand::AcceptInv(rid.clone())).unwrap(),
                false => self.backend.send(BKCommand::RejectInv(rid.clone())).unwrap(),
            }
            self.internal.send(InternalCommand::RemoveInv(rid.clone())).unwrap();
        }
        self.invitation_roomid = None;
    }

    pub fn remove_inv(&mut self, roomid: String) {
        self.rooms.remove(&roomid);
        self.roomlist.remove_room(roomid);
    }

    pub fn search_invite_user(&self, term: Option<String>) {
        if let Some(t) = term {
            self.backend.send(BKCommand::UserSearch(t)).unwrap();
        }
    }

    pub fn user_search_finished(&self, users: Vec<Member>) {
        match self.search_type {
            SearchType::Invite => {
                let listbox = self.gtk_builder
                    .get_object::<gtk::ListBox>("user_search_box")
                    .expect("Can't find user_search_box in ui file.");
                let scroll = self.gtk_builder
                    .get_object::<gtk::Widget>("user_search_scroll")
                    .expect("Can't find user_search_scroll in ui file.");
                self.search_finished(users, listbox, scroll);
            },
            SearchType::DirectChat => {
                let listbox = self.gtk_builder
                    .get_object::<gtk::ListBox>("direct_chat_search_box")
                    .expect("Can't find direct_chat_search_box in ui file.");
                let scroll = self.gtk_builder
                    .get_object::<gtk::Widget>("direct_chat_search_scroll")
                    .expect("Can't find direct_chat_search_scroll in ui file.");
                self.search_finished(users, listbox, scroll);
            }
        }
    }

    pub fn search_finished(&self, users: Vec<Member>,
                           listbox: gtk::ListBox,
                           scroll: gtk::Widget) {
        for ch in listbox.get_children().iter() {
            listbox.remove(ch);
        }
        scroll.hide();

        for (i, u) in users.iter().enumerate() {
            let w;
            {
                let mb = widgets::MemberBox::new(u, &self);
                w = mb.widget(true);
            }

            let tx = self.internal.clone();
            w.connect_button_press_event(clone!(u => move |_, _| {
                tx.send(InternalCommand::ToInvite(u.clone())).unwrap();
                glib::signal::Inhibit(true)
            }));

            listbox.insert(&w, i as i32);
            scroll.show();
        }
    }

    pub fn close_invite_dialog(&mut self) {
        let listbox = self.gtk_builder
            .get_object::<gtk::ListBox>("user_search_box")
            .expect("Can't find user_search_box in ui file.");
        let scroll = self.gtk_builder
            .get_object::<gtk::Widget>("user_search_scroll")
            .expect("Can't find user_search_scroll in ui file.");
        let to_invite = self.gtk_builder
            .get_object::<gtk::ListBox>("to_invite")
            .expect("Can't find to_invite in ui file.");
        let entry = self.gtk_builder
            .get_object::<gtk::Entry>("invite_entry")
            .expect("Can't find invite_entry in ui file.");
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("invite_user_dialog")
            .expect("Can't find invite_user_dialog in ui file.");

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

    pub fn invite(&mut self) {
        if let &Some(ref r) = &self.active_room {
            for user in &self.invite_list {
                self.backend.send(BKCommand::Invite(r.clone(), user.uid.clone())).unwrap();
            }
        }
        self.close_invite_dialog();
    }

    pub fn close_direct_chat_dialog(&mut self) {
        let listbox = self.gtk_builder
            .get_object::<gtk::ListBox>("direct_chat_search_box")
            .expect("Can't find direct_chat_search_box in ui file.");
        let scroll = self.gtk_builder
            .get_object::<gtk::Widget>("direct_chat_search_scroll")
            .expect("Can't find direct_chat_search_scroll in ui file.");
        let to_invite = self.gtk_builder
            .get_object::<gtk::ListBox>("to_chat")
            .expect("Can't find to_chat in ui file.");
        let entry = self.gtk_builder
            .get_object::<gtk::Entry>("to_chat_entry")
            .expect("Can't find to_chat_entry in ui file.");
        let dialog = self.gtk_builder
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

    pub fn set_active_room(&mut self, room: &Room) {
        self.member_limit = 50;
        self.room_panel(RoomPanel::Loading);

        let msg_entry: gtk::Entry = self.gtk_builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");
        if let Some(msg) = msg_entry.get_text() {
            let active_room_id = self.active_room.clone().unwrap_or_default();
            if msg.len() > 0 {
                self.unsent_messages.insert(active_room_id, (msg, msg_entry.get_position()));
            } else {
                self.unsent_messages.remove(&active_room_id);
            }
        }

        self.active_room = Some(room.id.clone());
        self.clear_tmp_msgs();
        self.autoscroll = true;

        self.remove_messages();

        let mut getmessages = true;
        self.shown_messages = 0;

        let msgs = room.messages.iter().rev()
                                .take(globals::INITIAL_MESSAGES)
                                .collect::<Vec<&Message>>();
        for (i, msg) in msgs.iter().enumerate() {
            let command = InternalCommand::AddRoomMessage((*msg).clone(),
                                                          MsgPos::Top,
                                                          None,
                                                          i == msgs.len() - 1,
                                                          self.is_last_viewed(&msg));
            self.internal.send(command).unwrap();
        }
        self.internal.send(InternalCommand::SetPanel(RoomPanel::Room)).unwrap();

        if !room.messages.is_empty() {
            getmessages = false;
            if let Some(msg) = room.messages.iter().last() {
                self.scroll_down();
                self.mark_as_read(msg);
            }
        }

        // getting room details
        self.backend.send(BKCommand::SetRoom(room.clone())).unwrap();
        self.reload_members(&room);

        self.set_room_topic_label(room.topic.clone());

        let name_label = self.gtk_builder
            .get_object::<gtk::Label>("room_name")
            .expect("Can't find room_name in ui file.");
        let edit = self.gtk_builder
            .get_object::<gtk::Entry>("room_name_entry")
            .expect("Can't find room_name_entry in ui file.");

        name_label.set_text(&room.name.clone().unwrap_or_default());
        edit.set_text(&room.name.clone().unwrap_or_default());

        let mut size = 24;
        if let Some(r) = room.topic.clone() {
            if !r.is_empty() {
                size = 16;
            }
        }

        self.set_current_room_avatar(room.avatar.clone(), size);
        let id = self.gtk_builder
            .get_object::<gtk::Label>("room_id")
            .expect("Can't find room_id in ui file.");
        id.set_text(&room.id.clone());
        self.set_current_room_detail(String::from("m.room.name"), room.name.clone());
        self.set_current_room_detail(String::from("m.room.topic"), room.topic.clone());

        if getmessages {
            self.backend.send(BKCommand::GetRoomMessages(self.active_room.clone().unwrap_or_default())).unwrap();
        }
    }

    pub fn set_room_detail(&mut self, roomid: String, key: String, value: Option<String>) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            let k: &str = &key;
            match k {
                "m.room.name" => { r.name = value.clone(); }
                "m.room.topic" => { r.topic = value.clone(); }
                _ => {}
            };
        }

        if roomid == self.active_room.clone().unwrap_or_default() {
            self.set_current_room_detail(key, value);
        }
    }

    pub fn set_room_avatar(&mut self, roomid: String, avatar: Option<String>) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            r.avatar = avatar.clone();
            self.roomlist.set_room_avatar(roomid.clone(), r.avatar.clone());
        }

        if roomid == self.active_room.clone().unwrap_or_default() {
            let mut size = 24;
            if let Some(r) = self.rooms.get_mut(&roomid) {
                if !r.clone().topic.unwrap_or_default().is_empty() {
                    size = 16;
                }
            }
            self.set_current_room_avatar(avatar, size);
        }
    }

    pub fn set_current_room_detail(&self, key: String, value: Option<String>) {
        let value = value.unwrap_or_default();
        let k: &str = &key;
        match k {
            "m.room.name" => {
                let name_label = self.gtk_builder
                    .get_object::<gtk::Label>("room_name")
                    .expect("Can't find room_name in ui file.");
                let edit = self.gtk_builder
                    .get_object::<gtk::Entry>("room_name_entry")
                    .expect("Can't find room_name_entry in ui file.");

                name_label.set_text(&value);
                edit.set_text(&value);

            }
            "m.room.topic" => {
                self.set_room_topic_label(Some(value.clone()));

                let edit = self.gtk_builder
                    .get_object::<gtk::Entry>("room_topic_entry")
                    .expect("Can't find room_topic_entry in ui file.");

                edit.set_text(&value);
            }
            _ => println!("no key {}", key),
        };
    }

    pub fn set_current_room_avatar(&self, avatar: Option<String>, size: i32) {
        let image = self.gtk_builder
            .get_object::<gtk::Box>("room_image")
            .expect("Can't find room_image in ui file.");
        for ch in image.get_children() {
            image.remove(&ch);
        }

        let config = self.gtk_builder
            .get_object::<gtk::Image>("room_avatar_image")
            .expect("Can't find room_avatar_image in ui file.");

        if avatar.is_some() && !avatar.clone().unwrap().is_empty() {
            image.add(&widgets::Avatar::circle_avatar(avatar.clone().unwrap(), Some(size)));
            if let Ok(pixbuf) = Pixbuf::new_from_file_at_size(&avatar.clone().unwrap(), 100, 100) {
                config.set_from_pixbuf(&pixbuf);
            }
        } else {
            let w = widgets::Avatar::avatar_new(Some(size));
            w.default(String::from("camera-photo-symbolic"), Some(size));
            image.add(&w);
            config.set_from_icon_name("camera-photo-symbolic", 1);
        }
    }

    pub fn scroll_down(&self) {
        let scroll = self.gtk_builder
            .get_object::<gtk::ScrolledWindow>("messages_scroll")
            .expect("Can't find message_scroll in ui file.");

        let s = scroll.clone();
        gtk::timeout_add(500, move || {
            if let Some(adj) = s.get_vadjustment() {
                adj.set_value(adj.get_upper() - adj.get_page_size());
            }
            gtk::Continue(false)
        });
    }

    fn should_group(&self, msg: &Message, prev: &Message) -> bool {
        let same_sender = msg.sender == prev.sender;

        match same_sender {
            true => {
                let diff = msg.date.signed_duration_since(prev.date);
                let minutes = diff.num_minutes();
                minutes < globals::MINUTES_TO_SPLIT_MSGS && !self.has_small_mtype(prev)
            },
            false => false,
        }
    }

    fn has_small_mtype(&self, msg: &Message) -> bool {
        match msg.mtype.as_ref() {
            "m.emote" => true,
            _ => false,
        }
    }

    pub fn is_last_viewed(&self, msg: &Message) -> bool {
        match self.last_viewed_messages.get(&msg.room) {
            Some(lvm) if lvm == msg => true,
            _ => false,
        }
    }

    pub fn add_room_message(&mut self,
                            msg: Message,
                            msgpos: MsgPos,
                            prev: Option<Message>,
                            force_full: bool,
                            last: bool) {
        let msg_entry: gtk::Entry = self.gtk_builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");
        let messages = self.gtk_builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        let mut calc_prev = prev;
        if !force_full && calc_prev.is_none() {
            if let Some(r) = self.rooms.get(&msg.room) {
                calc_prev = match r.messages.iter().position(|ref m| m.id == msg.id) {
                    Some(pos) if pos > 0 => r.messages.get(pos - 1).cloned(),
                    _ => None
                };
            }
        }

        if msg.room == self.active_room.clone().unwrap_or_default() {
            if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
                let m;
                let mut is_small_widget = true;
                {
                    let mb = widgets::MessageBox::new(r, &msg, &self);
                    let entry = msg_entry.clone();
                    mb.username_event_box.connect_button_press_event(move |eb, _| {
                        if let Some(label) = eb.get_children().iter().nth(0) {
                            if let Ok(l) = label.clone().downcast::<gtk::Label>() {
                                if let Some(t) = l.get_text() {
                                    let mut pos = entry.get_position();
                                    entry.insert_text(&t[..], &mut pos);
                                }
                            }
                        }
                        glib::signal::Inhibit(false)
                    });
                    m = match calc_prev {
                        Some(ref p) if self.should_group(&msg, p) => mb.small_widget(),
                        Some(_) if self.has_small_mtype(&msg) => mb.small_widget(),
                        _ => { is_small_widget = false; mb.widget()},
                    }
                }

                if last {
                    if let Some(style) = m.get_style_context() {
                        style.add_class("msg-last-viewed-widget");
                    }
                }

                match msgpos {
                    MsgPos::Bottom => messages.add(&m),
                    MsgPos::Top => messages.insert(&m, 1),
                };
                if !is_small_widget {
                    m.get_parent().unwrap().set_margin_top(12);
                }
                self.shown_messages += 1;
            }
            self.remove_tmp_room_message(&msg);
        }
    }

    pub fn add_tmp_room_message(&mut self, msg: Message) {
        let messages = self.gtk_builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            let m;
            {
                let mb = widgets::MessageBox::new(r, &msg, &self);
                m = mb.widget();
            }

            messages.add(&m);
        }

        if let Some(w) = messages.get_children().iter().last() {
            self.tmp_msgs.push(TmpMsg {
                    msg: msg.clone(),
                    widget: w.clone(),
            });

            self.scroll_down();
        };
    }

    pub fn clear_tmp_msgs(&mut self) {
        let messages = self.gtk_builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");
        for t in self.tmp_msgs.iter() {
            messages.remove(&t.widget);
        }
        self.tmp_msgs.clear();
    }

    pub fn remove_tmp_room_message(&mut self, msg: &Message) {
        let messages = self.gtk_builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        let mut rmidx = None;

        for (i, t) in self.tmp_msgs.iter().enumerate() {
            if t.msg.sender == msg.sender &&
               t.msg.mtype == msg.mtype &&
               t.msg.room == msg.room &&
               t.msg.body == msg.body {

                messages.remove(&t.widget);
                rmidx = Some(i);
                break;
            }
        }

        if rmidx.is_some() {
            self.tmp_msgs.remove(rmidx.unwrap());
        }
    }

    pub fn set_room_notifications(&mut self, roomid: String, n: i32, h: i32) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            r.notifications = n;
            r.highlight = h;
            self.roomlist.set_room_notifications(roomid, r.notifications, r.highlight);
        }
    }

    pub fn mark_as_read(&mut self, msg: &Message) {
        self.last_viewed_messages.insert(msg.room.clone(), msg.clone());
        self.backend.send(BKCommand::MarkAsRead(msg.room.clone(),
                                                msg.id.clone().unwrap_or_default())).unwrap();
    }

    pub fn add_room_member(&mut self, m: Member) {
        let store: gtk::ListStore = self.gtk_builder
            .get_object("members_store")
            .expect("Couldn't find members_store in ui file.");

        let name = m.get_alias();
        if let Some(r) = self.rooms.get_mut(&self.active_room.clone().unwrap_or_default()) {
            // only show 200 members...
            if r.members.len() < 200 {
                store.insert_with_values(None, &[0, 1], &[&name, &(m.uid)]);
            }
        }
    }

    pub fn send_message(&mut self, msg: String) {
        if msg.is_empty() {
            // Not sending empty messages
            return;
        }

        let room = self.active_room.clone();
        let now = Local::now();

        let mtype = strn!("m.text");

        let mut m = Message {
            sender: self.uid.clone().unwrap_or_default(),
            mtype: mtype,
            body: msg.clone(),
            room: room.clone().unwrap_or_default(),
            date: now,
            thumb: None,
            url: None,
            id: None,
        };

        if msg.starts_with("/me ") {
            m.body = msg.trim_left_matches("/me ").to_owned();
            m.mtype = strn!("m.emote");
        };


        self.add_tmp_room_message(m.clone());
        self.backend.send(BKCommand::SendMsg(m)).unwrap();
    }

    pub fn attach_file(&mut self) {
        let window: gtk::ApplicationWindow = self.gtk_builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");
        let dialog = gtk::FileChooserDialog::new(None,
                                                 Some(&window),
                                                 gtk::FileChooserAction::Open);

        let btn = dialog.add_button("Select", 1);
        btn.get_style_context().unwrap().add_class("suggested-action");

        let backend = self.backend.clone();
        let room = self.active_room.clone().unwrap_or_default();
        dialog.connect_response(move |dialog, resp| {
            if resp == 1 {
                if let Some(fname) = dialog.get_filename() {
                    let f = strn!(fname.to_str().unwrap_or(""));
                    backend.send(BKCommand::AttachFile(room.clone(), f)).unwrap();
                }
            }
            dialog.destroy();
        });

        let backend = self.backend.clone();
        let room = self.active_room.clone().unwrap_or_default();
        dialog.connect_file_activated(move |dialog| {
            if let Some(fname) = dialog.get_filename() {
                let f = strn!(fname.to_str().unwrap_or(""));
                backend.send(BKCommand::AttachFile(room.clone(), f)).unwrap();
            }
            dialog.destroy();
        });

        dialog.show();
    }

    pub fn load_more_messages(&mut self) {
        if self.loading_more {
            return;
        }

        self.loading_more = true;
        self.load_more_btn.set_label("loading...");

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            if self.shown_messages < r.messages.len() {
                let msgs = r.messages.iter().rev()
                                     .skip(self.shown_messages)
                                     .take(globals::INITIAL_MESSAGES)
                                     .collect::<Vec<&Message>>();
                for (i, msg) in msgs.iter().enumerate() {
                    let command = InternalCommand::AddRoomMessage((*msg).clone(),
                                                                  MsgPos::Top,
                                                                  None,
                                                                  i == msgs.len() - 1,
                                                                  self.is_last_viewed(&msg));
                    self.internal.send(command).unwrap();
                }
                self.internal.send(InternalCommand::LoadMoreNormal).unwrap();
            } else if let Some(m) = r.messages.get(0) {
                self.backend.send(BKCommand::GetMessageContext(m.clone())).unwrap();
            }
        }
    }

    pub fn load_more_normal(&mut self) {
        self.load_more_btn.set_label("load more messages");
        self.loading_more = false;
    }

    pub fn init_protocols(&self) {
        self.backend.send(BKCommand::DirectoryProtocols).unwrap();
    }

    pub fn set_protocols(&self, protocols: Vec<Protocol>) {
        let combo = self.gtk_builder
            .get_object::<gtk::ListStore>("protocol_model")
            .expect("Can't find protocol_model in ui file.");
        combo.clear();

        for p in protocols {
            combo.insert_with_values(None, &[0, 1], &[&p.desc, &p.id]);
        }

        self.gtk_builder
            .get_object::<gtk::ComboBox>("directory_combo")
            .expect("Can't find directory_combo in ui file.")
            .set_active(0);
    }

    pub fn search_rooms(&self, more: bool) {
        let combo_store = self.gtk_builder
            .get_object::<gtk::ListStore>("protocol_model")
            .expect("Can't find protocol_model in ui file.");
        let combo = self.gtk_builder
            .get_object::<gtk::ComboBox>("directory_combo")
            .expect("Can't find directory_combo in ui file.");

        let active = combo.get_active();
        let protocol: String = match combo_store.iter_nth_child(None, active) {
            Some(it) => {
                let v = combo_store.get_value(&it, 1);
                v.get().unwrap()
            }
            None => String::from(""),
        };

        let q = self.gtk_builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");

        let btn = self.gtk_builder
            .get_object::<gtk::Button>("directory_search_button")
            .expect("Can't find directory_search_button in ui file.");
        btn.set_label("Searching...");
        btn.set_sensitive(false);

        if !more {
            let directory = self.gtk_builder
                .get_object::<gtk::ListBox>("directory_room_list")
                .expect("Can't find directory_room_list in ui file.");
            for ch in directory.get_children() {
                directory.remove(&ch);
            }
        }

        self.backend
            .send(BKCommand::DirectorySearch(q.get_text().unwrap(), protocol, more))
            .unwrap();
    }

    pub fn load_more_rooms(&self) {
        self.search_rooms(true);
    }

    pub fn set_directory_room(&self, room: Room) {
        let directory = self.gtk_builder
            .get_object::<gtk::ListBox>("directory_room_list")
            .expect("Can't find directory_room_list in ui file.");

        let rb = widgets::RoomBox::new(&room, &self);
        let room_widget = rb.widget();
        directory.add(&room_widget);

        self.enable_directory_search();
    }

    pub fn enable_directory_search(&self) {
        let btn = self.gtk_builder
            .get_object::<gtk::Button>("directory_search_button")
            .expect("Can't find directory_search_button in ui file.");
        btn.set_label("Search");
        btn.set_sensitive(true);
    }

    pub fn inapp_notify(&self, msg: &str) {
        let inapp: gtk::Revealer = self.gtk_builder
            .get_object("inapp_revealer")
            .expect("Can't find inapp_revealer in ui file.");
        let label: gtk::Label = self.gtk_builder
            .get_object("inapp_label")
            .expect("Can't find inapp_label in ui file.");
        label.set_text(msg);
        inapp.set_reveal_child(true);
    }

    pub fn hide_inapp_notify(&self) {
        let inapp: gtk::Revealer = self.gtk_builder
            .get_object("inapp_revealer")
            .expect("Can't find inapp_revealer in ui file.");
        inapp.set_reveal_child(false);
    }

    pub fn notify(&self, msg: &Message) {
        let roomname = match self.rooms.get(&msg.room) {
            Some(r) => r.name.clone().unwrap_or_default(),
            None => msg.room.clone(),
        };

        let mut body = msg.body.clone();
        body.truncate(80);

        let (tx, rx): (Sender<(String, String)>, Receiver<(String, String)>) = channel();
        self.backend.send(BKCommand::GetUserInfoAsync(msg.sender.clone(), tx)).unwrap();
        let bk = self.internal.clone();
        let m = msg.clone();
        gtk::timeout_add(50, move || match rx.try_recv() {
            Err(TryRecvError::Empty) => gtk::Continue(true),
            Err(TryRecvError::Disconnected) => gtk::Continue(false),
            Ok((name, avatar)) => {
                let summary = format!("@{} / {}", name, roomname);

                let bk = bk.clone();
                let m = m.clone();
                let body = body.clone();
                let summary = summary.clone();
                let avatar = avatar.clone();
                thread::spawn(move || {
                    let mut notification = Notification::new();
                    notification.summary(&summary);
                    notification.body(&body);
                    notification.icon(&avatar);
                    notification.action("default", "default");

                    if let Ok(n) = notification.show() {
                        n.wait_for_action({|action|
                            match action {
                                "default" => {
                                    bk.send(InternalCommand::NotifyClicked(m)).unwrap();
                                },
                                _ => ()
                            }
                        });
                    }
                });

                gtk::Continue(false)
            }
        });
    }

    pub fn show_room_messages(&mut self, msgs: Vec<Message>, init: bool) -> Option<()> {
        for msg in msgs.iter() {
            if let Some(r) = self.rooms.get_mut(&msg.room) {
                r.messages.push(msg.clone());
            }
        }

        let mut prev = None;
        for msg in msgs.iter() {
            let mut should_notify = msg.body.contains(&self.username.clone()?);
            // not notifying the initial messages
            should_notify = should_notify && !init;
            // not notifying my own messages
            should_notify = should_notify && (msg.sender != self.uid.clone()?);

            if should_notify {
                self.notify(msg);
            }

            let command = InternalCommand::AddRoomMessage(msg.clone(), MsgPos::Bottom, prev, false,
                                                          self.is_last_viewed(&msg));
            self.internal.send(command).unwrap();
            prev = Some(msg.clone());

            if !init {
                self.roomlist.moveup(msg.room.clone());
                self.roomlist.set_bold(msg.room.clone(), true);
            }
        }

        if !msgs.is_empty() {
            let active_room = self.active_room.clone().unwrap_or_default();
            let fs = msgs.iter().filter(|x| x.room == active_room);
            if let Some(msg) = fs.last() {
                if self.autoscroll {
                    self.scroll_down();
                }
                self.mark_as_read(msg);
            }
        }

        if init {
            self.room_panel(RoomPanel::Room);
        }

        Some(())
    }

    pub fn show_room_messages_top(&mut self, msgs: Vec<Message>) {
        if msgs.is_empty() {
            self.load_more_normal();
            return;
        }

        for msg in msgs.iter().rev() {
            if let Some(r) = self.rooms.get_mut(&msg.room) {
                r.messages.insert(0, msg.clone());
            }
        }

        let size = msgs.len() - 1;
        for i in 0..size+1 {
            let msg = &msgs[size - i];

            let prev = match i {
                n if size - n > 0 => msgs.get(size - n - 1).cloned(),
                _ => None
            };

            let command = InternalCommand::AddRoomMessage(msg.clone(), MsgPos::Top, prev, false,
                                                          self.is_last_viewed(&msg));
            self.internal.send(command).unwrap();

        }
        self.internal.send(InternalCommand::LoadMoreNormal).unwrap();
    }

    pub fn show_room_dialog(&self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("room_config_dialog")
            .expect("Can't find room_config_dialog in ui file.");

        dialog.present();
    }

    pub fn show_invite_user_dialog(&mut self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("invite_user_dialog")
            .expect("Can't find invite_user_dialog in ui file.");
        let scroll = self.gtk_builder
            .get_object::<gtk::Widget>("user_search_scroll")
            .expect("Can't find user_search_scroll in ui file.");
        let title = self.gtk_builder
            .get_object::<gtk::Label>("invite_title")
            .expect("Can't find invite_title in ui file.");
        self.search_type = SearchType::Invite;

        if let Some(aroom) = self.active_room.clone() {
            if let Some(r) = self.rooms.get(&aroom) {
                if let &Some(ref name) = &r.name {
                    title.set_text(&format!("Invite to {}", name));
                } else {
                    title.set_text("Invite");
                }
            }
        }

        dialog.present();
        scroll.hide();
    }

    pub fn show_direct_chat_dialog(&mut self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("direct_chat_dialog")
            .expect("Can't find direct_chat_dialog in ui file.");
        let scroll = self.gtk_builder
            .get_object::<gtk::Widget>("direct_chat_search_scroll")
            .expect("Can't find direct_chat_search_scroll in ui file.");
        self.search_type = SearchType::DirectChat;

        dialog.present();
        scroll.hide();
    }

    pub fn really_leave_active_room(&mut self) {
        let r = self.active_room.clone().unwrap_or_default();
        self.backend.send(BKCommand::LeaveRoom(r.clone())).unwrap();
        self.rooms.remove(&r);
        self.active_room = None;
        self.clear_tmp_msgs();
        self.room_panel(RoomPanel::NoRoom);

        self.roomlist.remove_room(r);
    }

    pub fn leave_active_room(&self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::MessageDialog>("leave_room_dialog")
            .expect("Can't find leave_room_dialog in ui file.");

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            dialog.set_property_text(Some(&format!("Leave {}?", r.name.clone().unwrap_or_default())));
            dialog.present();
        }
    }

    pub fn new_room_dialog(&self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("new_room_dialog")
            .expect("Can't find new_room_dialog in ui file.");
        dialog.present();
    }

    pub fn create_new_room(&mut self) {
        let name = self.gtk_builder
            .get_object::<gtk::Entry>("new_room_name")
            .expect("Can't find new_room_name in ui file.");
        let preset = self.gtk_builder
            .get_object::<gtk::ComboBox>("new_room_preset")
            .expect("Can't find new_room_preset in ui file.");

        let n = name.get_text().unwrap_or(String::from(""));

        let p = match preset.get_active_iter() {
            None => backend::RoomType::Private,
            Some(iter) => {
                match preset.get_model() {
                    None => backend::RoomType::Private,
                    Some(model) => {
                        match model.get_value(&iter, 1).get().unwrap() {
                            "private_chat" => backend::RoomType::Private,
                            "public_chat" => backend::RoomType::Public,
                            _ => backend::RoomType::Private,
                        }
                    }
                }
            }
        };

        let internal_id: String = thread_rng().gen_ascii_chars().take(10).collect();
        self.backend.send(BKCommand::NewRoom(n.clone(), p, internal_id.clone())).unwrap();

        let fakeroom = Room::new(internal_id.clone(), Some(n));
        self.new_room(fakeroom, None);
        self.roomlist.set_selected(Some(internal_id.clone()));
        self.set_active_room_by_id(internal_id);
        self.room_panel(RoomPanel::Loading);
    }

    pub fn join_to_room_dialog(&mut self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("join_room_dialog")
            .expect("Can't find join_room_dialog in ui file.");
        dialog.present();
    }

    pub fn join_to_room(&mut self) {
        let name = self.gtk_builder
            .get_object::<gtk::Entry>("join_room_name")
            .expect("Can't find join_room_name in ui file.");

        let n = name.get_text().unwrap_or(String::from(""));

        self.backend.send(BKCommand::JoinRoom(n.clone())).unwrap();
    }

    pub fn new_room(&mut self, r: Room, internal_id: Option<String>) {
        if let Some(id) = internal_id {
            self.remove_room(id);
        }

        if !self.rooms.contains_key(&r.id) {
            self.rooms.insert(r.id.clone(), r.clone());
        }

        self.roomlist.add_room(r.clone());
        self.roomlist.moveup(r.id.clone());
        self.roomlist.set_selected(Some(r.id.clone()));

        self.set_active_room_by_id(r.id);
    }

    pub fn added_to_fav(&mut self, roomid: String, tofav: bool) {
        if let Some(ref mut r) = self.rooms.get_mut(&roomid) {
            r.fav = tofav;
        }
    }

    pub fn change_room_config(&mut self) {
        let name = self.gtk_builder
            .get_object::<gtk::Entry>("room_name_entry")
            .expect("Can't find room_name_entry in ui file.");
        let topic = self.gtk_builder
            .get_object::<gtk::Entry>("room_topic_entry")
            .expect("Can't find room_topic_entry in ui file.");
        let avatar_fs = self.gtk_builder
            .get_object::<gtk::FileChooserDialog>("file_chooser_dialog")
            .expect("Can't find file_chooser_dialog in ui file.");

        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            if let Some(n) = name.get_text() {
                if n != r.name.clone().unwrap_or_default() {
                    let command = BKCommand::SetRoomName(r.id.clone(), n.clone());
                    self.backend.send(command).unwrap();
                }
            }
            if let Some(t) = topic.get_text() {
                if t != r.topic.clone().unwrap_or_default() {
                    let command = BKCommand::SetRoomTopic(r.id.clone(), t.clone());
                    self.backend.send(command).unwrap();
                }
            }
            if let Some(f) = avatar_fs.get_filename() {
                if let Some(name) = f.to_str() {
                    let command = BKCommand::SetRoomAvatar(r.id.clone(), String::from(name));
                    self.backend.send(command).unwrap();
                }
            }
        }
    }

    pub fn room_name_change(&mut self, roomid: String, name: Option<String>) {
        if !self.rooms.contains_key(&roomid) {
            return;
        }

        {
            let r = self.rooms.get_mut(&roomid).unwrap();
            r.name = name.clone();
        }

        if roomid == self.active_room.clone().unwrap_or_default() {
            self.gtk_builder
                .get_object::<gtk::Label>("room_name")
                .expect("Can't find room_name in ui file.")
                .set_text(&name.clone().unwrap_or_default());
        }

        self.roomlist.rename_room(roomid.clone(), name);
    }

    pub fn room_topic_change(&mut self, roomid: String, topic: Option<String>) {
        if !self.rooms.contains_key(&roomid) {
            return;
        }

        {
            let r = self.rooms.get_mut(&roomid).unwrap();
            r.topic = topic.clone();
        }

        if roomid == self.active_room.clone().unwrap_or_default() {
            self.set_room_topic_label(topic);
        }
    }

    pub fn set_room_topic_label(&self, topic: Option<String>) {
        let t = self.gtk_builder
            .get_object::<gtk::Label>("room_topic")
            .expect("Can't find room_topic in ui file.");
        let n = self.gtk_builder
                .get_object::<gtk::Label>("room_name")
                .expect("Can't find room_name in ui file.");

        match topic {
            None => {
                t.set_tooltip_text("");
                n.set_tooltip_text("");
                t.hide();
            },
            Some(ref topic) if topic.is_empty() => {
                t.set_tooltip_text("");
                n.set_tooltip_text("");
                t.hide();
            },
            Some(ref topic) => {
                t.set_tooltip_text(&topic[..]);
                n.set_tooltip_text(&topic[..]);
                t.set_markup(&markup(&topic.split('\n').nth(0).unwrap_or_default()));
                t.show();
            }
        };
    }

    pub fn new_room_avatar(&self, roomid: String) {
        if !self.rooms.contains_key(&roomid) {
            return;
        }

        self.backend.send(BKCommand::GetRoomAvatar(roomid)).unwrap();
    }

    pub fn room_member_event(&mut self, ev: Event) {
        // NOTE: maybe we should show this events in the message list to notify enters and leaves
        // to the user

        let sender = ev.sender.clone();
        match ev.content["membership"].as_str() {
            Some("leave") => {
                if let Some(r) = self.rooms.get_mut(&self.active_room.clone().unwrap_or_default()) {
                    r.members.remove(&sender);
                }
            }
            Some("join") => {
                let m = Member {
                    avatar: Some(strn!(ev.content["avatar_url"].as_str().unwrap_or(""))),
                    alias: Some(strn!(ev.content["displayname"].as_str().unwrap_or(""))),
                    uid: sender.clone(),
                };
                if let Some(r) = self.rooms.get_mut(&self.active_room.clone().unwrap_or_default()) {
                    r.members.insert(m.uid.clone(), m.clone());
                }
            }
            // ignoring other memberships
            _ => {}
        }

        if ev.room != self.active_room.clone().unwrap_or_default() {
            // if it isn't the current room, this event is not important for me
            return;
        }

        let store = self.gtk_builder
            .get_object::<gtk::ListStore>("members_store")
            .expect("Can't find members_store in ui file.");

        match ev.content["membership"].as_str() {
            Some("leave") => {
                if let Some(iter) = store.get_iter_first() {
                    loop {
                        let v1 = store.get_value(&iter, 1);
                        let id: &str = v1.get().unwrap();
                        if id == sender {
                            store.remove(&iter);
                        }
                        if !store.iter_next(&iter) {
                            break;
                        }
                    }
                }
                self.show_all_members();
            }
            Some("join") => {
                let m = Member {
                    avatar: Some(strn!(ev.content["avatar_url"].as_str().unwrap_or(""))),
                    alias: Some(strn!(ev.content["displayname"].as_str().unwrap_or(""))),
                    uid: sender.clone(),
                };
                self.add_room_member(m);
                self.show_all_members();
            }
            // ignoring other memberships
            _ => {}
        }
    }

    pub fn toggle_search(&self) {
        let r: gtk::Revealer = self.gtk_builder
            .get_object("search_revealer")
            .expect("Couldn't find search_revealer in ui file.");
        r.set_reveal_child(!r.get_child_revealed());
    }

    pub fn search(&mut self, term: Option<String>) {
        let r = self.active_room.clone().unwrap_or_default();
        self.remove_messages();
        self.backend.send(BKCommand::Search(r, term)).unwrap();

        self.gtk_builder
            .get_object::<gtk::Stack>("search_button_stack")
            .expect("Can't find search_button_stack in ui file.")
            .set_visible_child_name("searching");
    }

    pub fn search_end(&self) {
        self.gtk_builder
            .get_object::<gtk::Stack>("search_button_stack")
            .expect("Can't find search_button_stack in ui file.")
            .set_visible_child_name("normal");
    }

    pub fn show_error(&self, msg: String) {
        let window: gtk::Window = self.gtk_builder
            .get_object("main_window")
            .expect("Couldn't find main_window in ui file.");
        let dialog = gtk::MessageDialog::new(Some(&window),
                                             gtk::DialogFlags::MODAL,
                                             gtk::MessageType::Warning,
                                             gtk::ButtonsType::Ok,
                                             &msg);
        dialog.show();
        dialog.connect_response(move |d, _| { d.destroy(); });
    }

    pub fn paste(&self) {
        if let Some(display) = gdk::Display::get_default() {
            if let Some(clipboard) = gtk::Clipboard::get_default(&display) {
                if clipboard.wait_is_image_available() {
                    if let Some(pixb) = clipboard.wait_for_image() {
                        self.draw_image_paste_dialog(&pixb);

                        // removing text from clipboard
                        clipboard.set_text("");
                        clipboard.set_image(&pixb);
                    }
                } else {
                    // TODO: manage code pasting
                }
            }
        }
    }

    fn draw_image_paste_dialog(&self, pixb: &Pixbuf) {
        let w = pixb.get_width();
        let h = pixb.get_height();
        let scaled;
        if w > 600 {
            scaled = pixb.scale_simple(600, h*600/w, gdk_pixbuf_sys::GDK_INTERP_BILINEAR);
        } else {
            scaled = Ok(pixb.clone());
        }

        if let Ok(pb) = scaled {
            let window: gtk::ApplicationWindow = self.gtk_builder
                .get_object("main_window")
                .expect("Can't find main_window in ui file.");
            let img = gtk::Image::new();
            let dialog = gtk::Dialog::new_with_buttons(
                Some("Image from Clipboard"),
                Some(&window),
                gtk::DialogFlags::MODAL|
                gtk::DialogFlags::USE_HEADER_BAR|
                gtk::DialogFlags::DESTROY_WITH_PARENT,
                &[]);

            img.set_from_pixbuf(&pb);
            img.show();
            dialog.get_content_area().add(&img);
            dialog.present();

            if let Some(hbar) = dialog.get_header_bar() {
                let bar = hbar.downcast::<gtk::HeaderBar>().unwrap();
                let closebtn = gtk::Button::new_with_label("Cancel");
                let okbtn = gtk::Button::new_with_label("Send");
                okbtn.get_style_context().unwrap().add_class("suggested-action");

                bar.set_show_close_button(false);
                bar.pack_start(&closebtn);
                bar.pack_end(&okbtn);
                bar.show_all();

                closebtn.connect_clicked(clone!(dialog => move |_| {
                    dialog.destroy();
                }));
                let room = self.active_room.clone().unwrap_or_default();
                let bk = self.backend.clone();
                okbtn.connect_clicked(clone!(pixb, dialog => move |_| {
                    if let Ok(data) = get_pixbuf_data(&pixb) {
                        bk.send(BKCommand::AttachImage(room.clone(), data)).unwrap();
                    }
                    dialog.destroy();
                }));

                okbtn.grab_focus();
            }
        }
    }

    pub fn notification_cliked(&mut self, msg: Message) {
        self.activate();
        let mut room = None;
        if let Some(r) = self.rooms.get(&msg.room) {
            room = Some(r.clone());
        }

        if let Some(r) = room {
            self.set_active_room_by_id(r.id.clone());
        }
    }

    pub fn activate(&self) {
        let window: gtk::Window = self.gtk_builder
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

    pub fn clean_member_list(&self) {
        let mlist: gtk::ListBox = self.gtk_builder
            .get_object("member_list")
            .expect("Couldn't find member_list in ui file.");

        let childs = mlist.get_children();
        let n = childs.len() - 1;
        for ch in childs.iter().take(n) {
            mlist.remove(ch);
        }
    }

    pub fn show_members(&self, members: Vec<Member>) {
        self.clean_member_list();

        let mlist: gtk::ListBox = self.gtk_builder
            .get_object("member_list")
            .expect("Couldn't find member_list in ui file.");

        let msg_entry: gtk::Entry = self.gtk_builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");

        // limiting the number of members to show in the list
        for member in members.iter().take(self.member_limit) {
            let w;
            let m = member.clone();

            {
                let mb = widgets::MemberBox::new(&m, &self);
                w = mb.widget(false);
            }

            let msg = msg_entry.clone();
            w.connect_button_press_event(move |_, _| {
                if let Some(ref a) = m.alias {
                    let mut pos = msg.get_position();
                    msg.insert_text(&a.clone(), &mut pos);
                }
                glib::signal::Inhibit(true)
            });

            let p = mlist.get_children().len() - 1;
            mlist.insert(&w, p as i32);
        }

        if members.len() > self.member_limit {
            let newlabel = format!("and {} more", members.len() - self.member_limit);
            self.more_members_btn.set_label(&newlabel);
            self.more_members_btn.show();
        } else {
            self.more_members_btn.hide();
        }
    }

    pub fn show_all_members(&self) {
        let inp: gtk::SearchEntry = self.gtk_builder
            .get_object("members_search")
            .expect("Couldn't find members_searcn in ui file.");
        let text = inp.get_text();
        if let Some(r) = self.rooms.get(&self.active_room.clone().unwrap_or_default()) {
            let members = match text {
                // all members if no search text
                None => r.members.values().cloned().collect(),
                Some(t) => {
                    // members with the text in the alias
                    r.members.values().filter(move |x| {
                        match x.alias {
                            None => false,
                            Some(ref a) => a.to_lowercase().contains(&t.to_lowercase())
                        }
                    }).cloned().collect()
                }
            };
            self.show_members(members);
        }
    }

    pub fn about_dialog(&self) {
        let window: gtk::ApplicationWindow = self.gtk_builder
            .get_object("main_window")
            .expect("Can't find main_window in ui file.");

        let dialog = gtk::AboutDialog::new();
        dialog.set_logo_icon_name(APP_ID);
        dialog.set_comments("A Matrix.org client for GNOME");
        dialog.set_copyright("© 2017–2018 Daniel García Moreno, et al.");
        dialog.set_license_type(gtk::License::Gpl30);
        dialog.set_modal(true);
        dialog.set_version(env!("CARGO_PKG_VERSION"));
        dialog.set_program_name("Fractal");
        dialog.set_website("https://wiki.gnome.org/Fractal");
        dialog.set_website_label("Learn more about Fractal");
        dialog.set_transient_for(&window);

        dialog.set_artists(&[
            "Tobias Bernard",
        ]);

        dialog.set_authors(&[
            "Daniel García Moreno",
            "Jordan Petridis",
            "Alexandre Franke",
            "Saurav Sachidanand",
        ]);

        dialog.add_credit_section("Name by", &["Regina Bíró"]);

        dialog.show();
    }

    pub fn filter_rooms(&self, term: Option<String>) {
        self.roomlist.filter_rooms(term);
    }

    pub fn set_room_members(&mut self, members: Vec<Member>) {
        if let Some(aroom) = self.active_room.clone() {
            let mut room: Option<Room> = None;
            if let Some(r) = self.rooms.get_mut(&aroom) {
                r.members = HashMap::new();
                for m in members {
                    r.members.insert(m.uid.clone(), m);
                }
                room = Some(r.clone());
            }

            if let Some(r) = room {
                self.reload_members(&r);
            }
        }
    }

    pub fn reload_members(&mut self, room: &Room) {
        let members = self.gtk_builder
            .get_object::<gtk::ListStore>("members_store")
            .expect("Can't find members_store in ui file.");
        members.clear();

        self.clean_member_list();

        for (_, m) in room.members.iter() {
            self.add_room_member(m.clone());
        }

        self.show_all_members();
    }

    pub fn add_to_invite(&mut self, u: Member) {
        let listboxid = match self.search_type {
            SearchType::Invite => "to_invite",
            SearchType::DirectChat => "to_chat",
        };

        let to_invite = self.gtk_builder
            .get_object::<gtk::ListBox>(listboxid)
            .expect("Can't find to_invite in ui file.");

        if self.invite_list.contains(&u) {
            return;
        }

        if let SearchType::DirectChat = self.search_type {
            self.invite_list = vec![];
            for ch in to_invite.get_children().iter() {
                to_invite.remove(ch);
            }
        }

        self.invite_list.push(u.clone());

        let w;
        {
            let mb = widgets::MemberBox::new(&u, &self);
            w = mb.widget(true);
        }

        let mbox;

        mbox = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let btn = gtk::Button::new();
        let img = gtk::Image::new_from_icon_name("window-close-symbolic", 2);
        btn.get_style_context().unwrap().add_class("circular");
        btn.set_image(&img);

        mbox.pack_start(&w, true, true, 0);
        mbox.pack_start(&btn, false, false, 0);
        mbox.show_all();

        let tx = self.internal.clone();
        let uid = u.uid.clone();
        btn.connect_clicked(move |_| {
            tx.send(InternalCommand::RmInvite(uid.clone())).unwrap();
        });

        let size = (self.invite_list.len() - 1) as i32;
        to_invite.insert(&mbox, size);
    }

    pub fn rm_from_invite(&mut self, uid: String) {
        let invid;
        let dialogid;

        match self.search_type {
            SearchType::Invite => {
                invid = "to_invite";
                dialogid = "invite_user_dialog";
            }
            SearchType::DirectChat => {
                invid = "to_chat";
                dialogid = "direct_chat_dialog";
            }
        };

        let to_invite = self.gtk_builder
            .get_object::<gtk::ListBox>(invid)
            .expect("Can't find to_invite in ui file.");
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>(dialogid)
            .expect("Can't find invite_user_dialog in ui file.");

        let idx = self.invite_list.iter().position(|x| x.uid == uid);
        if let Some(i) = idx {
            self.invite_list.remove(i);
            if let Some(r) = to_invite.get_row_at_index(i as i32) {
                to_invite.remove(&r);
            }
        }
        dialog.resize(300, 200);
    }

    pub fn autocomplete_tab(&self, ev: &gdk::Event) {
        let listbox = self.gtk_builder
            .get_object::<gtk::ListBox>("autocomplete_listbox")
            .expect("Can't find autocomplete_listbox in ui file.");

        if let Some(row) = listbox.get_row_at_index(0) {
            if let Some(w) = row.get_children().first() {
                let _ = w.emit("button-press-event", &[ev]);
            }
        }
    }

    pub fn autocomplete(&self, text: Option<String>) {
        let msg_entry: gtk::Entry = self.gtk_builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");
        let popover = self.gtk_builder
            .get_object::<gtk::Popover>("autocomplete_popover")
            .expect("Can't find autocomplete_popover in ui file.");
        let listbox = self.gtk_builder
            .get_object::<gtk::ListBox>("autocomplete_listbox")
            .expect("Can't find autocomplete_listbox in ui file.");

        for ch in listbox.get_children().iter() {
            listbox.remove(ch);
        }

        match text {
            None => {
                popover.popdown();
                return;
            }
            Some(ref t) if t.len() < 3 => {
                popover.popdown();
                return;
            }
            Some(txt) => {
                if let Some(last) = txt.split(' ').last() {
                    let n = last.len();
                    if n < 3 {
                        popover.popdown();
                        return;
                    }

                    let mut show = false;
                    let w = last.to_lowercase();

                    // going too deep...
                    if let Some(aroom) = self.active_room.clone() {
                        if let Some(r) = self.rooms.get(&aroom) {
                            for (_, m) in r.members.iter() {
                                let alias = m.alias.clone().unwrap_or_default();
                                if alias.to_lowercase().starts_with(&w) {
                                    let widget;
                                    {
                                        let mb = widgets::MemberBox::new(&m, &self);
                                        widget = mb.widget(true);
                                    }

                                    widget.connect_button_press_event(clone!(w, alias, msg_entry, popover => move |_, _| {
                                        let mut pos = msg_entry.get_position();
                                        msg_entry.delete_text(pos - w.len() as i32, -1);
                                        msg_entry.insert_text(&alias, &mut pos);
                                        msg_entry.set_position(pos);
                                        popover.popdown();
                                        glib::signal::Inhibit(true)
                                    }));

                                    listbox.add(&widget);
                                    show = true;
                                }
                            }
                        }
                    }

                    popover.set_relative_to(Some(&msg_entry));
                    popover.set_modal(false);
                    if show {
                        popover.popup();
                    } else {
                        popover.popdown();
                    }
                }
            }
        };
    }
}

/// State for the main thread.
///
/// It takes care of starting up the application and for loading and accessing the
/// UI.
pub struct App {
    /// Used to access the UI elements.
    gtk_builder: gtk::Builder,

    op: Arc<Mutex<AppOp>>,
}

impl App {
    /// Create an App instance
    pub fn new() {
        let gtk_app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::empty())
            .expect("Failed to initialize GtkApplication");

        gtk_app.set_accels_for_action("app.quit", &["<Ctrl>Q"]);

        gtk_app.connect_startup(move |gtk_app| {
            let (tx, rx): (Sender<BKResponse>, Receiver<BKResponse>) = channel();
            let (itx, irx): (Sender<InternalCommand>, Receiver<InternalCommand>) = channel();

            let bk = Backend::new(tx);
            let apptx = bk.run();

            let gtk_builder = gtk::Builder::new_from_resource("/org/gnome/Fractal/main_window.glade");
            let window: gtk::Window = gtk_builder
                .get_object("main_window")
                .expect("Couldn't find main_window in ui file.");
            window.set_application(gtk_app);

            /* we have to overwrite the default behavior for valign of the title widget
             * since it is force to be centered */
            gtk_builder
            .get_object::<gtk::MenuButton>("room_menu_button")
            .expect("Can't find back_button in ui file.").set_valign(gtk::Align::Fill);

            let op = Arc::new(Mutex::new(
                AppOp::new(gtk_app.clone(), gtk_builder.clone(), apptx, itx)
            ));

            unsafe {
                OP = Some(op.clone());
            }

            backend_loop(rx);
            appop_loop(irx);

            let app = App {
                gtk_builder: gtk_builder,
                op: op.clone(),
            };

            gtk_app.connect_activate(move |_| { op.lock().unwrap().activate() });

            app.connect_gtk();
            app.run();
        });

        gtk_app.run(&[]);
    }

    pub fn connect_gtk(&self) {
        // Set up shutdown callback
        let window: gtk::Window = self.gtk_builder
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
        let chat: gtk::Widget = self.gtk_builder
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

        self.create_load_more_btn();
        self.connect_more_members_btn();
        self.create_actions();

        self.connect_headerbars();
        self.connect_login_view();

        self.connect_msg_scroll();

        self.connect_send();
        self.connect_attach();

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

    fn create_actions(&self) {
        let settings = gio::SimpleAction::new("settings", None);
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

    fn connect_headerbars(&self) {
        let op = self.op.clone();
        let btn = self.gtk_builder
            .get_object::<gtk::Button>("back_button")
            .expect("Can't find back_button in ui file.");
        btn.connect_clicked(move |_| {
            op.lock().unwrap().set_state(AppState::Chat);
        });
    }

    fn connect_leave_room_dialog(&self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("leave_room_dialog")
            .expect("Can't find leave_room_dialog in ui file.");
        let cancel = self.gtk_builder
            .get_object::<gtk::Button>("leave_room_cancel")
            .expect("Can't find leave_room_cancel in ui file.");
        let confirm = self.gtk_builder
            .get_object::<gtk::Button>("leave_room_confirm")
            .expect("Can't find leave_room_confirm in ui file.");

        cancel.connect_clicked(clone!(dialog => move |_| {
            dialog.hide();
        }));

        let op = self.op.clone();
        confirm.connect_clicked(clone!(dialog => move |_| {
            dialog.hide();
            op.lock().unwrap().really_leave_active_room();
        }));
    }

    fn connect_new_room_dialog(&self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("new_room_dialog")
            .expect("Can't find new_room_dialog in ui file.");
        let cancel = self.gtk_builder
            .get_object::<gtk::Button>("cancel_new_room")
            .expect("Can't find cancel_new_room in ui file.");
        let confirm = self.gtk_builder
            .get_object::<gtk::Button>("new_room_button")
            .expect("Can't find new_room_button in ui file.");
        let entry = self.gtk_builder
            .get_object::<gtk::Entry>("new_room_name")
            .expect("Can't find new_room_name in ui file.");

        cancel.connect_clicked(clone!(entry, dialog => move |_| {
            dialog.hide();
            entry.set_text("");
        }));

        let op = self.op.clone();
        confirm.connect_clicked(clone!(entry, dialog => move |_| {
            dialog.hide();
            op.lock().unwrap().create_new_room();
            entry.set_text("");
        }));

        let op = self.op.clone();
        entry.connect_activate(clone!(dialog => move |entry| {
            dialog.hide();
            op.lock().unwrap().create_new_room();
            entry.set_text("");
        }));
    }

    fn connect_join_room_dialog(&self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("join_room_dialog")
            .expect("Can't find join_room_dialog in ui file.");
        let cancel = self.gtk_builder
            .get_object::<gtk::Button>("cancel_join_room")
            .expect("Can't find cancel_join_room in ui file.");
        let confirm = self.gtk_builder
            .get_object::<gtk::Button>("join_room_button")
            .expect("Can't find join_room_button in ui file.");
        let entry = self.gtk_builder
            .get_object::<gtk::Entry>("join_room_name")
            .expect("Can't find join_room_name in ui file.");

        cancel.connect_clicked(clone!(entry, dialog => move |_| {
            dialog.hide();
            entry.set_text("");
        }));

        let op = self.op.clone();
        confirm.connect_clicked(clone!(entry, dialog => move |_| {
            dialog.hide();
            op.lock().unwrap().join_to_room();
            entry.set_text("");
        }));

        let op = self.op.clone();
        entry.connect_activate(clone!(dialog => move |entry| {
            dialog.hide();
            op.lock().unwrap().join_to_room();
            entry.set_text("");
        }));
    }

    fn connect_room_config(&self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("room_config_dialog")
            .expect("Can't find room_config_dialog in ui file.");
        let btn = self.gtk_builder
            .get_object::<gtk::Button>("room_dialog_close")
            .expect("Can't find room_dialog_close in ui file.");
        btn.connect_clicked(clone!(dialog => move |_| {
            dialog.hide();
        }));

        let avatar = self.gtk_builder
            .get_object::<gtk::Image>("room_avatar_image")
            .expect("Can't find room_avatar_image in ui file.");
        let avatar_btn = self.gtk_builder
            .get_object::<gtk::Button>("room_avatar_filechooser")
            .expect("Can't find room_avatar_filechooser in ui file.");
        let avatar_fs = self.gtk_builder
            .get_object::<gtk::FileChooserDialog>("file_chooser_dialog")
            .expect("Can't find file_chooser_dialog in ui file.");

        let fs_set = self.gtk_builder
            .get_object::<gtk::Button>("file_chooser_set")
            .expect("Can't find file_chooser_set in ui file.");
        let fs_cancel = self.gtk_builder
            .get_object::<gtk::Button>("file_chooser_cancel")
            .expect("Can't find file_chooser_cancel in ui file.");
        let fs_preview = self.gtk_builder
            .get_object::<gtk::Image>("file_chooser_preview")
            .expect("Can't find file_chooser_preview in ui file.");

        fs_cancel.connect_clicked(clone!(avatar_fs => move |_| {
            avatar_fs.hide();
        }));

        fs_set.connect_clicked(clone!(avatar_fs, avatar => move |_| {
            avatar_fs.hide();
            if let Some(fname) = avatar_fs.get_filename() {
                if let Some(name) = fname.to_str() {
                    if let Ok(pixbuf) = Pixbuf::new_from_file_at_size(name, 100, 100) {
                        avatar.set_from_pixbuf(&pixbuf);
                    } else {
                        avatar.set_from_icon_name("image-missing", 5);
                    }
                }
            }
        }));

        avatar_fs.connect_selection_changed(move |fs| {
            if let Some(fname) = fs.get_filename() {
                if let Some(name) = fname.to_str() {
                    if let Ok(pixbuf) = Pixbuf::new_from_file_at_size(name, 100, 100) {
                        fs_preview.set_from_pixbuf(&pixbuf);
                    }
                }
            }
        });

        avatar_btn.connect_clicked(clone!(avatar_fs => move |_| {
            avatar_fs.present();
        }));

        let btn = self.gtk_builder
            .get_object::<gtk::Button>("room_dialog_set")
            .expect("Can't find room_dialog_set in ui file.");
        let op = self.op.clone();
        btn.connect_clicked(clone!(dialog => move |_| {
            op.lock().unwrap().change_room_config();
            dialog.hide();
        }));
    }

    fn connect_directory(&self) {
        let btn = self.gtk_builder
            .get_object::<gtk::Button>("directory_search_button")
            .expect("Can't find directory_search_button in ui file.");
        let q = self.gtk_builder
            .get_object::<gtk::Entry>("directory_search_entry")
            .expect("Can't find directory_search_entry in ui file.");

        let scroll = self.gtk_builder
            .get_object::<gtk::ScrolledWindow>("directory_scroll")
            .expect("Can't find directory_scroll in ui file.");

        let mut op = self.op.clone();
        btn.connect_clicked(move |_| { op.lock().unwrap().search_rooms(false); });

        op = self.op.clone();
        scroll.connect_edge_reached(move |_, dir| if dir == gtk::PositionType::Bottom {
            op.lock().unwrap().load_more_rooms();
        });

        op = self.op.clone();
        q.connect_activate(move |_| { op.lock().unwrap().search_rooms(false); });
    }

    fn create_load_more_btn(&self) {
        let messages = self.gtk_builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        let btn = self.op.lock().unwrap().load_more_btn.clone();
        btn.set_halign(gtk::Align::Center);
        btn.set_margin_top (12);
        btn.set_margin_bottom (12);
        btn.show();
        row.add(&btn);
        row.show();
        messages.add(&row);

        let op = self.op.clone();
        btn.connect_clicked(move |_| { op.lock().unwrap().load_more_messages(); });
    }

    fn connect_more_members_btn(&self) {
        let mlist: gtk::ListBox = self.gtk_builder
            .get_object("member_list")
            .expect("Couldn't find member_list in ui file.");

        let btn = self.op.lock().unwrap().more_members_btn.clone();
        btn.show();
        let op = self.op.clone();
        btn.connect_clicked(move |_| {
            op.lock().unwrap().member_limit += 50;
            op.lock().unwrap().show_all_members();
        });
        mlist.add(&btn);
    }

    fn connect_msg_scroll(&self) {
        let s = self.gtk_builder
            .get_object::<gtk::ScrolledWindow>("messages_scroll")
            .expect("Can't find message_scroll in ui file.");

        let op = self.op.clone();
        s.connect_edge_overshot(move |_, dir| if dir == gtk::PositionType::Top {
            op.lock().unwrap().load_more_messages();
        });

        let op = self.op.clone();
        if let Some(adj) = s.get_vadjustment() {
            adj.connect_changed(move |adj| {
                let bottom = adj.get_upper() - adj.get_page_size();
                if adj.get_value() == bottom {
                    op.lock().unwrap().autoscroll = true;
                } else {
                    op.lock().unwrap().autoscroll = false;
                }
            });
        }
    }

    fn connect_send(&self) {
        let msg_entry: gtk::Entry = self.gtk_builder
            .get_object("msg_entry")
            .expect("Couldn't find msg_entry in ui file.");

        let mut op = self.op.clone();
        msg_entry.connect_activate(move |entry| if let Some(text) = entry.get_text() {
            op.lock().unwrap().send_message(text);
            entry.set_text("");
        });

        op = self.op.clone();
        msg_entry.connect_paste_clipboard(move |_| {
            op.lock().unwrap().paste();
        });

        op = self.op.clone();
        msg_entry.connect_key_press_event(move |_, ev| {
            if ev.get_keyval() == 65289 {
                op.lock().unwrap().autocomplete_tab(ev);
                return Inhibit(true);
            }
            Inhibit(false)
        });

        op = self.op.clone();
        msg_entry.connect_key_release_event(move |e, _| {
            op.lock().unwrap().autocomplete(e.get_text());
            Inhibit(false)
        });
    }

    fn connect_attach(&self) {
        let attach_button: gtk::Button = self.gtk_builder
            .get_object("attach_button")
            .expect("Couldn't find attach_button in ui file.");

        let op = self.op.clone();
        attach_button.connect_clicked(move |_| {
            op.lock().unwrap().attach_file();
        });
    }

    fn connect_login_view(&self) {
        let advbtn: gtk::Button = self.gtk_builder
            .get_object("login_advanced_button")
            .expect("Couldn't find login_advanced_button in ui file.");
        let adv: gtk::Revealer = self.gtk_builder
            .get_object("login_advanced")
            .expect("Couldn't find login_advanced in ui file.");
        advbtn.connect_clicked(move |_| {
            adv.set_reveal_child(!adv.get_child_revealed());
        });

        self.connect_login_button();
        self.set_login_focus_chain();
    }

    fn set_login_focus_chain(&self) {
        let focus_chain = [
            "login_username",
            "login_password",
            "login_button",
            "login_advanced_button",
            "login_server",
            "login_idp",
        ];

        let mut v: Vec<gtk::Widget> = vec![];
        for i in focus_chain.iter() {
            let w = self.gtk_builder.get_object(i).expect("Couldn't find widget");
            v.push(w);
        }

        let grid: gtk::Grid = self.gtk_builder.get_object("login_grid")
            .expect("Couldn't find login_grid widget");
        grid.set_focus_chain(&v);
    }

    fn connect_search(&self) {
        let input: gtk::Entry = self.gtk_builder
            .get_object("search_input")
            .expect("Couldn't find search_input in ui file.");

        let btn: gtk::Button = self.gtk_builder
            .get_object("search")
            .expect("Couldn't find search in ui file.");

        let op = self.op.clone();
        input.connect_activate(move |inp| op.lock().unwrap().search(inp.get_text()));
        let op = self.op.clone();
        btn.connect_clicked(move |_| op.lock().unwrap().search(input.get_text()));
    }

    fn connect_member_search(&self) {
        let input: gtk::SearchEntry = self.gtk_builder
            .get_object("members_search")
            .expect("Couldn't find members_searcn in ui file.");

        let op = self.op.clone();
        input.connect_search_changed(move |_| {
            op.lock().unwrap().show_all_members();
        });
    }

    fn connect_login_button(&self) {
        // Login click
        let btn: gtk::Button = self.gtk_builder
            .get_object("login_button")
            .expect("Couldn't find login_button in ui file.");
        let username: gtk::Entry = self.gtk_builder
            .get_object("login_username")
            .expect("Couldn't find login_username in ui file.");
        let password: gtk::Entry = self.gtk_builder
            .get_object("login_password")
            .expect("Couldn't find login_password in ui file.");

        let op = self.op.clone();
        btn.connect_clicked(move |_| op.lock().unwrap().login());
        let op = self.op.clone();
        username.connect_activate(move |_| op.lock().unwrap().login());
        let op = self.op.clone();
        password.connect_activate(move |_| op.lock().unwrap().login());

        self.gtk_builder
            .get_object::<gtk::Label>("login_error_msg")
            .expect("Can't find login_error_msg in ui file.").hide();
    }

    fn connect_invite_dialog(&self) {
        let op = self.op.clone();
        let dialog = self.gtk_builder
            .get_object::<gtk::MessageDialog>("invite_dialog")
            .expect("Can't find invite_dialog in ui file.");
        let accept = self.gtk_builder
            .get_object::<gtk::Button>("invite_accept")
            .expect("Can't find invite_accept in ui file.");
        let reject = self.gtk_builder
            .get_object::<gtk::Button>("invite_reject")
            .expect("Can't find invite_reject in ui file.");

        reject.connect_clicked(clone!(dialog, op => move |_| {
            op.lock().unwrap().accept_inv(false);
            dialog.hide();
        }));

        accept.connect_clicked(clone!(dialog, op => move |_| {
            op.lock().unwrap().accept_inv(true);
            dialog.hide();
        }));
    }

    fn connect_invite_user(&self) {
        let op = &self.op;

        let cancel = self.gtk_builder
            .get_object::<gtk::Button>("cancel_invite")
            .expect("Can't find cancel_invite in ui file.");
        let invite = self.gtk_builder
            .get_object::<gtk::Button>("invite_button")
            .expect("Can't find invite_button in ui file.");
        let entry = self.gtk_builder
            .get_object::<gtk::Entry>("invite_entry")
            .expect("Can't find invite_entry in ui file.");

        // this is used to cancel the timeout and not search for every key input. We'll wait 500ms
        // without key release event to launch the search
        let source_id: Arc<Mutex<Option<glib::source::SourceId>>> = Arc::new(Mutex::new(None));
        entry.connect_key_release_event(clone!(op => move |entry, _| {
            {
                let mut id = source_id.lock().unwrap();
                if let Some(sid) = id.take() {
                    glib::source::source_remove(sid);
                }
            }

            let sid = gtk::timeout_add(500, clone!(op, entry, source_id => move || {
                op.lock().unwrap().search_invite_user(entry.get_text());
                *(source_id.lock().unwrap()) = None;
                gtk::Continue(false)
            }));

            *(source_id.lock().unwrap()) = Some(sid);
            glib::signal::Inhibit(false)
        }));

        cancel.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().close_invite_dialog();
        }));
        invite.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().invite();
        }));
    }

    fn connect_direct_chat(&self) {
        let op = &self.op;

        let cancel = self.gtk_builder
            .get_object::<gtk::Button>("cancel_direct_chat")
            .expect("Can't find cancel_direct_chat in ui file.");
        let invite = self.gtk_builder
            .get_object::<gtk::Button>("direct_chat_button")
            .expect("Can't find direct_chat_button in ui file.");
        let entry = self.gtk_builder
            .get_object::<gtk::Entry>("to_chat_entry")
            .expect("Can't find to_chat_entry in ui file.");

        // this is used to cancel the timeout and not search for every key input. We'll wait 500ms
        // without key release event to launch the search
        let source_id: Arc<Mutex<Option<glib::source::SourceId>>> = Arc::new(Mutex::new(None));
        entry.connect_key_release_event(clone!(op => move |entry, _| {
            {
                let mut id = source_id.lock().unwrap();
                if let Some(sid) = id.take() {
                    glib::source::source_remove(sid);
                }
            }

            let sid = gtk::timeout_add(500, clone!(op, entry, source_id => move || {
                op.lock().unwrap().search_invite_user(entry.get_text());
                *(source_id.lock().unwrap()) = None;
                gtk::Continue(false)
            }));

            *(source_id.lock().unwrap()) = Some(sid);
            glib::signal::Inhibit(false)
        }));

        cancel.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().close_direct_chat_dialog();
        }));
        invite.connect_clicked(clone!(op => move |_| {
            op.lock().unwrap().start_chat();
        }));
    }

    pub fn connect_roomlist_search(&self) {
        let op = &self.op;

        let search_btn = self.gtk_builder
            .get_object::<gtk::ToggleButton>("room_search_button")
            .expect("Can't find room_search_button in ui file.");
        let search_bar = self.gtk_builder
            .get_object::<gtk::SearchBar>("room_list_searchbar")
            .expect("Can't find room_list_searchbar in ui file.");
        let search_entry = self.gtk_builder
            .get_object::<gtk::SearchEntry>("room_list_search")
            .expect("Can't find room_list_search in ui file.");

        search_btn.connect_toggled(clone!(search_bar => move |btn| {
            search_bar.set_search_mode(btn.get_active());
        }));

        search_bar.connect_property_search_mode_enabled_notify(clone!(search_btn => move |bar| {
            search_btn.set_active(bar.get_search_mode());
        }));

        search_entry.connect_search_changed(clone!(op => move |entry| {
            op.lock().unwrap().filter_rooms(entry.get_text());
        }));

        // hidding left and right boxes to align with top buttons
        let boxes = search_bar.get_children()[0].clone().downcast::<gtk::Revealer>().unwrap() // revealer
                              .get_children()[0].clone().downcast::<gtk::Box>().unwrap(); // box
        boxes.get_children()[0].clone().downcast::<gtk::Box>().unwrap().hide();
        boxes.get_children()[1].clone().set_hexpand(true);
        boxes.get_children()[1].clone().set_halign(gtk::Align::Fill);
        boxes.get_children()[2].clone().downcast::<gtk::Box>().unwrap().hide();
    }

    pub fn run(&self) {
        self.op.lock().unwrap().init();

        glib::set_application_name("fractal");
        glib::set_prgname(Some("fractal"));

        let provider = gtk::CssProvider::new();
        provider.load_from_resource("/org/gnome/Fractal/app.css");
        gtk::StyleContext::add_provider_for_screen(&gdk::Screen::get_default().unwrap(), &provider, 600);
    }
}

fn backend_loop(rx: Receiver<BKResponse>) {
    thread::spawn(move || {
        let mut shutting_down = false;
        loop {
            let recv = rx.recv();

            if let Err(RecvError) = recv {
                // stopping this backend loop thread
                break;
            }

            if shutting_down {
                // ignore this event, we're shutting down this thread
                continue;
            }

            match recv {
                Err(RecvError) => { break; }
                Ok(BKResponse::ShutDown) => { shutting_down = true; }
                Ok(BKResponse::Token(uid, tk)) => {
                    APPOP!(bk_login, (uid, tk));

                    // after login
                    APPOP!(sync);
                }
                Ok(BKResponse::Logout) => {
                    APPOP!(bk_logout);
                }
                Ok(BKResponse::Name(username)) => {
                    let u = Some(username);
                    APPOP!(set_username, (u));
                }
                Ok(BKResponse::Avatar(path)) => {
                    let av = Some(path);
                    APPOP!(set_avatar, (av));
                }
                Ok(BKResponse::Sync(since)) => {
                    println!("SYNC");
                    let s = Some(since);
                    APPOP!(synced, (s));
                }
                Ok(BKResponse::Rooms(rooms, default)) => {
                    APPOP!(update_rooms, (rooms, default));
                }
                Ok(BKResponse::NewRooms(rooms)) => {
                    APPOP!(new_rooms, (rooms));
                }
                Ok(BKResponse::RoomDetail(room, key, value)) => {
                    let v = Some(value);
                    APPOP!(set_room_detail, (room, key, v));
                }
                Ok(BKResponse::RoomAvatar(room, avatar)) => {
                    let a = Some(avatar);
                    APPOP!(set_room_avatar, (room, a));
                }
                Ok(BKResponse::RoomMembers(members)) => {
                    APPOP!(set_room_members, (members));
                }
                Ok(BKResponse::RoomMessages(msgs)) => {
                    let init = false;
                    APPOP!(show_room_messages, (msgs, init));
                }
                Ok(BKResponse::RoomMessagesInit(msgs)) => {
                    let init = true;
                    APPOP!(show_room_messages, (msgs, init));
                }
                Ok(BKResponse::RoomMessagesTo(msgs)) => {
                    APPOP!(show_room_messages_top, (msgs));
                }
                Ok(BKResponse::SendMsg) => {
                    APPOP!(sync);
                }
                Ok(BKResponse::DirectoryProtocols(protocols)) => {
                    APPOP!(set_protocols, (protocols));
                }
                Ok(BKResponse::DirectorySearch(rooms)) => {
                    if rooms.len() == 0 {
                        let error = "No rooms found".to_string();
                        APPOP!(show_error, (error));
                        APPOP!(enable_directory_search);
                    }

                    for room in rooms {
                        APPOP!(set_directory_room, (room));
                    }
                }
                Ok(BKResponse::JoinRoom) => {
                    APPOP!(reload_rooms);
                }
                Ok(BKResponse::LeaveRoom) => { }
                Ok(BKResponse::SetRoomName) => { }
                Ok(BKResponse::SetRoomTopic) => { }
                Ok(BKResponse::SetRoomAvatar) => { }
                Ok(BKResponse::MarkedAsRead(r, _)) => {
                    APPOP!(clear_room_notifications, (r));
                }
                Ok(BKResponse::RoomNotifications(r, n, h)) => {
                    APPOP!(set_room_notifications, (r, n, h));
                }

                Ok(BKResponse::RoomName(roomid, name)) => {
                    let n = Some(name);
                    APPOP!(room_name_change, (roomid, n));
                }
                Ok(BKResponse::RoomTopic(roomid, topic)) => {
                    let t = Some(topic);
                    APPOP!(room_topic_change, (roomid, t));
                }
                Ok(BKResponse::NewRoomAvatar(roomid)) => {
                    APPOP!(new_room_avatar, (roomid));
                }
                Ok(BKResponse::RoomMemberEvent(ev)) => {
                    APPOP!(room_member_event, (ev));
                }
                Ok(BKResponse::Media(fname)) => {
                    Command::new("xdg-open")
                                .arg(&fname)
                                .spawn()
                                .expect("failed to execute process");
                }
                Ok(BKResponse::AttachedFile(msg)) => {
                    APPOP!(add_tmp_room_message, (msg));
                }
                Ok(BKResponse::SearchEnd) => {
                    APPOP!(search_end);
                }
                Ok(BKResponse::NewRoom(r, internal_id)) => {
                    let id = Some(internal_id);
                    APPOP!(new_room, (r, id));
                }
                Ok(BKResponse::AddedToFav(r, tofav)) => {
                    APPOP!(added_to_fav, (r, tofav));
                }
                Ok(BKResponse::UserSearch(users)) => {
                    APPOP!(user_search_finished, (users));
                }

                // errors
                Ok(BKResponse::NewRoomError(err, internal_id)) => {
                    println!("ERROR: {:?}", err);

                    let error = "Can't create the room, try again".to_string();
                    let panel = RoomPanel::NoRoom;
                    APPOP!(remove_room, (internal_id));
                    APPOP!(show_error, (error));
                    APPOP!(room_panel, (panel));
                },
                Ok(BKResponse::JoinRoomError(err)) => {
                    println!("ERROR: {:?}", err);
                    let error = format!("Can't join to the room, try again.");
                    let panel = RoomPanel::NoRoom;
                    APPOP!(show_error, (error));
                    APPOP!(room_panel, (panel));
                },
                Ok(BKResponse::LoginError(_)) => {
                    let error = "Can't login, try again".to_string();
                    let st = AppState::Login;
                    APPOP!(show_error, (error));
                    APPOP!(set_state, (st));
                },
                Ok(BKResponse::SendMsgError(_)) => {
                    let error = "Error sending message".to_string();
                    APPOP!(show_error, (error));
                }
                Ok(BKResponse::DirectoryError(_)) => {
                    let error = "Error searching for rooms".to_string();
                    APPOP!(show_error, (error));
                    APPOP!(enable_directory_search);
                }
                Ok(BKResponse::SyncError(err)) => {
                    println!("SYNC Error: {:?}", err);
                    APPOP!(sync_error);
                }
                Ok(err) => {
                    println!("Query error: {:?}", err);
                }
            };
        }
    });
}


#[derive(Debug)]
pub enum InternalCommand {
    AddRoomMessage(Message, MsgPos, Option<Message>, bool, bool),
    SetPanel(RoomPanel),
    NotifyClicked(Message),
    SelectRoom(Room),
    LoadMoreNormal,
    RemoveInv(String),

    ToInvite(Member),
    RmInvite(String),
}


fn appop_loop(rx: Receiver<InternalCommand>) {
    thread::spawn(move || {
        loop {
            let recv = rx.recv();
            match recv {
                Ok(InternalCommand::AddRoomMessage(msg, pos, prev, force_full, last)) => {
                    APPOP!(add_room_message, (msg, pos, prev, force_full, last));
                }
                Ok(InternalCommand::ToInvite(member)) => {
                    APPOP!(add_to_invite, (member));
                }
                Ok(InternalCommand::RmInvite(uid)) => {
                    APPOP!(rm_from_invite, (uid));
                }
                Ok(InternalCommand::SetPanel(st)) => {
                    APPOP!(room_panel, (st));
                }
                Ok(InternalCommand::NotifyClicked(msg)) => {
                    APPOP!(notification_cliked, (msg));
                }
                Ok(InternalCommand::SelectRoom(r)) => {
                    let id = r.id;
                    APPOP!(set_active_room_by_id, (id));
                }
                Ok(InternalCommand::LoadMoreNormal) => {
                    APPOP!(load_more_normal);
                }
                Ok(InternalCommand::RemoveInv(rid)) => {
                    APPOP!(remove_inv, (rid));
                }
                Err(_) => {
                    break;
                }
            };
        }
    });
}
