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

use std::collections::HashSet;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use std::time::Duration as StdDuration;

use crate::error::Error;
use crate::types::Event;
use crate::types::JoinedRoom;
use crate::types::Member;
use crate::types::Message;
use crate::types::SyncResponse;
use crate::types::{Room, RoomMembership, RoomTag};

use reqwest::header::CONTENT_TYPE;

use crate::globals;

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
    ($method: expr, $url: expr, $attrs: expr, $okcb: expr, $errcb: expr, $timeout: expr) => {
        thread::spawn(move || {
            let js = json_q($method, $url, $attrs, $timeout);

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

fn evc(events: &Vec<JsonValue>, t: &str, field: &str) -> String {
    events
        .iter()
        .find(|x| x["type"] == t)
        .and_then(|js| js["content"][field].as_str())
        .map(Into::into)
        .unwrap_or_default()
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

pub fn get_rooms_from_sync_response(r: &SyncResponse, userid: &str, baseu: &Url) -> Vec<Room> {
    let join = r.rooms.join.clone();
    let leave = r.rooms.leave.clone();
    let invite = r.rooms.invite.clone();

    // getting the list of direct rooms
    let direct: HashSet<String> = parse_m_direct(&r.account_data.events)
        .values()
        .flatten()
        .cloned()
        .collect();

    let mut rooms: Vec<Room> = vec![];
    for (k, room) in join.iter() {
        let stevents = &room.state.events;
        let timeline = &room.timeline;
        let ephemeral = &room.ephemeral;
        let dataevs = &room.account_data.events;
        let name = calculate_room_name(stevents, userid);
        let mut room_tag = RoomTag::None;
        for tag in dataevs.iter().filter(|x| x["type"] == "m.tag") {
            if tag["content"]["tags"]["m.favourite"].as_object().is_some() {
                room_tag = RoomTag::Favourite;
            }
        }
        let mut r = Room::new(k.clone(), RoomMembership::Joined(room_tag));

        r.name = name;
        r.avatar = Some(evc(stevents, "m.room.avatar", "url"));
        r.alias = Some(evc(stevents, "m.room.canonical_alias", "alias"));
        r.topic = Some(evc(stevents, "m.room.topic", "topic"));
        r.direct = direct.contains(k);
        r.notifications = room.unread_notifications.notification_count;
        r.highlight = room.unread_notifications.highlight_count;

        r.prev_batch = timeline.prev_batch.clone();

        let ms = Message::from_json_events_iter(&k, timeline.events.iter());
        r.messages.extend(ms);

        r.add_receipt_from_json(
            ephemeral
                .events
                .iter()
                .filter(|ev| ev["type"] == "m.receipt")
                .collect(),
        );
        // Adding fully read to the receipts events
        if let Some(fread) = dataevs.into_iter().find(|x| x["type"] == "m.fully_read") {
            if let Some(ev) = fread["content"]["event_id"].as_str() {
                r.add_receipt_from_fully_read(userid, ev);
            }
        }

        let mevents = stevents.iter().filter(|x| x["type"] == "m.room.member");

        for ev in mevents {
            let member = parse_room_member(ev);
            if let Some(m) = member {
                r.members.insert(m.uid.clone(), m.clone());
            }
        }

        // power levels info
        r.power_levels = get_admins(stevents);

        rooms.push(r);
    }

    // left rooms
    for k in leave.keys() {
        let r = Room::new(k.clone(), RoomMembership::Left);
        rooms.push(r);
    }

    // invitations
    for (k, room) in invite.iter() {
        let stevents = &room.invite_state.events;
        let name = calculate_room_name(stevents, userid);

        if let Some(ev) = stevents
            .iter()
            .find(|x| x["membership"] == "invite" && x["state_key"] == userid)
        {
            if let Ok((alias, avatar)) =
                get_user_avatar(baseu, ev["sender"].as_str().unwrap_or_default())
            {
                let inv_sender = Member {
                    alias: Some(alias),
                    avatar: Some(avatar),
                    uid: String::from(userid),
                };
                let mut r = Room::new(k.clone(), RoomMembership::Invited(inv_sender));
                r.name = name;

                r.avatar = Some(evc(stevents, "m.room.avatar", "url"));
                r.alias = Some(evc(stevents, "m.room.canonical_alias", "alias"));
                r.topic = Some(evc(stevents, "m.room.topic", "topic"));
                r.direct = direct.contains(k);

                rooms.push(r);
            }
        }
    }

    rooms
}

fn get_admins(stevents: &Vec<JsonValue>) -> HashMap<String, i32> {
    let mut admins = HashMap::new();

    let plevents = stevents
        .iter()
        .filter(|x| x["type"] == "m.room.power_levels");

    for ev in plevents {
        if let Some(users) = ev["content"]["users"].as_object() {
            for u in users.keys() {
                let level = users[u].as_i64().unwrap_or_default();
                admins.insert(u.to_string(), level as i32);
            }
        }
    }

    admins
}

pub fn get_rooms_timeline_from_joined_rooms(join: &HashMap<String, JoinedRoom>) -> Vec<Message> {
    join.iter()
        .flat_map(|(k, room)| {
            let events = room.timeline.events.iter();
            Message::from_json_events_iter(&k, events).into_iter()
        })
        .collect()
}

pub fn parse_sync_events(join: &HashMap<String, JoinedRoom>) -> Vec<Event> {
    join.iter()
        .flat_map(|(k, room)| {
            room.timeline
                .events
                .iter()
                .filter(|x| x["type"] != "m.room.message")
                .map(move |ev| Event {
                    room: k.clone(),
                    sender: ev["sender"].as_str().map(Into::into).unwrap_or_default(),
                    content: ev["content"].clone(),
                    stype: ev["type"].as_str().map(Into::into).unwrap_or_default(),
                    id: ev["id"].as_str().map(Into::into).unwrap_or_default(),
                })
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

    let r = json_q("get", &url, &json!(null), globals::TIMEOUT)?;
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
            "{\"filter_json\": { \"contains_url\": true, \"not_types\": [\"m.sticker\"] } }"
                .to_string(),
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

    let r = json_q("get", &url, &json!(null), globals::TIMEOUT)?;
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
    let client = reqwest::Client::new();
    let conn = client.get(url);
    let mut res = conn.send()?;

    let mut buffer = Vec::new();
    res.read_to_end(&mut buffer)?;

    Ok(buffer)
}

pub fn put_media(url: &str, file: Vec<u8>) -> Result<JsonValue, Error> {
    let client = reqwest::Client::new();
    let mime = tree_magic::from_u8(&file);

    let conn = client.post(url).body(file).header(CONTENT_TYPE, mime);

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

pub fn json_q(
    method: &str,
    url: &Url,
    attrs: &JsonValue,
    timeout: u64,
) -> Result<JsonValue, Error> {
    let clientb = reqwest::ClientBuilder::new();
    let client = match timeout {
        0 => clientb.timeout(None).build()?,
        n => clientb.timeout(StdDuration::from_secs(n)).build()?,
    };

    let mut conn = match method {
        "post" => client.post(url.as_str()),
        "put" => client.put(url.as_str()),
        "delete" => client.delete(url.as_str()),
        _ => client.get(url.as_str()),
    };

    if !attrs.is_null() {
        conn = conn.json(attrs);
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

    match json_q("get", &url, &attrs, globals::TIMEOUT) {
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

pub fn get_room_st(base: &Url, tk: &str, roomid: &str) -> Result<JsonValue, Error> {
    let url = client_url(
        base,
        &format!("rooms/{}/state", roomid),
        &[("access_token", String::from(tk))],
    )?;

    let attrs = json!(null);
    let st = json_q("get", &url, &attrs, globals::TIMEOUT)?;
    Ok(st)
}

pub fn get_room_avatar(base: &Url, tk: &str, userid: &str, roomid: &str) -> Result<String, Error> {
    let st = get_room_st(base, tk, roomid)?;
    let events = st.as_array().ok_or(Error::BackendError)?;

    // we look for members that aren't me
    let filter = |x: &&JsonValue| {
        (x["type"] == "m.room.member"
            && x["content"]["membership"] == "join"
            && x["sender"] != userid)
    };
    let members = events.iter().filter(&filter);
    let mut members2 = events.iter().filter(&filter);

    let m1 = match members2.next() {
        Some(m) => m["content"]["avatar_url"].as_str().unwrap_or_default(),
        None => "",
    };

    let mut fname = match members.count() {
        1 => {
            if let Ok(dest) = cache_path(&roomid) {
                media(&base, m1, Some(&dest)).unwrap_or_default()
            } else {
                String::new()
            }
        }
        _ => String::new(),
    };

    if fname.is_empty() {
        fname = String::new();
    }

    Ok(fname)
}

fn calculate_room_name(events: &Vec<JsonValue>, userid: &str) -> Option<String> {
    // looking for "m.room.name" event
    if let Some(name) = events.iter().find(|x| x["type"] == "m.room.name") {
        if let Some(name) = name["content"]["name"].as_str() {
            if !name.to_string().is_empty() {
                return Some(name.to_string());
            }
        }
    }

    // looking for "m.room.canonical_alias" event
    if let Some(name) = events
        .iter()
        .find(|x| x["type"] == "m.room.canonical_alias")
    {
        if let Some(name) = name["content"]["alias"].as_str() {
            return Some(name.to_string());
        }
    }

    // we look for members that aren't me
    let filter = |x: &&JsonValue| {
        (x["type"] == "m.room.member"
            && ((x["content"]["membership"] == "join" && x["sender"] != userid)
                || (x["content"]["membership"] == "invite" && x["state_key"] != userid)))
    };
    let c = events.iter().filter(&filter);
    let members = events.iter().filter(&filter);
    let mut members2 = events.iter().filter(&filter);

    if c.count() == 0 {
        // we don't have information to calculate the name
        return None;
    }

    let m1 = match members2.next() {
        Some(m) => {
            let sender = m["sender"].as_str().unwrap_or("NONAMED");
            m["content"]["displayname"].as_str().unwrap_or(sender)
        }
        None => "",
    };
    let m2 = match members2.next() {
        Some(m) => {
            let sender = m["sender"].as_str().unwrap_or("NONAMED");
            m["content"]["displayname"].as_str().unwrap_or(sender)
        }
        None => "",
    };

    let name = match members.count() {
        0 => String::from("EMPTY ROOM"),
        1 => String::from(m1),
        2 => format!("{} and {}", m1, m2),
        _ => format!("{} and Others", m1),
    };

    Some(name)
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

pub fn parse_room_member(msg: &JsonValue) -> Option<Member> {
    let sender = msg["sender"].as_str().unwrap_or_default();

    let c = &msg["content"];

    let membership = c["membership"].as_str();
    if membership.is_none() || membership.unwrap() != "join" {
        return None;
    }

    let displayname = match c["displayname"].as_str() {
        None => None,
        Some(s) => Some(String::from(s)),
    };
    let avatar_url = match c["avatar_url"].as_str() {
        None => None,
        Some(s) => Some(String::from(s)),
    };

    Some(Member {
        uid: String::from(sender),
        alias: displayname,
        avatar: avatar_url,
    })
}

pub fn encode_uid(userid: &str) -> String {
    utf8_percent_encode(userid, USERINFO_ENCODE_SET).collect::<String>()
}
