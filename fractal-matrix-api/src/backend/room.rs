use log::error;
use serde_json::json;

use ruma_identifiers::{Error as IdError, EventId, RoomId, UserId};
use std::fs;
use url::Url;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::Duration;

use crate::error::Error;
use crate::globals;
use std::thread;

use crate::util::cache_dir_path;
use crate::util::dw_media;
use crate::util::get_prev_batch_from;
use crate::util::ContentType;
use crate::util::ResultExpectLog;
use crate::util::HTTP_CLIENT;

use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::backend::types::RoomType;

use crate::r0::config::get_global_account_data::request as get_global_account_data;
use crate::r0::config::get_global_account_data::Parameters as GetGlobalAccountDataParameters;
use crate::r0::config::set_global_account_data::request as set_global_account_data;
use crate::r0::config::set_global_account_data::Parameters as SetGlobalAccountDataParameters;
use crate::r0::config::set_room_account_data::request as set_room_account_data;
use crate::r0::config::set_room_account_data::Parameters as SetRoomAccountDataParameters;
use crate::r0::filter::RoomEventFilter;
use crate::r0::media::create_content::request as create_content;
use crate::r0::media::create_content::Parameters as CreateContentParameters;
use crate::r0::media::create_content::Response as CreateContentResponse;
use crate::r0::membership::invite_user::request as invite_user;
use crate::r0::membership::invite_user::Body as InviteUserBody;
use crate::r0::membership::invite_user::Parameters as InviteUserParameters;
use crate::r0::membership::join_room_by_id_or_alias::request as join_room_req;
use crate::r0::membership::join_room_by_id_or_alias::Parameters as JoinRoomParameters;
use crate::r0::membership::leave_room::request as leave_room_req;
use crate::r0::membership::leave_room::Parameters as LeaveRoomParameters;
use crate::r0::message::create_message_event::request as create_message_event;
use crate::r0::message::create_message_event::Parameters as CreateMessageEventParameters;
use crate::r0::message::create_message_event::Response as CreateMessageEventResponse;
use crate::r0::message::get_message_events::request as get_messages_events;
use crate::r0::message::get_message_events::Direction as GetMessagesEventsDirection;
use crate::r0::message::get_message_events::Parameters as GetMessagesEventsParams;
use crate::r0::message::get_message_events::Response as GetMessagesEventsResponse;
use crate::r0::read_marker::set_read_marker::request as set_read_marker;
use crate::r0::read_marker::set_read_marker::Body as SetReadMarkerBody;
use crate::r0::read_marker::set_read_marker::Parameters as SetReadMarkerParameters;
use crate::r0::redact::redact_event::request as redact_event;
use crate::r0::redact::redact_event::Body as RedactEventBody;
use crate::r0::redact::redact_event::Parameters as RedactEventParameters;
use crate::r0::redact::redact_event::Response as RedactEventResponse;
use crate::r0::room::create_room::request as create_room;
use crate::r0::room::create_room::Body as CreateRoomBody;
use crate::r0::room::create_room::Parameters as CreateRoomParameters;
use crate::r0::room::create_room::Response as CreateRoomResponse;
use crate::r0::room::create_room::RoomPreset;
use crate::r0::room::Visibility;
use crate::r0::state::create_state_events_for_key::request as create_state_events_for_key;
use crate::r0::state::create_state_events_for_key::Parameters as CreateStateEventsForKeyParameters;
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

pub fn get_room_detail(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    keys: String,
) -> Result<(RoomId, String, String), Error> {
    let params = GetStateEventsForKeyParameters { access_token };

    let request = get_state_events_for_key(base, &params, &room_id, &keys)?;
    let response: JsonValue = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    let k = keys.split('.').last().ok_or(Error::BackendError)?;
    let value = response[&k].as_str().map(Into::into).unwrap_or_default();

    Ok((room_id, keys, value))
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
                    base,
                    avatar.as_str(),
                    ContentType::default_thumbnail(),
                    dest,
                );
            }

            Ok((room_id.clone(), avatar))
        })
        .or_else(|err| match err {
            Error::MatrixError(errcode, _) if errcode == "M_NOT_FOUND" => Ok((room_id, None)),
            error => Err(error),
        })
}

pub fn get_room_members(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
) -> Result<(RoomId, Vec<Member>), Error> {
    let params = JoinedMembersParameters { access_token };

    let request = get_joined_members(base, &room_id, &params)?;
    let response: JoinedMembersResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    let ms = response.joined.into_iter().map(Member::from).collect();

    Ok((room_id, ms))
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

    let request = get_messages_events(base, &params, &room_id)?;
    let response: GetMessagesEventsResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    let prev_batch = response.end;
    let evs = response.chunk.iter().rev();
    let list = Message::from_json_events_iter(&room_id, evs)?;

    Ok((list, room_id, prev_batch))
}

pub fn get_room_messages_from_msg(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    msg: Message,
) -> Result<(Vec<Message>, RoomId, Option<String>), Error> {
    let event_id = msg.id.as_ref().ok_or(Error::BackendError)?;

    // first of all, we calculate the from param using the context api, then we call the
    // normal get_room_messages
    let from = get_prev_batch_from(base.clone(), access_token.clone(), &room_id, event_id)?;

    get_room_messages(base, access_token, room_id, from)
}

pub fn send_msg(
    base: Url,
    access_token: AccessToken,
    msg: Message,
) -> Result<(String, Option<EventId>), Error> {
    let room_id: RoomId = msg.room.clone();

    let params = CreateMessageEventParameters { access_token };

    let mut body = json!({
        "body": msg.body,
        "msgtype": msg.mtype,
    });

    if let Some(u) = msg.url.as_ref() {
        body["url"] = json!(u);
    }

    if let (Some(f), Some(f_b)) = (msg.format.as_ref(), msg.formatted_body.as_ref()) {
        body["formatted_body"] = json!(f_b);
        body["format"] = json!(f);
    }

    let extra_content_map = msg
        .extra_content
        .as_ref()
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    for (k, v) in extra_content_map {
        body[k] = v;
    }

    let txn_id = msg.get_txn_id();

    create_message_event(base, &params, &body, &room_id, "m.room.message", &txn_id)
        .map_err::<Error, _>(Into::into)
        .and_then(|request| {
            let response = HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<CreateMessageEventResponse>()?;

            Ok((txn_id.clone(), response.event_id))
        })
        .or(Err(Error::SendMsgError(txn_id)))
}

pub fn send_typing(
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    room_id: RoomId,
) -> Result<(), Error> {
    let params = TypingNotificationParameters { access_token };
    let body = TypingNotificationBody::Typing(Duration::from_secs(4));

    let request = send_typing_notification(base, &room_id, &user_id, &params, &body)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn redact_msg(
    base: Url,
    access_token: AccessToken,
    msg: Message,
) -> Result<(EventId, Option<EventId>), Error> {
    let room_id = &msg.room;
    let txn_id = msg.get_txn_id();
    let event_id = msg.id.clone().ok_or(Error::BackendError)?;

    let params = RedactEventParameters { access_token };

    let body = RedactEventBody {
        reason: "Deletion requested by the sender".into(),
    };

    redact_event(base, &params, &body, room_id, &event_id, &txn_id)
        .map_err::<Error, _>(Into::into)
        .and_then(|request| {
            let response = HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<RedactEventResponse>()?;

            Ok((event_id.clone(), response.event_id))
        })
        .or(Err(Error::SendMsgRedactionError(event_id)))
}

pub fn join_room(base: Url, access_token: AccessToken, room_id: RoomId) -> Result<RoomId, Error> {
    let room_id_or_alias_id = room_id.clone().into();

    let params = JoinRoomParameters {
        access_token,
        server_name: Default::default(),
    };

    let request = join_room_req(base, &room_id_or_alias_id, &params)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(room_id)
}

pub fn leave_room(base: Url, access_token: AccessToken, room_id: RoomId) -> Result<(), Error> {
    let params = LeaveRoomParameters { access_token };

    let request = leave_room_req(base, &room_id, &params)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn mark_as_read(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    event_id: EventId,
) -> Result<(RoomId, EventId), Error> {
    let params = SetReadMarkerParameters { access_token };

    let body = SetReadMarkerBody {
        fully_read: event_id.clone(),
        read: Some(event_id.clone()),
    };

    let request = set_read_marker(base, &params, &body, &room_id)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok((room_id, event_id))
}

pub fn set_room_name(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    name: String,
) -> Result<(), Error> {
    let params = CreateStateEventsForKeyParameters { access_token };

    let body = json!({
        "name": name,
    });

    let request = create_state_events_for_key(base, &params, &body, &room_id, "m.room.name")?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn set_room_topic(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    topic: String,
) -> Result<(), Error> {
    let params = CreateStateEventsForKeyParameters { access_token };

    let body = json!({
        "topic": topic,
    });

    let request = create_state_events_for_key(base, &params, &body, &room_id, "m.room.topic")?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn set_room_avatar(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    avatar: String,
) -> Result<(), Error> {
    let params = CreateStateEventsForKeyParameters {
        access_token: access_token.clone(),
    };

    let upload_file_response = upload_file(base.clone(), access_token, &avatar)?;

    let body = json!({ "url": upload_file_response.content_uri.as_str() });
    let request = create_state_events_for_key(base, &params, &body, &room_id, "m.room.avatar")?;
    HTTP_CLIENT.get_client()?.execute(request)?;

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

    if fname.starts_with("mxc://") && thumb.starts_with("mxc://") {
        tx.send(BKResponse::SentMsg(send_msg(baseu, tk, msg)))
            .expect_log("Connection closed");

        return Ok(());
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
            thread::spawn(clone!(msg, tx => move || {
                let query = send_msg(baseu, tk, msg);
                tx.send(BKResponse::SentMsg(query))
                    .expect_log("Connection closed");
            }));

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
    let request = create_content(base, &params_upload, contents)?;

    HTTP_CLIENT
        .get_client()?
        .execute(request)?
        .json()
        .map_err(Into::into)
}

pub fn new_room(
    base: Url,
    access_token: AccessToken,
    name: String,
    privacy: RoomType,
) -> Result<Room, Error> {
    let params = CreateRoomParameters { access_token };

    let (visibility, preset) = match privacy {
        RoomType::Public => (Visibility::Public, RoomPreset::PublicChat),
        RoomType::Private => (Visibility::Private, RoomPreset::PrivateChat),
    };

    let body = CreateRoomBody {
        name: Some(name.clone()),
        visibility: Some(visibility),
        preset: Some(preset),
        ..Default::default()
    };

    let request = create_room(base, &params, &body)?;
    let response: CreateRoomResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    Ok(Room {
        name: Some(name),
        ..Room::new(response.room_id, RoomMembership::Joined(RoomTag::None))
    })
}

fn update_direct_chats(
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    room_id: RoomId,
    user: Member,
) -> Result<(), Error> {
    let params = GetGlobalAccountDataParameters {
        access_token: access_token.clone(),
    };

    let request = get_global_account_data(base.clone(), &params, &user_id, "m.direct")?;
    let response: JsonValue = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    let mut directs = response
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
        .collect::<Result<HashMap<UserId, Vec<RoomId>>, IdError>>()?;

    if let Some(v) = directs.get_mut(&user.uid) {
        v.push(room_id);
    } else {
        directs.insert(user.uid, vec![room_id]);
    }

    let params = SetGlobalAccountDataParameters {
        access_token: access_token.clone(),
    };

    let request = set_global_account_data(base, &params, &json!(directs), &user_id, "m.direct")?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn direct_chat(
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    user: Member,
) -> Result<Room, Error> {
    let params = CreateRoomParameters {
        access_token: access_token.clone(),
    };

    let body = CreateRoomBody {
        invite: vec![user.uid.clone()],
        visibility: Some(Visibility::Private),
        preset: Some(RoomPreset::PrivateChat),
        is_direct: true,
        initial_state: vec![json!({
            "type": "m.room.history_visibility",
            "content": {
                "history_visibility": "invited"
            }
        })],
        ..Default::default()
    };

    let request = create_room(base.clone(), &params, &body)?;
    let response: CreateRoomResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    let directs = update_direct_chats(
        base,
        access_token,
        user_id,
        response.room_id.clone(),
        user.clone(),
    );

    if let Err(err) = directs {
        error!("Can't set m.direct: {:?}", err);
        return Err(err);
    }

    Ok(Room {
        name: user.alias,
        direct: true,
        ..Room::new(response.room_id, RoomMembership::Joined(RoomTag::None))
    })
}

pub fn add_to_fav(
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    room_id: RoomId,
    tofav: bool,
) -> Result<(RoomId, bool), Error> {
    let request = if tofav {
        let params = CreateTagParameters { access_token };
        let body = CreateTagBody { order: Some(0.5) };
        create_tag(base, &user_id, &room_id, "m.favourite", &params, &body)
    } else {
        let params = DeleteTagParameters { access_token };
        delete_tag(base, &user_id, &room_id, "m.favourite", &params)
    }?;

    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok((room_id, tofav))
}

pub fn invite(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    user_id: UserId,
) -> Result<(), Error> {
    let params = InviteUserParameters { access_token };
    let body = InviteUserBody { user_id };

    let request = invite_user(base, &room_id, &params, &body)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn set_language(
    access_token: AccessToken,
    base: Url,
    user_id: UserId,
    room_id: RoomId,
    input_language: String,
) -> Result<(), Error> {
    let params = SetRoomAccountDataParameters { access_token };

    let body = json!(Language { input_language });

    let response = set_room_account_data(
        base,
        &params,
        &body,
        &user_id,
        &room_id,
        "org.gnome.fractal.language",
    )
    .map_err(Into::into)
    .and_then(|request| {
        let _ = HTTP_CLIENT.get_client()?.execute(request)?;

        Ok(())
    });

    // FIXME: Manage errors in the AppOp loop
    if let Err(ref err) = response {
        error!(
            "Matrix failed to set room language with error code: {:?}",
            err
        );
    }

    response
}
