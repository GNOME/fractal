use std::thread;
use std::sync::mpsc::Sender;
use error::Error;
use backend::types::BKResponse;
use backend::types::Backend;

use util::dw_media;
use util::download_file;
use util::cache_dir_path;
use util::resolve_media_url;

pub fn get_thumb_async(bk: &Backend, media: String, tx: Sender<String>) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;

    semaphore!(bk.limit_threads, {
        match thumb!(&baseu, &media) {
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
    let baseu = bk.get_base_url()?;

    semaphore!(bk.limit_threads, {
        match media!(&baseu, &media) {
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

pub fn get_media(bk: &Backend, media: String) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;

    let tx = bk.tx.clone();
    thread::spawn(move || {
        match media!(&baseu, &media) {
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

    semaphore!(bk.limit_threads, {
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
        let name = url.split("/").last().unwrap_or_default();
        fname = cache_dir_path("files", name)?.clone();
    }

    thread::spawn(move || {
        match download_file(&url, fname, None) {
            Ok(fname) => { tx.send(fname).unwrap(); }
            Err(_) => { tx.send(String::from("")).unwrap(); }
        };
    });

    Ok(())
}
