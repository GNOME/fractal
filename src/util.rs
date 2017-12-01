extern crate glib;
extern crate url;
extern crate reqwest;
extern crate regex;
extern crate serde_json;
extern crate chrono;
extern crate time;
extern crate cairo;
extern crate gdk;
extern crate gdk_pixbuf;
extern crate mime;
extern crate tree_magic;

use self::gdk_pixbuf::Pixbuf;
use self::gdk::ContextExt;

use self::regex::Regex;

use self::serde_json::Value as JsonValue;

use self::url::Url;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use std::fs::File;
use std::fs::create_dir_all;
use std::io::prelude::*;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use self::chrono::prelude::*;
use self::time::Duration;
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

pub fn get_rooms_from_json(r: JsonValue, userid: &str, baseu: &Url) -> Result<Vec<Room>, Error> {
    let rooms = &r["rooms"];
    // TODO: do something with invite and leave
    //let invite = rooms["invite"].as_object().ok_or(Error::BackendError)?;
    //let leave = rooms["leave"].as_object().ok_or(Error::BackendError)?;

    let join = rooms["join"].as_object().ok_or(Error::BackendError)?;

    let mut rooms: Vec<Room> = vec![];
    for k in join.keys() {
        let room = join.get(k).ok_or(Error::BackendError)?;
        let stevents = &room["state"]["events"];
        let timeline = &room["timeline"];
        let name = calculate_room_name(stevents, userid)?;
        let mut r = Room::new(k.clone(), name);

        r.avatar = evc(stevents, "m.room.avatar", "url");
        r.alias = evc(stevents, "m.room.canonical_alias", "alias");
        r.topic = evc(stevents, "m.room.topic", "topic");
        r.notifications = room["unread_notifications"]["notification_count"]
            .as_i64()
            .unwrap_or(0) as i32;

        for ev in timeline["events"].as_array().unwrap_or(&vec![]) {
            let msg = parse_room_message(baseu, k.clone(), ev);
            r.messages.push(msg);
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

        rooms.push(r);
    }

    Ok(rooms)
}

pub fn get_rooms_timeline_from_json(baseu: &Url, r: &JsonValue) -> Result<Vec<Message>, Error> {
    let rooms = &r["rooms"];
    let join = rooms["join"].as_object().ok_or(Error::BackendError)?;

    let mut msgs: Vec<Message> = vec![];
    for k in join.keys() {
        let room = join.get(k).ok_or(Error::BackendError)?;
        let timeline = room["timeline"]["events"].as_array();
        if timeline.is_none() {
            return Ok(msgs);
        }

        let events = timeline.unwrap()
            .iter()
            .filter(|x| x["type"] == "m.room.message");

        for ev in events {
            let msg = parse_room_message(baseu, k.clone(), ev);
            msgs.push(msg);
        }
    }

    Ok(msgs)
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
        None => { cache_path(&media)?  }
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

pub fn age_to_datetime(age: i64) -> DateTime<Local> {
    let now = Local::now();
    let diff = Duration::seconds(age / 1000);
    now - diff
}

pub fn json_q(method: &str, url: &Url, attrs: &JsonValue, timeout: u64) -> Result<JsonValue, Error> {
    let mut clientb = reqwest::ClientBuilder::new();
    let client = match timeout {
        0 => clientb.build()?,
        n => clientb.timeout(StdDuration::from_secs(n)).build()?
    };

    let mut conn = match method {
        "post" => client.post(url.as_str()),
        "put" => client.put(url.as_str()),
        "delete" => client.delete(url.as_str()),
        _ => client.get(url.as_str()),
    };

    let conn2 = conn.json(attrs);
    let mut res = conn2.send()?;

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
            let name = String::from(js["displayname"].as_str().unwrap_or("@"));
            match js["avatar_url"].as_str() {
                Some(url) => {
                    let dest = cache_path(userid)?;
                    let img = dw_media(baseu, &url, true, Some(&dest), 64, 64)?;
                    Ok((name.clone(), circle_image(img)?))
                },
                None => Ok((name.clone(), identicon!(userid, name)?)),
            }
        }
        Err(_) => Ok((String::from(userid), identicon!(userid, String::from(&userid[1..2]))?)),
    }
}

pub fn get_user_avatar_img(baseu: &Url, userid: String, alias: String, avatar: String) -> Result<String, Error> {
    if avatar.is_empty() {
        return identicon!(&userid, alias);
    }

    let dest = cache_path(&userid)?;
    let img = dw_media(baseu, &avatar, true, Some(&dest), 64, 64)?;
    Ok(circle_image(img)?)
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
        1 => thumb!(&base, m1).unwrap_or(String::new()),
        _ => String::new(),
    };

    if fname.is_empty() {
        let roomname = calculate_room_name(&st, userid)?;
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

pub fn draw_identicon(fname: &str, name: String, mode: AvatarMode) -> Result<String, Error> {
    let colors = vec![
        Color { r: 69,  g: 189, b: 243, },
        Color { r: 224, g: 143, b: 112, },
        Color { r: 77,  g: 182, b: 172, },
        Color { r: 149, g: 117, b: 205, },
        Color { r: 176, g: 133, b: 94,  },
        Color { r: 240, g: 98,  b: 146, },
        Color { r: 163, g: 211, b: 108, },
        Color { r: 121, g: 134, b: 203, },
        Color { r: 241, g: 185, b: 29,  },
    ];

    let fname = cache_path(fname)?;

    let image = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40)?;
    let g = cairo::Context::new(&image);

    let c = &colors[calculate_hash(&fname) as usize % colors.len() as usize];
    g.set_source_rgba(c.r as f64 / 256., c.g as f64 / 256., c.b as f64 / 256., 1.);

    match mode {
        AvatarMode::Rect => g.rectangle(0., 0., 40., 40.),
        AvatarMode::Circle => {
            g.arc(20.0, 20.0, 20.0, 0.0, 2.0 * 3.14159);
            g.fill();
        }
    };

    g.set_font_size(24.);
    g.set_source_rgb(1.0, 1.0, 1.0);

    let first = match &name.chars().nth(0) {
        &Some(f) if f == '#' && name.len() > 1 => String::from(&name.to_uppercase()[1..2]),
        &Some(f) if f == '@' && name.len() > 1 => String::from(&name.to_uppercase()[1..2]),
        &Some(_) => String::from(&name.to_uppercase()[0..1]),
        &None => String::from("X"),
    };

    let te = g.text_extents(&first);
    g.move_to(20. - te.width / 2., 20. + te.height / 2.);
    g.show_text(&first);

    let mut buffer = File::create(&fname)?;
    image.write_to_png(&mut buffer)?;

    Ok(fname)
}

pub fn calculate_room_name(roomst: &JsonValue, userid: &str) -> Result<String, Error> {

    // looking for "m.room.name" event
    let events = roomst.as_array().ok_or(Error::BackendError)?;
    if let Some(name) = events.iter().find(|x| x["type"] == "m.room.name") {
        return Ok(String::from(name["content"]["name"].as_str().unwrap_or("WRONG NAME")));
    }
    // looking for "m.room.canonical_alias" event
    if let Some(name) = events.iter().find(|x| x["type"] == "m.room.canonical_alias") {
        return Ok(String::from(name["content"]["alias"].as_str().unwrap_or("WRONG ALIAS")));
    }

    // we look for members that aren't me
    let filter = |x: &&JsonValue| {
        (x["type"] == "m.room.member" && x["content"]["membership"] == "join" &&
         x["sender"] != userid)
    };
    let members = events.iter().filter(&filter);
    let mut members2 = events.iter().filter(&filter);

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

    Ok(name)
}

pub fn parse_room_message(baseu: &Url, roomid: String, msg: &JsonValue) -> Message {
    let sender = msg["sender"].as_str().unwrap_or("");
    let mut age = msg["age"].as_i64().unwrap_or(0);
    if age == 0 {
        age = msg["unsigned"]["age"].as_i64().unwrap_or(0);
    }

    let id = msg["event_id"].as_str().unwrap_or("");

    let c = &msg["content"];
    let mtype = c["msgtype"].as_str().unwrap_or("");
    let body = c["body"].as_str().unwrap_or("");

    let mut url = String::new();
    let mut thumb = String::new();

    match mtype {
        "m.image" | "m.file" | "m.video" | "m.audio" => {
            url = String::from(c["url"].as_str().unwrap_or(""));
            let mut t = String::from(c["info"]["thumbnail_url"].as_str().unwrap_or(""));
            if t.is_empty() && !url.is_empty() {
                t = url.clone();
            }
            thumb = media!(baseu, &t).unwrap_or(String::from(""));
        }
        _ => {}
    };

    Message {
        sender: String::from(sender),
        mtype: String::from(mtype),
        body: String::from(body),
        date: age_to_datetime(age),
        room: roomid.clone(),
        url: url,
        thumb: thumb,
        id: String::from(id),
    }
}

pub fn parse_room_member(msg: &JsonValue) -> Option<Member> {
    let sender = msg["sender"].as_str().unwrap_or("");

    let c = &msg["content"];

    let displayname = c["displayname"].as_str().unwrap_or("");
    let avatar_url = c["avatar_url"].as_str().unwrap_or("");

    if c["membership"].as_str().is_none() {
        return None;
    }

    Some(Member {
        uid: strn!(sender),
        alias: strn!(displayname),
        avatar: strn!(avatar_url),
    })
}

pub fn markup(s: &str) -> String {
    let mut out = String::from(s);

    out = String::from(out.trim());
    out = out.replace('&', "&amp;");
    out = out.replace('<', "&lt;");
    out = out.replace('>', "&gt;");

    let re = Regex::new(r"(?P<url>https?://[^\s&,)(]+(&\w=[\w._-]?)*(#[\w._-]+)?)").unwrap();
    out = String::from(re.replace_all(&out, "<a href=\"$url\">$url</a>"));

    out
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

    for msg in array.unwrap().iter().rev() {
        if msg["type"].as_str().unwrap_or("") != "m.room.message" {
            continue;
        }

        let m = parse_room_message(&baseu, roomid.clone(), msg);
        ms.push(m);
    }

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

pub fn get_pixbuf_data(pb: &Pixbuf) -> Result<Vec<u8>, Error> {
    let image = cairo::ImageSurface::create(cairo::Format::ARgb32, pb.get_width(), pb.get_height())?;
    let g = cairo::Context::new(&image);
    g.set_source_pixbuf(pb, 0 as f64, 0 as f64);
    g.paint();

    let mut buf: Vec<u8> = Vec::new();
    image.write_to_png(&mut buf)?;
    Ok(buf)
}

pub fn circle_image(fname: String) -> Result<String, Error> {
    let pb = Pixbuf::new_from_file_at_scale(&fname, 40, -1, true)?;
    let image = cairo::ImageSurface::create(cairo::Format::ARgb32, 40, 40)?;
    let g = cairo::Context::new(&image);
    g.set_antialias(cairo::Antialias::Good);
    let hpos: f64 = (40 - pb.get_height()) as f64 / 2.0;
    g.set_source_pixbuf(&pb, 0 as f64, hpos);

    g.arc(20.0, 20.0, 20.0, 0.0, 2.0 * 3.14159);
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
