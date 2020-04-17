use fractal_api::backend::user;
use fractal_api::url::Url;
use fractal_api::util::ResultExpectLog;
use glib::source::Continue;
use gtk;
use gtk::LabelExt;
use serde::{Deserialize, Serialize};
use std::thread;

use crate::types::Room;
use crate::types::RoomList;
use failure::Error;
use fractal_api::identifiers::UserId;
use std::collections::HashMap;

use crate::globals;

/* includes for avatar download */
use crate::backend::BKCommand;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::TryRecvError;

use crate::widgets::AvatarData;
use std::cell::RefCell;
use std::rc::Rc;

mod state;
pub use self::state::get;
pub use self::state::AppState;
pub use self::state::FCache;

// TODO: remove this struct
#[derive(Serialize, Deserialize)]
pub struct CacheData {
    pub since: Option<String>,
    pub rooms: RoomList,
    pub username: String,
    pub uid: UserId,
    pub device_id: String,
}

pub fn store(
    rooms: &RoomList,
    since: Option<String>,
    username: String,
    uid: UserId,
    device_id: String,
) -> Result<(), Error> {
    // don't store all messages in the cache
    let mut cacherooms: Vec<Room> = vec![];
    for r in rooms.values() {
        let mut r = r.clone();
        let skip = match r.messages.len() {
            n if n > globals::CACHE_SIZE => n - globals::CACHE_SIZE,
            _ => 0,
        };
        r.messages = r.messages.iter().skip(skip).cloned().collect();
        // setting prev_batch to none because we're removing some messages so the
        // prev_batch isn't valid now, it's not pointing to the stored last msg
        r.prev_batch = None;
        cacherooms.push(r);
    }

    let st = AppState {
        since,
        username,
        uid,
        device_id,
    };
    get().save_st(st)?;

    // This is slow because we iterate over all room msgs
    // in the future we shouldn't do that, we should remove the
    // Vec<Msg> from the room and treat messages as first level
    // cache objects with something like cache.get_msgs(room),
    // cache.get_msg(room_id, msg_id) and cache.save_msg(msg)
    get().save_rooms(cacherooms)?;

    Ok(())
}

pub fn load() -> Result<CacheData, Error> {
    let st = get().get_st()?;
    let rooms = get().get_rooms()?;
    let mut cacherooms: RoomList = HashMap::new();

    for r in rooms {
        cacherooms.insert(r.id.clone(), r);
    }

    let data = CacheData {
        since: st.since,
        username: st.username,
        uid: st.uid,
        device_id: st.device_id,
        rooms: cacherooms,
    };

    Ok(data)
}

/// this downloads a avatar and stores it in the cache folder
pub fn download_to_cache(
    backend: Sender<BKCommand>,
    server_url: Url,
    uid: UserId,
    data: Rc<RefCell<AvatarData>>,
) {
    let (tx, rx) = channel::<(String, String)>();
    let _ = backend.send(BKCommand::GetUserInfoAsync(server_url, uid, Some(tx)));

    gtk::timeout_add(50, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => Continue(true),
        Err(TryRecvError::Disconnected) => Continue(false),
        Ok(_resp) => {
            data.borrow_mut().redraw_pixbuf();
            Continue(false)
        }
    });
}

/* Get username based on the MXID, we should cache the username */
pub fn download_to_cache_username(
    server_url: Url,
    uid: UserId,
    label: gtk::Label,
    avatar: Option<Rc<RefCell<AvatarData>>>,
) {
    let (ctx, rx): (Sender<String>, Receiver<String>) = channel();
    thread::spawn(move || {
        let query = user::get_username_async(server_url, uid);
        ctx.send(query).expect_log("Connection closed");
    });
    gtk::timeout_add(50, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => Continue(true),
        Err(TryRecvError::Disconnected) => Continue(false),
        Ok(username) => {
            label.set_text(&username);
            if let Some(ref rc_data) = avatar {
                let mut data = rc_data.borrow_mut();
                data.redraw_fallback(Some(username));
            }

            Continue(false)
        }
    });
}

/* Download username for a given MXID and update a emote message
 * FIXME: We should cache this request and do it before we need to display the username in an emote*/
pub fn download_to_cache_username_emote(
    server_url: Url,
    uid: UserId,
    text: &str,
    label: gtk::Label,
    avatar: Option<Rc<RefCell<AvatarData>>>,
) {
    let (ctx, rx): (Sender<String>, Receiver<String>) = channel();
    thread::spawn(move || {
        let query = user::get_username_async(server_url, uid);
        ctx.send(query).expect_log("Connection closed");
    });
    let text = text.to_string();
    gtk::timeout_add(50, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => Continue(true),
        Err(TryRecvError::Disconnected) => Continue(false),
        Ok(username) => {
            label.set_markup(&format!("<b>{}</b> {}", &username, text));
            if let Some(ref rc_data) = avatar {
                let mut data = rc_data.borrow_mut();
                data.redraw_fallback(Some(username));
            }

            Continue(false)
        }
    });
}
