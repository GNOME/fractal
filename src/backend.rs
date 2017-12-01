extern crate url;
extern crate serde_json;
extern crate tree_magic;
extern crate chrono;

use self::chrono::prelude::*;

use self::serde_json::Value as JsonValue;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use self::url::Url;
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc::channel;
use std::sync::mpsc::RecvError;

use util::*;
use globals;
use error::Error;

use types::Message;
use types::Member;
use types::Protocol;
use types::Room;
use types::Event;

use std::fs::File;
use std::io::prelude::*;

use cache::CacheMap;


pub struct BackendData {
    user_id: String,
    access_token: String,
    server_url: String,
    since: String,
    msgid: i32,
    rooms_since: String,
    join_to_room: String,
}

pub struct Backend {
    tx: Sender<BKResponse>,
    data: Arc<Mutex<BackendData>>,
    internal_tx: Option<Sender<BKCommand>>,

    // user info cache, uid -> (name, avatar)
    user_info_cache: CacheMap<Arc<Mutex<(String, String)>>>,
}

#[derive(Debug)]
pub enum BKCommand {
    Login(String, String, String),
    Logout,
    #[allow(dead_code)]
    Register(String, String, String),
    #[allow(dead_code)]
    Guest(String),
    GetUsername,
    GetAvatar,
    Sync,
    SyncForced,
    GetRoomMessages(String),
    GetMessageContext(Message),
    GetRoomAvatar(String),
    GetThumbAsync(String, Sender<String>),
    GetAvatarAsync(Option<Member>, Sender<String>),
    GetMedia(String),
    GetUserInfoAsync(String, Sender<(String, String)>),
    SendMsg(Message),
    SetRoom(Room),
    ShutDown,
    DirectoryProtocols,
    DirectorySearch(String, String, bool),
    JoinRoom(String),
    MarkAsRead(String, String),
    LeaveRoom(String),
    SetRoomName(String, String),
    SetRoomTopic(String, String),
    SetRoomAvatar(String, String),
    AttachFile(String, String),
    AttachImage(String, Vec<u8>),
    Search(String, Option<String>),
    NotifyClicked(Message),
    NewRoom(String, RoomType),
}

#[derive(Debug)]
pub enum BKResponse {
    Token(String, String),
    Logout,
    Name(String),
    Avatar(String),
    Sync(String),
    Rooms(Vec<Room>, Option<Room>),
    RoomDetail(String, String, String),
    RoomAvatar(String, String),
    NewRoomAvatar(String),
    RoomMemberEvent(Event),
    RoomMessages(Vec<Message>),
    RoomMessagesInit(Vec<Message>),
    RoomMessagesTo(Vec<Message>),

    #[allow(dead_code)]
    RoomMembers(Vec<Member>),

    SendMsg,
    DirectoryProtocols(Vec<Protocol>),
    DirectorySearch(Vec<Room>),
    JoinRoom,
    LeaveRoom,
    MarkedAsRead(String, String),
    SetRoomName,
    SetRoomTopic,
    SetRoomAvatar,
    RoomName(String, String),
    RoomTopic(String, String),
    Media(String),
    AttachedFile(Message),
    SearchEnd,
    NotificationClicked(Message),
    NewRoom(Room),

    //errors
    UserNameError(Error),
    AvatarError(Error),
    LoginError(Error),
    LogoutError(Error),
    GuestLoginError(Error),
    SyncError(Error),
    RoomDetailError(Error),
    RoomAvatarError(Error),
    RoomMessagesError(Error),

    #[allow(dead_code)]
    RoomMembersError(Error),

    SendMsgError(Error),
    SetRoomError(Error),
    CommandError(Error),
    DirectoryError(Error),
    JoinRoomError(Error),
    MarkAsReadError(Error),
    LeaveRoomError(Error),
    SetRoomNameError(Error),
    SetRoomTopicError(Error),
    SetRoomAvatarError(Error),
    GetRoomAvatarError(Error),
    MediaError(Error),
    AttachFileError(Error),
    SearchError(Error),
    NewRoomError(Error),
}

#[derive(Debug)]
pub enum RoomType {
    Public,
    Private,
}


impl Backend {
    pub fn new(tx: Sender<BKResponse>) -> Backend {
        let data = BackendData {
            user_id: String::from("Guest"),
            access_token: String::from(""),
            server_url: String::from("https://matrix.org"),
            since: String::from(""),
            msgid: 1,
            rooms_since: String::from(""),
            join_to_room: String::from(""),
        };
        Backend {
            tx: tx,
            internal_tx: None,
            data: Arc::new(Mutex::new(data)),
            user_info_cache: CacheMap::new().timeout(120),
        }
    }

    fn get_base_url(&self) -> Result<Url, Error> {
        let s = self.data.lock().unwrap().server_url.clone();
        let url = Url::parse(&s)?;
        Ok(url)
    }

    fn url(&self, path: &str, params: Vec<(&str, String)>) -> Result<Url, Error> {
        let base = self.get_base_url()?;
        let tk = self.data.lock().unwrap().access_token.clone();

        let mut params2 = params.to_vec();
        params2.push(("access_token", tk.clone()));

        client_url!(&base, path, params2)
    }

    pub fn command_recv(&mut self, cmd: Result<BKCommand, RecvError>) -> bool {
        let tx = self.tx.clone();

        match cmd {
            Ok(BKCommand::Login(user, passwd, server)) => {
                let r = self.login(user, passwd, server);
                bkerror!(r, tx, BKResponse::LoginError);
            }
            Ok(BKCommand::Logout) => {
                let r = self.logout();
                bkerror!(r, tx, BKResponse::LogoutError);
            }
            Ok(BKCommand::Register(user, passwd, server)) => {
                let r = self.register(user, passwd, server);
                bkerror!(r, tx, BKResponse::LoginError);
            }
            Ok(BKCommand::Guest(server)) => {
                let r = self.guest(server);
                bkerror!(r, tx, BKResponse::GuestLoginError);
            }
            Ok(BKCommand::GetUsername) => {
                let r = self.get_username();
                bkerror!(r, tx, BKResponse::UserNameError);
            }
            Ok(BKCommand::GetAvatar) => {
                let r = self.get_avatar();
                bkerror!(r, tx, BKResponse::AvatarError);
            }
            Ok(BKCommand::Sync) => {
                let r = self.sync();
                bkerror!(r, tx, BKResponse::SyncError);
            }
            Ok(BKCommand::SyncForced) => {
                self.data.lock().unwrap().since = String::from("");
                let r = self.sync();
                bkerror!(r, tx, BKResponse::SyncError);
            }
            Ok(BKCommand::GetRoomMessages(room)) => {
                let r = self.get_room_messages(room);
                bkerror!(r, tx, BKResponse::RoomMessagesError);
            }
            Ok(BKCommand::GetMessageContext(message)) => {
                let r = self.get_message_context(message);
                bkerror!(r, tx, BKResponse::RoomMessagesError);
            }
            Ok(BKCommand::GetUserInfoAsync(sender, ctx)) => {
                let r = self.get_user_info_async(&sender, ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::GetThumbAsync(media, ctx)) => {
                let r = self.get_thumb_async(media, ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::GetAvatarAsync(member, ctx)) => {
                let r = self.get_avatar_async(member, ctx);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::GetMedia(media)) => {
                let r = self.get_media(media);
                bkerror!(r, tx, BKResponse::CommandError);
            }
            Ok(BKCommand::SendMsg(msg)) => {
                let r = self.send_msg(msg);
                bkerror!(r, tx, BKResponse::SendMsgError);
            }
            Ok(BKCommand::SetRoom(room)) => {
                let r = self.set_room(room);
                bkerror!(r, tx, BKResponse::SetRoomError);
            }
            Ok(BKCommand::GetRoomAvatar(room)) => {
                let r = self.get_room_avatar(room);
                bkerror!(r, tx, BKResponse::GetRoomAvatarError);
            }
            Ok(BKCommand::DirectoryProtocols) => {
                let r = self.protocols();
                bkerror!(r, tx, BKResponse::DirectoryError);
            }
            Ok(BKCommand::DirectorySearch(dq, dtp, more)) => {
                let q = match dq {
                    ref a if a.is_empty() => None,
                    b => Some(b),
                };

                let tp = match dtp {
                    ref a if a.is_empty() => None,
                    b => Some(b),
                };

                let r = self.room_search(q, tp, more);
                bkerror!(r, tx, BKResponse::DirectoryError);
            }
            Ok(BKCommand::JoinRoom(roomid)) => {
                let r = self.join_room(roomid);
                bkerror!(r, tx, BKResponse::JoinRoomError);
            }
            Ok(BKCommand::LeaveRoom(roomid)) => {
                let r = self.leave_room(roomid);
                bkerror!(r, tx, BKResponse::LeaveRoomError);
            }
            Ok(BKCommand::MarkAsRead(roomid, evid)) => {
                let r = self.mark_as_read(roomid, evid);
                bkerror!(r, tx, BKResponse::MarkAsReadError);
            }
            Ok(BKCommand::SetRoomName(roomid, name)) => {
                let r = self.set_room_name(roomid, name);
                bkerror!(r, tx, BKResponse::SetRoomNameError);
            }
            Ok(BKCommand::SetRoomTopic(roomid, topic)) => {
                let r = self.set_room_topic(roomid, topic);
                bkerror!(r, tx, BKResponse::SetRoomTopicError);
            }
            Ok(BKCommand::SetRoomAvatar(roomid, fname)) => {
                let r = self.set_room_avatar(roomid, fname);
                bkerror!(r, tx, BKResponse::SetRoomAvatarError);
            }
            Ok(BKCommand::AttachFile(roomid, fname)) => {
                let r = self.attach_file(roomid, fname);
                bkerror!(r, tx, BKResponse::AttachFileError);
            }
            Ok(BKCommand::AttachImage(roomid, image)) => {
                let r = self.attach_image(roomid, image);
                bkerror!(r, tx, BKResponse::AttachFileError);
            }
            Ok(BKCommand::Search(roomid, term)) => {
                let r = self.search(roomid, term);
                bkerror!(r, tx, BKResponse::SearchError);
            }
            Ok(BKCommand::NotifyClicked(message)) => {
                tx.send(BKResponse::NotificationClicked(message)).unwrap();
            }
            Ok(BKCommand::NewRoom(name, privacy)) => {
                let r = self.new_room(name, privacy);
                bkerror!(r, tx, BKResponse::NewRoomError);
            }
            Ok(BKCommand::ShutDown) => {
                return false;
            }
            Err(_) => {
                return false;
            }
        };

        true
    }

    pub fn run(mut self) -> Sender<BKCommand> {
        let (apptx, rx): (Sender<BKCommand>, Receiver<BKCommand>) = channel();

        self.internal_tx = Some(apptx.clone());
        thread::spawn(move || loop {
            let cmd = rx.recv();
            if !self.command_recv(cmd) {
                break;
            }
        });

        apptx
    }

    pub fn set_room(&self, room: Room) -> Result<(), Error> {
        self.get_room_detail(room.id.clone(), String::from("m.room.topic"))?;
        self.get_room_avatar(room.id.clone())?;

        Ok(())
    }

    pub fn guest(&self, server: String) -> Result<(), Error> {
        let s = server.clone();
        let url = Url::parse(&s).unwrap().join("/_matrix/client/r0/register?kind=guest")?;
        self.data.lock().unwrap().server_url = s;

        let data = self.data.clone();
        let tx = self.tx.clone();
        let attrs = json!({});
        post!(&url, &attrs,
              |r: JsonValue| {
            let uid = String::from(r["user_id"].as_str().unwrap_or(""));
            let tk = String::from(r["access_token"].as_str().unwrap_or(""));
            data.lock().unwrap().user_id = uid.clone();
            data.lock().unwrap().access_token = tk.clone();
            data.lock().unwrap().since = String::from("");
            tx.send(BKResponse::Token(uid, tk)).unwrap();
            tx.send(BKResponse::Rooms(vec![], None)).unwrap();
        },
              |err| tx.send(BKResponse::GuestLoginError(err)).unwrap());

        Ok(())
    }

    pub fn login(&self, user: String, password: String, server: String) -> Result<(), Error> {
        let s = server.clone();
        self.data.lock().unwrap().server_url = s;
        let url = self.url("login", vec![])?;

        let attrs = json!({
            "type": "m.login.password",
            "user": user,
            "password": password
        });

        let data = self.data.clone();

        let tx = self.tx.clone();
        post!(&url, &attrs,
            |r: JsonValue| {
                let uid = String::from(r["user_id"].as_str().unwrap_or(""));
                let tk = String::from(r["access_token"].as_str().unwrap_or(""));

                if uid.is_empty() || tk.is_empty() {
                    tx.send(BKResponse::LoginError(Error::BackendError)).unwrap();
                } else {
                    data.lock().unwrap().user_id = uid.clone();
                    data.lock().unwrap().access_token = tk.clone();
                    data.lock().unwrap().since = String::new();
                    tx.send(BKResponse::Token(uid, tk)).unwrap();
                }
            },
            |err| { tx.send(BKResponse::LoginError(err)).unwrap() }
        );

        Ok(())
    }

    pub fn logout(&self) -> Result<(), Error> {
        let url = self.url("logout", vec![])?;
        let attrs = json!({});

        let data = self.data.clone();
        let tx = self.tx.clone();
        post!(&url, &attrs,
            |_| {
                data.lock().unwrap().user_id = String::new();
                data.lock().unwrap().access_token = String::new();
                data.lock().unwrap().since = String::new();
                tx.send(BKResponse::Logout).unwrap();
            },
            |err| { tx.send(BKResponse::LogoutError(err)).unwrap() }
        );
        Ok(())
    }

    pub fn register(&self, user: String, password: String, server: String) -> Result<(), Error> {
        let s = server.clone();
        self.data.lock().unwrap().server_url = s;
        let url = self.url("register", vec![("kind", strn!("user"))])?;

        let attrs = json!({
            "auth": {"type": "m.login.password"},
            "username": user,
            "bind_email": false,
            "password": password
        });

        let data = self.data.clone();
        let tx = self.tx.clone();
        post!(&url, &attrs,
            |r: JsonValue| {
                println!("RESPONSE: {:#?}", r);
                let uid = String::from(r["user_id"].as_str().unwrap_or(""));
                let tk = String::from(r["access_token"].as_str().unwrap_or(""));

                data.lock().unwrap().user_id = uid.clone();
                data.lock().unwrap().access_token = tk.clone();
                data.lock().unwrap().since = String::from("");
                tx.send(BKResponse::Token(uid, tk)).unwrap();
            },
            |err| { tx.send(BKResponse::LoginError(err)).unwrap() }
        );

        Ok(())
    }

    pub fn get_username(&self) -> Result<(), Error> {
        let id = self.data.lock().unwrap().user_id.clone();
        let url = self.url(&format!("profile/{}/displayname", id.clone()), vec![])?;
        let tx = self.tx.clone();
        get!(&url,
            |r: JsonValue| {
                let name = String::from(r["displayname"].as_str().unwrap_or(&id));
                tx.send(BKResponse::Name(name)).unwrap();
            },
            |err| { tx.send(BKResponse::UserNameError(err)).unwrap() }
        );

        Ok(())
    }

    pub fn get_avatar(&self) -> Result<(), Error> {
        let baseu = self.get_base_url()?;
        let userid = self.data.lock().unwrap().user_id.clone();

        let tx = self.tx.clone();
        thread::spawn(move || match get_user_avatar(&baseu, &userid) {
            Ok((_, fname)) => {
                tx.send(BKResponse::Avatar(fname)).unwrap();
            }
            Err(err) => {
                tx.send(BKResponse::AvatarError(err)).unwrap();
            }
        });

        Ok(())
    }

    pub fn sync(&self) -> Result<(), Error> {
        let tk = self.data.lock().unwrap().access_token.clone();
        if tk.is_empty() {
            return Err(Error::BackendError);
        }

        let since = self.data.lock().unwrap().since.clone();
        let userid = self.data.lock().unwrap().user_id.clone();

        let mut params: Vec<(&str, String)> = vec![];
        let timeout = 120;

        params.push(("full_state", strn!("false")));
        params.push(("timeout", strn!("30000")));

        if since.is_empty() {
            let filter = format!("{{
                \"room\": {{
                    \"state\": {{
                        \"types\": [\"m.room.*\"],
                    }},
                    \"timeline\": {{
                        \"types\": [\"m.room.message\"],
                        \"limit\": {},
                    }},
                    \"ephemeral\": {{ \"types\": [] }}
                }},
                \"presence\": {{ \"types\": [] }},
                \"event_format\": \"client\",
                \"event_fields\": [\"type\", \"content\", \"sender\", \"event_id\", \"age\", \"unsigned\"]
            }}", globals::PAGE_LIMIT);

            params.push(("filter", strn!(filter)));
        } else {
            params.push(("since", since.clone()));
        }

        let baseu = self.get_base_url()?;
        let url = self.url("sync", params)?;

        let tx = self.tx.clone();
        let data = self.data.clone();

        let attrs = json!(null);

        thread::spawn(move || {
            match json_q("get", &url, &attrs, timeout) {
                Ok(r) => {
                    let next_batch = String::from(r["next_batch"].as_str().unwrap_or(""));
                    if since.is_empty() {
                        let rooms = match get_rooms_from_json(r, &userid, &baseu) {
                            Ok(rs) => rs,
                            Err(err) => {
                                tx.send(BKResponse::SyncError(err)).unwrap();
                                vec![]
                            }
                        };

                        let mut def: Option<Room> = None;
                        let jtr = data.lock().unwrap().join_to_room.clone();
                        if !jtr.is_empty() {
                            if let Some(r) = rooms.iter().find(|x| x.id == jtr) {
                                def = Some(r.clone());
                            }
                        }
                        tx.send(BKResponse::Rooms(rooms, def)).unwrap();
                    } else {
                        // Message events
                        match get_rooms_timeline_from_json(&baseu, &r) {
                            Ok(msgs) => tx.send(BKResponse::RoomMessages(msgs)).unwrap(),
                            Err(err) => tx.send(BKResponse::RoomMessagesError(err)).unwrap(),
                        };
                        // Other events
                        match parse_sync_events(&r) {
                            Err(err) => tx.send(BKResponse::SyncError(err)).unwrap(),
                            Ok(events) => {
                                for ev in events {
                                    match ev.stype.as_ref() {
                                        "m.room.name" => {
                                            let name = strn!(ev.content["name"].as_str().unwrap_or(""));
                                            tx.send(BKResponse::RoomName(ev.room.clone(), name)).unwrap();
                                        }
                                        "m.room.topic" => {
                                            let t = strn!(ev.content["topic"].as_str().unwrap_or(""));
                                            tx.send(BKResponse::RoomTopic(ev.room.clone(), t)).unwrap();
                                        }
                                        "m.room.avatar" => {
                                            tx.send(BKResponse::NewRoomAvatar(ev.room.clone())).unwrap();
                                        }
                                        "m.room.member" => {
                                            tx.send(BKResponse::RoomMemberEvent(ev)).unwrap();
                                        }
                                        _ => {
                                            println!("EVENT NOT MANAGED: {:?}", ev);
                                        }
                                    }
                                }
                            }
                        };
                    }

                    tx.send(BKResponse::Sync(next_batch.clone())).unwrap();
                    data.lock().unwrap().since = next_batch;
                },
                Err(err) => { tx.send(BKResponse::SyncError(err)).unwrap() }
            };
        });

        Ok(())
    }

    pub fn get_room_detail(&self, roomid: String, key: String) -> Result<(), Error> {
        let url = self.url(&format!("rooms/{}/state/{}", roomid, key), vec![])?;

        let tx = self.tx.clone();
        let keys = key.clone();
        get!(&url,
            |r: JsonValue| {
                let mut value = String::from("");
                let k = keys.split('.').last().unwrap();

                match r[&k].as_str() {
                    Some(x) => { value = String::from(x); },
                    None => {}
                }
                tx.send(BKResponse::RoomDetail(roomid, key, value)).unwrap();
            },
            |err| { tx.send(BKResponse::RoomDetailError(err)).unwrap() }
        );

        Ok(())
    }

    pub fn get_room_avatar(&self, roomid: String) -> Result<(), Error> {
        let userid = self.data.lock().unwrap().user_id.clone();
        let baseu = self.get_base_url()?;
        let tk = self.data.lock().unwrap().access_token.clone();
        let url = self.url(&format!("rooms/{}/state/m.room.avatar", roomid), vec![])?;

        let tx = self.tx.clone();
        get!(&url,
            |r: JsonValue| {
                let avatar;

                match r["url"].as_str() {
                    Some(u) => {
                        avatar = thumb!(&baseu, u).unwrap_or(String::from(""));
                    },
                    None => {
                        avatar = get_room_avatar(&baseu, &tk, &userid, &roomid)
                            .unwrap_or(String::from(""));
                    }
                }
                tx.send(BKResponse::RoomAvatar(roomid, avatar)).unwrap();
            },
            |err: Error| {
                match err {
                    Error::MatrixError(ref js) if js["errcode"].as_str().unwrap_or("") == "M_NOT_FOUND" => {
                        let avatar = get_room_avatar(&baseu, &tk, &userid, &roomid)
                            .unwrap_or(String::from(""));
                        tx.send(BKResponse::RoomAvatar(roomid, avatar)).unwrap();
                    },
                    _ => {
                        tx.send(BKResponse::RoomAvatarError(err)).unwrap();
                    }
                }
            }
        );

        Ok(())
    }

    pub fn get_room_messages(&self, roomid: String) -> Result<(), Error> {
        let baseu = self.get_base_url()?;
        let tk = self.data.lock().unwrap().access_token.clone();

        let tx = self.tx.clone();
        thread::spawn(move || {
            match get_initial_room_messages(&baseu, tk, roomid.clone(),
                                            globals::PAGE_LIMIT as usize,
                                            globals::PAGE_LIMIT, None) {
                Ok((ms, _, _)) => {
                    tx.send(BKResponse::RoomMessagesInit(ms)).unwrap();
                }
                Err(err) => {
                    tx.send(BKResponse::RoomMessagesError(err)).unwrap();
                }
            }
        });

        Ok(())
    }

    pub fn get_message_context(&self, msg: Message) -> Result<(), Error> {
        let url = self.url(&format!("rooms/{}/context/{}", msg.room, msg.id),
                           vec![("limit", String::from("40"))])?;

        let tx = self.tx.clone();
        let baseu = self.get_base_url()?;
        let roomid = msg.room.clone();
        get!(&url,
            |r: JsonValue| {
                let mut ms: Vec<Message> = vec![];
                let array = r["events_before"].as_array();
                for msg in array.unwrap().iter().rev() {
                    if msg["type"].as_str().unwrap_or("") != "m.room.message" {
                        continue;
                    }

                    let m = parse_room_message(&baseu, roomid.clone(), msg);
                    ms.push(m);
                }
                tx.send(BKResponse::RoomMessagesTo(ms)).unwrap();
            },
            |err| { tx.send(BKResponse::RoomMessagesError(err)).unwrap() }
        );

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_room_members(&self, roomid: String) -> Result<(), Error> {
        let url = self.url(&format!("rooms/{}/members", roomid), vec![])?;

        let tx = self.tx.clone();
        get!(&url,
            |r: JsonValue| {
                //println!("{:#?}", r);
                let mut ms: Vec<Member> = vec![];
                for member in r["chunk"].as_array().unwrap().iter().rev() {
                    if member["type"].as_str().unwrap() != "m.room.member" {
                        continue;
                    }

                    let content = &member["content"];
                    if content["membership"].as_str().unwrap() != "join" {
                        continue;
                    }

                    let m = Member {
                        alias: String::from(content["displayname"].as_str().unwrap_or("")),
                        uid: String::from(member["sender"].as_str().unwrap()),
                        avatar: String::from(content["avatar_url"].as_str().unwrap_or("")),
                    };
                    ms.push(m);
                }
                tx.send(BKResponse::RoomMembers(ms)).unwrap();
            },
            |err| { tx.send(BKResponse::RoomMembersError(err)).unwrap() }
        );

        Ok(())
    }

    pub fn get_user_info_async(&mut self,
                               uid: &str,
                               tx: Sender<(String, String)>)
                               -> Result<(), Error> {
        let baseu = self.get_base_url()?;

        let u = String::from(uid);

        if let Some(info) = self.user_info_cache.get(&u) {
            let i = info.lock().unwrap().clone();
            if !i.0.is_empty() || !i.1.is_empty() {
                tx.send(i).unwrap();
                return Ok(())
            }
        }

        let info = Arc::new(Mutex::new((String::new(), String::new())));
        let cache_key = u.clone();
        let cache_value = info.clone();

        thread::spawn(move || {
            let i0 = info.lock();
            match get_user_avatar(&baseu, &u) {
                Ok(info) => {
                    tx.send(info.clone()).unwrap();
                    let mut i = i0.unwrap();
                    i.0 = info.0;
                    i.1 = info.1;
                }
                Err(_) => {
                    tx.send((String::new(), String::new())).unwrap();
                }
            };
        });

        self.user_info_cache.insert(cache_key, cache_value);

        Ok(())
    }

    pub fn get_thumb_async(&self, media: String, tx: Sender<String>) -> Result<(), Error> {
        let baseu = self.get_base_url()?;

        thread::spawn(move || {
            match thumb!(&baseu, &media) {
                Ok(fname) => {
                    tx.send(fname).unwrap();
                }
                Err(_) => {
                    tx.send(String::from("")).unwrap();
                }
            };
        });

        Ok(())
    }

    pub fn get_avatar_async(&self, member: Option<Member>, tx: Sender<String>) -> Result<(), Error> {
        let baseu = self.get_base_url()?;

        if member.is_none() {
            tx.send(String::new()).unwrap();
            return Ok(());
        }

        let m = member.unwrap();

        let uid = m.uid.clone();
        let alias = m.get_alias().clone();
        let avatar = m.avatar.clone();

        thread::spawn(move || {
            match get_user_avatar_img(&baseu, uid, alias, avatar) {
                Ok(fname) => {
                    tx.send(fname.clone()).unwrap();
                }
                Err(_) => {
                    tx.send(String::new()).unwrap();
                }
            }
        });

        Ok(())
    }

    pub fn get_media(&self, media: String) -> Result<(), Error> {
        let baseu = self.get_base_url()?;

        let tx = self.tx.clone();
        thread::spawn(move || {
            match media!(&baseu, &media) {
                Ok(fname) => {
                    tx.send(BKResponse::Media(fname)).unwrap();
                }
                Err(err) => {
                    tx.send(BKResponse::MediaError(err)).unwrap();
                }
            };
        });

        Ok(())
    }

    pub fn send_msg(&self, msg: Message) -> Result<(), Error> {
        let roomid = msg.room.clone();
        let msgid;

        {
            let mut data = self.data.lock().unwrap();
            data.msgid = data.msgid + 1;
            msgid = data.msgid;
        }

        let url = self.url(&format!("rooms/{}/send/m.room.message/{}", roomid, msgid), vec![])?;

        let attrs = json!({
            "body": msg.body.clone(),
            "url": msg.url.clone(),
            "msgtype": msg.mtype.clone()
        });

        let tx = self.tx.clone();
        query!("put", &url, &attrs,
            move |_| {
                tx.send(BKResponse::SendMsg).unwrap();
            },
            |err| { tx.send(BKResponse::SendMsgError(err)).unwrap(); }
        );

        Ok(())
    }

    pub fn protocols(&self) -> Result<(), Error> {
        let baseu = self.get_base_url()?;
        let tk = self.data.lock().unwrap().access_token.clone();
        let mut url = baseu.join("/_matrix/client/unstable/thirdparty/protocols")?;
        url.query_pairs_mut().clear()
            .append_pair("access_token", &tk);

        let tx = self.tx.clone();
        let s = self.data.lock().unwrap().server_url.clone();
        get!(&url,
            move |r: JsonValue| {
                let mut protocols: Vec<Protocol> = vec![];

                protocols.push(Protocol {
                    id: String::from(""),
                    desc: String::from(s.split('/').last().unwrap_or("")),
                });

                let prs = r.as_object().unwrap();
                for k in prs.keys() {
                    let ins = prs[k]["instances"].as_array().unwrap();
                    for i in ins {
                        let p = Protocol{
                            id: String::from(i["instance_id"].as_str().unwrap()),
                            desc: String::from(i["desc"].as_str().unwrap()),
                        };
                        protocols.push(p);
                    }
                }

                tx.send(BKResponse::DirectoryProtocols(protocols)).unwrap();
            },
            |err| { tx.send(BKResponse::DirectoryError(err)).unwrap(); }
        );

        Ok(())
    }

    pub fn room_search(&self,
                       query: Option<String>,
                       third_party: Option<String>,
                       more: bool)
                       -> Result<(), Error> {

        let url = self.url("publicRooms", vec![])?;

        let mut attrs = json!({"limit": 20});

        if let Some(q) = query {
            attrs["filter"] = json!({
                "generic_search_term": q
            });
        }

        if let Some(tp) = third_party {
            attrs["third_party_instance_id"] = json!(tp);
        }

        if more {
            let since = self.data.lock().unwrap().rooms_since.clone();
            attrs["since"] = json!(since);
        }

        let tx = self.tx.clone();
        let data = self.data.clone();
        post!(&url, &attrs,
            move |r: JsonValue| {
                let next_branch = r["next_batch"].as_str().unwrap_or("");
                data.lock().unwrap().rooms_since = String::from(next_branch);

                let mut rooms: Vec<Room> = vec![];
                for room in r["chunk"].as_array().unwrap() {
                    let alias = String::from(room["canonical_alias"].as_str().unwrap_or(""));
                    let id = String::from(room["room_id"].as_str().unwrap_or(""));
                    let name = String::from(room["name"].as_str().unwrap_or(""));
                    let mut r = Room::new(id, name);
                    r.alias = alias;
                    r.avatar = String::from(room["avatar_url"].as_str().unwrap_or(""));
                    r.topic = String::from(room["topic"].as_str().unwrap_or(""));
                    r.n_members = room["num_joined_members"].as_i64().unwrap_or(0) as i32;
                    r.world_readable = room["world_readable"].as_bool().unwrap_or(false);
                    r.guest_can_join = room["guest_can_join"].as_bool().unwrap_or(false);
                    rooms.push(r);
                }

                tx.send(BKResponse::DirectorySearch(rooms)).unwrap();
            },
            |err| { tx.send(BKResponse::DirectoryError(err)).unwrap(); }
        );

        Ok(())
    }

    pub fn join_room(&self, roomid: String) -> Result<(), Error> {
        let url = self.url(&format!("rooms/{}/join", roomid), vec![])?;

        let tx = self.tx.clone();
        let data = self.data.clone();
        post!(&url,
            move |_: JsonValue| {
                data.lock().unwrap().join_to_room = roomid.clone();
                tx.send(BKResponse::JoinRoom).unwrap();
            },
            |err| { tx.send(BKResponse::JoinRoomError(err)).unwrap(); }
        );

        Ok(())
    }

    pub fn leave_room(&self, roomid: String) -> Result<(), Error> {
        let url = self.url(&format!("rooms/{}/leave", roomid), vec![])?;

        let tx = self.tx.clone();
        post!(&url,
            move |_: JsonValue| {
                tx.send(BKResponse::LeaveRoom).unwrap();
            },
            |err| { tx.send(BKResponse::LeaveRoomError(err)).unwrap(); }
        );

        Ok(())
    }

    pub fn mark_as_read(&self, roomid: String, eventid: String) -> Result<(), Error> {
        let url = self.url(&format!("rooms/{}/receipt/m.read/{}", roomid, eventid), vec![])?;

        let tx = self.tx.clone();
        let r = roomid.clone();
        let e = eventid.clone();
        post!(&url,
            move |_: JsonValue| { tx.send(BKResponse::MarkedAsRead(r, e)).unwrap(); },
            |err| { tx.send(BKResponse::MarkAsReadError(err)).unwrap(); }
        );

        Ok(())
    }

    pub fn set_room_name(&self, roomid: String, name: String) -> Result<(), Error> {
        let url = self.url(&format!("rooms/{}/state/m.room.name", roomid), vec![])?;

        let attrs = json!({
            "name": name,
        });

        let tx = self.tx.clone();
        query!("put", &url, &attrs,
            |_| { tx.send(BKResponse::SetRoomName).unwrap(); },
            |err| { tx.send(BKResponse::SetRoomNameError(err)).unwrap(); }
        );

        Ok(())
    }

    pub fn set_room_topic(&self, roomid: String, topic: String) -> Result<(), Error> {
        let url = self.url(&format!("rooms/{}/state/m.room.topic", roomid), vec![])?;

        let attrs = json!({
            "topic": topic,
        });

        let tx = self.tx.clone();
        query!("put", &url, &attrs,
            |_| { tx.send(BKResponse::SetRoomTopic).unwrap(); },
            |err| { tx.send(BKResponse::SetRoomTopicError(err)).unwrap(); }
        );

        Ok(())
    }

    pub fn set_room_avatar(&self, roomid: String, avatar: String) -> Result<(), Error> {
        let baseu = self.get_base_url()?;
        let tk = self.data.lock().unwrap().access_token.clone();
        let params = vec![("access_token", tk.clone())];
        let mediaurl = media_url!(&baseu, "upload", params)?;
        let roomurl = self.url(&format!("rooms/{}/state/m.room.avatar", roomid), vec![])?;

        let mut file = File::open(&avatar)?;
        let mut contents: Vec<u8> = vec![];
        file.read_to_end(&mut contents)?;

        let tx = self.tx.clone();
        thread::spawn(
            move || {
                match put_media(mediaurl.as_str(), contents) {
                    Err(err) => {
                        tx.send(BKResponse::SetRoomAvatarError(err)).unwrap();
                    }
                    Ok(js) => {
                        let uri = js["content_uri"].as_str().unwrap_or("");
                        let attrs = json!({ "url": uri });
                        match json_q("put", &roomurl, &attrs, 0) {
                            Ok(_) => {
                                tx.send(BKResponse::SetRoomAvatar).unwrap();
                            },
                            Err(err) => {
                                tx.send(BKResponse::SetRoomAvatarError(err)).unwrap();
                            }
                        };
                    }
                };
            },
        );

        Ok(())
    }

    pub fn attach_image(&self, roomid: String, image: Vec<u8>) -> Result<(), Error> {
        self.attach_send(roomid, strn!("Screenshot"), image, "m.image")
    }

    pub fn attach_file(&self, roomid: String, path: String) -> Result<(), Error> {
        let mut file = File::open(&path)?;
        let mut contents: Vec<u8> = vec![];
        file.read_to_end(&mut contents)?;

        let p: &Path = Path::new(&path);
        let mime = tree_magic::from_filepath(p);

        let mtype = match mime.as_ref() {
            "image/gif" => "m.image",
            "image/png" => "m.image",
            "image/jpeg" => "m.image",
            "image/jpg" => "m.image",
            _ => "m.file"
        };

        let body = strn!(path.split("/").last().unwrap_or(&path));
        self.attach_send(roomid, body, contents, mtype)
    }

    pub fn attach_send(&self, roomid: String, body: String, contents: Vec<u8>, mtype: &str) -> Result<(), Error> {
        let baseu = self.get_base_url()?;
        let tk = self.data.lock().unwrap().access_token.clone();
        let params = vec![("access_token", tk.clone())];
        let mediaurl = media_url!(&baseu, "upload", params)?;

        let now = Local::now();
        let userid = self.data.lock().unwrap().user_id.clone();

        let mut m = Message {
            sender: userid,
            mtype: strn!(mtype),
            body: body,
            room: roomid.clone(),
            date: now,
            thumb: String::from(""),
            url: String::from(""),
            id: String::from(""),
        };

        let tx = self.tx.clone();
        let itx = self.internal_tx.clone();
        thread::spawn(
            move || {
                match put_media(mediaurl.as_str(), contents) {
                    Err(err) => {
                        tx.send(BKResponse::AttachFileError(err)).unwrap();
                    }
                    Ok(js) => {
                        let uri = js["content_uri"].as_str().unwrap_or("");
                        m.url = strn!(uri);
                        if let Some(t) = itx {
                            t.send(BKCommand::SendMsg(m.clone())).unwrap();
                        }
                        tx.send(BKResponse::AttachedFile(m)).unwrap();
                    }
                };
            },
        );

        Ok(())
    }

    pub fn search(&self, roomid: String, term: Option<String>) -> Result<(), Error> {
        let tx = self.tx.clone();

        match term {
            Some(ref t) if !t.is_empty() => {
                self.make_search(roomid, t.clone())
            }
            _ => {
                tx.send(BKResponse::SearchEnd).unwrap();
                self.get_room_messages(roomid)
            }
        }
    }

    pub fn new_room(&self, name: String, privacy: RoomType) -> Result<(), Error> {
        let url = self.url("createRoom", vec![])?;
        let attrs = json!({
            "invite": [],
            "invite_3pid": [],
            "name": &name,
            "visibility": match privacy {
                RoomType::Public => "public",
                RoomType::Private => "private",
            },
            "topic": "",
            "preset": match privacy {
                RoomType::Public => "public_chat",
                RoomType::Private => "private_chat",
            },
        });

        let n = name.clone();
        let tx = self.tx.clone();
        post!(&url, &attrs,
            move |r: JsonValue| {
                let id = strn!(r["room_id"].as_str().unwrap_or(""));
                let name = n;
                let r = Room::new(id, name);
                tx.send(BKResponse::NewRoom(r)).unwrap();
            },
            |err| { tx.send(BKResponse::NewRoomError(err)).unwrap(); }
        );
        Ok(())
    }

    pub fn make_search(&self, roomid: String, term: String) -> Result<(), Error> {
        let url = self.url("search", vec![])?;

        let attrs = json!({
            "search_categories": {
                "room_events": {
                    "keys": ["content.body"],
                    "search_term": term,
                    "filter": {
                        "rooms": [ roomid.clone() ],
                    },
                    "order_by": "recent",
                },
            },
        });

        let tx = self.tx.clone();
        let baseu = self.get_base_url()?;

        thread::spawn(move || {
            match json_q("post", &url, &attrs, 0) {
                Ok(js) => {
                    tx.send(BKResponse::SearchEnd).unwrap();
                    let mut ms: Vec<Message> = vec![];

                    let res = &js["search_categories"]["room_events"]["results"];
                    for search in res.as_array().unwrap().iter().rev() {
                        let msg = &search["result"];
                        if msg["type"].as_str().unwrap_or("") != "m.room.message" {
                            continue;
                        }

                        let m = parse_room_message(&baseu, roomid.clone(), msg);
                        ms.push(m);
                    }
                    tx.send(BKResponse::RoomMessagesInit(ms)).unwrap();
                }
                Err(err) => {
                    tx.send(BKResponse::SearchEnd).unwrap();
                    tx.send(BKResponse::SearchError(err)).unwrap()
                }
            };
        });

        Ok(())
    }
}
