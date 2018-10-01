use mdl::Model;
use mdl::Store;
use mdl::Cache;

use failure::Error;

use std::sync::{Arc, Mutex, MutexGuard};
use std::cell::RefCell;

use types::Room;
use types::Message;
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
    fn key(&self) -> String { "state".to_string() }
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
        let msgs: Vec<Message> = AppMsg::all(store, &key)?.iter()
            .map(|m| m.msg.clone()).collect();
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
        // messages should have always an ID to be stored in the cache
        // in other case we'll store with the "msg:room:" key.
        let id = self.msg.id.clone().unwrap_or_default();
        format!("msg:{}:{}", self.msg.room, id)
    }
}

// Cache

#[derive(Clone)]
pub struct FCache {
    cache: Arc<Mutex<Cache>>,
}

impl FCache {
    pub fn get_store(&self) -> MutexGuard<Cache> {
        self.cache.lock().unwrap()
    }

    #[allow(dead_code)]
    pub fn get_room(&self, id: &str) -> Result<Room, Error> {
        let cache = &*self.get_store();
        let r = AppRoom::get(cache, id)?;
        Ok(r.room.into_inner())
    }

    pub fn get_rooms(&self) -> Result<Vec<Room>, Error> {
        let cache = &*self.get_store();
        let rooms = AppRoom::all(cache, "room")?
            .iter().map(|r| r.room.borrow().clone()).collect();
        Ok(rooms)
    }

    pub fn save_rooms(&self, rooms: Vec<Room>) -> Result<(), Error> {
        for r in rooms {
            self.save_room(r)?;
        }
        Ok(())
    }

    pub fn save_room(&self, room: Room) -> Result<(), Error> {
        let cache = &*self.get_store();
        let approom = AppRoom { room: RefCell::new(room) };
        approom.store(cache)?;

        Ok(())
    }

    pub fn get_st(&self) -> Result<AppState, Error> {
        let cache = &*self.get_store();
        AppState::get(cache, "state")
    }

    pub fn save_st(&self, st: AppState) -> Result<(), Error> {
        let cache = &*self.get_store();
        st.store(cache)?;

        Ok(())
    }
}

// The cache object, it's the same for the whole process
lazy_static! {
    static ref CACHE: FCache = {
        let db: String = cache_path("cache.mdl")
            .expect("Fatal error: Can't start the cache");
        let mdl_cache = Cache::new(&db)
            .expect("Fatal error: Can't start the cache");
        let cache = Arc::new(Mutex::new(mdl_cache));
        FCache { cache }
    };
}

pub fn get() -> FCache {
    return CACHE.clone();
}

