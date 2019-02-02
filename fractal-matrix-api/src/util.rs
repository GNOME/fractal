use lazy_static::lazy_static;
use log::error;
use serde_json::json;

use serde_json::Value as JsonValue;

use directories::ProjectDirs;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use url::percent_encoding::{utf8_percent_encode, USERINFO_ENCODE_SET};
use url::Url;

use std::fs::create_dir_all;
use std::fs::File;
use std::io::prelude::*;

use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crate::client::Client;
use crate::error::Error;
use crate::r0::filter::RoomEventFilter;
use crate::types::Message;

use reqwest::header::CONTENT_LENGTH;
use reqwest::header::CONTENT_TYPE;

use crate::globals;

lazy_static! {
    pub static ref HTTP_CLIENT: Client = Client::new();
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
            $tx.send($type(e)).unwrap();
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
    ($method: expr, $url: expr, $attrs: expr, $okcb: expr, $errcb: expr) => {
        query!($method, $url, $attrs, $okcb, $errcb, globals::TIMEOUT);
    };
    ($method: expr, $url: expr, $okcb: expr, $errcb: expr) => {
        let attrs = json!(null);
        query!($method, $url, &attrs, $okcb, $errcb)
    };
}

pub fn parse_m_direct(events: &Vec<JsonValue>) -> HashMap<String, Vec<String>> {
    events
        .iter()
        .find(|x| x["type"] == "m.direct")
        .and_then(|js| js["content"].as_object())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|(k, v)| {
            let value = v
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|rid| rid.as_str().map(Into::into).unwrap_or_default())
                .collect();
            (k.clone(), value)
        })
        .collect()
}

pub fn get_prev_batch_from(
    baseu: &Url,
    tk: &str,
    roomid: &str,
    evid: &str,
) -> Result<String, Error> {
    let params = &[("access_token", String::from(tk)), ("limit", 0.to_string())];

    let path = format!("rooms/{}/context/{}", roomid, evid);
    let url = client_url(baseu, &path, params)?;

    let r = json_q("get", &url, &json!(null))?;
    let prev_batch = r["start"].to_string().trim_matches('"').to_string();

    Ok(prev_batch)
}

pub fn get_room_media_list(
    baseu: &Url,
    tk: &str,
    roomid: &str,
    limit: i32,
    first_media_id: Option<String>,
    prev_batch: &Option<String>,
) -> Result<(Vec<Message>, String), Error> {
    let mut params = vec![
        ("dir", String::from("b")),
        ("limit", format!("{}", limit)),
        ("access_token", String::from(tk)),
        (
            "filter",
            serde_json::to_string(&RoomEventFilter {
                contains_url: true,
                not_types: vec!["m.sticker"],
                ..Default::default()
            })
            .expect("Failed to serialize room media list request filter"),
        ),
    ];

    match prev_batch {
        Some(ref pb) => params.push(("from", pb.clone())),
        None => {
            if let Some(id) = first_media_id {
                params.push(("from", get_prev_batch_from(baseu, tk, &roomid, &id)?))
            }
        }
    };

    let path = format!("rooms/{}/messages", roomid);
    let url = client_url(baseu, &path, &params)?;

    let r = json_q("get", &url, &json!(null))?;
    let array = r["chunk"].as_array();
    let prev_batch = r["end"].to_string().trim_matches('"').to_string();
    if array.is_none() || array.unwrap().is_empty() {
        return Ok((vec![], prev_batch));
    }

    let evs = array.unwrap().iter().rev();
    let media_list = Message::from_json_events_iter(roomid, evs);

    Ok((media_list, prev_batch))
}

pub fn get_media(url: &str) -> Result<Vec<u8>, Error> {
    let conn = HTTP_CLIENT.get_client()?.get(url);
    let mut res = conn.send()?;

    let mut buffer = Vec::new();
    res.read_to_end(&mut buffer)?;

    Ok(buffer)
}

pub fn put_media(url: &str, file: Vec<u8>) -> Result<JsonValue, Error> {
    let (mime, _) = gio::content_type_guess(None, file.as_slice());

    let conn = HTTP_CLIENT
        .get_client()?
        .post(url)
        .body(file)
        .header(CONTENT_TYPE, mime);

    let mut res = conn.send()?;

    match res.json() {
        Ok(js) => Ok(js),
        Err(_) => Err(Error::BackendError),
    }
}

pub fn resolve_media_url(base: &Url, url: &str, thumb: bool, w: i32, h: i32) -> Result<Url, Error> {
    let caps = globals::MATRIX_RE
        .captures(url)
        .ok_or(Error::BackendError)?;
    let server = String::from(&caps["server"]);
    let media = String::from(&caps["media"]);

    let mut params: Vec<(&str, String)> = vec![];

    let path = if thumb {
        params.push(("width", format!("{}", w)));
        params.push(("height", format!("{}", h)));
        params.push(("method", String::from("scale")));
        format!("thumbnail/{}/{}", server, media)
    } else {
        format!("download/{}/{}", server, media)
    };

    media_url(base, &path, &params)
}

pub fn dw_media(
    base: &Url,
    url: &str,
    thumb: bool,
    dest: Option<&str>,
    w: i32,
    h: i32,
) -> Result<String, Error> {
    let caps = globals::MATRIX_RE
        .captures(url)
        .ok_or(Error::BackendError)?;
    let server = String::from(&caps["server"]);
    let media = String::from(&caps["media"]);

    let mut params: Vec<(&str, String)> = vec![];

    let path = if thumb {
        params.push(("width", format!("{}", w)));
        params.push(("height", format!("{}", h)));
        params.push(("method", String::from("crop")));
        format!("thumbnail/{}/{}", server, media)
    } else {
        format!("download/{}/{}", server, media)
    };

    let url = media_url(base, &path, &params)?;

    let fname = match dest {
        None if thumb => cache_dir_path("thumbs", &media)?,
        None => cache_dir_path("medias", &media)?,
        Some(d) => String::from(d),
    };

    download_file(url.as_str(), fname, dest)
}

pub fn media(base: &Url, url: &str, dest: Option<&str>) -> Result<String, Error> {
    dw_media(base, url, false, dest, 0, 0)
}

pub fn thumb(base: &Url, url: &str, dest: Option<&str>) -> Result<String, Error> {
    dw_media(
        base,
        url,
        true,
        dest,
        globals::THUMBNAIL_SIZE,
        globals::THUMBNAIL_SIZE,
    )
}

pub fn download_file(url: &str, fname: String, dest: Option<&str>) -> Result<String, Error> {
    let pathname = fname.clone();
    let p = Path::new(&pathname);
    if p.is_file() {
        if dest.is_none() {
            return Ok(fname);
        }

        let moddate = p.metadata()?.modified()?;
        // one minute cached
        if moddate.elapsed()?.as_secs() < 60 {
            return Ok(fname);
        }
    }

    let mut file = File::create(&fname)?;
    let buffer = get_media(url)?;
    file.write_all(&buffer)?;

    Ok(fname)
}

pub fn json_q(method: &str, url: &Url, attrs: &JsonValue) -> Result<JsonValue, Error> {
    let mut conn = match method {
        "post" => HTTP_CLIENT.get_client()?.post(url.as_str()),
        "put" => HTTP_CLIENT.get_client()?.put(url.as_str()),
        "delete" => HTTP_CLIENT.get_client()?.delete(url.as_str()),
        _ => HTTP_CLIENT.get_client()?.get(url.as_str()),
    };

    if !attrs.is_null() {
        conn = conn.json(attrs);
    } else if method == "post" {
        conn = conn.header(CONTENT_LENGTH, 0);
    }

    let mut res = conn.send()?;

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

pub fn get_user_avatar(baseu: &Url, userid: &str) -> Result<(String, String), Error> {
    let url = client_url(baseu, &format!("profile/{}", encode_uid(userid)), &[])?;
    let attrs = json!(null);

    match json_q("get", &url, &attrs) {
        Ok(js) => {
            let name = match js["displayname"].as_str() {
                Some(n) if n.is_empty() => userid.to_string(),
                Some(n) => n.to_string(),
                None => userid.to_string(),
            };

            match js["avatar_url"].as_str() {
                Some(url) => {
                    let dest = cache_path(userid)?;
                    let img = thumb(baseu, &url, Some(&dest))?;
                    Ok((name.clone(), img))
                }
                None => Ok((name.clone(), String::new())),
            }
        }
        Err(_) => Ok((String::from(userid), String::new())),
    }
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

pub fn client_url(base: &Url, path: &str, params: &[(&str, String)]) -> Result<Url, Error> {
    build_url(base, &format!("/_matrix/client/r0/{}", path), params)
}

pub fn scalar_url(base: &Url, path: &str, params: &[(&str, String)]) -> Result<Url, Error> {
    build_url(base, &format!("api/{}", path), params)
}

pub fn media_url(base: &Url, path: &str, params: &[(&str, String)]) -> Result<Url, Error> {
    build_url(base, &format!("/_matrix/media/r0/{}", path), params)
}

pub fn cache_path(name: &str) -> Result<String, Error> {
    cache_dir_path("", name)
}

pub fn cache_dir_path(dir: &str, name: &str) -> Result<String, Error> {
    let path = &ProjectDirs::from("org", "GNOME", "Fractal")
        .as_ref()
        .map(|project_dir| project_dir.cache_dir())
        .unwrap_or(&std::env::temp_dir().join("fractal"))
        .join(dir);

    if !path.is_dir() {
        create_dir_all(path)?;
    }

    path.join(name)
        .to_str()
        .map(Into::into)
        .ok_or(Error::CacheError)
}

pub fn get_user_avatar_img(baseu: &Url, userid: &str, avatar: &str) -> Result<String, Error> {
    if avatar.is_empty() {
        return Ok(String::new());
    }

    let dest = cache_path(&userid)?;
    thumb(baseu, &avatar, Some(&dest))
}

pub fn encode_uid(userid: &str) -> String {
    utf8_percent_encode(userid, USERINFO_ENCODE_SET).collect::<String>()
}
