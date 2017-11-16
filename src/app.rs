extern crate gtk;
extern crate gdk_pixbuf;
extern crate secret_service;
extern crate chrono;
extern crate gdk;
extern crate notify_rust;
extern crate pango;

use self::notify_rust::Notification;

use util::*;
use self::chrono::prelude::*;

use self::secret_service::SecretService;
use self::secret_service::EncryptionType;

use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;
use std::sync::mpsc::{Sender, Receiver};
use std::collections::HashMap;
use std::process::Command;
use std::thread;

use gio::ApplicationExt;
use gio::SimpleActionExt;
use gio::ActionMapExt;
use glib;
use gio;
use self::gdk_pixbuf::Pixbuf;
use self::gtk::prelude::*;

use backend::Backend;
use backend::BKCommand;
use backend::BKResponse;
use backend;

use types::Member;
use types::MemberList;
use types::Message;
use types::Protocol;
use types::Room;
use types::RoomList;
use types::Event;

use widgets;
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

    pub syncing: bool,
    tmp_msgs: Vec<TmpMsg>,

    pub username: String,
    pub uid: String,

    pub active_room: String,
    pub members: MemberList,
    pub rooms: RoomList,
    pub load_more_btn: gtk::Button,

    pub state: AppState,
}

#[derive(Debug)]
pub enum MsgPos {
    Top,
    Bottom,
}

#[derive(Debug)]
pub enum RoomPanel {
    Room,
    NoRoom,
    Loading,
}


#[derive(Debug)]
pub enum AppState {
    Login,
    Chat,
    Directory,
    Loading,
}

impl AppOp {
    pub fn new(app: gtk::Application, builder: gtk::Builder, tx: Sender<BKCommand>) -> AppOp {
        AppOp {
            gtk_builder: builder,
            gtk_app: app,
            load_more_btn: gtk::Button::new_with_label("Load more messages"),
            backend: tx,
            active_room: String::from(""),
            members: HashMap::new(),
            rooms: HashMap::new(),
            username: String::new(),
            uid: String::new(),
            syncing: false,
            tmp_msgs: vec![],
            state: AppState::Login,
        }
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
    }

    pub fn login(&mut self) {
        self.set_state(AppState::Loading);

        let user_entry: gtk::Entry = self.gtk_builder
            .get_object("login_username")
            .expect("Can't find login_username in ui file.");
        let pass_entry: gtk::Entry = self.gtk_builder
            .get_object("login_password")
            .expect("Can't find login_password in ui file.");
        let server_entry: gtk::Entry = self.gtk_builder
            .get_object("login_server")
            .expect("Can't find login_server in ui file.");

        let username = match user_entry.get_text() {
            Some(s) => s,
            None => String::from(""),
        };

        let password = match pass_entry.get_text() {
            Some(s) => s,
            None => String::from(""),
        };

        self.connect(username, password, server_entry.get_text());
    }

    #[allow(dead_code)]
    pub fn register(&self) {
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
            self.show_error("Passwords didn't match, try again");
            return;
        }

        let server_url = match server_entry.get_text() {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        //self.store_pass(username.clone(), password.clone(), server_url.clone())
        //    .unwrap_or_else(|_| {
        //        // TODO: show an error
        //        println!("Error: Can't store the password using libsecret");
        //    });

        self.show_user_loading();
        let uname = username.clone();
        let pass = password.clone();
        let ser = server_url.clone();
        self.backend.send(BKCommand::Register(uname, pass, ser)).unwrap();
        self.hide_popup();
    }

    pub fn connect(&self, username: String, password: String, server: Option<String>) {
        let server_url = match server {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        self.store_pass(username.clone(), password.clone(), server_url.clone())
            .unwrap_or_else(|_| {
                // TODO: show an error
                println!("Error: Can't store the password using libsecret");
            });

        self.show_user_loading();
        let uname = username.clone();
        let pass = password.clone();
        let ser = server_url.clone();
        self.backend.send(BKCommand::Login(uname, pass, ser)).unwrap();
        self.hide_popup();
    }

    #[allow(dead_code)]
    pub fn connect_guest(&self, server: Option<String>) {
        let server_url = match server {
            Some(s) => s,
            None => String::from("https://matrix.org"),
        };

        self.show_user_loading();
        self.backend.send(BKCommand::Guest(server_url)).unwrap();
        self.hide_popup();
    }

    pub fn get_username(&self) {
        self.backend.send(BKCommand::GetUsername).unwrap();
        self.backend.send(BKCommand::GetAvatar).unwrap();
    }

    pub fn set_username(&mut self, username: &str) {
        //self.gtk_builder
        //    .get_object::<gtk::Label>("display_name_label")
        //    .expect("Can't find display_name_label in ui file.")
        //    .set_text(username);
        //self.show_username();
        self.username = String::from(username);
    }

    pub fn set_uid(&mut self, uid: &str) {
        self.uid = String::from(uid);
    }

    pub fn set_avatar(&self, fname: &str) {
        let image = self.gtk_builder
            .get_object::<gtk::Image>("profile_image")
            .expect("Can't find profile_image in ui file.");

        if let Ok(pixbuf) = Pixbuf::new_from_file_at_size(fname, 20, 20) {
            image.set_from_pixbuf(&pixbuf);
        } else {
            image.set_from_icon_name("image-missing", 2);
        }

        self.show_username();
    }

    pub fn show_username(&self) {
        //self.gtk_builder
        //    .get_object::<gtk::Stack>("user_button_stack")
        //    .expect("Can't find user_button_stack in ui file.")
        //    .set_visible_child_name("user_connected_page");
    }

    pub fn show_user_loading(&self) {
        //self.gtk_builder
        //    .get_object::<gtk::Stack>("user_button_stack")
        //    .expect("Can't find user_button_stack in ui file.")
        //    .set_visible_child_name("user_loading_page");
    }

    pub fn hide_popup(&self) {
        //let user_menu: gtk::Popover = self.gtk_builder
        //    .get_object("user_menu")
        //    .expect("Couldn't find user_menu in ui file.");
        //user_menu.hide();
    }

    pub fn disconnect(&self) {
        self.backend.send(BKCommand::ShutDown).unwrap();
    }

    pub fn logout(&self) {
        let _ = self.delete_pass();
        self.backend.send(BKCommand::Logout).unwrap();
    }

    pub fn delete_pass(&self) -> Result<(), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;

        // deleting previous items
        let allpass = collection.get_all_items()?;
        let passwds = allpass.iter()
            .filter(|x| x.get_label().unwrap_or(strn!("")) == "fractal");
        for p in passwds {
            p.delete()?;
        }

        Ok(())
    }

    pub fn store_pass(&self,
                      username: String,
                      password: String,
                      server: String)
                      -> Result<(), Error> {
        let ss = SecretService::new(EncryptionType::Dh)?;
        let collection = ss.get_default_collection()?;

        // deleting previous items
        self.delete_pass()?;

        // create new item
        collection.create_item(
            "fractal", // label
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

        let passwd = allpass.iter()
            .find(|x| x.get_label().unwrap_or(strn!("")) == "fractal");

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
            self.set_rooms(r, None);
            self.username = data.username;
            self.uid = data.uid;
        } else {
            self.set_state(AppState::Login);
        }

        if let Ok(pass) = self.get_pass() {
            self.connect(pass.0, pass.1, Some(pass.2));
        } else {
            self.set_state(AppState::Login);
        }
    }

    pub fn room_panel(&self, t: RoomPanel) {
        let s = self.gtk_builder
            .get_object::<gtk::Stack>("room_view_stack")
            .expect("Can't find room_view_stack in ui file.");
        let detail = self.gtk_builder
            .get_object::<gtk::Widget>("room_details_box")
            .expect("Can't find room_details_box in ui file.");

        let v = match t {
            RoomPanel::Loading => "loading",
            RoomPanel::Room => "room_view",
            RoomPanel::NoRoom => "noroom",
        };

        s.set_visible_child_name(v);

        match v {
            "noroom" => {
                detail.hide();
            },
            _ => {
                detail.show();
            }
        }
    }

    pub fn sync(&mut self) {
        if !self.syncing {
            self.syncing = true;
            self.backend.send(BKCommand::Sync).unwrap();
        }
    }

    pub fn set_rooms(&mut self, rooms: Vec<Room>, def: Option<Room>) {
        let store: gtk::TreeStore = self.gtk_builder
            .get_object("rooms_tree_store")
            .expect("Couldn't find rooms_tree_store in ui file.");

        let mut array: Vec<Room> = vec![];

        self.rooms.clear();
        store.clear();

        for r in rooms {
            self.rooms.insert(r.id.clone(), r.clone());
            array.push(r);
        }

        array.sort_by(|x, y| x.name.to_lowercase().cmp(&y.name.to_lowercase()));

        for v in array {
            let ns = match v.notifications {
                0 => String::new(),
                i => format!("{}", i),
            };

            store.insert_with_values(None, None, &[0, 1, 2], &[&v.name, &v.id, &ns]);
        }

        let mut godef = def;
        if !self.active_room.is_empty() {
            if let Some(r) = self.rooms.get(&self.active_room) {
                godef = Some(r.clone());
            }
        }

        if let Some(d) = godef {
            self.set_active_room(&d);
        } else {
            self.set_state(AppState::Chat);
            self.room_panel(RoomPanel::NoRoom);
            self.active_room = String::new();
        }

        self.cache_rooms();
    }

    pub fn cache_rooms(&self) {
        // serializing rooms
        if let Err(_) = cache::store(&self.rooms, self.username.clone(), self.uid.clone()) {
            println!("Error caching rooms");
        };
    }

    pub fn reload_rooms(&mut self) {
        self.set_state(AppState::Loading);
        self.backend.send(BKCommand::SyncForced).unwrap();
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
            self.set_active_room(&r);
        }
    }

    pub fn set_active_room(&mut self, room: &Room) {
        self.active_room = room.id.clone();

        self.remove_messages();

        let mut getmessages = true;
        for msg in room.messages.iter() {
            self.add_room_message(msg, MsgPos::Bottom);
        }
        if !room.messages.is_empty() {
            getmessages = false;
            if let Some(msg) = room.messages.iter().last() {
                self.scroll_down();
                self.mark_as_read(msg);
            }
        }

        // getting room details
        self.backend.send(BKCommand::SetRoom(room.clone())).unwrap();

        self.members.clear();
        let members = self.gtk_builder
            .get_object::<gtk::ListStore>("members_store")
            .expect("Can't find members_store in ui file.");
        members.clear();

        let name_label = self.gtk_builder
            .get_object::<gtk::Label>("room_name")
            .expect("Can't find room_name in ui file.");
        let edit = self.gtk_builder
            .get_object::<gtk::Entry>("room_name_entry")
            .expect("Can't find room_name_entry in ui file.");
        let topic_label = self.gtk_builder
            .get_object::<gtk::Label>("room_topic")
            .expect("Can't find room_topic in ui file.");

        topic_label.set_text(&room.topic);
        name_label.set_text(&room.name);
        edit.set_text(&room.name);

        self.set_current_room_avatar(room.avatar.clone());

        if getmessages {
            self.backend.send(BKCommand::GetRoomMessages(self.active_room.clone())).unwrap();
            self.room_panel(RoomPanel::Loading);
        } else {
            self.room_panel(RoomPanel::Room);
        }
    }

    pub fn set_room_detail(&mut self, roomid: String, key: String, value: String) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            let k: &str = &key;
            match k {
                "m.room.name" => { r.name = value.clone(); }
                "m.room.topic" => { r.topic = value.clone(); }
                _ => {}
            };
        }

        if roomid == self.active_room {
            self.set_current_room_detail(key, value);
        }
    }

    pub fn set_room_avatar(&mut self, roomid: String, avatar: String) {
        if let Some(r) = self.rooms.get_mut(&roomid) {
            r.avatar = avatar.clone();
        }

        if roomid == self.active_room {
            self.set_current_room_avatar(avatar);
        }
    }

    pub fn set_current_room_detail(&self, key: String, value: String) {
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
                let topic_label = self.gtk_builder
                    .get_object::<gtk::Label>("room_topic")
                    .expect("Can't find room_topic in ui file.");
                let edit = self.gtk_builder
                    .get_object::<gtk::Entry>("room_topic_entry")
                    .expect("Can't find room_topic_entry in ui file.");

                topic_label.set_tooltip_text(&value[..]);
                topic_label.set_text(&value);
                edit.set_text(&value);
            }
            _ => println!("no key {}", key),
        };
    }

    pub fn set_current_room_avatar(&self, avatar: String) {
        let image = self.gtk_builder
            .get_object::<gtk::Image>("room_image")
            .expect("Can't find room_image in ui file.");
        let config = self.gtk_builder
            .get_object::<gtk::Image>("room_avatar_image")
            .expect("Can't find room_avatar_image in ui file.");

        if !avatar.is_empty() {
            if let Ok(pixbuf) = Pixbuf::new_from_file_at_size(&avatar, 40, 40) {
                image.set_from_pixbuf(&pixbuf);
                config.set_from_pixbuf(&pixbuf);
            }
        } else {
            image.set_from_icon_name("image-missing", 5);
            config.set_from_icon_name("image-missing", 5);
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

    pub fn add_room_message(&mut self, msg: &Message, msgpos: MsgPos) {
        let messages = self.gtk_builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        let mut prev = None;
        if let Some(r) = self.rooms.get(&msg.room) {
            if let Some(pos) = r.messages.iter().position(|ref m| m.id == msg.id) {
                if pos > 0 {
                    if let Some(p) = r.messages.get(pos - 1) {
                        prev = Some(p.clone());
                    }
                }
            }
        }

        if msg.room == self.active_room {
            let m;
            {
                let mb = widgets::MessageBox::new(msg, &self);
                m = match prev {
                    Some(ref p) if p.sender == msg.sender => mb.small_widget(),
                    _ => mb.widget(),
                }
            }

            match msgpos {
                MsgPos::Bottom => messages.add(&m),
                MsgPos::Top => messages.insert(&m, 1),
            };
            self.remove_tmp_room_message(msg);
        } else {
            self.update_room_notifications(&msg.room, |n| n + 1);
        }
    }

    pub fn add_tmp_room_message(&mut self, msg: &Message) {
        let m;
        let messages = self.gtk_builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        {
            let mb = widgets::MessageBox::new(msg, &self);
            m = mb.widget();
        }

        messages.add(&m);
        if let Some(w) = messages.get_children().iter().last() {
            self.tmp_msgs.push(TmpMsg {
                    msg: msg.clone(),
                    widget: w.clone(),
            });

            self.scroll_down();
        };
    }

    pub fn remove_tmp_room_message(&mut self, msg: &Message) {
        let messages = self.gtk_builder
            .get_object::<gtk::ListBox>("message_list")
            .expect("Can't find message_list in ui file.");

        let mut rmidxs = vec![];

        for (i, t) in self.tmp_msgs.iter().enumerate() {
            if t.msg.sender == msg.sender &&
               t.msg.mtype == msg.mtype &&
               t.msg.room == msg.room &&
               t.msg.body == msg.body {

                messages.remove(&t.widget);
                //t.widget.destroy();
                rmidxs.push(i);
            }
        }

        for i in rmidxs {
            self.tmp_msgs.remove(i);
        }
    }

    pub fn update_room_notifications(&self, roomid: &str, f: fn(i32) -> i32) {
        let store: gtk::TreeStore = self.gtk_builder
            .get_object("rooms_tree_store")
            .expect("Couldn't find rooms_tree_store in ui file.");

        if let Some(iter) = store.get_iter_first() {
            loop {
                let v1 = store.get_value(&iter, 1);
                let id: &str = v1.get().unwrap();
                let v2 = store.get_value(&iter, 2);
                let ns: &str = v2.get().unwrap();
                let res: Result<i32, _> = ns.parse();
                let n: i32 = f(res.unwrap_or(0));
                let formatted = match n {
                    0 => String::from(""),
                    i => format!("{}", i),
                };
                if id == roomid {
                    store.set_value(&iter, 2, &gtk::Value::from(&formatted));
                }
                if !store.iter_next(&iter) {
                    break;
                }
            }
        }
    }

    pub fn mark_as_read(&self, msg: &Message) {
        self.backend.send(BKCommand::MarkAsRead(msg.room.clone(),
                                                msg.id.clone())).unwrap();
    }

    pub fn add_room_member(&mut self, m: Member) {
        let store: gtk::ListStore = self.gtk_builder
            .get_object("members_store")
            .expect("Couldn't find members_store in ui file.");

        let name = m.get_alias();

        // only show 200 members...
        if self.members.len() < 200 {
            store.insert_with_values(None, &[0, 1], &[&name, &(m.uid)]);
        }

        self.members.insert(m.uid.clone(), m);
    }

    pub fn send_message(&mut self, msg: String) {
        let room = self.active_room.clone();
        let now = Local::now();

        let m = Message {
            sender: self.uid.clone(),
            mtype: strn!("m.text"),
            body: msg.clone(),
            room: room.clone(),
            date: now,
            thumb: String::from(""),
            url: String::from(""),
            id: String::from(""),
        };

        self.add_tmp_room_message(&m);
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
        let room = self.active_room.clone();
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
        let room = self.active_room.clone();
        dialog.connect_file_activated(move |dialog| {
            if let Some(fname) = dialog.get_filename() {
                let f = strn!(fname.to_str().unwrap_or(""));
                backend.send(BKCommand::AttachFile(room.clone(), f)).unwrap();
            }
            dialog.destroy();
        });

        dialog.show();
    }

    pub fn load_more_messages(&self) {
        let room = self.active_room.clone();
        self.load_more_btn.set_label("loading...");
        self.backend.send(BKCommand::GetRoomMessagesTo(room)).unwrap();
    }

    pub fn load_more_normal(&self) {
        self.load_more_btn.set_label("load more messages");
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

        let btn = self.gtk_builder
            .get_object::<gtk::Button>("directory_search_button")
            .expect("Can't find directory_search_button in ui file.");
        btn.set_label("Search");
        btn.set_sensitive(true);
    }

    pub fn notify(&self, msg: &Message) {
        let roomname = match self.rooms.get(&msg.room) {
            Some(r) => r.name.clone(),
            None => msg.room.clone(),
        };

        let mut body = msg.body.clone();
        body.truncate(80);

        let (tx, rx): (Sender<(String, String)>, Receiver<(String, String)>) = channel();
        self.backend.send(BKCommand::GetUserInfoAsync(msg.sender.clone(), tx)).unwrap();
        let bk = self.backend.clone();
        let m = msg.clone();
        gtk::timeout_add(50, move || match rx.try_recv() {
            Err(_) => gtk::Continue(true),
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
                                    bk.send(BKCommand::NotifyClicked(m)).unwrap();
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

    pub fn show_room_messages(&mut self, msgs: Vec<Message>, init: bool) {
        for msg in msgs.iter() {
            if let Some(r) = self.rooms.get_mut(&msg.room) {
                r.messages.push(msg.clone());
            }
        }

        for msg in msgs.iter() {
            let mut should_notify = msg.body.contains(&self.username);
            // not notifying the initial messages
            should_notify = should_notify && !init;
            // not notifying my own messages
            should_notify = should_notify && (msg.sender != self.uid);

            if should_notify {
                self.notify(msg);
            }

            self.add_room_message(msg, MsgPos::Bottom);
        }

        if !msgs.is_empty() {
            let fs = msgs.iter().filter(|x| x.room == self.active_room);
            if let Some(msg) = fs.last() {
                self.scroll_down();
                self.mark_as_read(msg);
            }
        }

        if init {
            self.room_panel(RoomPanel::Room);
        }
    }

    pub fn show_room_messages_top(&mut self, msgs: Vec<Message>) {
        for msg in msgs.iter().rev() {
            if let Some(r) = self.rooms.get_mut(&msg.room) {
                r.messages.insert(0, msg.clone());
            }
        }

        for msg in msgs.iter().rev() {
            self.add_room_message(msg, MsgPos::Top);
        }

        self.load_more_normal();
    }

    pub fn show_room_dialog(&self) {
        let dialog = self.gtk_builder
            .get_object::<gtk::Dialog>("room_config_dialog")
            .expect("Can't find room_config_dialog in ui file.");

        dialog.show();
    }

    pub fn leave_active_room(&mut self) {
        let r = self.active_room.clone();
        self.backend.send(BKCommand::LeaveRoom(r.clone())).unwrap();
        self.rooms.remove(&r);
        self.active_room = String::new();
        self.room_panel(RoomPanel::NoRoom);

        let store: gtk::TreeStore = self.gtk_builder
            .get_object("rooms_tree_store")
            .expect("Couldn't find rooms_tree_store in ui file.");

        if let Some(iter) = store.get_iter_first() {
            loop {
                let v1 = store.get_value(&iter, 1);
                let id: &str = v1.get().unwrap();
                if id == r {
                    store.remove(&iter);
                }
                if !store.iter_next(&iter) {
                    break;
                }
            }
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
            .get_object::<gtk::FileChooserButton>("room_avatar_filechooser")
            .expect("Can't find room_avatar_filechooser in ui file.");

        if let Some(r) = self.rooms.get(&self.active_room) {
            if let Some(n) = name.get_text() {
                if n != r.name {
                    let command = BKCommand::SetRoomName(r.id.clone(), n.clone());
                    self.backend.send(command).unwrap();
                }
            }
            if let Some(t) = topic.get_text() {
                if t != r.topic {
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

    pub fn room_name_change(&mut self, roomid: String, name: String) {
        let store: gtk::TreeStore = self.gtk_builder
            .get_object("rooms_tree_store")
            .expect("Couldn't find rooms_tree_store in ui file.");

        {
            let r = self.rooms.get_mut(&roomid).unwrap();
            r.name = name.clone();
        }

        if roomid == self.active_room {
            self.gtk_builder
                .get_object::<gtk::Label>("room_name")
                .expect("Can't find room_name in ui file.")
                .set_text(&name);
        }

        if let Some(iter) = store.get_iter_first() {
            loop {
                let v1 = store.get_value(&iter, 1);
                let id: &str = v1.get().unwrap();
                if id == roomid {
                    store.set_value(&iter, 0, &gtk::Value::from(&name));
                }
                if !store.iter_next(&iter) {
                    break;
                }
            }
        }
    }

    pub fn room_topic_change(&mut self, roomid: String, topic: String) {
        {
            let r = self.rooms.get_mut(&roomid).unwrap();
            r.topic = topic.clone();
        }

        if roomid == self.active_room {
            let t = self.gtk_builder
                .get_object::<gtk::Label>("room_topic")
                .expect("Can't find room_topic in ui file.");

            t.set_tooltip_text(&topic[..]);
            t.set_text(&topic);
        }
    }

    pub fn new_room_avatar(&self, roomid: String) {
        self.backend.send(BKCommand::GetRoomAvatar(roomid)).unwrap();
    }

    pub fn room_member_event(&mut self, ev: Event) {
        // NOTE: maybe we should show this events in the message list to notify enters and leaves
        // to the user

        if ev.room != self.active_room {
            // if it's the current room, this event is not important for me
            return;
        }

        let store = self.gtk_builder
            .get_object::<gtk::ListStore>("members_store")
            .expect("Can't find members_store in ui file.");

        let sender = ev.sender.clone();
        match ev.content["membership"].as_str() {
            Some("leave") => {
                self.members.remove(&sender);
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
            }
            Some("join") => {
                let m = Member {
                    avatar: strn!(ev.content["avatar_url"].as_str().unwrap_or("")),
                    alias: strn!(ev.content["displayname"].as_str().unwrap_or("")),
                    uid: sender.clone(),
                };
                self.add_room_member(m);
            }
            Some(_) => {
                // ignoring other memberships
            }
            None => {}
        }
    }

    pub fn toggle_search(&self) {
        let r: gtk::Revealer = self.gtk_builder
            .get_object("search_revealer")
            .expect("Couldn't find search_revealer in ui file.");
        r.set_reveal_child(!r.get_child_revealed());
    }

    pub fn search(&mut self, term: Option<String>) {
        let r = self.active_room.clone();
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

    pub fn show_error(&self, msg: &str) {
        let window: gtk::Window = self.gtk_builder
            .get_object("main_window")
            .expect("Couldn't find main_window in ui file.");
        let dialog = gtk::MessageDialog::new(Some(&window),
                                             gtk::DIALOG_MODAL,
                                             gtk::MessageType::Warning,
                                             gtk::ButtonsType::Ok,
                                             msg);
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
            scaled = pixb.scale_simple(600, h*600/w, gdk_pixbuf::InterpType::Hyper);
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
                gtk::DIALOG_MODAL|
                gtk::DIALOG_USE_HEADER_BAR|
                gtk::DIALOG_DESTROY_WITH_PARENT,
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
                let room = self.active_room.clone();
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

    pub fn room_batch_end(&mut self, room: String, batch: String) {
        if let Some(r) = self.rooms.get_mut(&room) {
            r.batch_end = batch;
        }
    }

    pub fn notification_cliked(&mut self, msg: Message) {
        self.activate();
        let mut room = None;
        if let Some(r) = self.rooms.get(&msg.room) {
            room = Some(r.clone());
        }

        if let Some(r) = room {
            self.set_active_room(&r);
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

        gtk_app.connect_startup(move |gtk_app| {
            let (tx, rx): (Sender<BKResponse>, Receiver<BKResponse>) = channel();

            let bk = Backend::new(tx);
            let apptx = bk.run();

            let gtk_builder = gtk::Builder::new_from_resource("/org/gnome/fractal/main_window.glade");
            let window: gtk::Window = gtk_builder
                .get_object("main_window")
                .expect("Couldn't find main_window in ui file.");
            window.set_application(gtk_app);

            let op = Arc::new(Mutex::new(
                AppOp::new(gtk_app.clone(), gtk_builder.clone(), apptx)
            ));

            sync_loop(op.clone());
            backend_loop(op.clone(), rx);

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
        let pxbf = Pixbuf::new_from_resource("/org/gnome/fractal/org.gnome.Fractal.svg").unwrap();
        window.set_icon(&pxbf);
        window.show_all();

        let op = self.op.clone();
        window.connect_delete_event(move |_, _| {
            op.lock().unwrap().quit();
            Inhibit(false)
        });

        self.create_load_more_btn();
        self.create_actions();

        self.connect_headerbars();
        self.connect_login_view();

        self.connect_room_treeview();

        self.connect_msg_scroll();

        self.connect_send();
        self.connect_attach();

        self.connect_directory();
        self.connect_room_config();

        self.connect_search();
    }

    fn create_actions(&self) {
        let settings = gio::SimpleAction::new("settings", None);
        let dir = gio::SimpleAction::new("directory", None);
        let chat = gio::SimpleAction::new("start_chat", None);
        let logout = gio::SimpleAction::new("logout", None);

        let room = gio::SimpleAction::new("room_details", None);
        let search = gio::SimpleAction::new("search", None);
        let leave = gio::SimpleAction::new("leave_room", None);

        self.op.lock().unwrap().gtk_app.add_action(&settings);
        self.op.lock().unwrap().gtk_app.add_action(&dir);
        self.op.lock().unwrap().gtk_app.add_action(&chat);
        self.op.lock().unwrap().gtk_app.add_action(&logout);

        self.op.lock().unwrap().gtk_app.add_action(&room);
        self.op.lock().unwrap().gtk_app.add_action(&search);
        self.op.lock().unwrap().gtk_app.add_action(&leave);

        settings.connect_activate(move |_, _| { println!("SETTINGS"); });
        chat.connect_activate(move |_, _| { println!("START CHAT"); });
        settings.set_enabled(false);
        chat.set_enabled(false);

        let op = self.op.clone();
        dir.connect_activate(move |_, _| { op.lock().unwrap().set_state(AppState::Directory); });
        let op = self.op.clone();
        logout.connect_activate(move |_, _| { op.lock().unwrap().logout(); });

        let op = self.op.clone();
        room.connect_activate(move |_, _| { op.lock().unwrap().show_room_dialog(); });
        let op = self.op.clone();
        search.connect_activate(move |_, _| { op.lock().unwrap().toggle_search(); });
        let op = self.op.clone();
        leave.connect_activate(move |_, _| { op.lock().unwrap().leave_active_room(); });
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
        let avatar_fs = self.gtk_builder
            .get_object::<gtk::FileChooserButton>("room_avatar_filechooser")
            .expect("Can't find room_avatar_filechooser in ui file.");
        avatar_fs.connect_selection_changed(move |fs| {
            if let Some(fname) = fs.get_filename() {
                if let Some(name) = fname.to_str() {
                    if let Ok(pixbuf) = Pixbuf::new_from_file_at_size(name, 40, 40) {
                        avatar.set_from_pixbuf(&pixbuf);
                    } else {
                        avatar.set_from_icon_name("image-missing", 5);
                    }
                }
            }
        });

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

        let btn = self.op.lock().unwrap().load_more_btn.clone();
        btn.show();
        messages.add(&btn);

        let op = self.op.clone();
        btn.connect_clicked(move |_| { op.lock().unwrap().load_more_messages(); });
    }

    fn connect_msg_scroll(&self) {
        let s = self.gtk_builder
            .get_object::<gtk::ScrolledWindow>("messages_scroll")
            .expect("Can't find message_scroll in ui file.");

        let op = self.op.clone();
        s.connect_edge_overshot(move |_, dir| if dir == gtk::PositionType::Top {
            op.lock().unwrap().load_more_messages();
        });
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

    fn connect_login_button(&self) {
        // Login click
        let btn: gtk::Button = self.gtk_builder
            .get_object("login_button")
            .expect("Couldn't find login_button in ui file.");

        let op = self.op.clone();
        btn.connect_clicked(move |_| op.lock().unwrap().login());
    }

    fn connect_room_treeview(&self) {
        // room selection
        let treeview: gtk::TreeView = self.gtk_builder
            .get_object("rooms_tree_view")
            .expect("Couldn't find rooms_tree_view in ui file.");

        let op = self.op.clone();
        treeview.set_activate_on_single_click(true);
        treeview.connect_row_activated(move |view, path, _| {
            let iter = view.get_model().unwrap().get_iter(path).unwrap();
            let id = view.get_model().unwrap().get_value(&iter, 1);
            op.lock().unwrap().set_active_room_by_id(id.get().unwrap());
        });
    }

    pub fn run(&self) {
        self.op.lock().unwrap().init();

        glib::set_application_name("fractal");
        glib::set_prgname(Some("fractal"));

        let provider = gtk::CssProvider::new();
        provider.load_from_resource("/org/gnome/fractal/app.css");
        gtk::StyleContext::add_provider_for_screen(&gdk::Screen::get_default().unwrap(), &provider, 600);
    }
}

fn sync_loop(op: Arc<Mutex<AppOp>>) {
    // Sync loop every 3 seconds
    gtk::timeout_add(1000, move || {
        op.lock().unwrap().sync();
        gtk::Continue(true)
    });
}

fn backend_loop(op: Arc<Mutex<AppOp>>, rx: Receiver<BKResponse>) {
    gtk::timeout_add(500, move || {
        let recv = rx.try_recv();
        match recv {
            Ok(BKResponse::Token(uid, _)) => {
                op.lock().unwrap().set_state(AppState::Chat);
                op.lock().unwrap().set_uid(&uid);
                op.lock().unwrap().set_username(&uid);
                op.lock().unwrap().get_username();
                op.lock().unwrap().sync();

                op.lock().unwrap().init_protocols();
            }
            Ok(BKResponse::Logout) => {
                op.lock().unwrap().set_state(AppState::Login);
                op.lock().unwrap().set_uid("");
                op.lock().unwrap().set_username("");
            }
            Ok(BKResponse::Name(username)) => {
                op.lock().unwrap().set_username(&username);
            }
            Ok(BKResponse::Avatar(path)) => {
                op.lock().unwrap().set_avatar(&path);
            }
            Ok(BKResponse::Sync) => {
                println!("SYNC");
                op.lock().unwrap().syncing = false;
            }
            Ok(BKResponse::Rooms(rooms, default)) => {
                op.lock().unwrap().set_rooms(rooms, default);
            }
            Ok(BKResponse::RoomDetail(room, key, value)) => {
                op.lock().unwrap().set_room_detail(room, key, value);
            }
            Ok(BKResponse::RoomAvatar(room, avatar)) => {
                op.lock().unwrap().set_room_avatar(room, avatar);
            }
            Ok(BKResponse::RoomMessages(msgs)) => {
                op.lock().unwrap().show_room_messages(msgs, false);
            }
            Ok(BKResponse::RoomMessagesInit(msgs)) => {
                op.lock().unwrap().show_room_messages(msgs, true);
            }
            Ok(BKResponse::RoomMessagesTo(msgs)) => {
                op.lock().unwrap().show_room_messages_top(msgs);
            }
            Ok(BKResponse::RoomMembers(members)) => {
                let mut ms = members;
                ms.sort_by(|x, y| {
                    x.get_alias().to_lowercase().cmp(&y.get_alias().to_lowercase())
                });
                for m in ms {
                    op.lock().unwrap().add_room_member(m);
                }
            }
            Ok(BKResponse::RoomBatchEnd(roomid, batch)) => {
                op.lock().unwrap().room_batch_end(roomid, batch);
            }
            Ok(BKResponse::SendMsg) => {
                op.lock().unwrap().sync();
            }
            Ok(BKResponse::DirectoryProtocols(protocols)) => {
                op.lock().unwrap().set_protocols(protocols);
            }
            Ok(BKResponse::DirectorySearch(rooms)) => {
                for room in rooms {
                    op.lock().unwrap().set_directory_room(room);
                }
            }
            Ok(BKResponse::JoinRoom) => {
                op.lock().unwrap().reload_rooms();
            }
            Ok(BKResponse::LeaveRoom) => { }
            Ok(BKResponse::SetRoomName) => { }
            Ok(BKResponse::SetRoomTopic) => { }
            Ok(BKResponse::SetRoomAvatar) => { }
            Ok(BKResponse::MarkedAsRead(r, _)) => {
                op.lock().unwrap().update_room_notifications(&r, |_| 0);
            }

            Ok(BKResponse::RoomName(roomid, name)) => {
                op.lock().unwrap().room_name_change(roomid, name);
            }
            Ok(BKResponse::RoomTopic(roomid, topic)) => {
                op.lock().unwrap().room_topic_change(roomid, topic);
            }
            Ok(BKResponse::NewRoomAvatar(roomid)) => {
                op.lock().unwrap().new_room_avatar(roomid);
            }
            Ok(BKResponse::RoomMemberEvent(ev)) => {
                op.lock().unwrap().room_member_event(ev);
            }
            Ok(BKResponse::Media(fname)) => {
                Command::new("xdg-open")
                            .arg(&fname)
                            .spawn()
                            .expect("failed to execute process");
            }
            Ok(BKResponse::AttachedFile(msg)) => {
                op.lock().unwrap().add_tmp_room_message(&msg);
            }
            Ok(BKResponse::SearchEnd) => {
                op.lock().unwrap().search_end();
            }
            Ok(BKResponse::NotificationClicked(msg)) => {
                op.lock().unwrap().notification_cliked(msg);
            }

            // errors
            Ok(BKResponse::LoginError(_)) => {
                op.lock().unwrap().show_error("Can't login, try again");
                op.lock().unwrap().set_state(AppState::Login);
            },
            Ok(BKResponse::SyncError(_)) => {
                println!("SYNC Error");
                op.lock().unwrap().syncing = false;
            }
            Ok(err) => {
                println!("Query error: {:?}", err);
            }
            Err(_) => {}
        };

        gtk::Continue(true)
    });
}
