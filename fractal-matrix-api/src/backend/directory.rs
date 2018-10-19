use serde_json;
use serde_json::Value as JsonValue;
use url;
use url::Url;

use globals;

use std::thread;
use error::Error;
use backend::types::BKResponse;
use backend::types::Backend;

use util::json_q;
use util::cache_path;
use util::media;

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

            if let Some(prs) = r.as_object() {
                for k in prs.keys() {
                    let ins = prs[k]["instances"].as_array();
                    for i in ins.unwrap_or(&vec![]) {
                        let p = Protocol{
                            id: String::from(i["instance_id"].as_str().unwrap_or_default()),
                            desc: String::from(i["desc"].as_str().unwrap_or_default()),
                        };
                        protocols.push(p);
                    }
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
                   third_party: Option<String>,
                   more: bool)
                   -> Result<(), Error> {

    let mut params: Vec<(&str, String)> = Vec::new();

    if let Some(mut hs) = homeserver {
        // Extract the hostname if `homeserver` is an URL
        if let Ok(homeserver_url) = Url::parse(&hs) {
            hs = homeserver_url.host_str().unwrap_or_default().to_string();
        }

        params.push(("server", hs));
    }

    let url = bk.url("publicRooms", params)?;
    let base = bk.get_base_url()?;

    let mut attrs = json!({"limit": globals::ROOM_DIRECTORY_LIMIT});

    if let Some(q) = query {
        attrs["filter"] = json!({
            "generic_search_term": q
        });
    }

    if let Some(tp) = third_party {
        attrs["third_party_instance_id"] = json!(tp);
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
                let mut r = Room::new(id.clone(), Some(name));
                r.alias = Some(alias);
                r.avatar = Some(String::from(room["avatar_url"].as_str().unwrap_or("")));
                r.topic = Some(String::from(room["topic"].as_str().unwrap_or("")));
                r.n_members = room["num_joined_members"].as_i64().unwrap_or(0) as i32;
                r.world_readable = room["world_readable"].as_bool().unwrap_or(false);
                r.guest_can_join = room["guest_can_join"].as_bool().unwrap_or(false);
                /* download the avatar */
                if let Some(avatar) = r.avatar.clone() {
                    if let Ok(dest) = cache_path(&id) {
                        media(&base.clone(), &avatar, Some(&dest)).unwrap_or_default();
                    }
                }
                rooms.push(r);
            }

            tx.send(BKResponse::DirectorySearch(rooms)).unwrap();
        },
        |err| { tx.send(BKResponse::DirectoryError(err)).unwrap(); }
    );

    Ok(())
}
