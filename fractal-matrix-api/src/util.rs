use crate::{
    error::Error,
    globals,
    types::{Event, Member, Message, Room},
    JsonValue,
};
use log::error;
use reqwest::{self, header::CONTENT_TYPE};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, create_dir_all},
    io::Read, // FIXME: reqwest::Response isn't re-exporting this, but it should
    path::Path,
    sync::{Arc, Condvar, Mutex},
    thread,
};
use url::{
    percent_encoding::{utf8_percent_encode, USERINFO_ENCODE_SET},
    Url,
};

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
            fn from(_: $from) -> Self {
                $to
            }
        }
    };
}

#[macro_export]
macro_rules! bkerror {
    ($result: ident, $tx: ident, $type: expr) => {
        $result.or_else(|e| $tx.send($type(e))).unwrap();
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
    ($method: expr, $url: expr, $attrs: expr, $okcb: expr, $errcb: expr, $timeout: expr) => {
        thread::spawn(move || match json_q($method, $url, $attrs, $timeout) {
            Ok(r) => $okcb(r),
            Err(err) => $errcb(err),
        });
    };
    ($method: expr, $url: expr, $attrs: expr, $okcb: expr, $errcb: expr) => {
        query!($method, $url, $attrs, $okcb, $errcb, globals::TIMEOUT);
    };
    ($method: expr, $url: expr, $okcb: expr, $errcb: expr) => {
        query!($method, $url, &json!(null), $okcb, $errcb)
    };
}

fn evc(events: &JsonValue, t: &str, field: &str) -> String {
    events
        .as_array()
        .and_then(|arr| arr.iter().find(|x| x["type"] == t))
        .and_then(|js| js["content"][field].as_str())
        .unwrap_or_default()
        .to_string()
}

pub fn parse_m_direct(r: &JsonValue) -> HashMap<String, Vec<String>> {
    r["account_data"]["events"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .find(|x| x["type"] == "m.direct")
        .and_then(|js| js["content"].as_object())
        .iter()
        .flat_map(|m| m.iter())
        .map(|(k, v)| {
            let value = v
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|rid| rid.as_str().unwrap_or_default().to_string())
                .collect();
            (k.to_string(), value)
        })
        .collect()
}

pub fn get_rooms_from_json(r: &JsonValue, userid: &str, baseu: &Url) -> Result<Vec<Room>, Error> {
    let rooms = &r["rooms"];

    let join = rooms["join"].as_object().ok_or(Error::BackendError)?;
    let leave = rooms["leave"].as_object().ok_or(Error::BackendError)?;
    let invite = rooms["invite"].as_object().ok_or(Error::BackendError)?;

    // getting the list of direct rooms
    let direct = parse_m_direct(r)
        .values()
        .flat_map(|v| v.iter())
        .cloned()
        .collect::<HashSet<String>>();

    let joined = join.iter().map(|(k, room)| {
        let k = k.to_string();
        let stevents = &room["state"]["events"];
        let timeline = &room["timeline"];
        let ephemeral = &room["ephemeral"];
        let dataevs = &room["account_data"]["events"];
        let name = calculate_room_name(stevents, userid)?;
        let mut r = Room::new(k.clone(), name);

        r.avatar = Some(evc(stevents, "m.room.avatar", "url"));
        r.alias = Some(evc(stevents, "m.room.canonical_alias", "alias"));
        r.topic = Some(evc(stevents, "m.room.topic", "topic"));
        r.direct = direct.contains(&k);
        r.notifications = room["unread_notifications"]["notification_count"]
            .as_i64()
            .unwrap_or_default() as i32;
        r.highlight = room["unread_notifications"]["highlight_count"]
            .as_i64()
            .unwrap_or_default() as i32;
        r.prev_batch = timeline["prev_batch"].as_str().map(Into::into);
        r.fav = dataevs
            .as_array()
            .iter()
            .flat_map(|evs| evs.iter())
            .filter(|x| x["type"] == "m.tag")
            .any(|tag| tag["content"]["tags"]["m.favourite"].as_object().is_some());
        r.messages = timeline["events"]
            .as_array()
            .map(|evs| Message::from_json_events_iter(&k, evs.iter()))
            .unwrap_or(r.messages);

        ephemeral["events"].as_array().map(|evs| {
            r.add_receipt_from_json(evs.iter().filter(|ev| ev["type"] == "m.receipt").collect());
        });
        // Adding fully read to the receipts events
        dataevs
            .as_array()
            .and_then(|evs| evs.iter().find(|x| x["type"] == "m.fully_read"))
            .and_then(|fread| fread["content"]["event_id"].as_str())
            .map(|ev| r.add_receipt_from_fully_read(userid, ev));

        r.members = stevents
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x["type"] == "m.room.member")
            .filter_map(|ev| parse_room_member(ev).map(|m| (m.uid.clone(), m)))
            .collect();

        // power levels info
        r.power_levels = get_admins(stevents);

        Ok(r)
    });

    // left rooms
    let left = leave.keys().map(|k| {
        let mut r = Room::new(k.to_string(), None);
        r.left = true;
        Ok(r)
    });

    // invitations
    let invites = invite.keys().map(|k| {
        let room = invite.get(k).ok_or(Error::BackendError)?;
        let stevents = &room["invite_state"]["events"];
        let name = calculate_room_name(stevents, userid)?;
        let mut r = Room::new(k.to_string(), name);

        r.inv = true;
        r.avatar = Some(evc(stevents, "m.room.avatar", "url"));
        r.alias = Some(evc(stevents, "m.room.canonical_alias", "alias"));
        r.topic = Some(evc(stevents, "m.room.topic", "topic"));
        r.direct = direct.contains(k);
        r.inv_sender = stevents
            .as_array()
            .and_then(|arr| {
                arr.iter()
                    .find(|x| x["membership"] == "invite" && x["state_key"] == userid)
            })
            .and_then(|ev| get_user_avatar(baseu, ev["sender"].as_str().unwrap_or_default()).ok())
            .map(|(alias, avatar)| Member {
                alias: Some(alias),
                avatar: Some(avatar),
                uid: userid.to_string(),
            })
            .or(r.inv_sender);

        Ok(r)
    });

    std::iter::empty()
        .chain(joined)
        .chain(left)
        .chain(invites)
        .collect()
}

pub fn get_admins(stevents: &JsonValue) -> HashMap<String, i32> {
    stevents
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["type"] == "m.room.power_levels")
        .filter_map(|ev| ev["content"]["users"].as_object())
        .flat_map(|users| {
            users.keys().map(move |u| {
                let level = users[u].as_i64().unwrap_or_default() as i32;
                (u.to_string(), level)
            })
        })
        .collect()
}

pub fn get_rooms_timeline_from_json(
    baseu: &Url,
    r: &JsonValue,
    tk: &str,
    prev_batch: &str,
) -> Result<Vec<Message>, Error> {
    let rooms = &r["rooms"];
    let join = rooms["join"].as_object().ok_or(Error::BackendError)?;

    let msgs = join
        .keys()
        .map(|k| {
            let room = &join[k]["timeline"];

            let fill_the_gap = if let (Some(true), Some(pb)) =
                (room["limited"].as_bool(), room["prev_batch"].as_str())
            {
                fill_room_gap(baseu, tk.to_string(), k, &prev_batch, pb)?
            } else {
                vec![]
            };

            let timeline = room["events"].as_array();
            if timeline.is_none() {
                return Ok(fill_the_gap.into_iter().chain(vec![]));
            }

            let events = timeline.unwrap().iter();
            let ms = Message::from_json_events_iter(&k, events);
            Ok(fill_the_gap.into_iter().chain(ms))
        })
        .collect::<Result<Vec<_>, Error>>()?
        .into_iter()
        .flatten()
        .collect();

    Ok(msgs)
}

pub fn get_rooms_notifies_from_json(r: &JsonValue) -> Result<Vec<(String, i32, i32)>, Error> {
    let rooms = &r["rooms"];
    let join = rooms["join"].as_object().ok_or(Error::BackendError)?;

    let out = join
        .iter()
        .map(|(k, room)| {
            let n = room["unread_notifications"]["notification_count"]
                .as_i64()
                .unwrap_or_default() as i32;
            let h = room["unread_notifications"]["highlight_count"]
                .as_i64()
                .unwrap_or_default() as i32;
            (k.to_string(), n, h)
        })
        .collect();

    Ok(out)
}

pub fn parse_sync_events(r: &JsonValue) -> Result<Vec<Event>, Error> {
    let rooms = &r["rooms"];

    let evs = rooms["join"]
        .as_object()
        .ok_or(Error::BackendError)?
        .iter()
        .map(|(k, room)| (k, room["timeline"]["events"].as_array()))
        .take_while(|(_, timeline)| timeline.is_some())
        .flat_map(|(k, timeline)| {
            timeline
                .unwrap()
                .iter()
                .filter(|x| x["type"] != "m.room.message")
                .map(move |ev| Event {
                    room: k.to_string(),
                    sender: ev["sender"].as_str().unwrap_or_default().to_string(),
                    content: ev["content"].clone(),
                    stype: ev["type"].as_str().unwrap_or_default().to_string(),
                    id: ev["id"].as_str().unwrap_or_default().to_string(),
                })
        })
        .collect();

    Ok(evs)
}

pub fn get_prev_batch_from(
    baseu: &Url,
    tk: &str,
    room_id: &str,
    evid: &str,
) -> Result<String, Error> {
    let params = &[("access_token", tk.to_string()), ("limit", 0.to_string())];

    let path = format!("rooms/{}/context/{}", room_id, evid);
    let url = client_url(baseu, &path, params)?;

    let r = json_q("get", &url, &json!(null), globals::TIMEOUT)?;
    let prev_batch = r["start"].to_string().trim_matches('"').to_string();

    Ok(prev_batch)
}

pub fn get_room_media_list(
    baseu: &Url,
    tk: &str,
    room_id: &str,
    limit: i32,
    first_media_id: Option<String>,
    prev_batch: Option<String>,
) -> Result<(Vec<Message>, String), Error> {
    let prev_batch = if prev_batch.is_some() {
        prev_batch
    } else if let Some(id) = first_media_id {
        Some(get_prev_batch_from(baseu, tk, room_id, &id)?)
    } else {
        None
    };

    let params = [
        ("dir", "b".to_string()),
        ("limit", limit.to_string()),
        ("access_token", tk.to_string()),
        (
            "filter",
            "{\"filter_json\": { \"contains_url\": true, \"not_types\": [\"m.sticker\"] } }"
                .to_string(),
        ),
    ]
    .iter()
    .cloned()
    .chain(prev_batch.map(|pb| ("from", pb)))
    .collect::<Vec<_>>();

    let path = format!("rooms/{}/messages", room_id);
    let url = client_url(baseu, &path, &params)?;

    let r = json_q("get", &url, &json!(null), globals::TIMEOUT)?;
    let array = r["chunk"].as_array();
    let prev_batch = r["end"].to_string().trim_matches('"').to_string();
    if array.is_none() || array.unwrap().is_empty() {
        return Ok((vec![], prev_batch));
    }

    let evs = array.unwrap().iter().rev();
    let media_list = Message::from_json_events_iter(room_id, evs);

    Ok((media_list, prev_batch))
}

pub fn get_media(url: &str) -> Result<Vec<u8>, Error> {
    reqwest::Client::new()
        .get(url)
        .send()?
        .bytes()
        .filter_map(|b| {
            b.and_then(|b| Ok(Ok(b)))
                .or_else(|err| {
                    if err.kind() == std::io::ErrorKind::Interrupted {
                        Err(err)
                    } else {
                        Ok(Err(Error::from(err)))
                    }
                })
                .ok()
        })
        .collect()
}

pub fn put_media(url: &str, file: Vec<u8>) -> Result<JsonValue, Error> {
    let mime = tree_magic::from_u8(&file);

    reqwest::Client::new()
        .post(url)
        .body(file)
        .header(CONTENT_TYPE, mime)
        .send()?
        .json()
        .map_err(|_| Error::BackendError)
}

pub fn resolve_media_url(base: &Url, url: &str, thumb: bool, w: i32, h: i32) -> Result<Url, Error> {
    let caps = globals::MATRIX_RE
        .captures(url)
        .ok_or(Error::BackendError)?;
    let server = caps["server"].to_string();
    let media = caps["media"].to_string();

    let params;
    let path;

    if thumb {
        params = vec![
            ("width", w.to_string()),
            ("height", h.to_string()),
            ("method", "scale".to_string()),
        ];
        path = format!("thumbnail/{}/{}", server, media);
    } else {
        params = vec![];
        path = format!("download/{}/{}", server, media);
    }

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
    let server = &caps["server"];
    let media = &caps["media"];

    let params;
    let path;

    if thumb {
        params = vec![
            ("width", w.to_string()),
            ("height", h.to_string()),
            ("method", "crop".to_string()),
        ];
        path = format!("thumbnail/{}/{}", server, media);
    } else {
        params = vec![];
        path = format!("download/{}/{}", server, media);
    }

    let url = media_url(base, &path, &params)?;

    let fname = match dest {
        None if thumb => cache_dir_path("thumbs", media),
        None => cache_dir_path("medias", media),
        Some(d) => Ok(d.to_string()),
    }?;

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
    // This chain will return Err if the file doesn't exist or
    // there is any error, so in both cases we default to a value that
    // returns false in the comparison to proceed to try to (re)write
    // the file
    if Path::new(&fname)
        .metadata()
        .ok()
        .filter(|_| dest.is_some())
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.elapsed().ok())
        .map(|t| t.as_secs())
        .unwrap_or(60)
        < 60
    {
        return Ok(fname);
    }

    let contents = get_media(url)?;
    fs::write(&fname, contents)?;

    Ok(fname)
}

pub fn json_q(
    method: &str,
    url: &Url,
    attrs: &JsonValue,
    timeout: u64,
) -> Result<JsonValue, Error> {
    let client = reqwest::ClientBuilder::new()
        .timeout(match timeout {
            0 => None,
            n => Some(std::time::Duration::from_secs(n)),
        })
        .build()?;

    let conn_method = match method {
        "post" => reqwest::Client::post,
        "put" => reqwest::Client::put,
        "delete" => reqwest::Client::delete,
        _ => reqwest::Client::get,
    };

    let conn = if attrs.is_null() {
        conn_method(&client, url.as_str())
    } else {
        conn_method(&client, url.as_str()).json(attrs)
    };

    let mut res = conn.send()?;
    let json = res.json::<JsonValue>();

    if res.status().is_success() {
        json.or(Err(Error::BackendError)).and_then(|js| {
            js.clone()
                .as_object() // Option
                .filter(|error| error.contains_key("errcode"))
                .map(|_| {
                    error!("{:#?}", js.clone());
                    Err(Error::MatrixError(js.clone()))
                })
                .unwrap_or(Ok(js))
        })
    } else {
        json.or_else(|err| Err(Error::ReqwestError(err)))
            .and_then(|js| Err(Error::MatrixError(js)))
    }
}

pub fn get_user_avatar(baseu: &Url, userid: &str) -> Result<(String, String), Error> {
    let url = client_url(baseu, &format!("profile/{}", encode_uid(userid)), &[])?;
    let attrs = json!(null);

    if let Ok(js) = json_q("get", &url, &attrs, globals::TIMEOUT) {
        let name = match js["displayname"].as_str() {
            Some(n) if n.is_empty() => userid.to_string(),
            Some(n) => n.to_string(),
            None => userid.to_string(),
        };

        if let Some(url) = js["avatar_url"].as_str() {
            let dest = cache_path(userid)?;
            let img = thumb(baseu, &url, Some(&dest))?;
            Ok((name, img))
        } else {
            Ok((name, String::new()))
        }
    } else {
        Ok((userid.to_string(), String::new()))
    }
}

pub fn get_room_st(base: &Url, tk: &str, room_id: &str) -> Result<JsonValue, Error> {
    let url = client_url(
        base,
        &format!("rooms/{}/state", room_id),
        &[("access_token", tk.to_string())],
    )?;

    let attrs = json!(null);
    json_q("get", &url, &attrs, globals::TIMEOUT)
}

pub fn get_room_avatar(base: &Url, tk: &str, userid: &str, room_id: &str) -> Result<String, Error> {
    let st = get_room_st(base, tk, room_id)?;
    let events = st.as_array().ok_or(Error::BackendError)?;

    // we look for members that aren't me
    let filter = |x: &&JsonValue| {
        (x["type"] == "m.room.member"
            && x["content"]["membership"] == "join"
            && x["sender"] != userid)
    };

    let members = events.iter().filter(filter);
    let first_member = members.clone().next();

    let fname = cache_path(room_id)
        .ok()
        .filter(|_| first_member.is_some() && members.count() == 1)
        .and_then(|dest| {
            let m1 = first_member
                .and_then(|m| m["content"]["avatar_url"].as_str())
                .unwrap_or_default();
            media(&base, m1, Some(&dest)).ok()
        })
        .unwrap_or_default();

    Ok(fname)
}

pub fn calculate_room_name(roomst: &JsonValue, userid: &str) -> Result<Option<String>, Error> {
    let events = roomst.as_array().ok_or(Error::BackendError)?;
    // looking for "m.room.name" event
    if let Some(name) = events.iter().find(|x| x["type"] == "m.room.name") {
        if let Some(name) = name["content"]["name"].as_str() {
            if !name.is_empty() {
                return Ok(Some(name.to_string()));
            }
        }
    }
    // looking for "m.room.canonical_alias" event
    if let Some(name) = events
        .iter()
        .find(|x| x["type"] == "m.room.canonical_alias")
    {
        if let Some(name) = name["content"]["alias"].as_str() {
            return Ok(Some(name.to_string()));
        }
    }

    // we look for members that aren't me
    let filter = |x: &&JsonValue| {
        (x["type"] == "m.room.member"
            && ((x["content"]["membership"] == "join" && x["sender"] != userid)
                || (x["content"]["membership"] == "invite" && x["state_key"] != userid)))
    };

    let members = events.iter().filter(filter);

    if members.clone().count() == 0 {
        // we don't have information to calculate the name
        return Ok(None);
    }

    macro_rules! get_next_m {
        () => {
            members
                .clone()
                .next()
                .and_then(|m| {
                    let sender = m["sender"].as_str().or(Some("NONAMED"));
                    m["content"]["displayname"].as_str().or(sender)
                })
                .unwrap_or_default()
        };
    }

    let m1 = get_next_m!();
    let m2 = get_next_m!();

    let name = match members.count() {
        0 => "EMPTY ROOM".to_string(),
        1 => m1.to_string(),
        2 => format!("{} and {}", m1, m2),
        _ => format!("{} and Others", m1),
    };

    Ok(Some(name))
}

/// Recursive function that tries to get all messages in a room from a batch id to a batch id,
/// following the response pagination
pub fn fill_room_gap(
    baseu: &Url,
    tk: String,
    room_id: &str,
    from: &str,
    to: &str,
) -> Result<Vec<Message>, Error> {
    let params = &[
        ("dir", "f".to_string()),
        ("limit", globals::PAGE_LIMIT.to_string()),
        ("access_token", tk.clone()),
        ("from", from.to_string()),
        ("to", to.to_string()),
    ];

    let path = format!("rooms/{}/messages", room_id);
    let url = client_url(baseu, &path, params)?;

    let r = json_q("get", &url, &json!(null), globals::TIMEOUT)?;
    let nend = r["end"].as_str().unwrap_or_default();
    let array = r["chunk"].as_array().filter(|array| !array.is_empty());
    if array.is_none() {
        return Ok(Vec::new());
    }

    let ms = fill_room_gap(baseu, tk, room_id, nend, to)?
        .iter()
        .rev()
        .chain(Message::from_json_events_iter(room_id, array.unwrap().iter()).iter())
        .cloned()
        .collect();

    Ok(ms)
}

pub fn build_url(base: &Url, path: &str, params: &[(&str, String)]) -> Result<Url, Error> {
    let url = base.join(path)?;

    if !params.is_empty() {
        Ok(Url::parse_with_params(url.as_str(), params)?)
    } else {
        Ok(url)
    }
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
    let ref path = directories::ProjectDirs::from("org", "GNOME", "Fractal")
        .map(|project_dir| project_dir.cache_dir().to_path_buf())
        .unwrap_or(std::env::temp_dir().join("fractal"))
        .join(dir)
        .join(name);

    if !path.is_dir() {
        create_dir_all(path.parent().unwrap())?;
    }

    path.to_str().map(Into::into).ok_or(Error::CacheError)
}

pub fn get_user_avatar_img(baseu: &Url, userid: &str, avatar: &str) -> Result<String, Error> {
    if avatar.is_empty() {
        return Ok(String::new());
    }

    let dest = cache_path(&userid)?;
    thumb(baseu, &avatar, Some(&dest))
}

pub fn parse_room_member(msg: &JsonValue) -> Option<Member> {
    let c = &msg["content"];
    c["membership"].as_str().filter(|m| m == &"join")?;

    Some(Member {
        uid: msg["sender"].as_str().unwrap_or_default().to_string(),
        alias: c["displayname"].as_str().map(Into::into),
        avatar: c["avatar_url"].as_str().map(Into::into),
    })
}

pub fn encode_uid(userid: &str) -> String {
    utf8_percent_encode(userid, USERINFO_ENCODE_SET).collect()
}
