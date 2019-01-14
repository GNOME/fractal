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
use crate::types::PublicRooms;
use crate::types::Room;

pub fn protocols(bk: &Backend) {
    let baseu = bk.get_base_url();
    let tk = bk.data.lock().unwrap().access_token.clone();
    let mut url = baseu
        .join("/_matrix/client/r0/thirdparty/protocols")
        .expect("Wrong URL in protocols()");
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
            let rooms = serde_json::from_value::<PublicRooms>(r)
                .map(|pr| {
                    data.lock().unwrap().rooms_since = pr.next_batch.unwrap_or_default();

                    pr.chunk
                        .into_iter()
                        .map(Into::into)
                        .inspect(|r: &Room| {
                            if let Some(avatar) = r.avatar.clone() {
                                if let Ok(dest) = cache_path(&r.id) {
                                    media(&base.clone(), &avatar, Some(&dest)).unwrap_or_default();
                                }
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            tx.send(BKResponse::DirectorySearch(rooms)).unwrap();
        },
        |err| {
            tx.send(BKResponse::DirectoryError(err)).unwrap();
        }
    );

    Ok(())
}
