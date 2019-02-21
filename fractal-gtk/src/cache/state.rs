use lazy_static::lazy_static;
use mdl::Cache;
use mdl::Model;
use mdl::Store;
use serde::{Deserialize, Serialize};

use failure::err_msg;
use failure::Error;

use std::cell::RefCell;
use std::fs::remove_dir_all;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::types::Message;
use crate::types::Room;
use fractal_api::util::cache_path;

// Models

/// The application state, here we store the current state
/// information like the username, the last sync "since"
/// param and that kind of information
#[derive(Serialize, Deserialize)]
pub struct AppState {
    pub since: Option<String>,
    pub username: String,
    pub uid: String,
    pub device_id: String,
}

/// Backend Room model but without the list of messages
/// The list of messages is stored in cache with the Msg
/// struct and this struct defines a convenience method
/// to get a Vec<Msg>
#[derive(Serialize, Deserialize)]
pub struct AppRoom {
    pub room: RefCell<Room>,
}

/// Message stored in the cache
#[derive(Serialize, Deserialize)]
pub struct AppMsg {
    pub msg: Message,
}

impl Model for AppState {
    fn key(&self) -> String {
        "state".to_string()
    }
}

impl AppRoom {
    #[allow(dead_code)]
    fn store_msgs<S: Store>(&self, store: &S) -> Result<(), Error> {
        for msg in self.room.borrow().messages.iter() {
            let m = AppMsg { msg: msg.clone() };
            m.store(store)?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn load_msgs<S: Store>(&mut self, store: &S) -> Result<(), Error> {
        let key = format!("msg:{}", self.room.borrow().id);
        let msgs: Vec<Message> = AppMsg::all(store, &key)?
            .iter()
            .map(|m| m.msg.clone())
            .collect();
        self.room.borrow_mut().messages = msgs;

        Ok(())
    }

    #[allow(dead_code)]
    fn clear_msgs(&self) {
        self.room.borrow_mut().messages = vec![];
    }
}

impl Model for AppRoom {
    fn key(&self) -> String {
        format!("room:{}", self.room.borrow().id)
    }
}

impl Model for AppMsg {
    fn key(&self) -> String {
        format!("msg:{}:{}", self.msg.room, self.msg.id)
    }
}

// Cache

#[derive(Clone)]
pub struct FCache {
    cache: Arc<Mutex<Option<Cache>>>,
}

impl FCache {
    fn get_store<'a>(&'a self) -> MutexGuard<'a, Option<Cache>> {
        let mut guard = self.cache.lock().unwrap();
        if guard.is_none() {
            let db: String = cache_path("cache.mdl").expect("Fatal error: Can't start the cache");
            let mdl_cache = Cache::new(&db).expect("Fatal error: Can't start the cache");
            *guard = Some(mdl_cache);
        }
        guard
    }

    pub fn destroy(&self) -> Result<(), Error> {
        let mut guard = self.cache.lock().unwrap();
        *guard = None;

        let fname = cache_path("cache.mdl").or(Err(err_msg("Can't remove cache file")))?;
        remove_dir_all(fname).or_else(|_| Err(err_msg("Can't remove cache file")))
    }

    #[allow(dead_code)]
    pub fn get_room(&self, id: &str) -> Result<Room, Error> {
        let cache = self.get_store();
        let r = AppRoom::get(cache.as_ref().unwrap(), id)?;
        Ok(r.room.into_inner())
    }

    pub fn get_rooms(&self) -> Result<Vec<Room>, Error> {
        let cache = self.get_store();
        let rooms = AppRoom::all(cache.as_ref().unwrap(), "room")?
            .iter()
            .map(|r| r.room.borrow().clone())
            .collect();
        Ok(rooms)
    }

    pub fn save_rooms(&self, rooms: Vec<Room>) -> Result<(), Error> {
        for r in rooms {
            self.save_room(r)?;
        }
        Ok(())
    }

    pub fn save_room(&self, room: Room) -> Result<(), Error> {
        let cache = self.get_store();
        let approom = AppRoom {
            room: RefCell::new(room),
        };
        approom.store(cache.as_ref().unwrap())?;

        Ok(())
    }

    pub fn get_st(&self) -> Result<AppState, Error> {
        let cache = self.get_store();
        AppState::get(cache.as_ref().unwrap(), "state")
    }

    pub fn save_st(&self, st: AppState) -> Result<(), Error> {
        let cache = self.get_store();
        st.store(cache.as_ref().unwrap())?;

        Ok(())
    }
}

// The cache object, it's the same for the whole process
lazy_static! {
    static ref CACHE: FCache = FCache {
        cache: Arc::new(Mutex::new(None))
    };
}

pub fn get() -> FCache {
    CACHE.clone()
}
