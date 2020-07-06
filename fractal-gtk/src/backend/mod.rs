use fractal_api::identifiers::{EventId, RoomId};
use fractal_api::url::Url;
use lazy_static::lazy_static;
use std::fs::write;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::client::Client;
use crate::error::Error;
use crate::util::cache_dir_path;
use fractal_api::r0::context::get_context::request as get_context;
use fractal_api::r0::context::get_context::Parameters as GetContextParameters;
use fractal_api::r0::context::get_context::Response as GetContextResponse;
use fractal_api::r0::media::get_content::request as get_content;
use fractal_api::r0::media::get_content::Parameters as GetContentParameters;
use fractal_api::r0::media::get_content_thumbnail::request as get_content_thumbnail;
use fractal_api::r0::media::get_content_thumbnail::Method;
use fractal_api::r0::media::get_content_thumbnail::Parameters as GetContentThumbnailParameters;
use fractal_api::r0::AccessToken;

pub mod directory;
pub mod media;
pub mod register;
pub mod room;
pub mod sync;
pub mod user;

lazy_static! {
    pub static ref HTTP_CLIENT: Client = Client::new();
}

#[derive(Clone, Debug)]
pub struct ThreadPool {
    thread_count: Arc<(Mutex<u8>, Condvar)>,
    limit: u8,
}

impl ThreadPool {
    pub fn new(limit: u8) -> Self {
        ThreadPool {
            thread_count: Arc::new((Mutex::new(0), Condvar::new())),
            limit,
        }
    }

    pub fn run<F>(&self, func: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let thread_count = self.thread_count.clone();
        let limit = self.limit;
        thread::spawn(move || {
            // waiting, less than {limit} threads at the same time
            let &(ref num, ref cvar) = &*thread_count;
            {
                let mut start = num.lock().unwrap();
                while *start >= limit {
                    start = cvar.wait(start).unwrap()
                }
                *start += 1;
            }

            func();

            // freeing the cvar for new threads
            {
                let mut counter = num.lock().unwrap();
                *counter -= 1;
            }
            cvar.notify_one();
        });
    }
}

pub enum ContentType {
    Download,
    Thumbnail(u64, u64),
}

impl ContentType {
    pub fn default_thumbnail() -> Self {
        ContentType::Thumbnail(128, 128)
    }

    pub fn is_thumbnail(&self) -> bool {
        match self {
            ContentType::Download => false,
            ContentType::Thumbnail(_, _) => true,
        }
    }
}

pub fn get_prev_batch_from(
    base: Url,
    access_token: AccessToken,
    room_id: &RoomId,
    event_id: &EventId,
) -> Result<String, Error> {
    let params = GetContextParameters {
        access_token,
        limit: 0,
        filter: Default::default(),
    };

    let request = get_context(base, &params, room_id, event_id)?;
    let response: GetContextResponse = HTTP_CLIENT.get_client().execute(request)?.json()?;
    let prev_batch = response.start.unwrap_or_default();

    Ok(prev_batch)
}

pub fn dw_media(
    base: Url,
    mxc: &str,
    media_type: ContentType,
    dest: Option<String>,
) -> Result<String, Error> {
    let mxc_url = Url::parse(mxc)?;

    if mxc_url.scheme() != "mxc" {
        return Err(Error::BackendError);
    }

    let server = mxc_url.host().ok_or(Error::BackendError)?.to_owned();
    let media_id = mxc_url
        .path_segments()
        .and_then(|mut ps| ps.next())
        .filter(|s| !s.is_empty())
        .ok_or(Error::BackendError)?;

    let request = if let ContentType::Thumbnail(width, height) = media_type {
        let params = GetContentThumbnailParameters {
            width,
            height,
            method: Some(Method::Crop),
            allow_remote: true,
        };
        get_content_thumbnail(base, &params, &server, &media_id)
    } else {
        let params = GetContentParameters::default();
        get_content(base, &params, &server, &media_id)
    }?;

    let fname = match dest {
        None if media_type.is_thumbnail() => cache_dir_path(Some("thumbs"), &media_id)?,
        None => cache_dir_path(Some("medias"), &media_id)?,
        Some(ref d) => d.clone(),
    };

    let fpath = Path::new(&fname);

    // If the file is already cached and recent enough, don't download it
    if fpath.is_file()
        && (dest.is_none() || fpath.metadata()?.modified()?.elapsed()?.as_secs() < 60)
    {
        Ok(fname)
    } else {
        HTTP_CLIENT
            .get_client()
            .execute(request)?
            .bytes()
            .collect::<Result<Vec<u8>, std::io::Error>>()
            .and_then(|media| write(&fname, media))
            .and(Ok(fname))
            .map_err(Into::into)
    }
}
