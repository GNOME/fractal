extern crate serde_json;

use std::collections::HashMap;
use std::fs::File;
use std::fs::remove_dir_all;
use std::io::prelude::*;

use types::RoomList;
use error::Error;

use fractal_api::util::cache_path;
use globals;

use types::Message;

#[derive(Serialize, Deserialize)]
pub struct CacheData {
    pub since: String,
    pub rooms: RoomList,
    pub last_viewed_messages: HashMap<String, Message>,
    pub username: String,
    pub uid: String,
}


pub fn store(
    rooms: &RoomList,
    last_viewed_messages: HashMap<String, Message>,
    since: String,
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
    }

    let data = CacheData {
        since: since,
        rooms: cacherooms,
        last_viewed_messages: last_viewed_messages,
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
