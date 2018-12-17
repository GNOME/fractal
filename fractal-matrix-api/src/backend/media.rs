use backend::types::{BKResponse, Backend};
use error::Error;
use globals;
use std::{sync::mpsc::Sender, thread};

use util;
use util::{
    cache_dir_path, download_file, get_room_media_list, resolve_media_url, semaphore, thumb,
};

use types::Message;

pub fn get_thumb_async(bk: &Backend, media: String, tx: Sender<String>) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;

    semaphore(bk.limit_threads.clone(), move || {
        let fname = thumb(&baseu, &media, None).unwrap_or_default();
        tx.send(fname).unwrap();
    });

    Ok(())
}

pub fn get_media_async(bk: &Backend, media: String, tx: Sender<String>) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;

    semaphore(bk.limit_threads.clone(), move || {
        let fname = util::media(&baseu, &media, None).unwrap_or_default();
        tx.send(fname).unwrap();
    });

    Ok(())
}

pub fn get_media_list_async(
    bk: &Backend,
    room_id: &str,
    first_media_id: Option<String>,
    prev_batch: Option<String>,
    tx: Sender<(Vec<Message>, String)>,
) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;
    let tk = bk.data.lock().unwrap().access_token.clone();
    let room = room_id.to_string();

    semaphore(bk.limit_threads.clone(), move || {
        let media_list = get_room_media_list(
            &baseu,
            &tk,
            &room,
            globals::PAGE_LIMIT,
            first_media_id,
            prev_batch,
        )
        .unwrap_or_default();
        tx.send(media_list).unwrap();
    });

    Ok(())
}

pub fn get_media(bk: &Backend, media: String) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;

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
    let baseu = bk.get_base_url()?;

    semaphore(bk.limit_threads.clone(), move || {
        let uri = resolve_media_url(&baseu, &media, false, 0, 0)
            .map(|uri| uri.to_string())
            .unwrap_or_default();
        tx.send(uri).unwrap();
    });

    Ok(())
}

pub fn get_file_async(url: String, tx: Sender<String>) -> Result<(), Error> {
    let name = url.split('/').last().unwrap_or_default();
    let fname = cache_dir_path("files", name)?;

    let url = url.clone();
    thread::spawn(move || {
        let fname = download_file(url.as_str(), fname, None).unwrap_or_default();
        tx.send(fname).unwrap();
    });

    Ok(())
}
