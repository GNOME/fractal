use JsonValue;

use globals;

pub use backend::types::{BKResponse, Backend};
use std::thread;
use url::Url;

use util::{cache_path, json_q, media};

use types::{Protocol, Room};

impl Backend {
    pub fn directory_protocols(&self) {
        let ctx = self.tx.clone();
        let tk = self.data.lock().unwrap().access_token.clone();
        let baseu = self.get_base_url();
        let url = baseu
            .join("/_matrix/client/unstable/thirdparty/protocols")
            .unwrap();
        let params = &[("access_token", &tk)];
        let url = Url::parse_with_params(url.as_str(), params).unwrap();

        let s = self.data.lock().unwrap().server_url.clone();
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

                ctx.send(BKResponse::DirectoryProtocols(protocols)).unwrap();
            },
            |err| {
                ctx.send(BKResponse::DirectoryError(err)).unwrap();
            }
        );
    }

    pub fn directory_search(
        &self,
        homeserver: String,
        query: String,
        third_party: String,
        more: bool,
    ) {
        let ctx = self.tx.clone();
        let homeserver = Some(homeserver).filter(|hs| !hs.is_empty());
        let query = Some(query).filter(|q| !q.is_empty());
        let third_party = Some(third_party).filter(|tp| !tp.is_empty());

        let params = homeserver
            .and_then(|hs| Url::parse(&hs).ok()) // Extract the hostname if `homeserver` is an URL
            .map(|homeserver_url| {
                vec![(
                    "server",
                    homeserver_url.host_str().unwrap_or_default().to_string(),
                )]
            })
            .unwrap_or_default();

        let url = self.url("publicRooms", params);
        let base = self.get_base_url();

        let mut attrs = json!({ "limit": globals::ROOM_DIRECTORY_LIMIT });

        if let Some(q) = query {
            attrs["filter"] = json!({ "generic_search_term": q });
        }

        if let Some(tp) = third_party {
            attrs["third_party_instance_id"] = json!(tp);
        }

        if more {
            let since = self.data.lock().unwrap().rooms_since.clone();
            attrs["since"] = json!(since);
        }

        let data = self.data.clone();
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
                        r.n_members =
                            room["num_joined_members"].as_i64().unwrap_or_default() as i32;
                        r.world_readable = room["world_readable"].as_bool().unwrap_or_default();
                        r.guest_can_join = room["guest_can_join"].as_bool().unwrap_or_default();
                        // download the avatar
                        if let Ok(dest) = cache_path(&id) {
                            media(&base, &avatar, Some(&dest)).unwrap_or_default();
                        }
                        r
                    })
                    .collect();

                ctx.send(BKResponse::DirectorySearch(rooms)).unwrap();
            },
            |err| {
                ctx.send(BKResponse::DirectoryError(err)).unwrap();
            }
        );
    }
}
