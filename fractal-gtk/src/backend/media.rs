use crate::error::Error;
use crate::globals;
use fractal_api::identifiers::{EventId, RoomId};
use fractal_api::url::Url;
use std::sync::mpsc::Sender;

use crate::backend::HTTP_CLIENT;
use crate::util::ResultExpectLog;
use fractal_api::r0::AccessToken;

use crate::types::Message;
use fractal_api::r0::filter::RoomEventFilter;
use fractal_api::r0::message::get_message_events::request as get_messages_events_req;
use fractal_api::r0::message::get_message_events::Direction as GetMessagesEventsDirection;
use fractal_api::r0::message::get_message_events::Parameters as GetMessagesEventsParams;
use fractal_api::r0::message::get_message_events::Response as GetMessagesEventsResponse;

use super::{dw_media, get_prev_batch_from, ContentType, ThreadPool};

pub type MediaResult = Result<String, Error>;
pub type MediaList = (Vec<Message>, String);

pub fn get_thumb_async(
    thread_pool: ThreadPool,
    baseu: Url,
    media: String,
    tx: Sender<MediaResult>,
) {
    thread_pool.run(move || {
        let fname = dw_media(baseu, &media, ContentType::default_thumbnail(), None);
        tx.send(fname).expect_log("Connection closed");
    });
}

pub fn get_media_async(
    thread_pool: ThreadPool,
    baseu: Url,
    media: String,
    tx: Sender<MediaResult>,
) {
    thread_pool.run(move || {
        let fname = dw_media(baseu, &media, ContentType::Download, None);
        tx.send(fname).expect_log("Connection closed");
    });
}

pub fn get_media_list_async(
    thread_pool: ThreadPool,
    baseu: Url,
    access_token: AccessToken,
    room_id: RoomId,
    first_media_id: EventId,
    prev_batch: Option<String>,
    tx: Sender<MediaList>,
) {
    thread_pool.run(move || {
        let media_list = prev_batch
            // FIXME: This should never be an empty token
            .or_else(|| get_prev_batch_from(baseu.clone(), access_token.clone(), &room_id, &first_media_id).ok())
            .and_then(|from| {
                get_room_media_list(
                    baseu,
                    access_token,
                    &room_id,
                    globals::PAGE_LIMIT as u64,
                    from,
                ).ok()
            })
            .unwrap_or_default();
        tx.send(media_list).expect_log("Connection closed");
    });
}

fn get_room_media_list(
    baseu: Url,
    access_token: AccessToken,
    room_id: &RoomId,
    limit: u64,
    prev_batch: String,
) -> Result<MediaList, Error> {
    let params = GetMessagesEventsParams {
        access_token,
        from: prev_batch,
        to: None,
        dir: GetMessagesEventsDirection::Backward,
        limit,
        filter: RoomEventFilter {
            contains_url: true,
            not_types: vec!["m.sticker"],
            ..Default::default()
        },
    };

    let request = get_messages_events_req(baseu, &params, room_id)?;
    let response: GetMessagesEventsResponse = HTTP_CLIENT.get_client().execute(request)?.json()?;

    let prev_batch = response.end.unwrap_or_default();
    let evs = response.chunk.iter().rev();
    let media_list = Message::from_json_events_iter(room_id, evs)?;

    Ok((media_list, prev_batch))
}
