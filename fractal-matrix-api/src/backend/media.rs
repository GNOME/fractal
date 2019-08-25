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

pub fn get_thumb_async(bk: &Backend, media: String, tx: Sender<String>) {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        match thumb(&baseu, &media, None) {
            Ok(fname) => {
                let _ = tx.send(fname);
            }
            Err(_) => {
                let _ = tx.send(String::new());
            }
        };
    });
}

pub fn get_media_async(bk: &Backend, media: String, tx: Sender<String>) {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        match util::media(&baseu, &media, None) {
            Ok(fname) => {
                let _ = tx.send(fname);
            }
            Err(_) => {
                let _ = tx.send(String::new());
            }
        };
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

    semaphore(bk.limit_threads.clone(), move || match get_room_media_list(
        &baseu,
        &tk,
        &room,
        globals::PAGE_LIMIT,
        first_media_id,
        &prev_batch,
    ) {
        Ok(media_list) => {
            let _ = tx.send(media_list);
        }
        Err(_) => {
            let _ = tx.send((Vec::new(), String::new()));
        }
    });
}

pub fn get_media(bk: &Backend, media: String) {
    let baseu = bk.get_base_url();

    let tx = bk.tx.clone();
    thread::spawn(move || {
        let fname = util::media(&baseu, &media, None);
        let _ = tx.send(BKResponse::Media(fname));
    });
}

pub fn get_media_url(bk: &Backend, media: String, tx: Sender<String>) {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        match resolve_media_url(&baseu, &media, false, 0, 0) {
            Ok(uri) => {
                let _ = tx.send(uri.to_string());
            }
            Err(_) => {
                let _ = tx.send(String::new());
            }
        };
    });
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
                let _ = tx.send(fname);
            }
            Err(_) => {
                let _ = tx.send(String::new());
            }
        };
    });

    Ok(())
}
