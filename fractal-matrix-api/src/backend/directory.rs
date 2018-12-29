use serde_json::json;
use serde_json::Value as JsonValue;
use url::Url;

use crate::globals;

use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::error::Error;
use std::str::Split;
use std::thread;

use crate::util::cache_path;
use crate::util::json_q;
use crate::util::media;

use crate::types::Protocol;
use crate::types::Room;

pub fn protocols(bk: &Backend) {
    let baseu = bk.get_base_url();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let mut url = baseu
        .join("/_matrix/client/unstable/thirdparty/protocols")
        .unwrap();
    url.query_pairs_mut()
        .clear()
        .append_pair("access_token", &tk);

    let tx = bk.tx.clone();
    get!(
        &url,
        move |r: JsonValue| {
            let mut protocols: Vec<Protocol> = vec![];

            protocols.push(Protocol {
                id: String::new(),
                desc: baseu
                    .path_segments()
                    .and_then(Split::last)
                    .map(Into::into)
                    .unwrap_or_default(),
            });

            if let Some(prs) = r.as_object() {
                for k in prs.keys() {
                    let ins = prs[k]["instances"].as_array();
                    for i in ins.unwrap_or(&vec![]) {
                        let p = Protocol {
                            id: String::from(i["instance_id"].as_str().unwrap_or_default()),
                            desc: String::from(i["desc"].as_str().unwrap_or_default()),
                        };
                        protocols.push(p);
                    }
                }
            }

            tx.send(BKResponse::DirectoryProtocols(protocols)).unwrap();
        },
        |err| {
            tx.send(BKResponse::DirectoryError(err)).unwrap();
        }
    );
}

pub fn room_search(
    bk: &Backend,
    homeserver: Option<String>,
    query: Option<String>,
    third_party: Option<String>,
    more: bool,
) -> Result<(), Error> {
    let mut params: Vec<(&str, String)> = Vec::new();

    if let Some(mut hs) = homeserver {
        // Extract the hostname if `homeserver` is an URL
        if let Ok(homeserver_url) = Url::parse(&hs) {
            hs = homeserver_url.host_str().unwrap_or_default().to_string();
        }

        params.push(("server", hs));
    }

    let url = bk.url("publicRooms", params)?;
    let base = bk.get_base_url();

    let mut attrs = json!({ "limit": globals::ROOM_DIRECTORY_LIMIT });

    if let Some(q) = query {
        attrs["filter"] = json!({ "generic_search_term": q });
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
    post!(
        &url,
        &attrs,
        move |r: JsonValue| {
            let next_branch = r["next_batch"].as_str().unwrap_or_default();
            data.lock().unwrap().rooms_since = String::from(next_branch);

            let mut rooms: Vec<Room> = vec![];
            for room in r["chunk"].as_array().unwrap() {
                let alias = String::from(room["canonical_alias"].as_str().unwrap_or_default());
                let id = String::from(room["room_id"].as_str().unwrap_or_default());
                let name = String::from(room["name"].as_str().unwrap_or_default());
                let mut r = Room::new(id.clone(), Some(name));
                r.alias = Some(alias);
                r.avatar = Some(String::from(
                    room["avatar_url"].as_str().unwrap_or_default(),
                ));
                r.topic = Some(String::from(room["topic"].as_str().unwrap_or_default()));
                r.n_members = room["num_joined_members"].as_i64().unwrap_or_default() as i32;
                r.world_readable = room["world_readable"].as_bool().unwrap_or_default();
                r.guest_can_join = room["guest_can_join"].as_bool().unwrap_or_default();
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
        |err| {
            tx.send(BKResponse::DirectoryError(err)).unwrap();
        }
    );

    Ok(())
}
