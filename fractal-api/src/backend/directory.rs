extern crate serde_json;
extern crate url;

use self::serde_json::Value as JsonValue;
use self::url::Url;

use globals;

use std::thread;
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
                   homeserver_hostname: Option<String>,
                   query: Option<String>,
                   third_party: Option<String>,
                   more: bool)
                   -> Result<(), Error> {
    // TODO: use the user's homeserver_hostname when `None` is passed
    let mut homeserver_hostname = homeserver_hostname.unwrap_or(String::from("matrix.org"));

    // Check that `homeserver_hostname` is really a hostname and not a URL
    if let Ok(homeserver_url) = Url::parse(&homeserver_hostname) {
        homeserver_hostname = homeserver_url.host_str().unwrap().to_string();
    }

    let mut url = bk.url("publicRooms", vec![])?;
    url.set_host(Some(&homeserver_hostname))?;
    println!("[DEBUG] The URL used: {}", url);

    let mut attrs = json!({"limit": 20});

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
                let mut r = Room::new(id, Some(name));
                r.alias = Some(alias);
                r.avatar = Some(String::from(room["avatar_url"].as_str().unwrap_or("")));
                r.topic = Some(String::from(room["topic"].as_str().unwrap_or("")));
                r.n_members = room["num_joined_members"].as_i64().unwrap_or(0) as i32;
                r.world_readable = room["world_readable"].as_bool().unwrap_or(false);
                r.guest_can_join = room["guest_can_join"].as_bool().unwrap_or(false);
                rooms.push(r);
            }

            tx.send(BKResponse::DirectorySearch(rooms)).unwrap();
        },
        |err| { tx.send(BKResponse::DirectoryError(err)).unwrap(); }
    );

    Ok(())
}
