use crate::backend::types::Backend;
use crate::error::Error;
use crate::globals;
use ruma_identifiers::RoomId;
use std::str::Split;
use std::sync::mpsc::Sender;
use std::thread;
use url::Url;

use crate::r0::AccessToken;
use crate::util::cache_dir_path;
use crate::util::download_file;
use crate::util::dw_media;
use crate::util::get_prev_batch_from;
use crate::util::semaphore;
use crate::util::ContentType;
use crate::util::ResultExpectLog;
use crate::util::HTTP_CLIENT;

use crate::r0::filter::RoomEventFilter;
use crate::r0::message::get_message_events::request as get_messages_events_req;
use crate::r0::message::get_message_events::Direction as GetMessagesEventsDirection;
use crate::r0::message::get_message_events::Parameters as GetMessagesEventsParams;
use crate::r0::message::get_message_events::Response as GetMessagesEventsResponse;
use crate::types::Message;

pub fn get_thumb_async(bk: &Backend, baseu: Url, media: String, tx: Sender<Result<String, Error>>) {
    semaphore(bk.limit_threads.clone(), move || {
        let fname = dw_media(&baseu, &media, ContentType::default_thumbnail(), None);
        tx.send(fname).expect_log("Connection closed");
    });
}

pub fn get_media_async(bk: &Backend, baseu: Url, media: String, tx: Sender<Result<String, Error>>) {
    semaphore(bk.limit_threads.clone(), move || {
        let fname = dw_media(&baseu, &media, ContentType::Download, None);
        tx.send(fname).expect_log("Connection closed");
    });
}

pub fn get_media_list_async(
    bk: &Backend,
    baseu: Url,
    access_token: AccessToken,
    room_id: RoomId,
    first_media_id: Option<String>,
    prev_batch: Option<String>,
    tx: Sender<(Vec<Message>, String)>,
) {
    semaphore(bk.limit_threads.clone(), move || {
        let media_list = prev_batch
            // FIXME: This should never be an empty token
            .or_else(|| {
                if let Some(ref id) = first_media_id {
                    get_prev_batch_from(&baseu, &access_token, &room_id, id).ok()
                } else {
                    None
                }
            })
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

pub fn get_file_async(url: Url, tx: Sender<String>) -> Result<(), Error> {
    let name = url
        .path_segments()
        .and_then(Split::last)
        .unwrap_or_default();
    let fname = cache_dir_path(Some("files"), name)?;

    thread::spawn(move || {
        let fname = download_file(url, fname, None).unwrap_or_default();
        tx.send(fname).expect_log("Connection closed");
    });

    Ok(())
}

fn get_room_media_list(
    baseu: Url,
    access_token: AccessToken,
    room_id: &RoomId,
    limit: u64,
    prev_batch: String,
) -> Result<(Vec<Message>, String), Error> {
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

    get_messages_events_req(baseu, &params, room_id)
        .map_err(Into::into)
        .and_then(|request| {
            let response = HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<GetMessagesEventsResponse>()?;

            let prev_batch = response.end.unwrap_or_default();
            let evs = response.chunk.iter().rev();
            let media_list = Message::from_json_events_iter(room_id, evs)?;

            Ok((media_list, prev_batch))
        })
}
