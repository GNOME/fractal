use log::error;
use serde_json::json;

use fractal_api::identifiers::{Error as IdError, EventId, RoomId, UserId};
use fractal_api::url::Url;
use std::fs;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::time::Duration;

use crate::error::Error;
use crate::globals;

use crate::actions::AppState;
use crate::backend::HTTP_CLIENT;
use crate::util::cache_dir_path;

use crate::types::Member;
use crate::types::Message;
use crate::types::{Room, RoomMembership, RoomTag};
use fractal_api::r0::config::get_global_account_data::request as get_global_account_data;
use fractal_api::r0::config::get_global_account_data::Parameters as GetGlobalAccountDataParameters;
use fractal_api::r0::config::set_global_account_data::request as set_global_account_data;
use fractal_api::r0::config::set_global_account_data::Parameters as SetGlobalAccountDataParameters;
use fractal_api::r0::config::set_room_account_data::request as set_room_account_data;
use fractal_api::r0::config::set_room_account_data::Parameters as SetRoomAccountDataParameters;
use fractal_api::r0::filter::RoomEventFilter;
use fractal_api::r0::media::create_content::request as create_content;
use fractal_api::r0::media::create_content::Parameters as CreateContentParameters;
use fractal_api::r0::media::create_content::Response as CreateContentResponse;
use fractal_api::r0::membership::invite_user::request as invite_user;
use fractal_api::r0::membership::invite_user::Body as InviteUserBody;
use fractal_api::r0::membership::invite_user::Parameters as InviteUserParameters;
use fractal_api::r0::membership::join_room_by_id_or_alias::request as join_room_req;
use fractal_api::r0::membership::join_room_by_id_or_alias::Parameters as JoinRoomParameters;
use fractal_api::r0::membership::leave_room::request as leave_room_req;
use fractal_api::r0::membership::leave_room::Parameters as LeaveRoomParameters;
use fractal_api::r0::message::create_message_event::request as create_message_event;
use fractal_api::r0::message::create_message_event::Parameters as CreateMessageEventParameters;
use fractal_api::r0::message::create_message_event::Response as CreateMessageEventResponse;
use fractal_api::r0::message::get_message_events::request as get_messages_events;
use fractal_api::r0::message::get_message_events::Direction as GetMessagesEventsDirection;
use fractal_api::r0::message::get_message_events::Parameters as GetMessagesEventsParams;
use fractal_api::r0::message::get_message_events::Response as GetMessagesEventsResponse;
use fractal_api::r0::read_marker::set_read_marker::request as set_read_marker;
use fractal_api::r0::read_marker::set_read_marker::Body as SetReadMarkerBody;
use fractal_api::r0::read_marker::set_read_marker::Parameters as SetReadMarkerParameters;
use fractal_api::r0::redact::redact_event::request as redact_event;
use fractal_api::r0::redact::redact_event::Body as RedactEventBody;
use fractal_api::r0::redact::redact_event::Parameters as RedactEventParameters;
use fractal_api::r0::redact::redact_event::Response as RedactEventResponse;
use fractal_api::r0::room::create_room::request as create_room;
use fractal_api::r0::room::create_room::Body as CreateRoomBody;
use fractal_api::r0::room::create_room::Parameters as CreateRoomParameters;
use fractal_api::r0::room::create_room::Response as CreateRoomResponse;
use fractal_api::r0::room::create_room::RoomPreset;
use fractal_api::r0::room::Visibility;
use fractal_api::r0::state::create_state_events_for_key::request as create_state_events_for_key;
use fractal_api::r0::state::create_state_events_for_key::Parameters as CreateStateEventsForKeyParameters;
use fractal_api::r0::state::get_state_events_for_key::request as get_state_events_for_key;
use fractal_api::r0::state::get_state_events_for_key::Parameters as GetStateEventsForKeyParameters;
use fractal_api::r0::sync::get_joined_members::request as get_joined_members;
use fractal_api::r0::sync::get_joined_members::Parameters as JoinedMembersParameters;
use fractal_api::r0::sync::get_joined_members::Response as JoinedMembersResponse;
use fractal_api::r0::sync::sync_events::Language;
use fractal_api::r0::tag::create_tag::request as create_tag;
use fractal_api::r0::tag::create_tag::Body as CreateTagBody;
use fractal_api::r0::tag::create_tag::Parameters as CreateTagParameters;
use fractal_api::r0::tag::delete_tag::request as delete_tag;
use fractal_api::r0::tag::delete_tag::Parameters as DeleteTagParameters;
use fractal_api::r0::typing::request as send_typing_notification;
use fractal_api::r0::typing::Body as TypingNotificationBody;
use fractal_api::r0::typing::Parameters as TypingNotificationParameters;
use fractal_api::r0::AccessToken;

use serde_json::Value as JsonValue;

use super::{
    dw_media, get_prev_batch_from, remove_matrix_access_token_if_present, ContentType, ShowError,
};
use crate::app::App;
use crate::i18n::i18n;
use crate::APPOP;

#[derive(Debug)]
pub struct RoomDetailError(Error);

impl<T: Into<Error>> From<T> for RoomDetailError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for RoomDetailError {}

pub fn get_room_detail(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    keys: String,
) -> Result<(RoomId, String, String), RoomDetailError> {
    let params = GetStateEventsForKeyParameters { access_token };

    let request = get_state_events_for_key(base, &params, &room_id, &keys)?;
    let response: JsonValue = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    let k = keys.split('.').last().ok_or(Error::BackendError)?;
    let value = response[&k].as_str().map(Into::into).unwrap_or_default();

    Ok((room_id, keys, value))
}

#[derive(Debug)]
pub struct RoomAvatarError(Error);

impl<T: Into<Error>> From<T> for RoomAvatarError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for RoomAvatarError {}

pub fn get_room_avatar(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
) -> Result<(RoomId, Option<Url>), RoomAvatarError> {
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
        .map_err(Into::into)
}

#[derive(Debug)]
pub struct RoomMembersError(Error);

impl<T: Into<Error>> From<T> for RoomMembersError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for RoomMembersError {}

pub fn get_room_members(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
) -> Result<(RoomId, Vec<Member>), RoomMembersError> {
    let params = JoinedMembersParameters { access_token };

    let request = get_joined_members(base, &room_id, &params)?;
    let response: JoinedMembersResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;

    let ms = response.joined.into_iter().map(Member::from).collect();

    Ok((room_id, ms))
}

#[derive(Debug)]
pub struct RoomMessagesToError(Error);

impl<T: Into<Error>> From<T> for RoomMessagesToError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for RoomMessagesToError {}

/* Load older messages starting by prev_batch
 * https://matrix.org/docs/spec/client_server/latest.html#get-matrix-client-r0-rooms-roomid-messages
 */
pub fn get_room_messages(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    from: String,
) -> Result<(Vec<Message>, RoomId, Option<String>), RoomMessagesToError> {
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
) -> Result<(Vec<Message>, RoomId, Option<String>), RoomMessagesToError> {
    let event_id = msg.id.as_ref().ok_or(Error::BackendError)?;

    // first of all, we calculate the from param using the context api, then we call the
    // normal get_room_messages
    let from = get_prev_batch_from(base.clone(), access_token.clone(), &room_id, event_id)?;

    get_room_messages(base, access_token, room_id, from)
}

#[derive(Debug)]
pub struct SendMsgError(String);

impl ShowError for SendMsgError {
    fn show_error(&self) {
        error!("sending {}: retrying send", self.0);
        APPOP!(retry_send);
    }
}

pub fn send_msg(
    base: Url,
    access_token: AccessToken,
    msg: Message,
) -> Result<(String, Option<EventId>), SendMsgError> {
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
        .or(Err(SendMsgError(txn_id)))
}

#[derive(Debug)]
pub struct SendTypingError(Error);

impl<T: Into<Error>> From<T> for SendTypingError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for SendTypingError {}

pub fn send_typing(
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    room_id: RoomId,
) -> Result<(), SendTypingError> {
    let params = TypingNotificationParameters { access_token };
    let body = TypingNotificationBody::Typing(Duration::from_secs(4));

    let request = send_typing_notification(base, &room_id, &user_id, &params, &body)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub struct SendMsgRedactionError;

impl ShowError for SendMsgRedactionError {
    fn show_error(&self) {
        let error = i18n("Error deleting message");
        APPOP!(show_error, (error));
    }
}

pub fn redact_msg(
    base: Url,
    access_token: AccessToken,
    msg: Message,
) -> Result<(EventId, Option<EventId>), SendMsgRedactionError> {
    let room_id = &msg.room;
    let txn_id = msg.get_txn_id();
    let event_id = msg.id.clone().ok_or(SendMsgRedactionError)?;

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
        .or(Err(SendMsgRedactionError))
}

#[derive(Debug)]
pub struct JoinRoomError(Error);

impl<T: Into<Error>> From<T> for JoinRoomError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for JoinRoomError {
    fn show_error(&self) {
        let err_str = format!("{:?}", self);
        error!(
            "{}",
            remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
        );
        let error = i18n("Can’t join the room, try again.").to_string();
        let state = AppState::NoRoom;
        APPOP!(show_error, (error));
        APPOP!(set_state, (state));
    }
}

pub fn join_room(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
) -> Result<RoomId, JoinRoomError> {
    let room_id_or_alias_id = room_id.clone().into();

    let params = JoinRoomParameters {
        access_token,
        server_name: Default::default(),
    };

    let request = join_room_req(base, &room_id_or_alias_id, &params)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(room_id)
}

#[derive(Debug)]
pub struct LeaveRoomError(Error);

impl<T: Into<Error>> From<T> for LeaveRoomError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for LeaveRoomError {}

pub fn leave_room(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
) -> Result<(), LeaveRoomError> {
    let params = LeaveRoomParameters { access_token };

    let request = leave_room_req(base, &room_id, &params)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub struct MarkedAsReadError(Error);

impl<T: Into<Error>> From<T> for MarkedAsReadError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for MarkedAsReadError {}

pub fn mark_as_read(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    event_id: EventId,
) -> Result<(RoomId, EventId), MarkedAsReadError> {
    let params = SetReadMarkerParameters { access_token };

    let body = SetReadMarkerBody {
        fully_read: event_id.clone(),
        read: Some(event_id.clone()),
    };

    let request = set_read_marker(base, &params, &body, &room_id)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok((room_id, event_id))
}
#[derive(Debug)]
pub struct SetRoomNameError(Error);

impl<T: Into<Error>> From<T> for SetRoomNameError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for SetRoomNameError {}

pub fn set_room_name(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    name: String,
) -> Result<(), SetRoomNameError> {
    let params = CreateStateEventsForKeyParameters { access_token };

    let body = json!({
        "name": name,
    });

    let request = create_state_events_for_key(base, &params, &body, &room_id, "m.room.name")?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub struct SetRoomTopicError(Error);

impl<T: Into<Error>> From<T> for SetRoomTopicError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for SetRoomTopicError {}

pub fn set_room_topic(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    topic: String,
) -> Result<(), SetRoomTopicError> {
    let params = CreateStateEventsForKeyParameters { access_token };

    let body = json!({
        "topic": topic,
    });

    let request = create_state_events_for_key(base, &params, &body, &room_id, "m.room.topic")?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub struct SetRoomAvatarError(Error);

impl<T: Into<Error>> From<T> for SetRoomAvatarError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl From<AttachedFileError> for SetRoomAvatarError {
    fn from(err: AttachedFileError) -> Self {
        Self(err.0)
    }
}

impl ShowError for SetRoomAvatarError {}

pub fn set_room_avatar(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    avatar: String,
) -> Result<(), SetRoomAvatarError> {
    let params = CreateStateEventsForKeyParameters {
        access_token: access_token.clone(),
    };

    let upload_file_response = upload_file(base.clone(), access_token, &avatar)?;

    let body = json!({ "url": upload_file_response.content_uri.as_str() });
    let request = create_state_events_for_key(base, &params, &body, &room_id, "m.room.avatar")?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub struct AttachedFileError(Error);

impl<T: Into<Error>> From<T> for AttachedFileError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for AttachedFileError {
    fn show_error(&self) {
        let err_str = format!("{:?}", self);
        error!(
            "attaching {}: retrying send",
            remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
        );
        APPOP!(retry_send);
    }
}

pub fn upload_file(
    base: Url,
    access_token: AccessToken,
    fname: &str,
) -> Result<CreateContentResponse, AttachedFileError> {
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

#[derive(Debug, Clone, Copy)]
pub enum RoomType {
    Public,
    Private,
}

#[derive(Debug)]
pub struct NewRoomError(Error);

impl<T: Into<Error>> From<T> for NewRoomError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for NewRoomError {
    fn show_error(&self) {
        let err_str = format!("{:?}", self);
        error!(
            "{}",
            remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
        );

        let error = i18n("Can’t create the room, try again");
        let state = AppState::NoRoom;
        APPOP!(show_error, (error));
        APPOP!(set_state, (state));
    }
}

pub fn new_room(
    base: Url,
    access_token: AccessToken,
    name: String,
    privacy: RoomType,
) -> Result<Room, NewRoomError> {
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

    let params = SetGlobalAccountDataParameters { access_token };

    let request = set_global_account_data(base, &params, &json!(directs), &user_id, "m.direct")?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

pub fn direct_chat(
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    user: Member,
) -> Result<Room, NewRoomError> {
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
        return Err(NewRoomError(err));
    }

    Ok(Room {
        name: user.alias,
        direct: true,
        ..Room::new(response.room_id, RoomMembership::Joined(RoomTag::None))
    })
}

#[derive(Debug)]
pub struct AddedToFavError(Error);

impl<T: Into<Error>> From<T> for AddedToFavError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for AddedToFavError {}

pub fn add_to_fav(
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    room_id: RoomId,
    tofav: bool,
) -> Result<(RoomId, bool), AddedToFavError> {
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

#[derive(Debug)]
pub struct InviteError(Error);

impl<T: Into<Error>> From<T> for InviteError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for InviteError {}

pub fn invite(
    base: Url,
    access_token: AccessToken,
    room_id: RoomId,
    user_id: UserId,
) -> Result<(), InviteError> {
    let params = InviteUserParameters { access_token };
    let body = InviteUserBody { user_id };

    let request = invite_user(base, &room_id, &params, &body)?;
    HTTP_CLIENT.get_client()?.execute(request)?;

    Ok(())
}

#[derive(Debug)]
pub struct ChangeLanguageError(Error);

impl<T: Into<Error>> From<T> for ChangeLanguageError {
    fn from(err: T) -> Self {
        Self(err.into())
    }
}

impl ShowError for ChangeLanguageError {
    fn show_error(&self) {
        let err_str = format!("{:?}", self);
        error!(
            "Error forming url to set room language: {}",
            remove_matrix_access_token_if_present(&err_str).unwrap_or(err_str)
        );
    }
}

pub fn set_language(
    access_token: AccessToken,
    base: Url,
    user_id: UserId,
    room_id: RoomId,
    input_language: String,
) -> Result<(), ChangeLanguageError> {
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
