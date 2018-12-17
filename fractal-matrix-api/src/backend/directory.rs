use serde_json::Value as JsonValue;
use url::Url;

use globals;

use backend::types::BKResponse;
use backend::types::Backend;
use error::Error;
use std::thread;

use util::cache_path;
use util::json_q;
use util::media;

use types::Protocol;
use types::Room;

pub fn protocols(bk: &Backend) -> Result<(), Error> {
    let tk = bk.data.lock().unwrap().access_token.clone();
    let baseu = bk.get_base_url()?;
    let url = baseu.join("/_matrix/client/unstable/thirdparty/protocols")?;
    let params = &[("access_token", &tk)];
    let url = Url::parse_with_params(url.as_str(), params)?;

    let tx = bk.tx.clone();
    let s = bk.data.lock().unwrap().server_url.clone();
    get!(
        &url,
        move |r: JsonValue| {
            let protocols = std::iter::once(Protocol::new(s))
                .chain(
                    r.as_object()
                        .iter()
                        .flat_map(|m| m.values())
                        .flat_map(|v| {
                            v["instances"]
                                .as_array()
                                .cloned()
                                .unwrap_or_default()
                                .into_iter()
                        })
                        .map(|i| Protocol {
                            id: i["instance_id"].as_str().unwrap_or_default().to_string(),
                            desc: i["desc"].as_str().unwrap_or_default().to_string(),
                        }),
                )
                .collect();

            tx.send(BKResponse::DirectoryProtocols(protocols)).unwrap();
        },
        |err| {
            tx.send(BKResponse::DirectoryError(err)).unwrap();
        }
    );

    Ok(())
}

pub fn room_search(
    bk: &Backend,
    homeserver: Option<String>,
    query: Option<String>,
    third_party: Option<String>,
    more: bool,
) -> Result<(), Error> {
    let params = homeserver
        .and_then(|hs| Url::parse(&hs).ok()) // Extract the hostname if `homeserver` is an URL
        .map(|homeserver_url| {
            vec![(
                "server",
                homeserver_url.host_str().unwrap_or_default().to_string(),
            )]
        })
        .unwrap_or_default();

    let url = bk.url("publicRooms", params)?;
    let base = bk.get_base_url()?;

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
            data.lock().unwrap().rooms_since =
                r["next_batch"].as_str().unwrap_or_default().to_string();

            let rooms = r["chunk"]
                .as_array()
                .unwrap()
                .iter()
                .map(|room| {
                    let alias = room["canonical_alias"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    let avatar = room["avatar_url"].as_str().unwrap_or_default().to_string();
                    let topic = room["topic"].as_str().unwrap_or_default().to_string();
                    let id = room["room_id"].as_str().unwrap_or_default().to_string();
                    let name = room["name"].as_str().unwrap_or_default().to_string();
                    let mut r = Room::new(id.clone(), Some(name));
                    r.alias = Some(alias);
                    r.avatar = Some(avatar.clone());
                    r.topic = Some(topic);
                    r.n_members = room["num_joined_members"].as_i64().unwrap_or_default() as i32;
                    r.world_readable = room["world_readable"].as_bool().unwrap_or_default();
                    r.guest_can_join = room["guest_can_join"].as_bool().unwrap_or_default();
                    // download the avatar
                    if let Ok(dest) = cache_path(&id) {
                        media(&base, &avatar, Some(&dest)).unwrap_or_default();
                    }
                    r
                })
                .collect();

            tx.send(BKResponse::DirectorySearch(rooms)).unwrap();
        },
        |err| {
            tx.send(BKResponse::DirectoryError(err)).unwrap();
        }
    );

    Ok(())
}
