use std::fs::File;
use std::fs::remove_dir_all;
use std::io::prelude::*;
use gtk;
use gtk::LabelExt;

use serde_json;
use types::RoomList;
use error::Error;

use fractal_api::util::cache_path;
use globals;

/* includes for avatar download */
use backend::BKCommand;
use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::channel;
use std::sync::mpsc::TryRecvError;

use std::cell::RefCell;
use std::rc::Rc;
use widgets::AvatarData;

use mdl;
use std::sync::{Arc, Mutex, MutexGuard};


#[derive(Serialize, Deserialize)]
pub struct CacheData {
    pub since: Option<String>,
    pub rooms: RoomList,
    pub username: String,
    pub uid: String,
}


pub fn store(
    rooms: &RoomList,
    since: Option<String>,
    username: String,
    uid: String
) -> Result<(), Error> {
    let fname = cache_path("rooms.json")?;

    let mut cacherooms = rooms.clone();
    for r in cacherooms.values_mut() {
        let skip = match r.messages.len() {
            n if n > globals::CACHE_SIZE => n - globals::CACHE_SIZE,
            _ => 0,
        };
        r.messages = r.messages.iter().skip(skip).cloned().collect();
        // setting prev_batch to none because we're removing some messages so the
        // prev_batch isn't valid now, it's not pointing to the stored last msg
        r.prev_batch = None;
    }

    let data = CacheData {
        since: since,
        rooms: cacherooms,
        username: username,
        uid: uid,
    };

    let serialized = serde_json::to_string(&data)?;
    File::create(fname)?.write_all(&serialized.into_bytes())?;

    Ok(())
}

pub fn load() -> Result<CacheData, Error> {
    let fname = cache_path("rooms.json")?;

    let mut file = File::open(fname)?;
    let mut serialized = String::new();
    file.read_to_string(&mut serialized)?;

   let deserialized: CacheData = serde_json::from_str(&serialized)?;

   Ok(deserialized)
}

pub fn destroy() -> Result<(), Error> {
    let fname = cache_path("")?;
    remove_dir_all(fname).or_else(|_| Err(Error::CacheError))
}

/// this downloads a avatar and stores it in the cache folder
pub fn download_to_cache(backend: Sender<BKCommand>,
                         name: String,
                         data: Rc<RefCell<AvatarData>>) {
    let (tx, rx) = channel::<(String, String)>();
    let _ = backend.send(BKCommand::GetUserInfoAsync(name.clone(), Some(tx)));

    gtk::timeout_add(50, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => gtk::Continue(false),
        Ok(_resp) => {
            data.borrow_mut().redraw_pixbuf();
            gtk::Continue(false)
        }
    });
}

/* Get username based on the MXID, we should cache the username */
pub fn download_to_cache_username(backend: Sender<BKCommand>,
                         uid: &str,
                         label: gtk::Label,
                         avatar: Option<Rc<RefCell<AvatarData>>>) {
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    backend.send(BKCommand::GetUserNameAsync(uid.to_string(), tx)).unwrap();
    gtk::timeout_add(50, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => gtk::Continue(false),
        Ok(username) => {
            label.set_text(&username);
            if let Some(ref rc_data) = avatar {
                let mut data = rc_data.borrow_mut();
                data.redraw_fallback(Some(username));
            }

            gtk::Continue(false)
        }
    });
}

/* Download username for a given MXID and update a emote message
 * FIXME: We should cache this request and do it before we need to display the username in an emote*/
pub fn download_to_cache_username_emote(backend: Sender<BKCommand>,
                         uid: &str,
                         text: &str,
                         label: gtk::Label,
                         avatar: Option<Rc<RefCell<AvatarData>>>) {
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    backend.send(BKCommand::GetUserNameAsync(uid.to_string(), tx)).unwrap();
    let text = text.to_string();
    gtk::timeout_add(50, move || match rx.try_recv() {
        Err(TryRecvError::Empty) => gtk::Continue(true),
        Err(TryRecvError::Disconnected) => gtk::Continue(false),
        Ok(username) => {
            label.set_markup(&format!("<b>{}</b> {}", &username, text));
            if let Some(ref rc_data) = avatar {
                let mut data = rc_data.borrow_mut();
                data.redraw_fallback(Some(username));
            }

            gtk::Continue(false)
        }
    });
}

#[derive(Clone)]
pub struct FCache {
    cache: Arc<Mutex<mdl::Cache>>,
}

impl FCache {
    pub fn c(&self) -> MutexGuard<mdl::Cache> {
        self.cache.lock().unwrap()
    }
}

// The cache object, it's the same for the whole process
lazy_static! {
    static ref CACHE: FCache = {
        let db: String = cache_path("cache.mdl")
            .expect("Fatal error: Can't start the cache");
        let mdl_cache = mdl::Cache::new(&db)
            .expect("Fatal error: Can't start the cache");
        let cache = Arc::new(Mutex::new(mdl_cache));
        FCache { cache }
    };
}

pub fn get() -> FCache {
    return CACHE.clone();
}
