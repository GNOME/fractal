use lazy_static::lazy_static;
use log::error;

use serde_json::Value as JsonValue;

use directories::ProjectDirs;
use ruma_identifiers::{Error as IdError, RoomId, UserId};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use url::Url;

use std::fs::{create_dir_all, write};

use std::sync::mpsc::SendError;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::client::Client;
use crate::error::Error;
use crate::r0::context::get_context::request as get_context;
use crate::r0::context::get_context::Parameters as GetContextParameters;
use crate::r0::context::get_context::Response as GetContextResponse;
use crate::r0::media::get_content::request as get_content;
use crate::r0::media::get_content::Parameters as GetContentParameters;
use crate::r0::media::get_content_thumbnail::request as get_content_thumbnail;
use crate::r0::media::get_content_thumbnail::Method;
use crate::r0::media::get_content_thumbnail::Parameters as GetContentThumbnailParameters;
use crate::r0::profile::get_profile::request as get_profile;
use crate::r0::profile::get_profile::Response as GetProfileResponse;
use crate::r0::AccessToken;

lazy_static! {
    pub static ref HTTP_CLIENT: Client = Client::new();
    static ref CACHE_PATH: PathBuf = ProjectDirs::from("org", "GNOME", "Fractal")
        .as_ref()
        .map(ProjectDirs::cache_dir)
        .map(Into::into)
        .unwrap_or(std::env::temp_dir().join("fractal"));
}

pub fn semaphore<F>(thread_count: Arc<(Mutex<u8>, Condvar)>, func: F)
where
    F: FnOnce() + Send + 'static,
{
    thread::spawn(move || {
        // waiting, less than 20 threads at the same time
        // this is a semaphore
        // TODO: use std::sync::Semaphore when it's on stable version
        // https://doc.rust-lang.org/1.1.0/std/sync/struct.Semaphore.html
        let &(ref num, ref cvar) = &*thread_count;
        {
            let mut start = num.lock().unwrap();
            while *start >= 20 {
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

// from https://stackoverflow.com/a/43992218/1592377
#[macro_export]
macro_rules! clone {
    (@param _) => ( _ );
    (@param $x:ident) => ( $x );
    ($($n:ident),+ => move || $body:expr) => (
        {
            $( let $n = $n.clone(); )+
            move || $body
        }
    );
    ($($n:ident),+ => move |$($p:tt),+| $body:expr) => (
        {
            $( let $n = $n.clone(); )+
            move |$(clone!(@param $p),)+| $body
        }
    );
}

#[macro_export]
macro_rules! bkerror {
    ($result: expr, $tx: ident, $type: expr) => {
        if let Err(e) = $result {
            let _ = $tx.send($type(Err(e)));
        }
    };
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

pub fn parse_m_direct(events: &Vec<JsonValue>) -> HashMap<UserId, Vec<RoomId>> {
    events
        .iter()
        .find(|x| x["type"] == "m.direct")
        .and_then(|js| js["content"].as_object())
        .cloned()
        .unwrap_or_default()
        .iter()
        // Synapse sometimes sends an object with the key "[object Object]"
        // instead of a user ID, so we have to skip those invalid objects
        // in the array in order to avoid discarding everything
        .filter_map(|(uid, rid)| {
            let value = rid
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|rid| RoomId::try_from(rid.as_str().unwrap_or_default()))
                .collect::<Result<Vec<RoomId>, IdError>>()
                .ok()?;
            Some((UserId::try_from(uid.as_str()).ok()?, value))
        })
        .collect()
}

pub fn get_prev_batch_from(
    base: Url,
    access_token: AccessToken,
    room_id: &RoomId,
    event_id: &str,
) -> Result<String, Error> {
    let params = GetContextParameters {
        access_token,
        limit: 0,
        filter: Default::default(),
    };

    let request = get_context(base, &params, room_id, event_id)?;
    let response: GetContextResponse = HTTP_CLIENT.get_client()?.execute(request)?.json()?;
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
            .get_client()?
            .execute(request)?
            .bytes()
            .collect::<Result<Vec<u8>, std::io::Error>>()
            .and_then(|media| write(&fname, media))
            .and(Ok(fname))
            .map_err(Into::into)
    }
}

pub fn get_user_avatar(base: Url, user_id: &UserId) -> Result<(String, String), Error> {
    let response = get_profile(base.clone(), user_id)
        .map_err::<Error, _>(Into::into)
        .and_then(|request| {
            HTTP_CLIENT
                .get_client()?
                .execute(request)?
                .json::<GetProfileResponse>()
                .map_err(Into::into)
        })?;

    let name = response
        .displayname
        .filter(|n| !n.is_empty())
        .unwrap_or(user_id.to_string());

    let img = response
        .avatar_url
        .map(|url| {
            let dest = cache_dir_path(None, &user_id.to_string())?;
            dw_media(
                base,
                url.as_str(),
                ContentType::default_thumbnail(),
                Some(dest),
            )
        })
        .unwrap_or(Ok(Default::default()))?;

    Ok((name, img))
}

pub fn cache_dir_path(dir: Option<&str>, name: &str) -> Result<String, Error> {
    let path = CACHE_PATH.join(dir.unwrap_or_default());

    if !path.is_dir() {
        create_dir_all(&path)?;
    }

    path.join(name)
        .to_str()
        .map(Into::into)
        .ok_or(Error::CacheError)
}

pub trait ResultExpectLog {
    fn expect_log(&self, log: &str);
}

impl<T> ResultExpectLog for Result<(), SendError<T>> {
    fn expect_log(&self, log: &str) {
        if self.is_err() {
            error!("{}", log);
        }
    }
}
