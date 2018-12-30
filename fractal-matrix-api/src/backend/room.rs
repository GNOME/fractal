use serde_json;
use urlencoding;

use std::fs::File;
use std::io::prelude::*;
use std::sync::mpsc::Sender;
use url::Url;

use error::Error;
use globals;
use std::thread;

use util;
use util::cache_path;
use util::json_q;
use util::put_media;
use util::thumb;
use util::{client_url, media_url};

use backend::types::BKCommand;
use backend::types::BKResponse;
use backend::types::Backend;
use backend::types::RoomType;

use types::Member;
use types::Message;
use types::Room;

use serde_json::Value as JsonValue;

pub fn set_room(bk: &Backend, id: String) -> Result<(), Error> {
    /* FIXME: remove clone and pass id by reference */
    get_room_detail(bk, id.clone(), String::from("m.room.topic"))?;
    get_room_avatar(bk, id.clone())?;
    get_room_members(bk, id)?;

    Ok(())
}

pub fn get_room_detail(bk: &Backend, roomid: String, key: String) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/state/{}", roomid, key), vec![])?;

    let tx = bk.tx.clone();
    let keys = key.clone();
    get!(
        &url,
        |r: JsonValue| {
            let k = keys.split('.').last().unwrap();

            let value = String::from(r[&k].as_str().unwrap_or(""));
            tx.send(BKResponse::RoomDetail(roomid, key, value)).unwrap();
        },
        |err| tx.send(BKResponse::RoomDetailError(err)).unwrap()
    );

    Ok(())
}

pub fn get_room_avatar(bk: &Backend, roomid: String) -> Result<(), Error> {
    let userid = bk.data.lock().unwrap().user_id.clone();
    let baseu = bk.get_base_url()?;
    let tk = bk.data.lock().unwrap().access_token.clone();
    let url = bk.url(&format!("rooms/{}/state/m.room.avatar", roomid), vec![])?;

    let tx = bk.tx.clone();
    get!(
        &url,
        |r: JsonValue| {
            let avatar = match r["url"].as_str() {
                Some(u) => {
                    if let Ok(dest) = cache_path(&roomid) {
                        thumb(&baseu, u, Some(&dest)).unwrap_or_default()
                    } else {
                        String::from("")
                    }
                }
                None => util::get_room_avatar(&baseu, &tk, &userid, &roomid).unwrap_or_default(),
            };
            tx.send(BKResponse::RoomAvatar(roomid, avatar)).unwrap();
        },
        |err: Error| match err {
            Error::MatrixError(ref js) if js["errcode"].as_str().unwrap_or("") == "M_NOT_FOUND" => {
                let avatar =
                    util::get_room_avatar(&baseu, &tk, &userid, &roomid).unwrap_or_default();
                tx.send(BKResponse::RoomAvatar(roomid, avatar)).unwrap();
            }
            _ => {
                tx.send(BKResponse::RoomAvatarError(err)).unwrap();
            }
        }
    );

    Ok(())
}

pub fn get_room_members(bk: &Backend, roomid: String) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/joined_members", roomid), vec![])?;

    let tx = bk.tx.clone();
    get!(
        &url,
        |r: JsonValue| {
            let joined = r["joined"].as_object().unwrap();
            let ms: Vec<Member> = joined
                .iter()
                .map(|(mxid, member_data)| {
                    let mut member: Member = serde_json::from_value(member_data.clone()).unwrap();
                    member.uid = mxid.to_string();
                    member
                })
                .collect();
            tx.send(BKResponse::RoomMembers(roomid, ms)).unwrap();
        },
        |err| tx.send(BKResponse::RoomMembersError(err)).unwrap()
    );

    Ok(())
}

/* Load older messages starting by prev_batch
 * https://matrix.org/docs/spec/client_server/latest.html#get-matrix-client-r0-rooms-roomid-messages
 */
pub fn get_room_messages(bk: &Backend, roomid: String, from: String) -> Result<(), Error> {
    let params = vec![
        ("from", from),
        ("dir", String::from("b")),
        ("limit", format!("{}", globals::PAGE_LIMIT)),
        (
            "filter",
            "{ \"types\": [\"m.room.message\", \"m.sticker\"] }".to_string(),
        ),
    ];
    let url = bk.url(&format!("rooms/{}/messages", roomid), params)?;
    let tx = bk.tx.clone();
    get!(
        &url,
        |r: JsonValue| {
            let array = r["chunk"].as_array();
            let evs = array.unwrap().iter().rev();
            let list = Message::from_json_events_iter(&roomid, evs);
            let prev_batch = r["end"].as_str().map(String::from);
            tx.send(BKResponse::RoomMessagesTo(list, roomid, prev_batch))
                .unwrap();
        },
        |err| tx.send(BKResponse::RoomMembersError(err)).unwrap()
    );

    Ok(())
}

pub fn get_room_messages_from_msg(bk: &Backend, roomid: String, msg: Message) -> Result<(), Error> {
    // first of all, we calculate the from param using the context api, then we call the
    // normal get_room_messages
    let baseu = bk.get_base_url()?;
    let tk = bk.data.lock().unwrap().access_token.clone();
    let id = msg.id.unwrap_or_default();
    let tx = bk.internal_tx.clone();

    thread::spawn(move || {
        if let Ok(from) = util::get_prev_batch_from(&baseu, &tk, &roomid, &id) {
            if let Some(t) = tx {
                t.send(BKCommand::GetRoomMessages(roomid, from)).unwrap();
            }
        }
    });

    Ok(())
}

fn parse_context(
    tx: Sender<BKResponse>,
    tk: String,
    baseu: Url,
    roomid: String,
    eid: &str,
    limit: i32,
) -> Result<(), Error> {
    let url = client_url(
        &baseu,
        &format!("rooms/{}/context/{}", roomid, eid),
        &vec![
            ("limit", format!("{}", limit)),
            ("access_token", tk.clone()),
        ],
    )?;

    get!(
        &url,
        |r: JsonValue| {
            let mut id: Option<String> = None;

            let mut ms: Vec<Message> = vec![];
            let array = r["events_before"].as_array();
            for msg in array.unwrap().iter().rev() {
                if id.is_none() {
                    id = Some(msg["event_id"].as_str().unwrap_or("").to_string());
                }

                if !Message::supported_event(&&msg) {
                    continue;
                }

                let m = Message::parse_room_message(&roomid, msg);
                ms.push(m);
            }

            if ms.is_empty() && id.is_some() {
                // there's no messages so we'll try with a bigger context
                if let Err(err) =
                    parse_context(tx.clone(), tk, baseu, roomid, &id.unwrap(), limit * 2)
                {
                    tx.send(BKResponse::RoomMessagesError(err)).unwrap();
                }
            } else {
                tx.send(BKResponse::RoomMessagesTo(ms, roomid, None))
                    .unwrap();
            }
        },
        |err| tx.send(BKResponse::RoomMessagesError(err)).unwrap()
    );

    Ok(())
}

pub fn get_message_context(bk: &Backend, msg: Message) -> Result<(), Error> {
    let tx = bk.tx.clone();
    let baseu = bk.get_base_url()?;
    let roomid = msg.room.clone();
    let msgid = msg.id.unwrap_or_default();
    let tk = bk.data.lock().unwrap().access_token.clone();

    parse_context(tx, tk, baseu, roomid, &msgid, globals::PAGE_LIMIT)?;

    Ok(())
}

pub fn send_msg(bk: &Backend, msg: Message) -> Result<(), Error> {
    let roomid = msg.room.clone();

    let id = msg.id.unwrap_or_default();
    let url = bk.url(
        &format!("rooms/{}/send/m.room.message/{}", roomid, id),
        vec![],
    )?;

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

    if let Some(xctx) = msg.extra_content {
        if let Some(xctx) = xctx.as_object() {
            for (k, v) in xctx {
                attrs[k] = v.clone();
            }
        }
    }

    let tx = bk.tx.clone();
    query!(
        "put",
        &url,
        &attrs,
        move |js: JsonValue| {
            let evid = js["event_id"].as_str().unwrap_or_default();
            tx.send(BKResponse::SentMsg(id, evid.to_string())).unwrap();
        },
        |_| {
            tx.send(BKResponse::SendMsgError(Error::SendMsgError(id)))
                .unwrap();
        }
    );

    Ok(())
}

pub fn redact_msg(bk: &Backend, msg: &Message) -> Result<(), Error> {
    let roomid = msg.room.clone();
    let msgid = msg.id.clone().unwrap_or_default();
    let txnid = msg.get_txn_id();

    let url = bk.url(
        &format!("rooms/{}/redact/{}/{}", roomid, msgid, txnid),
        vec![],
    )?;

    let attrs = json!({
        "reason": "Deletion requested by the sender"
    });

    let tx = bk.tx.clone();
    query!(
        "put",
        &url,
        &attrs,
        move |js: JsonValue| {
            let evid = js["event_id"].as_str().unwrap_or_default();
            tx.send(BKResponse::SentMsgRedaction(msgid, evid.to_string()))
                .unwrap();
        },
        |_| {
            tx.send(BKResponse::SendMsgRedactionError(
                Error::SendMsgRedactionError(msgid),
            ))
            .unwrap();
        }
    );

    Ok(())
}

pub fn join_room(bk: &Backend, roomid: String) -> Result<(), Error> {
    let url = bk.url(&format!("join/{}", urlencoding::encode(&roomid)), vec![])?;

    let tx = bk.tx.clone();
    let data = bk.data.clone();
    post!(
        &url,
        move |_: JsonValue| {
            data.lock().unwrap().join_to_room = roomid.clone();
            tx.send(BKResponse::JoinRoom).unwrap();
        },
        |err| {
            tx.send(BKResponse::JoinRoomError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn leave_room(bk: &Backend, roomid: &str) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/leave", roomid), vec![])?;

    let tx = bk.tx.clone();
    post!(
        &url,
        move |_: JsonValue| {
            tx.send(BKResponse::LeaveRoom).unwrap();
        },
        |err| {
            tx.send(BKResponse::LeaveRoomError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn mark_as_read(bk: &Backend, roomid: &str, eventid: &str) -> Result<(), Error> {
    let url = bk.url(
        &format!("rooms/{}/receipt/m.read/{}", roomid, eventid),
        vec![],
    )?;

    let tx = bk.tx.clone();
    let r = String::from(roomid);
    let e = String::from(eventid);
    post!(
        &url,
        move |_: JsonValue| {
            tx.send(BKResponse::MarkedAsRead(r, e)).unwrap();
        },
        |err| {
            tx.send(BKResponse::MarkAsReadError(err)).unwrap();
        }
    );

    // send fully_read event
    // This event API call isn't in the current doc but I found this in the
    // matrix-js-sdk
    // https://github.com/matrix-org/matrix-js-sdk/blob/master/src/base-apis.js#L851
    let url = bk.url(&format!("rooms/{}/read_markers", roomid), vec![])?;
    let attrs = json!({
        "m.fully_read": eventid,
        "m.read": json!(null),
    });
    post!(&url, &attrs, |_| {}, |_| {});

    Ok(())
}

pub fn set_room_name(bk: &Backend, roomid: &str, name: &str) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/state/m.room.name", roomid), vec![])?;

    let attrs = json!({
        "name": name,
    });

    let tx = bk.tx.clone();
    query!(
        "put",
        &url,
        &attrs,
        |_| {
            tx.send(BKResponse::SetRoomName).unwrap();
        },
        |err| {
            tx.send(BKResponse::SetRoomNameError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn set_room_topic(bk: &Backend, roomid: &str, topic: &str) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/state/m.room.topic", roomid), vec![])?;

    let attrs = json!({
        "topic": topic,
    });

    let tx = bk.tx.clone();
    query!(
        "put",
        &url,
        &attrs,
        |_| {
            tx.send(BKResponse::SetRoomTopic).unwrap();
        },
        |err| {
            tx.send(BKResponse::SetRoomTopicError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn set_room_avatar(bk: &Backend, roomid: &str, avatar: &str) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;
    let tk = bk.data.lock().unwrap().access_token.clone();
    let params = vec![("access_token", tk.clone())];
    let mediaurl = media_url(&baseu, "upload", &params)?;
    let roomurl = bk.url(&format!("rooms/{}/state/m.room.avatar", roomid), vec![])?;

    let mut file = File::open(&avatar)?;
    let mut contents: Vec<u8> = vec![];
    file.read_to_end(&mut contents)?;

    let tx = bk.tx.clone();
    thread::spawn(move || {
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
                    }
                    Err(err) => {
                        tx.send(BKResponse::SetRoomAvatarError(err)).unwrap();
                    }
                };
            }
        };
    });

    Ok(())
}

pub fn attach_file(bk: &Backend, msg: Message) -> Result<(), Error> {
    let fname = msg.url.clone().unwrap_or_default();

    if fname.starts_with("mxc://") {
        return send_msg(bk, msg);
    }

    let mut file = File::open(&fname)?;
    let mut contents: Vec<u8> = vec![];
    file.read_to_end(&mut contents)?;

    let baseu = bk.get_base_url()?;
    let tk = bk.data.lock().unwrap().access_token.clone();
    let params = vec![("access_token", tk.clone())];
    let mediaurl = media_url(&baseu, "upload", &params)?;

    let mut m = msg.clone();
    let tx = bk.tx.clone();
    let itx = bk.internal_tx.clone();
    thread::spawn(move || {
        match put_media(mediaurl.as_str(), contents) {
            Err(err) => {
                tx.send(BKResponse::AttachFileError(err)).unwrap();
            }
            Ok(js) => {
                let uri = js["content_uri"].as_str().unwrap_or("");
                m.url = Some(uri.to_string());
                if let Some(t) = itx {
                    t.send(BKCommand::SendMsg(m.clone())).unwrap();
                }
                tx.send(BKResponse::AttachedFile(m)).unwrap();
            }
        };
    });

    Ok(())
}

pub fn new_room(
    bk: &Backend,
    name: &str,
    privacy: RoomType,
    internal_id: String,
) -> Result<(), Error> {
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

    let n = String::from(name);
    let tx = bk.tx.clone();
    post!(
        &url,
        &attrs,
        move |r: JsonValue| {
            let id = String::from(r["room_id"].as_str().unwrap_or(""));
            let name = n;
            let r = Room::new(id, Some(name));
            tx.send(BKResponse::NewRoom(r, internal_id)).unwrap();
        },
        |err| {
            tx.send(BKResponse::NewRoomError(err, internal_id)).unwrap();
        }
    );
    Ok(())
}

pub fn direct_chat(bk: &Backend, user: &Member, internal_id: String) -> Result<(), Error> {
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
    let data = bk.data.clone();
    post!(
        &url,
        &attrs,
        move |r: JsonValue| {
            let id = String::from(r["room_id"].as_str().unwrap_or(""));
            let mut r = Room::new(id.clone(), m.alias.clone());
            r.direct = true;
            tx.send(BKResponse::NewRoom(r, internal_id)).unwrap();

            let directs = &mut data.lock().unwrap().m_direct;
            if directs.contains_key(&m.uid) {
                if let Some(v) = directs.get_mut(&m.uid) {
                    v.push(id.clone())
                };
            } else {
                directs.insert(m.uid.clone(), vec![id.clone()]);
            }

            let attrs = json!(directs.clone());
            match json_q("put", &direct_url, &attrs, 0) {
                Ok(_js) => {}
                Err(err) => {
                    error!("{:?}", err);
                }
            };
        },
        |err| {
            tx.send(BKResponse::NewRoomError(err, internal_id)).unwrap();
        }
    );

    Ok(())
}

pub fn add_to_fav(bk: &Backend, roomid: String, tofav: bool) -> Result<(), Error> {
    let userid = bk.data.lock().unwrap().user_id.clone();
    let url = bk.url(
        &format!("user/{}/rooms/{}/tags/m.favourite", userid, roomid),
        vec![],
    )?;

    let attrs = json!({
        "order": 0.5,
    });

    let tx = bk.tx.clone();
    let method = if tofav { "put" } else { "delete" };
    query!(
        method,
        &url,
        &attrs,
        |_| {
            tx.send(BKResponse::AddedToFav(roomid.clone(), tofav))
                .unwrap();
        },
        |err| {
            tx.send(BKResponse::AddToFavError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn invite(bk: &Backend, roomid: &str, userid: &str) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/invite", roomid), vec![])?;

    let attrs = json!({
        "user_id": userid,
    });

    let tx = bk.tx.clone();
    post!(&url, &attrs, |_| {}, |err| {
        tx.send(BKResponse::InviteError(err)).unwrap();
    });

    Ok(())
}
