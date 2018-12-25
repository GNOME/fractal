pub use backend::types::{BKResponse, Backend};
use error::Error;
use globals;
use std::{sync::mpsc::Sender, thread};

use util;
use util::{
    cache_dir_path, download_file, get_room_media_list, resolve_media_url, semaphore, thumb,
};

use types::Message;

impl Backend {
    pub fn get_thumb_async(&self, media: String, ctx: Sender<String>) {
        let baseu = self.get_base_url();

        semaphore(self.limit_threads.clone(), move || {
            let fname = thumb(&baseu, &media, None).unwrap_or_default();
            ctx.send(fname).unwrap();
        });
    }

    pub fn get_media_async(&self, media: String, ctx: Sender<String>) {
        let baseu = self.get_base_url();

        semaphore(self.limit_threads.clone(), move || {
            let fname = util::media(&baseu, &media, None).unwrap_or_default();
            ctx.send(fname).unwrap();
        });
    }

    pub fn get_media_list_async(
        &self,
        room_id: String,
        first_media_id: Option<String>,
        prev_batch: Option<String>,
        ctx: Sender<(Vec<Message>, String)>,
    ) {
        let baseu = self.get_base_url();
        let tk = self.data.lock().unwrap().access_token.clone();

        semaphore(self.limit_threads.clone(), move || {
            let media_list = get_room_media_list(
                &baseu,
                &tk,
                &room_id,
                globals::PAGE_LIMIT,
                first_media_id,
                prev_batch,
            )
            .unwrap_or_default();
            ctx.send(media_list).unwrap();
        });
    }

    pub fn get_media(&self, media: String) {
        let ctx = self.tx.clone();
        let baseu = self.get_base_url();

        thread::spawn(move || {
            match util::media(&baseu, &media, None) {
                Ok(fname) => {
                    ctx.send(BKResponse::Media(fname)).unwrap();
                }
                Err(err) => {
                    ctx.send(BKResponse::MediaError(err)).unwrap();
                }
            };
        });
    }

    pub fn get_media_url(&self, media: String, ctx: Sender<String>) {
        let baseu = self.get_base_url();

        semaphore(self.limit_threads.clone(), move || {
            let uri = resolve_media_url(&baseu, &media, false, 0, 0)
                .map(|uri| uri.to_string())
                .unwrap_or_default();
            ctx.send(uri).unwrap();
        });
    }

    pub fn get_file_async(&self, url: String, ctx: Sender<String>) {
        let tx = self.tx.clone();
        let r = get_file_async(url, ctx);
        bkerror!(r, tx, BKResponse::CommandError);
    }
}

fn get_file_async(url: String, ctx: Sender<String>) -> Result<(), Error> {
    let u = url.clone();
    let name = url.split('/').last().unwrap_or_default();
    let fname = cache_dir_path("files", name)?;

    thread::spawn(move || {
        let fname = download_file(&u, fname, None).unwrap_or_default();
        ctx.send(fname).unwrap();
    });

    Ok(())
}
