pub use crate::backend::types::{BKResponse, Backend};

use crate::{
    backend::types::{BKCommand, RoomType},
    error::Error,
    globals,
    types::{Member, Message, Room},
    util::{self, cache_path, client_url, json_q, media_url, put_media, thumb},
    JsonValue,
};
use log::error;
use serde_json::{self, json};
use std::{fs, sync::mpsc::Sender, thread};
use url::Url;
use urlencoding;

impl Backend {
    pub fn get_room_members(&self, room_id: String) {
        let ctx = self.tx.clone();
        let url = self.url(&format!("rooms/{}/joined_members", room_id), vec![]);

        get!(
            &url,
            |r: JsonValue| {
                let joined = r["joined"].as_object().unwrap();
                let ms = joined
                    .iter()
                    .map(|(mxid, member_data)| {
                        let mut member: Member =
                            serde_json::from_value(member_data.clone()).unwrap();
                        member.uid = mxid.to_string();
                        member
                    })
                    .collect();
                ctx.send(BKResponse::RoomMembers(room_id, ms)).unwrap();
            },
            |err| ctx.send(BKResponse::RoomMembersError(err)).unwrap()
        );
    }

    // Load older messages starting by prev_batch
    // https://matrix.org/docs/spec/client_server/latest.html#get-matrix-client-r0-rooms-room_id-messages
    pub fn get_room_messages(&self, room_id: String, from: String) {
        let ctx = self.tx.clone();
        let params = vec![
            ("from", from),
            ("dir", "b".to_string()),
            ("limit", globals::PAGE_LIMIT.to_string()),
            (
                "filter",
                "{ \"types\": [\"m.room.message\", \"m.sticker\"] }".to_string(),
            ),
        ];
        let url = self.url(&format!("rooms/{}/messages", room_id), params);
        get!(
            &url,
            |r: JsonValue| {
                let array = r["chunk"].as_array();
                let evs = array.unwrap().iter().rev();
                let list = Message::from_json_events_iter(&room_id, evs);
                let prev_batch = r["end"].as_str().map(Into::into);
                ctx.send(BKResponse::RoomMessagesTo(list, room_id, prev_batch))
                    .unwrap();
            },
            |err| ctx.send(BKResponse::RoomMembersError(err)).unwrap()
        );
    }

    pub fn get_room_messages_from_msg(&self, room_id: String, msg: Message) {
        let itx = self.internal_tx.clone();
        // first of all, we calculate the from param using the context api, then we call the
        // normal get_room_messages
        let baseu = self.get_base_url();
        let tk = self.data.lock().unwrap().access_token.clone();
        let id = msg.id.unwrap_or_default();

        thread::spawn(move || {
            if let Ok(from) = util::get_prev_batch_from(&baseu, &tk, &room_id, &id) {
                if let Some(itx) = itx {
                    itx.send(BKCommand::GetRoomMessages(room_id, from)).unwrap();
                }
            }
        });
    }

    pub fn get_message_context(&self, msg: Message) {
        let tx = self.tx.clone();
        let r = get_message_context(self, msg);
        bkerror!(r, tx, BKResponse::RoomMessagesError);
    }

    pub fn send_msg(&self, msg: Message) {
        let ctx = self.tx.clone();
        let room_id = msg.room.clone();

        let id = msg.id.unwrap_or_default();
        let url = self.url(
            &format!("rooms/{}/send/m.room.message/{}", room_id, id),
            vec![],
        );

        let mut attrs = json!({
            "body": msg.body.clone(),
            "msgtype": msg.mtype.clone()
        });

        if let Some(f) = msg.format {
            attrs["format"] = json!(f);
        }

        if let Some(f_b) = msg.formatted_body {
            attrs["formatted_body"] = json!(f_b);
        }

        if let Some(xctx) = msg.extra_content {
            if let Some(xctx) = xctx.as_object() {
                for (k, v) in xctx {
                    attrs[k] = v.clone();
                }
            }
        }

        put!(
            &url,
            &attrs,
            move |js: JsonValue| {
                let evid = js["event_id"].as_str().unwrap_or_default();
                ctx.send(BKResponse::SentMsg(id, evid.to_string())).unwrap();
            },
            |_| ctx
                .send(BKResponse::SendMsgError(Error::SendMsgError(id)))
                .unwrap()
        );
    }

    pub fn send_msg_redaction(&self, msg: Message) {
        let ctx = self.tx.clone();
        let room_id = msg.room.clone();
        let msgid = msg.id.clone().unwrap_or_default();
        let txnid = msg.get_txn_id();

        let url = self.url(
            &format!("rooms/{}/redact/{}/{}", room_id, msgid, txnid),
            vec![],
        );

        let attrs = json!({
            "reason": "Deletion requested by the sender"
        });

        put!(
            &url,
            &attrs,
            move |js: JsonValue| {
                let evid = js["event_id"].as_str().unwrap_or_default().to_string();
                ctx.send(BKResponse::SentMsgRedaction(msgid, evid)).unwrap();
            },
            |_| ctx
                .send(BKResponse::SendMsgRedactionError(
                    Error::SendMsgRedactionError(msgid)
                ))
                .unwrap()
        );
    }

    pub fn set_room(&self, id: String) {
        self.get_room_detail(id.clone(), "m.room.topic".to_string());
        self.get_room_avatar(id.clone());
        self.get_room_members(id);
    }

    pub fn get_room_avatar(&self, room_id: String) {
        let ctx = self.tx.clone();
        let userid = self.data.lock().unwrap().user_id.clone();
        let baseu = self.get_base_url();
        let tk = self.data.lock().unwrap().access_token.clone();
        let url = self.url(&format!("rooms/{}/state/m.room.avatar", room_id), vec![]);

        get!(
            &url,
            |r: JsonValue| {
                let avatar = r["url"]
                    .as_str()
                    .map(|u| {
                        cache_path(&room_id)
                            .and_then(|dest| thumb(&baseu, u, Some(&dest)))
                            .unwrap_or_default()
                    })
                    .or(util::get_room_avatar(&baseu, &tk, &userid, &room_id).ok())
                    .unwrap_or_default();
                ctx.send(BKResponse::RoomAvatar(room_id, avatar)).unwrap();
            },
            |err: Error| match err {
                Error::MatrixError(ref js)
                    if js["errcode"].as_str().unwrap_or_default() == "M_NOT_FOUND" =>
                {
                    let avatar =
                        util::get_room_avatar(&baseu, &tk, &userid, &room_id).unwrap_or_default();
                    ctx.send(BKResponse::RoomAvatar(room_id, avatar)).unwrap();
                }
                _ => ctx.send(BKResponse::RoomAvatarError(err)).unwrap(),
            }
        );
    }

    pub fn join_room(&self, room_id: String) {
        let ctx = self.tx.clone();
        let url = self.url(&format!("join/{}", urlencoding::encode(&room_id)), vec![]);

        let data = self.data.clone();
        post!(
            &url,
            move |_| {
                data.lock().unwrap().join_to_room = room_id.clone();
                ctx.send(BKResponse::JoinRoom).unwrap();
            },
            |err| ctx.send(BKResponse::JoinRoomError(err)).unwrap()
        );
    }

    pub fn leave_room(&self, room_id: String) {
        let ctx = self.tx.clone();
        let url = self.url(&format!("rooms/{}/leave", room_id), vec![]);

        post!(
            &url,
            move |_| ctx.send(BKResponse::LeaveRoom).unwrap(),
            |err| ctx.send(BKResponse::LeaveRoomError(err)).unwrap()
        );
    }

    pub fn mark_as_read(&self, room_id: String, eventid: String) {
        let ctx = self.tx.clone();
        let url = self.url(
            &format!("rooms/{}/receipt/m.read/{}", &room_id, &eventid),
            vec![],
        );

        let r = room_id.clone();
        let e = eventid.clone();
        post!(
            &url,
            move |_| ctx.send(BKResponse::MarkedAsRead(r, e)).unwrap(),
            |err| ctx.send(BKResponse::MarkAsReadError(err)).unwrap()
        );

        // send fully_read event
        // This event API call isn't in the current doc but I found this in the
        // matrix-js-sdk
        // https://github.com/matrix-org/matrix-js-sdk/blob/master/src/base-apis.js#L851
        let url = self.url(&format!("rooms/{}/read_markers", room_id), vec![]);
        let attrs = json!({
            "m.fully_read": eventid,
            "m.read": json!(null),
        });
        post!(&url, &attrs, |_| {}, |_| {});
    }

    pub fn set_room_name(&self, room_id: String, name: String) {
        let ctx = self.tx.clone();
        let url = self.url(&format!("rooms/{}/state/m.room.name", room_id), vec![]);

        let attrs = json!({
            "name": name,
        });

        put!(
            &url,
            &attrs,
            |_| ctx.send(BKResponse::SetRoomName).unwrap(),
            |err| ctx.send(BKResponse::SetRoomNameError(err)).unwrap()
        );
    }

    pub fn set_room_topic(&self, room_id: String, topic: String) {
        let ctx = self.tx.clone();
        let url = self.url(&format!("rooms/{}/state/m.room.topic", room_id), vec![]);

        let attrs = json!({
            "topic": topic,
        });

        put!(
            &url,
            &attrs,
            |_| ctx.send(BKResponse::SetRoomTopic).unwrap(),
            |err| ctx.send(BKResponse::SetRoomTopicError(err)).unwrap()
        );
    }

    pub fn set_room_avatar(&self, room_id: String, fname: String) {
        let tx = self.tx.clone();
        let r = set_room_avatar(self, room_id, fname);
        bkerror!(r, tx, BKResponse::SetRoomAvatarError);
    }

    pub fn attach_file(&self, msg: Message) {
        let tx = self.tx.clone();
        let r = attach_file(self, msg);
        bkerror!(r, tx, BKResponse::AttachFileError);
    }

    pub fn new_room(&self, name: String, privacy: RoomType, internal_id: String) {
        let ctx = self.tx.clone();
        let url = self.url("createRoom", vec![]);
        let attrs = json!({
            "invite": [],
            "invite_3pid": [],
            "name": name,
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

        post!(
            &url,
            &attrs,
            move |r: JsonValue| {
                let id = r["room_id"].as_str().unwrap_or_default().to_string();
                let r = Room::new(id, Some(name));
                ctx.send(BKResponse::NewRoom(r, internal_id)).unwrap();
            },
            |err| ctx
                .send(BKResponse::NewRoomError(err, internal_id))
                .unwrap()
        );
    }

    pub fn direct_chat(&self, user: Member, internal_id: String) {
        let ctx = self.tx.clone();
        let url = self.url("createRoom", vec![]);
        let attrs = json!({
            "invite": [user.uid.clone()],
            "invite_3pid": [],
            "visibility": "private",
            "preset": "private_chat",
            "is_direct": true,
        });

        let userid = self.data.lock().unwrap().user_id.clone();
        let direct_url = self.url(&format!("user/{}/account_data/m.direct", userid), vec![]);

        let data = self.data.clone();
        post!(
            &url,
            &attrs,
            move |r: JsonValue| {
                let id = r["room_id"].as_str().unwrap_or_default().to_string();
                let mut r = Room::new(id.clone(), user.alias);
                r.direct = true;
                ctx.send(BKResponse::NewRoom(r, internal_id)).unwrap();

                let directs = &mut data.lock().unwrap().m_direct;
                if directs.contains_key(&user.uid) {
                    if let Some(v) = directs.get_mut(&user.uid) {
                        v.push(id)
                    };
                } else {
                    directs.insert(user.uid, vec![id]);
                }

                let attrs = json!(directs.clone());
                json_q("put", &direct_url, &attrs, 0)
                    .map_err(|err| error!("{:?}", err))
                    .unwrap_or_default();
            },
            |err| ctx
                .send(BKResponse::NewRoomError(err, internal_id))
                .unwrap()
        );
    }

    pub fn add_to_fav(&self, room_id: String, tofav: bool) {
        let ctx = self.tx.clone();
        let userid = self.data.lock().unwrap().user_id.clone();
        let url = self.url(
            &format!("user/{}/rooms/{}/tags/m.favourite", userid, room_id),
            vec![],
        );

        let attrs = json!({
            "order": 0.5,
        });

        let method = if tofav { "put" } else { "delete" };
        query!(
            method,
            &url,
            &attrs,
            |_| ctx
                .send(BKResponse::AddedToFav(room_id.clone(), tofav))
                .unwrap(),
            |err| ctx.send(BKResponse::AddToFavError(err)).unwrap()
        );
    }

    pub fn accept_inv(&self, room_id: String) {
        self.join_room(room_id)
    }

    pub fn reject_inv(&self, room_id: String) {
        self.leave_room(room_id)
    }

    pub fn invite(&self, room_id: String, userid: String) {
        let ctx = self.tx.clone();
        let url = self.url(&format!("rooms/{}/invite", room_id), vec![]);

        let attrs = json!({
            "user_id": userid,
        });

        post!(&url, &attrs, |_| {}, |err| ctx
            .send(BKResponse::InviteError(err))
            .unwrap());
    }

    fn get_room_detail(&self, room_id: String, key: String) {
        let ctx = self.tx.clone();
        let url = self.url(&format!("rooms/{}/state/{}", room_id, key), vec![]);

        let keys = key.clone();
        get!(
            &url,
            |r: JsonValue| {
                let k = keys.split('.').last().unwrap();
                let value = r[&k].as_str().unwrap_or_default().to_string();
                ctx.send(BKResponse::RoomDetail(room_id, key, value))
                    .unwrap();
            },
            |err| ctx.send(BKResponse::RoomDetailError(err)).unwrap()
        );
    }
}

fn get_message_context(bk: &Backend, msg: Message) -> Result<(), Error> {
    let ctx = bk.tx.clone();
    let baseu = bk.get_base_url();
    let room_id = msg.room.clone();
    let msgid = msg.id.unwrap_or_default();
    let tk = bk.data.lock().unwrap().access_token.clone();

    parse_context(ctx, tk, baseu, room_id, &msgid, globals::PAGE_LIMIT)?;

    Ok(())
}

fn set_room_avatar(bk: &Backend, room_id: String, fname: String) -> Result<(), Error> {
    let ctx = bk.tx.clone();
    let baseu = bk.get_base_url();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let params = &[("access_token", tk.clone())];
    let mediaurl = media_url(&baseu, "upload", params)?;
    let roomurl = bk.url(&format!("rooms/{}/state/m.room.avatar", &room_id), vec![]);

    let contents = fs::read(&fname)?;

    thread::spawn(move || match put_media(mediaurl.as_str(), contents) {
        Ok(js) => {
            let attrs = json!({ "url": js["content_uri"].as_str().unwrap_or_default() });
            put!(
                &roomurl,
                &attrs,
                |_| ctx.send(BKResponse::SetRoomAvatar).unwrap(),
                |err| ctx.send(BKResponse::SetRoomAvatarError(err)).unwrap(),
                0
            );
        }
        Err(err) => ctx.send(BKResponse::SetRoomAvatarError(err)).unwrap(),
    });

    Ok(())
}

fn attach_file(bk: &Backend, msg: Message) -> Result<(), Error> {
    let ctx = bk.tx.clone();
    let itx = bk.internal_tx.clone();
    let fname = msg.url.clone().unwrap_or_default();

    if fname.starts_with("mxc://") {
        bk.send_msg(msg);
        return Ok(());
    }

    let contents = fs::read(&fname)?;

    let baseu = bk.get_base_url();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let params = &[("access_token", tk)];
    let mediaurl = media_url(&baseu, "upload", params)?;

    let mut m = msg.clone();
    thread::spawn(move || match put_media(mediaurl.as_str(), contents) {
        Ok(js) => {
            m.url = js["content_uri"].as_str().map(Into::into);
            itx.map(|t| t.send(BKCommand::SendMsg(m.clone())).unwrap())
                .unwrap_or_default();
            ctx.send(BKResponse::AttachedFile(m)).unwrap();
        }
        Err(err) => ctx.send(BKResponse::AttachFileError(err)).unwrap(),
    });

    Ok(())
}

fn parse_context(
    ctx: Sender<BKResponse>,
    tk: String,
    baseu: Url,
    room_id: String,
    eid: &str,
    limit: i32,
) -> Result<(), Error> {
    let url = client_url(
        &baseu,
        &format!("rooms/{}/context/{}", room_id, eid),
        &[("limit", limit.to_string()), ("access_token", tk.clone())],
    )?;

    get!(
        &url,
        |r: JsonValue| {
            let mut id: Option<String> = None;

            let ms = r["events_before"]
                .as_array()
                .unwrap()
                .iter()
                .rev()
                .filter_map(|msg| {
                    if id.is_none() {
                        id = Some(msg["event_id"].as_str().unwrap_or_default().to_string());
                    }
                    if Message::supported_event(&&msg) {
                        Some(Message::parse_room_message(&room_id, msg))
                    } else {
                        None
                    }
                })
                .collect::<Vec<Message>>();

            if ms.is_empty() && id.is_some() {
                // there's no messages so we'll try with a bigger context
                if let Err(err) =
                    parse_context(ctx.clone(), tk, baseu, room_id, &id.unwrap(), limit * 2)
                {
                    ctx.send(BKResponse::RoomMessagesError(err)).unwrap()
                }
            } else {
                ctx.send(BKResponse::RoomMessagesTo(ms, room_id, None))
                    .unwrap()
            }
        },
        |err| ctx.send(BKResponse::RoomMessagesError(err)).unwrap()
    );

    Ok(())
}
