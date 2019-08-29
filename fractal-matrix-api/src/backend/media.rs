use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::error::Error;
use crate::globals;
use serde_json::json;
use std::sync::mpsc::Sender;
use std::thread;
use url::Url;

use crate::util;
use crate::util::cache_dir_path;
use crate::util::client_url;
use crate::util::download_file;
use crate::util::get_prev_batch_from;
use crate::util::json_q;
use crate::util::resolve_media_url;
use crate::util::semaphore;
use crate::util::thumb;

use crate::r0::filter::RoomEventFilter;
use crate::types::Message;

pub fn get_thumb_async(bk: &Backend, media: String, tx: Sender<String>) -> Result<(), Error> {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        match thumb(&baseu, &media, None) {
            Ok(fname) => {
                tx.send(fname).unwrap();
            }
            Err(_) => {
                tx.send(String::new()).unwrap();
            }
        };
    });

    Ok(())
}

pub fn get_media_async(bk: &Backend, media: String, tx: Sender<String>) -> Result<(), Error> {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        match util::media(&baseu, &media, None) {
            Ok(fname) => {
                tx.send(fname).unwrap();
            }
            Err(_) => {
                tx.send(String::new()).unwrap();
            }
        };
    });

    Ok(())
}

pub fn get_media_list_async(
    bk: &Backend,
    roomid: &str,
    first_media_id: Option<String>,
    prev_batch: Option<String>,
    tx: Sender<(Vec<Message>, String)>,
) -> Result<(), Error> {
    let baseu = bk.get_base_url();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let room = String::from(roomid);

    semaphore(bk.limit_threads.clone(), move || match get_room_media_list(
        &baseu,
        &tk,
        &room,
        globals::PAGE_LIMIT,
        first_media_id,
        &prev_batch,
    ) {
        Ok(media_list) => {
            tx.send(media_list).unwrap();
        }
        Err(_) => {
            tx.send((Vec::new(), String::new())).unwrap();
        }
    });

    Ok(())
}

pub fn get_media(bk: &Backend, media: String) -> Result<(), Error> {
    let baseu = bk.get_base_url();

    let tx = bk.tx.clone();
    thread::spawn(move || {
        match util::media(&baseu, &media, None) {
            Ok(fname) => {
                tx.send(BKResponse::Media(fname)).unwrap();
            }
            Err(err) => {
                tx.send(BKResponse::MediaError(err)).unwrap();
            }
        };
    });

    Ok(())
}

pub fn get_media_url(bk: &Backend, media: String, tx: Sender<String>) -> Result<(), Error> {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        match resolve_media_url(&baseu, &media, false, 0, 0) {
            Ok(uri) => {
                tx.send(uri.to_string()).unwrap();
            }
            Err(_) => {
                tx.send(String::new()).unwrap();
            }
        };
    });

    Ok(())
}

pub fn get_file_async(url: String, tx: Sender<String>) -> Result<(), Error> {
    let fname;
    {
        let name = url.split('/').last().unwrap_or_default();
        fname = cache_dir_path("files", name)?.clone();
    }

    thread::spawn(move || {
        match download_file(&url, fname, None) {
            Ok(fname) => {
                tx.send(fname).unwrap();
            }
            Err(_) => {
                tx.send(String::new()).unwrap();
            }
        };
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
