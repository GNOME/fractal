extern crate serde_json;
extern crate tree_magic;
extern crate chrono;
extern crate url;
extern crate urlencoding;

use self::chrono::prelude::*;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::sync::mpsc::Sender;
use self::url::Url;

use globals;
use std::thread;
use error::Error;

use util::json_q;
use util::dw_media;
use util::get_initial_room_messages;
use util::build_url;
use util::put_media;
use util;

use backend::types::Backend;
use backend::types::BKResponse;
use backend::types::BKCommand;
use backend::types::RoomType;
use backend::room;

use types::Room;
use types::Member;
use types::Message;

use self::serde_json::Value as JsonValue;

pub fn set_room(bk: &Backend, room: Room) -> Result<(), Error> {
    get_room_detail(bk, room.id.clone(), String::from("m.room.topic"))?;
    get_room_avatar(bk, room.id.clone())?;
    get_room_members(bk, room.id.clone())?;

    Ok(())
}

pub fn get_room_detail(bk: &Backend, roomid: String, key: String) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/state/{}", roomid, key), vec![])?;

    let tx = bk.tx.clone();
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

pub fn get_room_avatar(bk: &Backend, roomid: String) -> Result<(), Error> {
    let userid = bk.data.lock().unwrap().user_id.clone();
    let baseu = bk.get_base_url()?;
    let tk = bk.data.lock().unwrap().access_token.clone();
    let url = bk.url(&format!("rooms/{}/state/m.room.avatar", roomid), vec![])?;

    let tx = bk.tx.clone();
    get!(&url,
        |r: JsonValue| {
            let avatar;

            match r["url"].as_str() {
                Some(u) => {
                    avatar = thumb!(&baseu, u).unwrap_or_default();
                },
                None => {
                    avatar = util::get_room_avatar(&baseu, &tk, &userid, &roomid)
                        .unwrap_or(String::from(""));
                }
            }
            tx.send(BKResponse::RoomAvatar(roomid, avatar)).unwrap();
        },
        |err: Error| {
            match err {
                Error::MatrixError(ref js) if js["errcode"].as_str().unwrap_or("") == "M_NOT_FOUND" => {
                    let avatar = util::get_room_avatar(&baseu, &tk, &userid, &roomid)
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

pub fn get_room_members(bk: &Backend, roomid: String) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/joined_members", roomid), vec![])?;

    let tx = bk.tx.clone();
    get!(&url,
        |r: JsonValue| {
            let joined = r["joined"].as_object().unwrap();
            let mut ms: Vec<Member> = vec![];
            for memberid in joined.keys() {
                let alias = &joined[memberid]["display_name"];
                let avatar = &joined[memberid]["avatar_url"];

                let m = Member {
                    alias: match alias.as_str() { None => None, Some(a) => Some(strn!(a)) },
                    avatar: match avatar.as_str() { None => None, Some(a) => Some(strn!(a)) },
                    uid: memberid.to_string(),
                };
                ms.push(m);
            }
            tx.send(BKResponse::RoomMembers(roomid, ms)).unwrap();
        },
        |err| { tx.send(BKResponse::RoomMembersError(err)).unwrap() }
    );

    Ok(())
}

pub fn get_room_messages(bk: &Backend, roomid: String) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;
    let tk = bk.data.lock().unwrap().access_token.clone();

    let tx = bk.tx.clone();
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

fn parse_context(tx: Sender<BKResponse>, tk: String, baseu: Url, roomid: String, eid: String, limit: i32) -> Result<(), Error> {
    let url = client_url!(&baseu, &format!("rooms/{}/context/{}", roomid, eid),
        vec![("limit", format!("{}", limit)), ("access_token", tk.clone())])?;

    get!(&url,
        |r: JsonValue| {
            let mut id: Option<String> = None;

            let mut ms: Vec<Message> = vec![];
            let array = r["events_before"].as_array();
            for msg in array.unwrap().iter().rev() {
                if let None = id {
                    id = Some(msg["event_id"].as_str().unwrap_or("").to_string());
                }

                if !Message::supported_event(&&msg) {
                    continue;
                }

                let m = Message::parse_room_message(roomid.clone(), msg);
                ms.push(m);
            }

            if ms.len() == 0 && id.is_some() {
                // there's no messages so we'll try with a bigger context
                if let Err(err) = parse_context(tx.clone(), tk, baseu, roomid, id.unwrap(), limit * 2) {
                    tx.send(BKResponse::RoomMessagesError(err)).unwrap();
                }
            } else {
                tx.send(BKResponse::RoomMessagesTo(ms)).unwrap();
            }
        },
        |err| { tx.send(BKResponse::RoomMessagesError(err)).unwrap() }
    );

    Ok(())
}

pub fn get_message_context(bk: &Backend, msg: Message) -> Result<(), Error> {
    let tx = bk.tx.clone();
    let baseu = bk.get_base_url()?;
    let roomid = msg.room.clone();
    let msgid = msg.id.unwrap_or_default();
    let tk = bk.data.lock().unwrap().access_token.clone();

    parse_context(tx, tk, baseu, roomid, msgid, globals::PAGE_LIMIT)?;

    Ok(())
}

pub fn send_msg(bk: &Backend, msg: Message) -> Result<(), Error> {
    let roomid = msg.room.clone();
    let msgid = msg.get_txn_id();

    let url = bk.url(&format!("rooms/{}/send/m.room.message/{}", roomid, msgid), vec![])?;

    let mut attrs = json!({
        "body": msg.body.clone(),
        "msgtype": msg.mtype.clone()
    });

    if let Some(u) = msg.url {
        attrs["url"] = json!(u);
    }

    if let (Some(f), Some(f_b)) = (msg.format, msg.formatted_body) {
        attrs["formatted_body"] = json!(f_b);
        attrs["format"] = json!(f);
    }

    let tx = bk.tx.clone();
    query!("put", &url, &attrs,
        move |_| {
            tx.send(BKResponse::SendMsg).unwrap();
        },
        |err| { tx.send(BKResponse::SendMsgError(err)).unwrap(); }
    );

    Ok(())
}

pub fn join_room(bk: &Backend, roomid: String) -> Result<(), Error> {
    let url = bk.url(&format!("join/{}", urlencoding::encode(&roomid)), vec![])?;

    let tx = bk.tx.clone();
    let data = bk.data.clone();
    post!(&url,
        move |_: JsonValue| {
            data.lock().unwrap().join_to_room = roomid.clone();
            tx.send(BKResponse::JoinRoom).unwrap();
        },
        |err| { tx.send(BKResponse::JoinRoomError(err)).unwrap(); }
    );

    Ok(())
}

pub fn leave_room(bk: &Backend, roomid: String) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/leave", roomid), vec![])?;

    let tx = bk.tx.clone();
    post!(&url,
        move |_: JsonValue| {
            tx.send(BKResponse::LeaveRoom).unwrap();
        },
        |err| { tx.send(BKResponse::LeaveRoomError(err)).unwrap(); }
    );

    Ok(())
}

pub fn mark_as_read(bk: &Backend, roomid: String, eventid: String) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/receipt/m.read/{}", roomid, eventid), vec![])?;

    let tx = bk.tx.clone();
    let r = roomid.clone();
    let e = eventid.clone();
    post!(&url,
        move |_: JsonValue| { tx.send(BKResponse::MarkedAsRead(r, e)).unwrap(); },
        |err| { tx.send(BKResponse::MarkAsReadError(err)).unwrap(); }
    );

    Ok(())
}

pub fn set_room_name(bk: &Backend, roomid: String, name: String) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/state/m.room.name", roomid), vec![])?;

    let attrs = json!({
        "name": name,
    });

    let tx = bk.tx.clone();
    query!("put", &url, &attrs,
        |_| { tx.send(BKResponse::SetRoomName).unwrap(); },
        |err| { tx.send(BKResponse::SetRoomNameError(err)).unwrap(); }
    );

    Ok(())
}

pub fn set_room_topic(bk: &Backend, roomid: String, topic: String) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/state/m.room.topic", roomid), vec![])?;

    let attrs = json!({
        "topic": topic,
    });

    let tx = bk.tx.clone();
    query!("put", &url, &attrs,
        |_| { tx.send(BKResponse::SetRoomTopic).unwrap(); },
        |err| { tx.send(BKResponse::SetRoomTopicError(err)).unwrap(); }
    );

    Ok(())
}

pub fn set_room_avatar(bk: &Backend, roomid: String, avatar: String) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;
    let tk = bk.data.lock().unwrap().access_token.clone();
    let params = vec![("access_token", tk.clone())];
    let mediaurl = media_url!(&baseu, "upload", params)?;
    let roomurl = bk.url(&format!("rooms/{}/state/m.room.avatar", roomid), vec![])?;

    let mut file = File::open(&avatar)?;
    let mut contents: Vec<u8> = vec![];
    file.read_to_end(&mut contents)?;

    let tx = bk.tx.clone();
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

pub fn attach_image(bk: &Backend, roomid: String, image: Vec<u8>) -> Result<(), Error> {
    attach_send(bk, roomid, strn!("Screenshot"), image, "m.image")
}

pub fn attach_file(bk: &Backend, roomid: String, path: String) -> Result<(), Error> {
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
    attach_send(bk, roomid, body, contents, mtype)
}

pub fn attach_send(bk: &Backend, roomid: String, body: String, contents: Vec<u8>, mtype: &str) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;
    let tk = bk.data.lock().unwrap().access_token.clone();
    let params = vec![("access_token", tk.clone())];
    let mediaurl = media_url!(&baseu, "upload", params)?;

    let now = Local::now();
    let userid = bk.data.lock().unwrap().user_id.clone();

    let mut m = Message {
        sender: userid,
        mtype: strn!(mtype),
        body: body,
        room: roomid.clone(),
        date: now,
        thumb: None,
        url: None,
        id: None,
        formatted_body: None,
        format: None,
    };

    let tx = bk.tx.clone();
    let itx = bk.internal_tx.clone();
    thread::spawn(
        move || {
            match put_media(mediaurl.as_str(), contents) {
                Err(err) => {
                    tx.send(BKResponse::AttachFileError(err)).unwrap();
                }
                Ok(js) => {
                    let uri = js["content_uri"].as_str().unwrap_or("");
                    m.url = Some(strn!(uri));
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

pub fn new_room(bk: &Backend, name: String, privacy: RoomType, internal_id: String) -> Result<(), Error> {
    let url = bk.url("createRoom", vec![])?;
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
    let tx = bk.tx.clone();
    post!(&url, &attrs,
        move |r: JsonValue| {
            let id = strn!(r["room_id"].as_str().unwrap_or(""));
            let name = n;
            let r = Room::new(id, Some(name));
            tx.send(BKResponse::NewRoom(r, internal_id)).unwrap();
        },
        |err| { tx.send(BKResponse::NewRoomError(err, internal_id)).unwrap(); }
    );
    Ok(())
}

pub fn direct_chat(bk: &Backend, user: Member, internal_id: String) -> Result<(), Error> {
    let url = bk.url("createRoom", vec![])?;
    let attrs = json!({
        "invite": [user.uid.clone()],
        "invite_3pid": [],
        "visibility": "private",
        "preset": "private_chat",
        "is_direct": true,
    });

    let userid = bk.data.lock().unwrap().user_id.clone();
    let direct_url = bk.url(&format!("user/{}/account_data/m.direct", userid), vec![])?;

    let m = user.clone();
    let tx = bk.tx.clone();
    post!(&url, &attrs,
        move |r: JsonValue| {
            let id = strn!(r["room_id"].as_str().unwrap_or(""));
            let mut r = Room::new(id.clone(), m.alias);
            r.direct = true;
            tx.send(BKResponse::NewRoom(r, internal_id)).unwrap();

            let attrs = json!({ m.uid.clone(): [id] });
            match json_q("put", &direct_url, &attrs, 0) {
                Ok(_js) => { }
                Err(err) => { println!("Error {:?}", err); }
            };
        },
        |err| { tx.send(BKResponse::NewRoomError(err, internal_id)).unwrap(); }
    );

    Ok(())
}

pub fn search(bk: &Backend, roomid: String, term: Option<String>) -> Result<(), Error> {
    let tx = bk.tx.clone();

    match term {
        Some(ref t) if !t.is_empty() => {
            make_search(bk, roomid, t.clone())
        }
        _ => {
            tx.send(BKResponse::SearchEnd).unwrap();
            room::get_room_messages(bk, roomid)
        }
    }
}

pub fn make_search(bk: &Backend, roomid: String, term: String) -> Result<(), Error> {
    let url = bk.url("search", vec![])?;

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

    let tx = bk.tx.clone();

    thread::spawn(move || {
        match json_q("post", &url, &attrs, 0) {
            Ok(js) => {
                tx.send(BKResponse::SearchEnd).unwrap();
                let res = &js["search_categories"]["room_events"]["results"];
                let events = res.as_array().unwrap().iter().rev();
                let ms = Message::from_json_events_iter(roomid.clone(), events);

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

pub fn add_to_fav(bk: &Backend, roomid: String, tofav: bool) -> Result<(), Error> {
    let userid = bk.data.lock().unwrap().user_id.clone();
    let url = bk.url(&format!("user/{}/rooms/{}/tags/m.favourite", userid, roomid), vec![])?;

    let attrs = json!({
        "order": 0.5,
    });

    let tx = bk.tx.clone();
    let method = match tofav { true => "put", false => "delete" };
    query!(method, &url, &attrs,
        |_| { tx.send(BKResponse::AddedToFav(roomid.clone(), tofav)).unwrap(); },
        |err| { tx.send(BKResponse::AddToFavError(err)).unwrap(); }
    );

    Ok(())
}

pub fn invite(bk: &Backend, roomid: String, userid: String) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/invite", roomid), vec![])?;

    let attrs = json!({
        "user_id": userid,
    });

    let tx = bk.tx.clone();
    post!(&url, &attrs,
        |_| { },
        |err| { tx.send(BKResponse::InviteError(err)).unwrap(); }
    );

    Ok(())
}
