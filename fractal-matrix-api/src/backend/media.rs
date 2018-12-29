use backend::types::BKResponse;
use backend::types::Backend;
use error::Error;
use globals;
use std::sync::mpsc::Sender;
use std::thread;

use util;
use util::cache_dir_path;
use util::download_file;
use util::get_room_media_list;
use util::resolve_media_url;
use util::semaphore;
use util::thumb;

use types::Message;

pub fn get_thumb_async(bk: &Backend, media: String, tx: Sender<String>) -> Result<(), Error> {
    let baseu = bk.get_base_url();

    semaphore(bk.limit_threads.clone(), move || {
        match thumb(&baseu, &media, None) {
            Ok(fname) => {
                tx.send(fname).unwrap();
            }
            Err(_) => {
                tx.send(String::from("")).unwrap();
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
                tx.send(String::from("")).unwrap();
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
                tx.send(String::from("")).unwrap();
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
                tx.send(String::from("")).unwrap();
            }
        };
    });

    Ok(())
}
