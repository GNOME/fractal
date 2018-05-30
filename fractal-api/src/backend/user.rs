extern crate serde_json;

use globals;
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use error::Error;
use util::json_q;
use util::get_user_avatar;
use util::get_user_avatar_img;
use backend::types::BKResponse;
use backend::types::Backend;

use types::Member;

use self::serde_json::Value as JsonValue;


pub fn get_username(bk: &Backend) -> Result<(), Error> {
    let id = bk.data.lock().unwrap().user_id.clone();
    let url = bk.url(&format!("profile/{}/displayname", id.clone()), vec![])?;
    let tx = bk.tx.clone();
    get!(&url,
        |r: JsonValue| {
            let name = String::from(r["displayname"].as_str().unwrap_or(&id));
            tx.send(BKResponse::Name(name)).unwrap();
        },
        |err| { tx.send(BKResponse::UserNameError(err)).unwrap() }
    );

    Ok(())
}

pub fn get_avatar(bk: &Backend) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;
    let userid = bk.data.lock().unwrap().user_id.clone();

    let tx = bk.tx.clone();
    thread::spawn(move || match get_user_avatar(&baseu, &userid) {
        Ok((_, fname)) => {
            tx.send(BKResponse::Avatar(fname)).unwrap();
        }
        Err(err) => {
            tx.send(BKResponse::AvatarError(err)).unwrap();
        }
    });

    Ok(())
}

pub fn get_user_info_async(bk: &mut Backend,
                           uid: &str,
                           tx: Sender<(String, String)>)
                           -> Result<(), Error> {
    let baseu = bk.get_base_url()?;

    let u = String::from(uid);

    if let Some(info) = bk.user_info_cache.get(&u) {
        let tx = tx.clone();
        let info = info.clone();
        thread::spawn(move || {
            let i = info.lock().unwrap().clone();
            tx.send(i).unwrap();
        });
        return Ok(())
    }

    let info = Arc::new(Mutex::new((String::new(), String::new())));
    let cache_key = u.clone();
    let cache_value = info.clone();

    semaphore!(bk.limit_threads, {
        let i0 = info.lock();
        match get_user_avatar(&baseu, &u) {
            Ok(info) => {
                tx.send(info.clone()).unwrap();
                let mut i = i0.unwrap();
                i.0 = info.0;
                i.1 = info.1;
            }
            Err(_) => {
                tx.send((String::new(), String::new())).unwrap();
            }
        };
    });

    bk.user_info_cache.insert(cache_key, cache_value);

    Ok(())
}

pub fn get_avatar_async(bk: &Backend, member: Option<Member>, tx: Sender<String>) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;

    if member.is_none() {
        tx.send(String::new()).unwrap();
        return Ok(());
    }

    let m = member.unwrap();

    let uid = m.uid.clone();
    let alias = m.get_alias();
    let avatar = m.avatar.clone();

    semaphore!(bk.limit_threads, {
        match get_user_avatar_img(&baseu, uid,
                                  alias,
                                  avatar.unwrap_or_default()) {
            Ok(fname) => { tx.send(fname.clone()).unwrap(); }
            Err(_) => { tx.send(String::new()).unwrap(); }
        }
    });

    Ok(())
}

pub fn search(bk: &Backend, term: String) -> Result<(), Error> {
    let url = bk.url(&format!("user_directory/search"), vec![])?;

    let attrs = json!({
        "search_term": term,
    });

    let tx = bk.tx.clone();
    post!(&url, &attrs,
        |js: JsonValue| {
            let mut users: Vec<Member> = vec![];
            if let Some(arr) = js["results"].as_array() {
                for member in arr.iter() {
                    let alias = match member["display_name"].as_str() {
                        None => None,
                        Some(a) => Some(a.to_string()),
                    };
                    let avatar = match member["avatar_url"].as_str() {
                        None => None,
                        Some(a) => Some(a.to_string()),
                    };

                    users.push(Member{
                        alias: alias,
                        uid: member["user_id"].as_str().unwrap_or_default().to_string(),
                        avatar: avatar,
                    });
                }
            }
            tx.send(BKResponse::UserSearch(users)).unwrap();
        },
        |err| {
            tx.send(BKResponse::CommandError(err)).unwrap(); }
    );

    Ok(())
}
