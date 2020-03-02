use log::error;
use serde_json::json;

use ruma_identifiers::{Error as IdError, RoomId, RoomIdOrAliasId, UserId};
use std::fs;
use std::sync::mpsc::Sender;
use url::Url;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::Error;
use crate::globals;
use std::thread;

use crate::util::cache_dir_path;
use crate::util::client_url;
use crate::util::dw_media;
use crate::util::get_prev_batch_from;
use crate::util::json_q;
use crate::util::ContentType;
use crate::util::ResultExpectLog;
use crate::util::HTTP_CLIENT;

use crate::backend::types::BKCommand;
use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::backend::types::BackendData;
use crate::backend::types::RoomType;

use crate::r0::filter::RoomEventFilter;
use crate::r0::media::create::request as create_content;
use crate::r0::media::create::Parameters as CreateContentParameters;
use crate::r0::media::create::Response as CreateContentResponse;
use crate::r0::membership::invite_user::request as invite_user;
use crate::r0::membership::invite_user::Body as InviteUserBody;
use crate::r0::membership::invite_user::Parameters as InviteUserParameters;
use crate::r0::membership::join_room_by_id_or_alias::request as join_room_req;
use crate::r0::membership::join_room_by_id_or_alias::Parameters as JoinRoomParameters;
use crate::r0::membership::leave_room::request as leave_room_req;
use crate::r0::membership::leave_room::Parameters as LeaveRoomParameters;
use crate::r0::message::get_message_events::request as get_messages_events;
use crate::r0::message::get_message_events::Direction as GetMessagesEventsDirection;
use crate::r0::message::get_message_events::Parameters as GetMessagesEventsParams;
use crate::r0::message::get_message_events::Response as GetMessagesEventsResponse;
use crate::r0::state::get_state_events_for_key::request as get_state_events_for_key;
use crate::r0::state::get_state_events_for_key::Parameters as GetStateEventsForKeyParameters;
use crate::r0::sync::get_joined_members::request as get_joined_members;
use crate::r0::sync::get_joined_members::Parameters as JoinedMembersParameters;
use crate::r0::sync::get_joined_members::Response as JoinedMembersResponse;
use crate::r0::sync::sync_events::Language;
use crate::r0::tag::create_tag::request as create_tag;
use crate::r0::tag::create_tag::Body as CreateTagBody;
use crate::r0::tag::create_tag::Parameters as CreateTagParameters;
use crate::r0::tag::delete_tag::request as delete_tag;
use crate::r0::tag::delete_tag::Parameters as DeleteTagParameters;
use crate::r0::typing::request as send_typing_notification;
use crate::r0::typing::Body as TypingNotificationBody;
use crate::r0::typing::Parameters as TypingNotificationParameters;
use crate::r0::AccessToken;
use crate::types::ExtraContent;
use crate::types::Member;
use crate::types::Message;
use crate::types::{Room, RoomMembership, RoomTag};

use serde_json::Value as JsonValue;

// FIXME: Remove this function, this is used only to request information we should already have
// when opening a room
pub fn set_room(bk: &Backend, base: Url, access_token: AccessToken, room_id: RoomId) {
    if let Some(itx) = bk.internal_tx.clone() {
        itx.send(BKCommand::GetRoomAvatar(
            base.clone(),
            access_token.clone(),
            room_id.clone(),
        ))
        .expect_log("Connection closed");

        let tx = bk.tx.clone();

        thread::spawn(move || {
            let query = get_room_detail(base, access_token, room_id, "m.room.topic".into());
            tx.send(BKResponse::RoomDetail(query))
                .expect_log("Connection closed");
        });
    }
}

fn get_room_detail(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    keys: String,
) -> Result<(RoomId, String, String), Error> {
    let params = GetStateEventsForKeyParameters { access_token };

    get_state_events_for_key(base, &params, &room_id, &keys)
        .map_err(Into::into)
        .and_then(|request| {
            let response = HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<JsonValue>()?;

            let k = keys.split('.').last().unwrap();
            let value = response[&k].as_str().map(Into::into).unwrap_or_default();

            Ok((room_id, keys, value))
        })
}

pub fn get_room_avatar(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
) -> Result<(RoomId, Option<Url>), Error> {
    let params = GetStateEventsForKeyParameters { access_token };

    get_state_events_for_key(base.clone(), &params, &room_id, "m.room.avatar")
        .map_err(Into::into)
        .and_then(|request| {
            let response = HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<JsonValue>()?;

            let avatar = response["url"].as_str().and_then(|s| Url::parse(s).ok());
            let dest = cache_dir_path(None, &room_id.to_string()).ok();
            if let Some(ref avatar) = avatar {
                let _ = dw_media(
                    &base,
                    avatar.as_str(),
                    ContentType::default_thumbnail(),
                    dest.as_ref().map(String::as_str),
                );
            }

            Ok((room_id.clone(), avatar))
        })
        .or_else(|err| match err {
            Error::MatrixError(ref js)
                if js["errcode"].as_str().unwrap_or_default() == "M_NOT_FOUND" =>
            {
                Ok((room_id, None))
            }
            error => Err(error),
        })
}

pub fn get_room_members(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
) -> Result<(RoomId, Vec<Member>), Error> {
    let params = JoinedMembersParameters { access_token };

    get_joined_members(base, &room_id, &params)
        .map_err(Into::into)
        .and_then(|request| {
            let response = HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<JoinedMembersResponse>()?;

            let ms = response.joined.into_iter().map(Member::from).collect();

            Ok((room_id, ms))
        })
}

/* Load older messages starting by prev_batch
 * https://matrix.org/docs/spec/client_server/latest.html#get-matrix-client-r0-rooms-roomid-messages
 */
pub fn get_room_messages(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    from: String,
) -> Result<(Vec<Message>, RoomId, Option<String>), Error> {
    let params = GetMessagesEventsParams {
        access_token,
        from,
        to: None,
        dir: GetMessagesEventsDirection::Backward,
        limit: globals::PAGE_LIMIT as u64,
        filter: RoomEventFilter {
            types: Some(vec!["m.room.message", "m.sticker"]),
            ..Default::default()
        },
    };

    get_messages_events(base, &params, &room_id)
        .map_err(Into::into)
        .and_then(|request| {
            let response = HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<GetMessagesEventsResponse>()?;

            let prev_batch = response.end;
            let evs = response.chunk.iter().rev();
            Message::from_json_events_iter(&room_id, evs)
                .map(|list| (list, room_id, prev_batch))
                .map_err(Into::into)
        })
}

pub fn get_room_messages_from_msg(
    bk: &Backend,
    baseu: Url,
    tk: AccessToken,
    room_id: RoomId,
    msg: Message,
) {
    // first of all, we calculate the from param using the context api, then we call the
    // normal get_room_messages
    let itx = bk.internal_tx.clone();

    thread::spawn(move || {
        if let Ok(from) = get_prev_batch_from(&baseu, &tk, &room_id, &msg.id) {
            if let Some(t) = itx {
                t.send(BKCommand::GetRoomMessages(baseu, tk, room_id, from))
                    .expect_log("Connection closed");
            }
        }
    });
}

fn parse_context(
    tx: Sender<BKResponse>,
    tk: AccessToken,
    baseu: Url,
    room_id: RoomId,
    eid: &str,
    limit: i32,
) -> Result<(), Error> {
    let url = client_url(
        &baseu,
        &format!("rooms/{}/context/{}", room_id, eid),
        &[
            ("limit", format!("{}", limit)),
            ("access_token", tk.to_string()),
        ],
    )?;

    get!(
        url,
        |response: JsonValue| {
            let mut id: Option<String> = None;

            let ms: Result<Vec<Message>, _> = response["events_before"]
                .as_array()
                .into_iter()
                .flatten()
                .rev()
                .inspect(|msg| {
                    if id.is_none() {
                        id = Some(msg["event_id"].as_str().unwrap_or_default().to_string());
                    }
                })
                .filter(|msg| Message::supported_event(&&msg))
                .map(|msg| Message::parse_room_message(&room_id, msg))
                .collect();

            match (ms, id) {
                (Ok(msgs), Some(ref id)) if msgs.is_empty() => {
                    // there's no messages so we'll try with a bigger context
                    if let Err(err) = parse_context(tx.clone(), tk, baseu, room_id, id, limit * 2) {
                        tx.send(BKResponse::RoomMessagesTo(Err(err)))
                            .expect_log("Connection closed");
                    }
                }
                (Ok(msgs), _) => {
                    tx.send(BKResponse::RoomMessagesTo(Ok((msgs, room_id, None))))
                        .expect_log("Connection closed");
                }
                (Err(err), _) => {
                    tx.send(BKResponse::RoomMessagesTo(Err(err.into())))
                        .expect_log("Connection closed");
                }
            }
        },
        |err| {
            tx.send(BKResponse::RoomMessagesTo(Err(err)))
                .expect_log("Connection closed");
        }
    );

    Ok(())
}

pub fn get_message_context(
    bk: &Backend,
    baseu: Url,
    tk: AccessToken,
    msg: Message,
) -> Result<(), Error> {
    let tx = bk.tx.clone();
    let room_id: RoomId = msg.room.clone();

    parse_context(tx, tk, baseu, room_id, &msg.id, globals::PAGE_LIMIT)?;

    Ok(())
}

pub fn send_msg(
    bk: &Backend,
    base: Url,
    access_token: AccessToken,
    msg: Message,
) -> Result<(), Error> {
    let room_id: RoomId = msg.room.clone();

    let url = bk.url(
        base,
        &access_token,
        &format!("rooms/{}/send/m.room.message/{}", room_id, msg.id),
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
    put!(
        url,
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

pub fn send_typing(
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    room_id: RoomId,
) -> Result<(), Error> {
    let params = TypingNotificationParameters { access_token };
    let body = TypingNotificationBody::Typing(Duration::from_secs(4));

    send_typing_notification(base, &room_id, &user_id, &params, &body)
        .map_err(Into::into)
        .and_then(|request| {
            HTTP_CLIENT
                .get_client()?
                .execute(request)
                .map_err(Into::into)
        })
        .and(Ok(()))
}

pub fn redact_msg(
    bk: &Backend,
    base: Url,
    access_token: AccessToken,
    msg: &Message,
) -> Result<(), Error> {
    let room_id: RoomId = msg.room.clone();
    let txnid = msg.id.clone();

    let url = bk.url(
        base,
        &access_token,
        &format!("rooms/{}/redact/{}/{}", room_id, msg.id, txnid),
        vec![],
    )?;

    let attrs = json!({
        "reason": "Deletion requested by the sender"
    });

    let msgid = msg.id.clone();
    let tx = bk.tx.clone();
    put!(
        url,
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

pub fn join_room(bk: &Backend, base: Url, access_token: AccessToken, room_id: RoomId) {
    let tx = bk.tx.clone();
    let data = bk.data.clone();

    let room_id_or_alias_id = RoomIdOrAliasId::RoomId(room_id.clone());

    let params = JoinRoomParameters {
        access_token,
        server_name: Default::default(),
    };

    thread::spawn(move || {
        let query = join_room_req(base, &room_id_or_alias_id, &params)
            .map_err(Into::into)
            .and_then(|request| {
                HTTP_CLIENT
                    .get_client()?
                    .execute(request)
                    .map_err(Into::into)
            })
            .and(Ok(()));

        if let Ok(_) = query {
            data.lock().unwrap().join_to_room = Some(room_id);
        }

        tx.send(BKResponse::JoinRoom(query))
            .expect_log("Connection closed");
    });
}

pub fn leave_room(base: Url, access_token: AccessToken, room_id: RoomId) -> Result<(), Error> {
    let params = LeaveRoomParameters { access_token };

    leave_room_req(base, &room_id, &params)
        .map_err(Into::into)
        .and_then(|request| {
            HTTP_CLIENT
                .get_client()?
                .execute(request)
                .map_err(Into::into)
        })
        .and(Ok(()))
}

pub fn mark_as_read(
    bk: &Backend,
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    eventid: String,
) -> Result<(), Error> {
    let url = bk.url(
        base.clone(),
        &access_token,
        &format!("rooms/{}/receipt/m.read/{}", room_id, eventid),
        vec![],
    )?;

    let r = room_id.clone();
    let e = eventid.clone();
    let tx = bk.tx.clone();
    post!(
        url,
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
    let url = bk.url(
        base,
        &access_token,
        &format!("rooms/{}/read_markers", room_id),
        vec![],
    )?;
    let attrs = json!({
        "m.fully_read": eventid,
        "m.read": json!(null),
    });
    post!(url, &attrs, |_| {}, |_| {});

    Ok(())
}

pub fn set_room_name(
    bk: &Backend,
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    name: String,
) -> Result<(), Error> {
    let url = bk.url(
        base,
        &access_token,
        &format!("rooms/{}/state/m.room.name", room_id),
        vec![],
    )?;

    let attrs = json!({
        "name": name,
    });

    let tx = bk.tx.clone();
    put!(
        url,
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

pub fn set_room_topic(
    bk: &Backend,
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    topic: String,
) -> Result<(), Error> {
    let url = bk.url(
        base,
        &access_token,
        &format!("rooms/{}/state/m.room.topic", room_id),
        vec![],
    )?;

    let attrs = json!({
        "topic": topic,
    });

    let tx = bk.tx.clone();
    put!(
        url,
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

pub fn set_room_avatar(
    bk: &Backend,
    baseu: Url,
    tk: AccessToken,
    room_id: RoomId,
    avatar: String,
) -> Result<(), Error> {
    let roomurl = bk.url(
        baseu.clone(),
        &tk,
        &format!("rooms/{}/state/m.room.avatar", room_id),
        vec![],
    )?;

    let tx = bk.tx.clone();
    thread::spawn(move || {
        let query = upload_file(baseu, tk, &avatar).and_then(|response| {
            let js = json!({ "url": response.content_uri.as_str() });

            HTTP_CLIENT
                .get_client()?
                .put(roomurl)
                .json(&js)
                .send()
                .map_err(Into::into)
                .and(Ok(()))
        });

        tx.send(BKResponse::SetRoomAvatar(query))
            .expect_log("Connection closed");
    });

    Ok(())
}

pub fn attach_file(
    bk: &Backend,
    baseu: Url,
    tk: AccessToken,
    mut msg: Message,
) -> Result<(), Error> {
    let fname = msg.url.clone().unwrap_or_default();
    let mut extra_content: Option<ExtraContent> = msg
        .clone()
        .extra_content
        .and_then(|c| serde_json::from_value(c).ok());

    let thumb = extra_content
        .clone()
        .and_then(|c| c.info.thumbnail_url)
        .unwrap_or_default();

    let tx = bk.tx.clone();
    let itx = bk.internal_tx.clone();

    if fname.starts_with("mxc://") && thumb.starts_with("mxc://") {
        return send_msg(bk, baseu, tk, msg);
    }

    thread::spawn(move || {
        if !thumb.is_empty() {
            match upload_file(baseu.clone(), tk.clone(), &thumb) {
                Err(err) => {
                    tx.send(BKResponse::AttachedFile(Err(err)))
                        .expect_log("Connection closed");
                }
                Ok(response) => {
                    let thumb_uri = response.content_uri.to_string();
                    msg.thumb = Some(thumb_uri.clone());
                    if let Some(ref mut xctx) = extra_content {
                        xctx.info.thumbnail_url = Some(thumb_uri);
                    }
                    msg.extra_content = serde_json::to_value(&extra_content).ok();
                }
            }

            if let Err(_e) = std::fs::remove_file(&thumb) {
                error!("Can't remove thumbnail: {}", thumb);
            }
        }

        let query = upload_file(baseu.clone(), tk.clone(), &fname).map(|response| {
            msg.url = Some(response.content_uri.to_string());
            if let Some(t) = itx {
                t.send(BKCommand::SendMsg(baseu, tk, msg.clone()))
                    .expect_log("Connection closed");
            }

            msg
        });

        tx.send(BKResponse::AttachedFile(query))
            .expect_log("Connection closed");
    });

    Ok(())
}

fn upload_file(
    base: Url,
    access_token: AccessToken,
    fname: &str,
) -> Result<CreateContentResponse, Error> {
    let params_upload = CreateContentParameters {
        access_token,
        filename: None,
    };

    let contents = fs::read(fname)?;

    create_content(base, &params_upload, contents)
        .map_err::<Error, _>(Into::into)
        .and_then(|request| {
            HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<CreateContentResponse>()
                .map_err(Into::into)
        })
}

pub fn new_room(
    bk: &Backend,
    base: Url,
    access_token: AccessToken,
    name: String,
    privacy: RoomType,
    internal_id: RoomId,
) -> Result<(), Error> {
    let url = bk.url(base, &access_token, "createRoom", vec![])?;
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

    let tx = bk.tx.clone();
    post!(
        url,
        &attrs,
        move |r: JsonValue| {
            let room_res = RoomId::try_from(r["room_id"].as_str().unwrap_or_default())
                .map_err(Into::into)
                .map(|room_id| Room {
                    name: Some(name),
                    ..Room::new(room_id, RoomMembership::Joined(RoomTag::None))
                });

            tx.send(BKResponse::NewRoom(room_res, internal_id))
                .expect_log("Connection closed");
        },
        |err| {
            tx.send(BKResponse::NewRoom(Err(err), internal_id))
                .expect_log("Connection closed");
        }
    );
    Ok(())
}

fn update_direct_chats(url: Url, data: Arc<Mutex<BackendData>>, user_id: UserId, room_id: RoomId) {
    get!(
        url.clone(),
        |r: JsonValue| {
            let directs: Result<HashMap<UserId, Vec<RoomId>>, IdError> = r
                .as_object()
                .into_iter()
                .flatten()
                .map(|(uid, rooms)| {
                    let roomlist = rooms
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| RoomId::try_from(x.as_str().unwrap_or_default()))
                        .collect::<Result<Vec<RoomId>, IdError>>()?;
                    Ok((UserId::try_from(uid.as_str())?, roomlist))
                })
                .collect();

            if let Ok(mut directs) = directs {
                if let Some(v) = directs.get_mut(&user_id) {
                    v.push(room_id);
                } else {
                    directs.insert(user_id, vec![room_id]);
                }
                data.lock().unwrap().m_direct = directs.clone();

                let attrs = json!(directs);
                put!(url, &attrs, |_| {}, |err| error!("{:?}", err));
            }
        },
        |err| {
            error!("Can't set m.direct: {:?}", err);
        }
    );
}

pub fn direct_chat(
    bk: &Backend,
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    user: Member,
    internal_id: RoomId,
) -> Result<(), Error> {
    let url = bk.url(base.clone(), &access_token, "createRoom", vec![])?;
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

    let direct_url = bk.url(
        base,
        &access_token,
        &format!("user/{}/account_data/m.direct", user_id),
        vec![],
    )?;

    let tx = bk.tx.clone();
    let data = bk.data.clone();
    post!(
        url,
        &attrs,
        move |r: JsonValue| {
            match RoomId::try_from(r["room_id"].as_str().unwrap_or_default()) {
                Ok(room_id) => {
                    let r = Room {
                        name: user.alias.clone(),
                        direct: true,
                        ..Room::new(room_id.clone(), RoomMembership::Joined(RoomTag::None))
                    };

                    tx.send(BKResponse::NewRoom(Ok(r), internal_id))
                        .expect_log("Connection closed");

                    update_direct_chats(direct_url, data, user.uid.clone(), room_id);
                }
                Err(err) => {
                    tx.send(BKResponse::NewRoom(Err(err.into()), internal_id))
                        .expect_log("Connection closed");
                }
            }
        },
        |err| {
            tx.send(BKResponse::NewRoom(Err(err), internal_id))
                .expect_log("Connection closed");
        }
    );

    Ok(())
}

pub fn add_to_fav(
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    room_id: RoomId,
    tofav: bool,
) -> Result<(RoomId, bool), Error> {
    let request_res = if tofav {
        let params = CreateTagParameters { access_token };
        let body = CreateTagBody { order: Some(0.5) };
        create_tag(base, &user_id, &room_id, "m.favourite", &params, &body)
    } else {
        let params = DeleteTagParameters { access_token };
        delete_tag(base, &user_id, &room_id, "m.favourite", &params)
    };

    request_res
        .map_err(Into::into)
        .and_then(|request| {
            HTTP_CLIENT
                .get_client()?
                .execute(request)
                .map_err(Into::into)
        })
        .and(Ok((room_id, tofav)))
}

pub fn invite(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    user_id: UserId,
) -> Result<(), Error> {
    let params = InviteUserParameters { access_token };
    let body = InviteUserBody { user_id };

    invite_user(base, &room_id, &params, &body)
        .map_err(Into::into)
        .and_then(|request| {
            HTTP_CLIENT
                .get_client()?
                .execute(request)
                .map_err(Into::into)
        })
        .and(Ok(()))
}

pub fn set_language(
    bk: &Backend,
    access_token: AccessToken,
    server: Url,
    user_id: UserId,
    room_id: RoomId,
    input_language: String,
) -> Result<(), Error> {
    let url = bk.url(
        server,
        &access_token,
        &format!(
            "user/{}/rooms/{}/account_data/org.gnome.fractal.language",
            user_id, room_id,
        ),
        vec![],
    )?;
    let body = json!(Language { input_language });

    // FIXME: Manage errors in the AppOp loop
    put!(url, &body, |_| {}, |err| {
        error!(
            "Matrix failed to set room language with error code: {:?}",
            err
        )
    });
    Ok(())
}
