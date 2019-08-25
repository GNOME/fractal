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
use crate::util::ResultExpectLog;

use crate::types::Message;

pub fn get_thumb_async(bk: &Backend, media: String, tx: Sender<String>) {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        let fname = thumb(&baseu, &media, None).unwrap_or_default();
        tx.send(fname).expect_log("Connection closed");
    });
}

pub fn get_media_async(bk: &Backend, media: String, tx: Sender<String>) {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        let fname = util::media(&baseu, &media, None).unwrap_or_default();
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
        let fname = util::media(&baseu, &media, None);
        tx.send(BKResponse::Media(fname))
            .expect_log("Connection closed");
    });
}

pub fn get_media_url(bk: &Backend, media: String, tx: Sender<String>) {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        let uri = resolve_media_url(&baseu, &media, false, 0, 0)
            .map(|u| u.to_string())
            .unwrap_or_default();
        tx.send(uri).expect_log("Connection closed");
    });
}

pub fn get_file_async(url: String, tx: Sender<String>) -> Result<(), Error> {
    let name = url.split('/').last().unwrap_or_default();
    let fname = cache_dir_path("files", name)?.clone();

    thread::spawn(move || {
        let fname = download_file(&url, fname, None).unwrap_or_default();
        tx.send(fname).expect_log("Connection closed");
    });

    Ok(())
}
