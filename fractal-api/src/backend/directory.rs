extern crate serde_json;
extern crate url;

use self::serde_json::Value as JsonValue;
use self::url::Url;

use globals;

use std::thread;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use error::Error;
use backend::types::BKResponse;
use backend::types::Backend;

use util::json_q;

use types::Room;
use types::Protocol;

pub fn protocols(bk: &Backend) -> Result<(), Error> {
    let baseu = bk.get_base_url()?;
    let tk = bk.data.lock().unwrap().access_token.clone();
    let mut url = baseu.join("/_matrix/client/unstable/thirdparty/protocols")?;
    url.query_pairs_mut().clear()
        .append_pair("access_token", &tk);

    let tx = bk.tx.clone();
    let s = bk.data.lock().unwrap().server_url.clone();
    get!(&url,
        move |r: JsonValue| {
            let mut protocols: Vec<Protocol> = vec![];

            protocols.push(Protocol {
                id: String::from(""),
                desc: String::from(s.split('/').last().unwrap_or("")),
            });

            let prs = r.as_object().unwrap();
            for k in prs.keys() {
                let ins = prs[k]["instances"].as_array().unwrap();
                for i in ins {
                    let p = Protocol{
                        id: String::from(i["instance_id"].as_str().unwrap()),
                        desc: String::from(i["desc"].as_str().unwrap()),
                    };
                    protocols.push(p);
                }
            }

            tx.send(BKResponse::DirectoryProtocols(protocols)).unwrap();
        },
        |err| { tx.send(BKResponse::DirectoryError(err)).unwrap(); }
    );

    Ok(())
}

pub fn room_search(bk: &Backend,
                   homeserver: Option<String>,
                   query: Option<String>,
                   third_parties: Option<Vec<String>>,
                   more: bool)
                   -> Result<(), Error> {

    let mut params: Vec<(&str, String)> = Vec::new();

    if let Some(mut hs) = homeserver {
        // Extract the hostname if `homeserver` is an URL
        if let Ok(homeserver_url) = Url::parse(&hs) {
            hs = homeserver_url.host_str().unwrap().to_string();
        }

        params.push(("server", hs));
    }

    let url = bk.url("publicRooms", params)?;

    let (rooms_sender, rooms_receiver) = channel::<Vec<Room>>();

    fetch_rooms(bk, url.clone(), query.clone(), None, more, rooms_sender.clone())?;

    let mut requests_count = 1;

    if let Some(tps) = third_parties {
        requests_count += tps.len();

        for tp in tps {
            fetch_rooms(bk, url.clone(), query.clone(), Some(tp.to_string()), more,
                        rooms_sender.clone())?;
        }
    }

    let mut rooms: Vec<Room> = vec![];
    for _ in 0..requests_count {
        rooms.append(&mut rooms_receiver.recv().unwrap());
    }

    rooms.sort_by(|a, b| a.n_members.cmp(&b.n_members));
    rooms.dedup_by(|a, b| a.id == b.id);
    rooms.reverse();

    bk.tx.send(BKResponse::DirectorySearch(rooms)).unwrap();
    Ok(())
}

fn fetch_rooms(bk: &Backend,
               url: Url,
               query: Option<String>,
               third_party: Option<String>,
               more: bool,
               room_sender: Sender<Vec<Room>>)
               -> Result<(), Error> {

    let mut attrs = json!({"limit": 20});

    if let Some(q) = query {
        attrs["filter"] = json!({
            "generic_search_term": q
        });
    }

    if let Some(tp) = third_party {
        if !tp.is_empty() {
            attrs["third_party_instance_id"] = json!(tp);
        }
    }

    if more {
        let since = bk.data.lock().unwrap().rooms_since.clone();
        attrs["since"] = json!(since);
    }

    let tx = bk.tx.clone();
    let data = bk.data.clone();
    post!(&url, &attrs,
        move |r: JsonValue| {
            let next_branch = r["next_batch"].as_str().unwrap_or("");
            data.lock().unwrap().rooms_since = String::from(next_branch);

            let mut rooms: Vec<Room> = vec![];
            for room in r["chunk"].as_array().unwrap() {
                let alias = String::from(room["canonical_alias"].as_str().unwrap_or(""));
                let id = String::from(room["room_id"].as_str().unwrap_or(""));
                let name = String::from(room["name"].as_str().unwrap_or(""));
                let mut r = Room::new(id, Some(name));
                r.alias = Some(alias);
                r.avatar = Some(String::from(room["avatar_url"].as_str().unwrap_or("")));
                r.topic = Some(String::from(room["topic"].as_str().unwrap_or("")));
                r.n_members = room["num_joined_members"].as_i64().unwrap_or(0) as i32;
                r.world_readable = room["world_readable"].as_bool().unwrap_or(false);
                r.guest_can_join = room["guest_can_join"].as_bool().unwrap_or(false);
                rooms.push(r);
            }

            room_sender.send(rooms).unwrap();
        },
        |err| { tx.send(BKResponse::DirectoryError(err)).unwrap(); }
    );

    Ok(())
}
