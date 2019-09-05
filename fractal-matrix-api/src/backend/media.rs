use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::error::Error;
use crate::globals;
use std::sync::mpsc::Sender;
use std::thread;

use crate::util;
use crate::util::cache_dir_path;
use crate::util::download_file;
use crate::util::get_room_media_list;
use crate::util::resolve_media_url;
use crate::util::semaphore;
use crate::util::thumb;

use crate::types::Message;

pub fn get_thumb_async(bk: &Backend, media: String, tx: Sender<String>) -> Result<(), Error> {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        match thumb(&baseu, &media, None) {
            Ok(fname) => {
                send!(tx, fname);
            }
            Err(_) => {
                send!(tx, String::new());
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
                send!(tx, fname);
            }
            Err(_) => {
                send!(tx, String::new());
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
            send!(tx, media_list);
        }
        Err(_) => {
            send!(tx, (Vec::new(), String::new()));
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
                send!(tx, BKResponse::Media(fname));
            }
            Err(err) => {
                send!(tx, BKResponse::MediaError(err));
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
                send!(tx, uri.to_string());
            }
            Err(_) => {
                send!(tx, String::new());
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
                send!(tx, fname);
            }
            Err(_) => {
                send!(tx, String::new());
            }
        };
    });

    Ok(())
}
