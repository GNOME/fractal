extern crate glib;
extern crate url;
extern crate reqwest;
extern crate regex;
extern crate serde_json;
extern crate cairo;
extern crate pango;
extern crate pangocairo;
extern crate gdk;
extern crate gdk_pixbuf;
extern crate mime;
extern crate tree_magic;
extern crate unicode_segmentation;

use self::unicode_segmentation::UnicodeSegmentation;

use self::pango::LayoutExt;

use self::gdk_pixbuf::Pixbuf;
use self::gdk_pixbuf::PixbufExt;
use self::gdk::ContextExt;

use self::regex::Regex;

use self::serde_json::Value as JsonValue;

use self::url::Url;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::collections::HashMap;

use std::fs::File;
use std::fs::create_dir_all;
use std::io::prelude::*;

use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use std::time::Duration as StdDuration;

use error::Error;
use types::Message;
use types::Room;
use types::Event;
use types::Member;

use self::reqwest::header::ContentType;
use self::mime::Mime;

use globals;


#[allow(dead_code)]
pub enum AvatarMode {
    Rect,
    Circle,
}


#[macro_export]
macro_rules! identicon {
    ($userid: expr, $name: expr) => { draw_identicon($userid, $name, AvatarMode::Circle) }
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
macro_rules! strn {
    ($p: expr) => (
        String::from($p)
    );
}

#[macro_export]
macro_rules! client_url {
    ($b: expr, $path: expr, $params: expr) => (
        build_url($b, &format!("/_matrix/client/r0/{}", $path), $params)
    )
}

#[macro_export]
macro_rules! scalar_url {
    ($b: expr, $path: expr, $params: expr) => (
        build_url($b, &format!("api/{}", $path), $params)
    )
}

#[macro_export]
macro_rules! media_url {
    ($b: expr, $path: expr, $params: expr) => (
        build_url($b, &format!("/_matrix/media/r0/{}", $path), $params)
    )
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
    }
}

#[macro_export]
macro_rules! get {
    ($url: expr, $attrs: expr, $okcb: expr, $errcb: expr) => {
        query!("get", $url, $attrs, $okcb, $errcb)
    };
    ($url: expr, $okcb: expr, $errcb: expr) => {
        query!("get", $url, $okcb, $errcb)
    };
}

#[macro_export]
macro_rules! post {
    ($url: expr, $attrs: expr, $okcb: expr, $errcb: expr) => {
        query!("post", $url, $attrs, $okcb, $errcb)
    };
    ($url: expr, $okcb: expr, $errcb: expr) => {
        query!("post", $url, $okcb, $errcb)
    };
}

#[macro_export]
macro_rules! query {
    ($method: expr, $url: expr, $attrs: expr, $okcb: expr, $errcb: expr) => {
        thread::spawn(move || {
            let js = json_q($method, $url, $attrs, globals::TIMEOUT);

            match js {
                Ok(r) => {
                    $okcb(r)
                },
                Err(err) => {
                    $errcb(err)
                }
            }
        });
    };
    ($method: expr, $url: expr, $okcb: expr, $errcb: expr) => {
        let attrs = json!(null);
        query!($method, $url, &attrs, $okcb, $errcb)
    };
}

#[allow(unused_macros)]
#[macro_export]
macro_rules! media {
    ($base: expr, $url: expr, $dest: expr) => {
        dw_media($base, $url, false, $dest, 0, 0)
    };
    ($base: expr, $url: expr) => {
        dw_media($base, $url, false, None, 0, 0)
    };
}

#[macro_export]
macro_rules! thumb {
    ($base: expr, $url: expr) => {
        dw_media($base, $url, true, None, 64, 64)
    };
    ($base: expr, $url: expr, $size: expr) => {
        dw_media($base, $url, true, None, $size, $size)
    };
    ($base: expr, $url: expr, $w: expr, $h: expr) => {
        dw_media($base, $url, true, None, $w, $h)
    };
}

pub fn evc(events: &JsonValue, t: &str, field: &str) -> String {
    if let Some(arr) = events.as_array() {
        return match arr.iter().find(|x| x["type"] == t) {
            Some(js) => String::from(js["content"][field].as_str().unwrap_or("")),
            None => String::new(),
        };
    }

    String::new()
}

pub fn get_rooms_from_json(r: &JsonValue, userid: &str, baseu: &Url) -> Result<Vec<Room>, Error> {
    let rooms = &r["rooms"];

    let join = rooms["join"].as_object().ok_or(Error::BackendError)?;
    let leave = rooms["leave"].as_object().ok_or(Error::BackendError)?;
    let invite = rooms["invite"].as_object().ok_or(Error::BackendError)?;
    let global_account = &r["account_data"]["events"].as_array();

    // getting the list of direct rooms
    let mut direct: HashSet<String> = HashSet::new();
    match global_account.unwrap_or(&vec![]).iter().find(|x| x["type"] == "m.direct") {
        Some(js) => {
            if let Some(content) = js["content"].as_object() {
                for i in content.keys() {
                    for room in content[i].as_array().unwrap_or(&vec![]) {
                        if let Some(roomid) = room.as_str() {
                            direct.insert(roomid.to_string());
                        }
                    }
                }
            }
        },
        None => {}
    };

    let mut rooms: Vec<Room> = vec![];
    for k in join.keys() {
        let room = join.get(k).ok_or(Error::BackendError)?;
        let stevents = &room["state"]["events"];
        let timeline = &room["timeline"];
        let dataevs = &room["account_data"]["events"];
        let name = calculate_room_name(stevents, userid)?;
        let mut r = Room::new(k.clone(), name);

        r.avatar = Some(evc(stevents, "m.room.avatar", "url"));
        r.alias = Some(evc(stevents, "m.room.canonical_alias", "alias"));
        r.topic = Some(evc(stevents, "m.room.topic", "topic"));
        r.direct = direct.contains(k);
        r.notifications = room["unread_notifications"]["notification_count"]
            .as_i64()
            .unwrap_or(0) as i32;
        r.highlight = room["unread_notifications"]["highlight_count"]
            .as_i64()
            .unwrap_or(0) as i32;

        for ev in dataevs.as_array() {
            for tag in ev.iter().filter(|x| x["type"] == "m.tag") {
                if let Some(_) = tag["content"]["tags"]["m.favourite"].as_object() {
                    r.fav = true;
                }
            }
        }

        if let Some(evs) = timeline["events"].as_array() {
            let ms = Message::from_json_events_iter(k.clone(), evs.iter());
            r.messages.extend(ms);
        }

        let mevents = stevents.as_array().unwrap()
            .iter()
            .filter(|x| x["type"] == "m.room.member");

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
        let mut r = Room::new(k.clone(), None);
        r.left = true;
        rooms.push(r);
    }

    // invitations
    for k in invite.keys() {
        let room = invite.get(k).ok_or(Error::BackendError)?;
        let stevents = &room["invite_state"]["events"];
        let name = calculate_room_name(stevents, userid)?;
        let mut r = Room::new(k.clone(), name);
        r.inv = true;

        r.avatar = Some(evc(stevents, "m.room.avatar", "url"));
        r.alias = Some(evc(stevents, "m.room.canonical_alias", "alias"));
        r.topic = Some(evc(stevents, "m.room.topic", "topic"));
        r.direct = direct.contains(k);

        if let Some(arr) = stevents.as_array() {
            if let Some(ev) = arr.iter()
                                 .find(|x| x["membership"] == "invite" && x["state_key"] == userid) {
                if let Ok((alias, avatar)) = get_user_avatar(baseu, ev["sender"].as_str().unwrap_or_default()) {
                    r.inv_sender = Some(
                        Member {
                            alias: Some(alias),
                            avatar: Some(avatar),
                            uid: strn!(userid),
                        }
                    );
                }
            }
        }

        rooms.push(r);
    }

    Ok(rooms)
}

pub fn get_admins(stevents: &JsonValue) -> HashMap<String, i32> {
    let mut admins = HashMap::new();

    let plevents = stevents.as_array().unwrap()
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

pub fn get_rooms_timeline_from_json(baseu: &Url,
                                    r: &JsonValue,
                                    tk: String,
                                    prev_batch: String)
                                    -> Result<Vec<Message>, Error> {
    let rooms = &r["rooms"];
    let join = rooms["join"].as_object().ok_or(Error::BackendError)?;

    let mut msgs: Vec<Message> = vec![];
    for k in join.keys() {
        let room = join.get(k).ok_or(Error::BackendError)?;

        if let (Some(true), Some(pb)) = (room["timeline"]["limited"].as_bool(),
                                         room["timeline"]["prev_batch"].as_str()) {
            let pbs = pb.to_string();
            let fill_the_gap = fill_room_gap(baseu,
                                             tk.clone(),
                                             k.clone(),
                                             prev_batch.clone(),
                                             pbs.clone())?;
            for m in fill_the_gap {
                msgs.push(m);
            }
        }

        let timeline = room["timeline"]["events"].as_array();
        if timeline.is_none() {
            continue;
        }

        let events = timeline.unwrap().iter();
        let ms = Message::from_json_events_iter(k.clone(), events);
        msgs.extend(ms);
    }

    Ok(msgs)
}

pub fn get_rooms_notifies_from_json(r: &JsonValue) -> Result<Vec<(String, i32, i32)>, Error> {
    let rooms = &r["rooms"];
    let join = rooms["join"].as_object().ok_or(Error::BackendError)?;

    let mut out: Vec<(String, i32, i32)> = vec![];
    for k in join.keys() {
        let room = join.get(k).ok_or(Error::BackendError)?;
        let n = room["unread_notifications"]["notification_count"]
            .as_i64()
            .unwrap_or(0) as i32;
        let h = room["unread_notifications"]["highlight_count"]
            .as_i64()
            .unwrap_or(0) as i32;

        out.push((k.clone(), n, h));
    }

    Ok(out)
}

pub fn parse_sync_events(r: &JsonValue) -> Result<Vec<Event>, Error> {
    let rooms = &r["rooms"];
    let join = rooms["join"].as_object().ok_or(Error::BackendError)?;

    let mut evs: Vec<Event> = vec![];
    for k in join.keys() {
        let room = join.get(k).ok_or(Error::BackendError)?;
        let timeline = room["timeline"]["events"].as_array();
        if timeline.is_none() {
            return Ok(evs);
        }

        let events = timeline.unwrap()
            .iter()
            .filter(|x| x["type"] != "m.room.message");

        for ev in events {
            //println!("ev: {:#?}", ev);
            evs.push(Event {
                room: k.clone(),
                sender: strn!(ev["sender"].as_str().unwrap_or("")),
                content: ev["content"].clone(),
                stype: strn!(ev["type"].as_str().unwrap_or("")),
                id: strn!(ev["id"].as_str().unwrap_or("")),
            });
        }
    }

    Ok(evs)
}

pub fn get_media(url: &str) -> Result<Vec<u8>, Error> {
    let client = reqwest::Client::new();
    let mut conn = client.get(url);
    let mut res = conn.send()?;

    let mut buffer = Vec::new();
    res.read_to_end(&mut buffer)?;

    Ok(buffer)
}

pub fn put_media(url: &str, file: Vec<u8>) -> Result<JsonValue, Error> {
    let client = reqwest::Client::new();
    let mut conn = client.post(url);
    let mime: Mime = (&tree_magic::from_u8(&file)).parse().unwrap();

    conn.body(file);

    conn.header(ContentType(mime));

    let mut res = conn.send()?;

    match res.json() {
        Ok(js) => Ok(js),
        Err(_) => Err(Error::BackendError),
    }
}

pub fn dw_media(base: &Url,
                url: &str,
                thumb: bool,
                dest: Option<&str>,
                w: i32,
                h: i32)
                -> Result<String, Error> {
    let re = Regex::new(r"mxc://(?P<server>[^/]+)/(?P<media>.+)")?;
    let caps = re.captures(url).ok_or(Error::BackendError)?;
    let server = String::from(&caps["server"]);
    let media = String::from(&caps["media"]);

    let mut params: Vec<(&str, String)> = vec![];
    let path: String;

    if thumb {
        params.push(("width", format!("{}", w)));
        params.push(("height", format!("{}", h)));
        params.push(("method", strn!("scale")));
        path = format!("thumbnail/{}/{}", server, media);
    } else {
        path = format!("download/{}/{}", server, media);
    }

    let url = media_url!(base, &path, params)?;

    let fname = match dest {
        None if thumb => { cache_dir_path("thumbs", &media)?  }
        None => { cache_dir_path("medias", &media)?  }
        Some(d) => String::from(d),
    };

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
    let buffer = get_media(url.as_str())?;
    file.write_all(&buffer)?;

    Ok(fname)
}

pub fn json_q(method: &str, url: &Url, attrs: &JsonValue, timeout: u64) -> Result<JsonValue, Error> {
    let mut clientb = reqwest::ClientBuilder::new();
    let client = match timeout {
        0 => clientb.timeout(None).build()?,
        n => clientb.timeout(StdDuration::from_secs(n)).build()?
    };

    let mut conn = match method {
        "post" => client.post(url.as_str()),
        "put" => client.put(url.as_str()),
        "delete" => client.delete(url.as_str()),
        _ => client.get(url.as_str()),
    };

    if !attrs.is_null() {
        conn.json(attrs);
    }

    let mut res = conn.send()?;

    //let mut content = String::new();
    //res.read_to_string(&mut content);
    //cb(content);

    if !res.status().is_success() {
        return match res.json() {
            Ok(js) => Err(Error::MatrixError(js)),
            Err(err) => Err(Error::ReqwestError(err))
        }
    }

    let json: Result<JsonValue, reqwest::Error> = res.json();
    match json {
        Ok(js) => {
            let js2 = js.clone();
            if let Some(error) = js.as_object() {
                if error.contains_key("errcode") {
                    println!("ERROR: {:#?}", js2);
                    return Err(Error::MatrixError(js2));
                }
            }
            Ok(js)
        }
        Err(_) => Err(Error::BackendError),
    }
}

pub fn get_user_avatar(baseu: &Url, userid: &str) -> Result<(String, String), Error> {
    let url = client_url!(baseu, &format!("profile/{}", userid), vec![])?;
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
                    let img = dw_media(baseu, &url, true, Some(&dest), 64, 64)?;
                    Ok((name.clone(), img))
                },
                None => Ok((name.clone(), identicon!(userid, name)?)),
            }
        }
        Err(_) => Ok((String::from(userid), identicon!(userid, String::from(&userid[1..2]))?)),
    }
}

pub fn get_room_st(base: &Url, tk: &str, roomid: &str) -> Result<JsonValue, Error> {
    let url = client_url!(base, &format!("rooms/{}/state", roomid), vec![("access_token", strn!(tk))])?;

    let attrs = json!(null);
    let st = json_q("get", &url, &attrs, globals::TIMEOUT)?;
    Ok(st)
}

pub fn get_room_avatar(base: &Url, tk: &str, userid: &str, roomid: &str) -> Result<String, Error> {
    let st = get_room_st(base, tk, roomid)?;
    let events = st.as_array().ok_or(Error::BackendError)?;

    // we look for members that aren't me
    let filter = |x: &&JsonValue| {
        (x["type"] == "m.room.member" && x["content"]["membership"] == "join" &&
         x["sender"] != userid)
    };
    let members = events.iter().filter(&filter);
    let mut members2 = events.iter().filter(&filter);

    let m1 = match members2.nth(0) {
        Some(m) => m["content"]["avatar_url"].as_str().unwrap_or(""),
        None => "",
    };

    let mut fname = match members.count() {
        1 => thumb!(&base, m1).unwrap_or_default(),
        _ => String::new(),
    };

    if fname.is_empty() {
        let roomname = match calculate_room_name(&st, userid)?{
            Some(ref name) => { name.clone() },
            None => { "X".to_string() },
        };
        fname = identicon!(roomid, roomname)?;
    }

    Ok(fname)
}

struct Color {
    r: i32,
    g: i32,
    b: i32,
}

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

pub fn get_initials(name: String) -> Result<String, Error> {
        let name = name.trim_right_matches(" (IRC)");
        let mut words = name.unicode_words();
        let first = words
            .next()
            .and_then(|w| UnicodeSegmentation::graphemes(w, true).next())
            .unwrap_or_default();
        let second = words
            .next()
            .and_then(|w| UnicodeSegmentation::graphemes(w, true).next())
            .unwrap_or_default();
        let initials = format!( "{}{}", first, second);

        Ok(initials)
}

pub fn draw_identicon(fname: &str, name: String, mode: AvatarMode) -> Result<String, Error> {
    // Our color palette with a darker and a muted variant for each one
    let colors = [
        [Color { r: 206, g: 77, b: 205, }, Color { r: 251, g: 224, b: 251, }],
        [Color { r: 121, g: 81, b: 192, }, Color { r: 231, g: 218, b: 251, }],
        [Color { r: 78, g: 99, b: 201, }, Color { r: 207, g: 215, b: 248, }],
        [Color { r: 66, g: 160, b: 243, }, Color { r: 214, g: 234, b: 252, }],
        [Color { r: 70, g: 189, b: 158, }, Color { r: 212, g: 248, b: 239, }],
        [Color { r: 117, g: 184, b: 45, }, Color { r: 220, g: 247, b: 191, }],
        [Color { r: 235, g: 121, b: 10, }, Color { r: 254, g: 235, b: 218, }],
        [Color { r: 227, g: 61, b: 34,  }, Color { r: 251, g: 219, b: 211, }],
        [Color { r: 109, g: 109, b: 109, }, Color { r: 219, g: 219, b: 219  }],
    ];

    let fname = cache_path(fname)?;

    let image = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40)?;
    let g = cairo::Context::new(&image);

    let color_index = calculate_hash(&fname) as usize % colors.len() as usize;
    let bg_c = &colors[color_index][0];
    g.set_source_rgba(bg_c.r as f64 / 256., bg_c.g as f64 / 256., bg_c.b as f64 / 256., 1.);

    match mode {
        AvatarMode::Rect => g.rectangle(0., 0., 40., 40.),
        AvatarMode::Circle => {
            g.arc(20.0, 20.0, 20.0, 0.0, 2.0 * 3.14159);
            g.fill();
        }
    };

    let fg_c = &colors[color_index][1];
    g.set_source_rgba(fg_c.r as f64 / 256., fg_c.g as f64 / 256., fg_c.b as f64 / 256., 1.);

    if !name.is_empty() {
        let initials = get_initials(name)?.to_uppercase();

        let layout = pangocairo::functions::create_layout(&g).unwrap();
        let fontdesc = pango::FontDescription::from_string("Cantarell Ultra-Bold 18");
        layout.set_font_description(&fontdesc);
        layout.set_text(&initials);
        // Move to center of the background shape we drew,
        // offset by half the size of the glyph
        let bx = image.get_width();
        let by = image.get_height();
        let (ox, oy) = layout.get_pixel_size();
        g.translate((bx - ox) as f64/2., (by - oy) as f64/2.);
        // Finally draw the glyph
        pangocairo::functions::show_layout(&g, &layout);
    }

    let mut buffer = File::create(&fname)?;
    image.write_to_png(&mut buffer)?;

    Ok(fname)
}

pub fn calculate_room_name(roomst: &JsonValue, userid: &str) -> Result<Option<String>, Error> {

    // looking for "m.room.name" event
    let events = roomst.as_array().ok_or(Error::BackendError)?;
    if let Some(name) = events.iter().find(|x| x["type"] == "m.room.name") {
        if let Some(name) = name["content"]["name"].as_str() {
            if !name.to_string().is_empty() {
                return Ok(Some(name.to_string()));
            }
        }
    }

    // looking for "m.room.canonical_alias" event
    if let Some(name) = events.iter().find(|x| x["type"] == "m.room.canonical_alias") {
        if let Some(name) = name["content"]["alias"].as_str() {
            return Ok(Some(name.to_string()));
        }
    }

    // we look for members that aren't me
    let filter = |x: &&JsonValue| {
        (x["type"] == "m.room.member" &&
         (
          (x["content"]["membership"] == "join" && x["sender"] != userid) ||
          (x["content"]["membership"] == "invite" && x["state_key"] != userid)
         )
        )
    };
    let c = events.iter().filter(&filter);
    let members = events.iter().filter(&filter);
    let mut members2 = events.iter().filter(&filter);

    if c.count() == 0 {
        // we don't have information to calculate the name
        return Ok(None);
    }

    let m1 = match members2.nth(0) {
        Some(m) => {
            let sender = m["sender"].as_str().unwrap_or("NONAMED");
            m["content"]["displayname"].as_str().unwrap_or(sender)
        },
        None => "",
    };
    let m2 = match members2.nth(1) {
        Some(m) => {
            let sender = m["sender"].as_str().unwrap_or("NONAMED");
            m["content"]["displayname"].as_str().unwrap_or(sender)
        },
        None => "",
    };

    let name = match members.count() {
        0 => String::from("EMPTY ROOM"),
        1 => String::from(m1),
        2 => format!("{} and {}", m1, m2),
        _ => format!("{} and Others", m1),
    };

    Ok(Some(name))
}

/// Recursive function that tries to get at least @get Messages for the room.
///
/// The @limit is the first "limit" param in the GET request.
/// The @end param is used as "from" param in the GET request, so we'll get
/// messages before that.
pub fn get_initial_room_messages(baseu: &Url,
                                 tk: String,
                                 roomid: String,
                                 get: usize,
                                 limit: i32,
                                 end: Option<String>)
                                 -> Result<(Vec<Message>, String, String), Error> {

    let mut ms: Vec<Message> = vec![];
    let mut nstart;
    let mut nend;

    let mut params = vec![
        ("dir", strn!("b")),
        ("limit", format!("{}", limit)),
        ("access_token", tk.clone()),
    ];

    match end {
        Some(ref e) => { params.push(("from", e.clone())) }
        None => {}
    };

    let path = format!("rooms/{}/messages", roomid);
    let url = client_url!(baseu, &path, params)?;

    let r = json_q("get", &url, &json!(null), globals::TIMEOUT)?;
    nend = String::from(r["end"].as_str().unwrap_or(""));
    nstart = String::from(r["start"].as_str().unwrap_or(""));

    let array = r["chunk"].as_array();
    if array.is_none() || array.unwrap().len() == 0 {
        return Ok((ms, nstart, nend));
    }

    let evs = array.unwrap().iter().rev();
    let msgs = Message::from_json_events_iter(roomid.clone(), evs);
    ms.extend(msgs);

    if ms.len() < get {
        let (more, s, e) =
            get_initial_room_messages(baseu, tk, roomid, get, limit * 2, Some(nend))?;
        nstart = s;
        nend = e;
        for m in more.iter().rev() {
            ms.insert(0, m.clone());
        }
    }

    Ok((ms, nstart, nend))
}

/// Recursive function that tries to get all messages in a room from a batch id to a batch id,
/// following the response pagination
pub fn fill_room_gap(baseu: &Url,
                     tk: String,
                     roomid: String,
                     from: String,
                     to: String)
                     -> Result<Vec<Message>, Error> {

    let mut ms: Vec<Message> = vec![];
    let nend;

    let mut params = vec![
        ("dir", strn!("f")),
        ("limit", format!("{}", globals::PAGE_LIMIT)),
        ("access_token", tk.clone()),
    ];

    params.push(("from", from.clone()));
    params.push(("to", to.clone()));

    let path = format!("rooms/{}/messages", roomid);
    let url = client_url!(baseu, &path, params)?;

    let r = json_q("get", &url, &json!(null), globals::TIMEOUT)?;
    nend = String::from(r["end"].as_str().unwrap_or(""));

    let array = r["chunk"].as_array();
    if array.is_none() || array.unwrap().len() == 0 {
        return Ok(ms);
    }

    let evs = array.unwrap().iter();
    let mevents = Message::from_json_events_iter(roomid.clone(), evs);
    ms.extend(mevents);

    // loading more until no more messages
    let more = fill_room_gap(baseu, tk, roomid, nend, to)?;
    for m in more.iter() {
        ms.insert(0, m.clone());
    }

    Ok(ms)
}

pub fn build_url(base: &Url, path: &str, params: Vec<(&str, String)>) -> Result<Url, Error> {
    let mut url = base.join(path)?;

    {
        let mut query = url.query_pairs_mut();
        query.clear();
        for (k, v) in params {
            query.append_pair(k, &v);
        }
    }

    Ok(url)
}

pub fn circle_image(fname: String) -> Result<String, Error> {
    use std::f64::consts::PI;

    let pb = Pixbuf::new_from_file_at_scale(&fname, 40, -1, true)?;
    let image = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40)?;
    let g = cairo::Context::new(&image);
    g.set_antialias(cairo::Antialias::Best);
    let hpos: f64 = (40.0 - (pb.get_height()) as f64) / 2.0;
    g.set_source_pixbuf(&pb, 0.0, hpos);

    g.arc(20.0, 20.0, 20.0, 0.0, 2.0 * PI);
    g.clip();

    g.paint();

    let mut buffer = File::create(&fname)?;
    image.write_to_png(&mut buffer)?;

    Ok(fname)
}

pub fn cache_path(name: &str) -> Result<String, Error> {
    let mut path = match glib::get_user_cache_dir() {
        Some(path) => path,
        None => PathBuf::from("/tmp"),
    };

    path.push("fractal");

    if !path.exists() {
        create_dir_all(&path)?;
    }

    path.push(name);

    Ok(path.into_os_string().into_string()?)
}

pub fn cache_dir_path(dir: &str, name: &str) -> Result<String, Error> {
    let mut path = match glib::get_user_cache_dir() {
        Some(path) => path,
        None => PathBuf::from("/tmp"),
    };

    path.push("fractal");
    path.push(dir);

    if !path.exists() {
        create_dir_all(&path)?;
    }

    path.push(name);

    Ok(path.into_os_string().into_string()?)
}

pub fn get_user_avatar_img(baseu: &Url, userid: String, alias: String, avatar: String) -> Result<String, Error> {
    if avatar.is_empty() {
        return identicon!(&userid, alias);
    }

    let dest = cache_path(&userid)?;
    let img = dw_media(baseu, &avatar, true, Some(&dest), 64, 64)?;
    Ok(img)
}

pub fn parse_room_member(msg: &JsonValue) -> Option<Member> {
    let sender = msg["sender"].as_str().unwrap_or("");

    let c = &msg["content"];

    let membership = c["membership"].as_str();
    if membership.is_none() || membership.unwrap() != "join" {
        return None;
    }

    let displayname = match c["displayname"].as_str() {
        None => None,
        Some(s) => Some(strn!(s))
    };
    let avatar_url = match c["avatar_url"].as_str() {
        None => None,
        Some(s) => Some(strn!(s))
    };

    Some(Member {
        uid: strn!(sender),
        alias: displayname,
        avatar: avatar_url,
    })
}
