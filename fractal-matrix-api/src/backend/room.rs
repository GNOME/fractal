use log::error;
use serde_json::json;

use reqwest::header::CONTENT_TYPE;
use std::fs::File;
use std::io::prelude::*;
use std::sync::mpsc::Sender;
use url::Url;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::Error;
use crate::globals;
use std::thread;

use crate::util::cache_dir_path;
use crate::util::dw_media;
use crate::util::get_prev_batch_from;
use crate::util::json_q;
use crate::util::ContentType;
use crate::util::ResultExpectLog;
use crate::util::HTTP_CLIENT;
use crate::util::{client_url, media_url};

use crate::backend::types::BKCommand;
use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::backend::types::BackendData;
use crate::backend::types::RoomType;

use crate::r0::filter::RoomEventFilter;
use crate::r0::sync::sync_events::Language;
use crate::types::ExtraContent;
use crate::types::Member;
use crate::types::Message;
use crate::types::{Room, RoomMembership, RoomTag};

use serde_json::Value as JsonValue;

// FIXME: Remove this function, this is used only to request information we should already have
// when opening a room
pub fn set_room(bk: &Backend, id: String) -> Result<(), Error> {
    /* FIXME: remove clone and pass id by reference */
    get_room_avatar(bk, id.clone())?;
    get_room_detail(bk, id.clone(), String::from("m.room.topic"))?;

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

            let value = String::from(r[&k].as_str().unwrap_or_default());
            tx.send(BKResponse::RoomDetail(Ok((roomid, key, value))))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::RoomDetail(Err(err)))
                .expect_log("Connection closed");
        }
    );

    Ok(())
}

pub fn get_room_avatar(bk: &Backend, roomid: String) -> Result<(), Error> {
    let url = bk.url(&format!("rooms/{}/state/m.room.avatar", roomid), vec![])?;
    let baseu = bk.get_base_url();
    let tx = bk.tx.clone();
    get!(
        &url,
        |r: JsonValue| {
            let avatar = r["url"].as_str().and_then(|s| Url::parse(s).ok());
            let dest = cache_dir_path(None, &roomid).ok();
            if let Some(ref avatar) = avatar {
                let _ = dw_media(
                    &baseu,
                    avatar.as_str(),
                    ContentType::default_thumbnail(),
                    dest.as_ref().map(String::as_str),
                );
            }
            tx.send(BKResponse::RoomAvatar(Ok((roomid, avatar))))
                .expect_log("Connection closed");
        },
        |err: Error| match err {
            Error::MatrixError(ref js)
                if js["errcode"].as_str().unwrap_or_default() == "M_NOT_FOUND" =>
            {
                tx.send(BKResponse::RoomAvatar(Ok((roomid, None))))
                    .expect_log("Connection closed");
            }
            _ => {
                tx.send(BKResponse::RoomAvatar(Err(err)))
                    .expect_log("Connection closed");
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
            tx.send(BKResponse::RoomMembers(Ok((roomid, ms))))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::RoomMembers(Err(err)))
                .expect_log("Connection closed");
        }
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
            serde_json::to_string(&RoomEventFilter {
                types: Some(vec!["m.room.message", "m.sticker"]),
                ..Default::default()
            })
            .expect("Failed to serialize room messages request filter"),
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
            tx.send(BKResponse::RoomMessagesTo(Ok((list, roomid, prev_batch))))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::RoomMessagesTo(Err(err)))
                .expect_log("Connection closed");
        }
    );

    Ok(())
}

pub fn get_room_messages_from_msg(bk: &Backend, roomid: String, msg: Message) -> Result<(), Error> {
    // first of all, we calculate the from param using the context api, then we call the
    // normal get_room_messages
    let baseu = bk.get_base_url();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let itx = bk.internal_tx.clone();

    thread::spawn(move || {
        if let Ok(from) = get_prev_batch_from(&baseu, &tk, &roomid, &msg.id) {
            if let Some(t) = itx {
                t.send(BKCommand::GetRoomMessages(roomid, from))
                    .expect_log("Connection closed");
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
        &[
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
                    id = Some(msg["event_id"].as_str().unwrap_or_default().to_string());
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
                    tx.send(BKResponse::RoomMessagesTo(Err(err)))
                        .expect_log("Connection closed");
                }
            } else {
                tx.send(BKResponse::RoomMessagesTo(Ok((ms, roomid, None))))
                    .expect_log("Connection closed");
            }
        },
        |err| {
            tx.send(BKResponse::RoomMessagesTo(Err(err)))
                .expect_log("Connection closed");
        }
    );

    Ok(())
}

pub fn get_message_context(bk: &Backend, msg: Message) -> Result<(), Error> {
    let tx = bk.tx.clone();
    let baseu = bk.get_base_url();
    let roomid = msg.room.clone();
    let tk = bk.data.lock().unwrap().access_token.clone();

    parse_context(tx, tk, baseu, roomid, &msg.id, globals::PAGE_LIMIT)?;

    Ok(())
}

pub fn send_msg(bk: &Backend, msg: Message) -> Result<(), Error> {
    let roomid = msg.room.clone();

    let url = bk.url(
        &format!("rooms/{}/send/m.room.message/{}", roomid, msg.id),
        vec![],
    )?;

    let mut attrs = json!({
        "body": msg.body.clone(),
        "msgtype": msg.mtype.clone()
    });

    if let Some(ref u) = msg.url {
        attrs["url"] = json!(u);
    }

    if let (Some(f), Some(f_b)) = (msg.format.as_ref(), msg.formatted_body.as_ref()) {
        attrs["formatted_body"] = json!(f_b);
        attrs["format"] = json!(f);
    }

    if let Some(xctx) = msg.extra_content.as_ref() {
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
            tx.send(BKResponse::SentMsg(Ok((msg.id, evid.to_string()))))
                .expect_log("Connection closed");
        },
        |_| {
            tx.send(BKResponse::SentMsg(Err(Error::SendMsgError(msg.id))))
                .expect_log("Connection closed");
        }
    );

    Ok(())
}

pub fn send_typing(bk: &Backend, roomid: String) -> Result<(), Error> {
    let userid = bk.data.lock().unwrap().user_id.clone();
    let url = bk.url(&format!("rooms/{}/typing/{}", roomid, userid), vec![])?;

    let attrs = json!({
        "timeout": 4000,
        "typing": true
    });

    let tx = bk.tx.clone();
    query!("put", &url, &attrs, move |_| {}, |err| {
        tx.send(BKResponse::SendTypingError(err))
            .expect_log("Connection closed");
    });

    Ok(())
}

pub fn redact_msg(bk: &Backend, msg: &Message) -> Result<(), Error> {
    let roomid = msg.room.clone();
    let txnid = msg.id.clone();

    let url = bk.url(
        &format!("rooms/{}/redact/{}/{}", roomid, msg.id, txnid),
        vec![],
    )?;

    let attrs = json!({
        "reason": "Deletion requested by the sender"
    });

    let msgid = msg.id.clone();
    let tx = bk.tx.clone();
    query!(
        "put",
        &url,
        &attrs,
        move |js: JsonValue| {
            let evid = js["event_id"].as_str().unwrap_or_default();
            tx.send(BKResponse::SentMsgRedaction(Ok((msgid, evid.to_string()))))
                .expect_log("Connection closed");
        },
        |_| {
            tx.send(BKResponse::SentMsgRedaction(Err(
                Error::SendMsgRedactionError(msgid),
            )))
            .expect_log("Connection closed");
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
            tx.send(BKResponse::JoinRoom(Ok(())))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::JoinRoom(Err(err)))
                .expect_log("Connection closed");
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
            tx.send(BKResponse::LeaveRoom(Ok(())))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::LeaveRoom(Err(err)))
                .expect_log("Connection closed");
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
            tx.send(BKResponse::MarkedAsRead(Ok((r, e))))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::MarkedAsRead(Err(err)))
                .expect_log("Connection closed");
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
            tx.send(BKResponse::SetRoomName(Ok(())))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::SetRoomName(Err(err)))
                .expect_log("Connection closed");
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
            tx.send(BKResponse::SetRoomTopic(Ok(())))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::SetRoomTopic(Err(err)))
                .expect_log("Connection closed");
        }
    );

    Ok(())
}

pub fn set_room_avatar(bk: &Backend, roomid: &str, avatar: &str) -> Result<(), Error> {
    let baseu = bk.get_base_url();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let params = &[("access_token", tk.clone())];
    let mediaurl = media_url(&baseu, "upload", params)?;
    let roomurl = bk.url(&format!("rooms/{}/state/m.room.avatar", roomid), vec![])?;

    let mut file = File::open(&avatar)?;
    let mut contents: Vec<u8> = vec![];
    file.read_to_end(&mut contents)?;

    let tx = bk.tx.clone();
    thread::spawn(move || {
        match put_media(mediaurl.as_str(), contents) {
            Err(err) => {
                tx.send(BKResponse::SetRoomAvatar(Err(err)))
                    .expect_log("Connection closed");
            }
            Ok(js) => {
                let uri = js["content_uri"].as_str().unwrap_or_default();
                let attrs = json!({ "url": uri });
                put!(
                    &roomurl,
                    &attrs,
                    |_| {
                        tx.send(BKResponse::SetRoomAvatar(Ok(())))
                            .expect_log("Connection closed");
                    },
                    |err| {
                        tx.send(BKResponse::SetRoomAvatar(Err(err)))
                            .expect_log("Connection closed");
                    }
                );
            }
        };
    });

    Ok(())
}

pub fn set_language(bk: &Backend, roomid: &str, language_code: &str) -> Result<(), Error> {
    let userid = bk.data.lock().unwrap().user_id.clone();
    let url = bk.url(
        &format!(
            "user/{}/rooms/{}/account_data/org.gnome.fractal.language",
            userid,
            roomid.clone()
        ),
        vec![],
    )?;
    let body = json!(Language {
        input_language: language_code.to_string(),
    });

    put!(&url, &body, |_| {}, |err| {
        error!(
            "Matrix failed to set room language with error code: {:?}",
            err
        )
    });
    Ok(())
}

pub fn attach_file(bk: &Backend, mut msg: Message) -> Result<(), Error> {
    let fname = msg.url.clone().unwrap_or_default();
    let mut extra_content: ExtraContent =
        serde_json::from_value(msg.clone().extra_content.unwrap()).unwrap();
    let thumb = extra_content.info.thumbnail_url.clone().unwrap_or_default();

    let tx = bk.tx.clone();
    let itx = bk.internal_tx.clone();
    let baseu = bk.get_base_url().clone();
    let tk = bk.data.lock().unwrap().access_token.clone();

    if fname.starts_with("mxc://") && thumb.starts_with("mxc://") {
        return send_msg(bk, msg);
    }

    thread::spawn(move || {
        if thumb != "" {
            match upload_file(&tk, &baseu, &thumb) {
                Err(err) => {
                    tx.send(BKResponse::AttachedFile(Err(err)))
                        .expect_log("Connection closed");
                }
                Ok(thumb_uri) => {
                    msg.thumb = Some(thumb_uri.to_string());
                    extra_content.info.thumbnail_url = Some(thumb_uri);
                    msg.extra_content = Some(serde_json::to_value(&extra_content).unwrap());
                }
            }
            if let Err(_e) = std::fs::remove_file(&thumb) {
                error!("Can't remove thumbnail: {}", thumb);
            }
        }

        match upload_file(&tk, &baseu, &fname) {
            Err(err) => {
                tx.send(BKResponse::AttachedFile(Err(err)))
                    .expect_log("Connection closed");
            }
            Ok(uri) => {
                msg.url = Some(uri.to_string());
                if let Some(t) = itx {
                    t.send(BKCommand::SendMsg(msg.clone()))
                        .expect_log("Connection closed");
                }
                tx.send(BKResponse::AttachedFile(Ok(msg)))
                    .expect_log("Connection closed");
            }
        };
    });

    Ok(())
}

fn upload_file(tk: &str, baseu: &Url, fname: &str) -> Result<String, Error> {
    let mut file = File::open(fname)?;
    let mut contents: Vec<u8> = vec![];
    file.read_to_end(&mut contents)?;

    let params = &[("access_token", tk.to_string())];
    let mediaurl = media_url(&baseu, "upload", params)?;

    match put_media(mediaurl.as_str(), contents) {
        Err(err) => Err(err),
        Ok(js) => Ok(js["content_uri"].as_str().unwrap_or_default().to_string()),
    }
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
            let id = String::from(r["room_id"].as_str().unwrap_or_default());
            let mut r = Room::new(id, RoomMembership::Joined(RoomTag::None));
            r.name = Some(n);
            tx.send(BKResponse::NewRoom(Ok(r), internal_id))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::NewRoom(Err(err), internal_id))
                .expect_log("Connection closed");
        }
    );
    Ok(())
}

pub fn update_direct_chats(url: Url, data: Arc<Mutex<BackendData>>, user: String, room: String) {
    get!(
        &url,
        |r: JsonValue| {
            let mut directs: HashMap<String, Vec<String>> = HashMap::new();
            let direct_obj = r.as_object().unwrap();

            direct_obj.iter().for_each(|(userid, rooms)| {
                let roomlist: Vec<String> = rooms
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_str().unwrap().to_string())
                    .collect();
                directs.insert(userid.clone(), roomlist);
            });

            if directs.contains_key(&user) {
                if let Some(v) = directs.get_mut(&user) {
                    v.push(room)
                };
            } else {
                directs.insert(user, vec![room]);
            }
            data.lock().unwrap().m_direct = directs.clone();

            let attrs = json!(directs.clone());
            put!(&url, &attrs, |_| {}, |err| error!("{:?}", err));
        },
        |err| {
            error!("Can't set m.direct: {:?}", err);
        }
    );
}

pub fn direct_chat(bk: &Backend, user: &Member, internal_id: String) -> Result<(), Error> {
    let url = bk.url("createRoom", vec![])?;
    let attrs = json!({
        "invite": [user.uid.clone()],
        "invite_3pid": [],
        "visibility": "private",
        "preset": "private_chat",
        "is_direct": true,
        "state_event": {
            "type": "m.room.history_visibility",
            "content": {
                "history_visibility": "invited"
            }
        }
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
            let id = String::from(r["room_id"].as_str().unwrap_or_default());
            let mut r = Room::new(id.clone(), RoomMembership::Joined(RoomTag::None));
            r.name = m.alias.clone();
            r.direct = true;
            tx.send(BKResponse::NewRoom(Ok(r), internal_id))
                .expect_log("Connection closed");

            update_direct_chats(direct_url, data, m.uid.clone(), id);
        },
        |err| {
            tx.send(BKResponse::NewRoom(Err(err), internal_id))
                .expect_log("Connection closed");
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
            tx.send(BKResponse::AddedToFav(Ok((roomid.clone(), tofav))))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::AddedToFav(Err(err)))
                .expect_log("Connection closed");
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
        tx.send(BKResponse::InviteError(err))
            .expect_log("Connection closed");
    });

    Ok(())
}

fn put_media(url: &str, file: Vec<u8>) -> Result<JsonValue, Error> {
    let (mime, _) = gio::content_type_guess(None, &file);

    HTTP_CLIENT
        .get_client()?
        .post(url)
        .body(file)
        .header(CONTENT_TYPE, mime.to_string())
        .send()?
        .json()
        .or(Err(Error::BackendError))
}
