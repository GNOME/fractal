use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::error::Error;
use crate::globals;
use serde_json::json;
use std::sync::mpsc::Sender;
use std::thread;
use url::Url;

use crate::util::cache_dir_path;
use crate::util::client_url;
use crate::util::download_file;
use crate::util::dw_media;
use crate::util::get_prev_batch_from;
use crate::util::json_q;
use crate::util::resolve_media_url;
use crate::util::semaphore;
use crate::util::ContentType;
use crate::util::ResultExpectLog;

use crate::r0::filter::RoomEventFilter;
use crate::types::Message;

pub fn get_thumb_async(bk: &Backend, media: String, tx: Sender<String>) {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        let fname =
            dw_media(&baseu, &media, ContentType::default_thumbnail(), None).unwrap_or_default();
        tx.send(fname).expect_log("Connection closed");
    });
}

pub fn get_media_async(bk: &Backend, media: String, tx: Sender<String>) {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        let fname = dw_media(&baseu, &media, ContentType::Download, None).unwrap_or_default();
        tx.send(fname).expect_log("Connection closed");
    });
}

pub fn get_media_list_async(
    bk: &Backend,
    roomid: &str,
    first_media_id: Option<String>,
    prev_batch: Option<String>,
    tx: Sender<(Vec<Message>, String)>,
) {
    let baseu = bk.get_base_url();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let room = String::from(roomid);

    semaphore(bk.limit_threads.clone(), move || {
        let media_list = get_room_media_list(
            &baseu,
            &tk,
            &room,
            globals::PAGE_LIMIT,
            first_media_id,
            &prev_batch,
        )
        .unwrap_or_default();
        tx.send(media_list).expect_log("Connection closed");
    });
}

pub fn get_media(bk: &Backend, media: String) {
    let baseu = bk.get_base_url();

    let tx = bk.tx.clone();
    thread::spawn(move || {
        let fname = dw_media(&baseu, &media, ContentType::Download, None);
        tx.send(BKResponse::Media(fname))
            .expect_log("Connection closed");
    });
}

pub fn get_media_url(bk: &Backend, media: String, tx: Sender<String>) {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        let uri = resolve_media_url(&baseu, &media, ContentType::Download)
            .map(Url::into_string)
            .unwrap_or_default();
        tx.send(uri).expect_log("Connection closed");
    });
}

pub fn get_file_async(url: String, tx: Sender<String>) -> Result<(), Error> {
    let name = url.split('/').last().unwrap_or_default();
    let fname = cache_dir_path(Some("files"), name)?;

    thread::spawn(move || {
        let fname = download_file(&url, fname, None).unwrap_or_default();
        tx.send(fname).expect_log("Connection closed");
    });

    Ok(())
}

fn get_room_media_list(
    baseu: &Url,
    tk: &str,
    roomid: &str,
    limit: i32,
    first_media_id: Option<String>,
    prev_batch: &Option<String>,
) -> Result<(Vec<Message>, String), Error> {
    let mut params = vec![
        ("dir", String::from("b")),
        ("limit", format!("{}", limit)),
        ("access_token", String::from(tk)),
        (
            "filter",
            serde_json::to_string(&RoomEventFilter {
                contains_url: true,
                not_types: vec!["m.sticker"],
                ..Default::default()
            })
            .expect("Failed to serialize room media list request filter"),
        ),
    ];

    match prev_batch {
        Some(ref pb) => params.push(("from", pb.clone())),
        None => {
            if let Some(id) = first_media_id {
                params.push(("from", get_prev_batch_from(baseu, tk, &roomid, &id)?))
            }
        }
    };

    let path = format!("rooms/{}/messages", roomid);
    let url = client_url(baseu, &path, &params)?;

    let r = json_q("get", &url, &json!(null))?;
    let array = r["chunk"].as_array();
    let prev_batch = r["end"].to_string().trim_matches('"').to_string();
    if array.is_none() || array.unwrap().is_empty() {
        return Ok((vec![], prev_batch));
    }

    let evs = array.unwrap().iter().rev();
    let media_list = Message::from_json_events_iter(roomid, evs);

    Ok((media_list, prev_batch))
}
