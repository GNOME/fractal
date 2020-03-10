use lazy_static::lazy_static;
use log::error;
use serde_json::json;

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
use crate::r0::profile::get_profile::request as get_profile;
use crate::r0::profile::get_profile::Response as GetProfileResponse;
use crate::r0::AccessToken;

use reqwest::header::CONTENT_LENGTH;

use crate::globals;

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
macro_rules! derror {
    ($from: path, $to: path) => {
        impl From<$from> for Error {
            fn from(_: $from) -> Error {
                $to
            }
        }
    };
}

#[macro_export]
macro_rules! bkerror {
    ($result: ident, $tx: ident, $type: expr) => {
        if let Err(e) = $result {
            $tx.send($type(e)).expect_log("Connection closed");
        }
    };
}

#[macro_export]
macro_rules! bkerror2 {
    ($result: expr, $tx: ident, $type: expr) => {
        if let Err(e) = $result {
            let _ = $tx.send($type(Err(e)));
        }
    };
}

#[macro_export]
macro_rules! get {
    ($($args: expr),+) => {
        query!("get", $($args),+)
    };
}

#[macro_export]
macro_rules! post {
    ($($args: expr),+) => {
        query!("post", $($args),+)
    };
}

#[macro_export]
macro_rules! put {
    ($($args: expr),+) => {
        query!("put", $($args),+)
    };
}

#[macro_export]
macro_rules! query {
    ($method: expr, $url: expr, $attrs: expr, $okcb: expr, $errcb: expr) => {
        thread::spawn(move || {
            let js = json_q($method, $url, $attrs);

            match js {
                Ok(r) => $okcb(r),
                Err(err) => $errcb(err),
            }
        });
    };
    ($method: expr, $url: expr, $okcb: expr, $errcb: expr) => {
        query!($method, $url, &json!(null), $okcb, $errcb)
    };
}

pub enum ContentType {
    Download,
    Thumbnail(i32, i32),
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

pub fn parse_m_direct(events: &Vec<JsonValue>) -> Result<HashMap<UserId, Vec<RoomId>>, IdError> {
    events
        .iter()
        .find(|x| x["type"] == "m.direct")
        .and_then(|js| js["content"].as_object())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|(uid, rid)| {
            let value = rid
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|rid| RoomId::try_from(rid.as_str().unwrap_or_default()))
                .collect::<Result<Vec<RoomId>, IdError>>()?;
            Ok((UserId::try_from(uid.as_str())?, value))
        })
        .collect()
}

pub fn get_prev_batch_from(
    baseu: &Url,
    tk: &AccessToken,
    room_id: &RoomId,
    evid: &str,
) -> Result<String, Error> {
    let params = &[("access_token", tk.to_string()), ("limit", 0.to_string())];

    let path = format!("rooms/{}/context/{}", room_id, evid);
    let url = build_url(baseu, &format!("/_matrix/client/r0/{}", path), params)?;

    let r = json_q("get", url, &json!(null))?;
    let prev_batch = r["start"].to_string().trim_matches('"').to_string();

    Ok(prev_batch)
}

pub fn dw_media(
    base: &Url,
    url: &str,
    media_type: ContentType,
    dest: Option<&str>,
) -> Result<String, Error> {
    let caps = globals::MATRIX_RE
        .captures(url)
        .ok_or(Error::BackendError)?;
    let server = String::from(&caps["server"]);
    let media = String::from(&caps["media"]);

    let (params, path) = if let ContentType::Thumbnail(w, h) = media_type {
        (
            vec![
                ("width", w.to_string()),
                ("height", h.to_string()),
                ("method", String::from("crop")),
            ],
            format!("thumbnail/{}/{}", server, media),
        )
    } else {
        (vec![], format!("download/{}/{}", server, media))
    };

    let url = build_url(base, &format!("/_matrix/media/r0/{}", path), &params)?;

    let fname = match dest {
        None if media_type.is_thumbnail() => cache_dir_path(Some("thumbs"), &media)?,
        None => cache_dir_path(Some("medias"), &media)?,
        Some(d) => String::from(d),
    };

    download_file(url, fname, dest)
}

pub fn download_file(url: Url, fname: String, dest: Option<&str>) -> Result<String, Error> {
    let fpath = Path::new(&fname);

    // If the file is already cached and recent enough, don't download it
    if fpath.is_file()
        && (dest.is_none() || fpath.metadata()?.modified()?.elapsed()?.as_secs() < 60)
    {
        Ok(fname)
    } else {
        HTTP_CLIENT
            .get_client()?
            .get(url)
            .send()?
            .bytes()
            .collect::<Result<Vec<u8>, std::io::Error>>()
            .and_then(|media| write(&fname, media))
            .and(Ok(fname))
            .map_err(Error::from)
    }
}

pub fn json_q(method: &str, url: Url, attrs: &JsonValue) -> Result<JsonValue, Error> {
    let mut conn = match method {
        "post" => HTTP_CLIENT.get_client()?.post(url),
        "put" => HTTP_CLIENT.get_client()?.put(url),
        "delete" => HTTP_CLIENT.get_client()?.delete(url),
        _ => HTTP_CLIENT.get_client()?.get(url),
    };

    if !attrs.is_null() {
        conn = conn.json(attrs);
    } else if method == "post" {
        conn = conn.header(CONTENT_LENGTH, 0);
    }

    let res = conn.send()?;

    //let mut content = String::new();
    //res.read_to_string(&mut content);
    //cb(content);

    if !res.status().is_success() {
        return match res.json() {
            Ok(js) => Err(Error::MatrixError(js)),
            Err(err) => Err(Error::ReqwestError(err)),
        };
    }

    let json: Result<JsonValue, reqwest::Error> = res.json();
    match json {
        Ok(js) => {
            let js2 = js.clone();
            if let Some(error) = js.as_object() {
                if error.contains_key("errcode") {
                    error!("{:#?}", js2);
                    return Err(Error::MatrixError(js2));
                }
            }
            Ok(js)
        }
        Err(_) => Err(Error::BackendError),
    }
}

pub fn get_user_avatar(base: &Url, user_id: &UserId) -> Result<(String, String), Error> {
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
                Some(&dest),
            )
        })
        .unwrap_or(Ok(Default::default()))?;

    Ok((name, img))
}

pub fn build_url(base: &Url, path: &str, params: &[(&str, String)]) -> Result<Url, Error> {
    let mut url = base.join(path)?;

    {
        // If len was 0 `?` would be appended without being needed.
        if !params.is_empty() {
            let mut query = url.query_pairs_mut();
            query.clear();
            for (k, v) in params {
                query.append_pair(k, &v);
            }
        }
    }

    Ok(url)
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
