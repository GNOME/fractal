use backend::types::{BKResponse, Backend};
use error::Error;
use globals;
use std::{thread, time};
use util::{
    get_rooms_from_json, get_rooms_notifies_from_json, get_rooms_timeline_from_json, json_q,
    parse_m_direct, parse_sync_events,
};

pub fn sync(bk: &Backend, new_since: Option<String>, initial: bool) -> Result<(), Error> {
    let tk = bk.data.lock().unwrap().access_token.clone();
    if tk.is_empty() {
        return Err(Error::BackendError);
    }

    let since = bk.data.lock().unwrap().since.clone().or(new_since);
    let userid = bk.data.lock().unwrap().user_id.clone();

    let mut params: Vec<(&str, String)> = vec![];
    params.push(("full_state", "false".to_string()));

    let timeout;

    if let Some(since) = since.clone() {
        params.push(("since", since));
    }

    if !initial {
        params.push(("timeout", "30000".to_string()));
        timeout = 30;
    } else {
        let filter = format!(r#"{{
            "room": {{
                "state": {{
                    "types": ["m.room.*"],
                    "not_types": ["m.room.member"]
                }},
                "timeline": {{
                    "types": ["m.room.message", "m.sticker"],
                    "limit": {}
                }},
                "ephemeral": {{ "types": [] }}
            }},
            "presence": {{ "types": [] }},
            "event_format": "client",
            "event_fields": ["type", "content", "sender", "origin_server_ts", "event_id", "unsigned"]
        }}"#, globals::PAGE_LIMIT);

        params.push(("filter", filter));
        params.push(("timeout", "0".to_string()));
        timeout = 0;
    }

    let baseu = bk.get_base_url()?;
    let url = bk.url("sync", params)?;

    let tx = bk.tx.clone();
    let data = bk.data.clone();

    let attrs = json!(null);

    thread::spawn(move || {
        match json_q("get", &url, &attrs, timeout) {
            Ok(r) => {
                let next_batch = r["next_batch"].as_str().unwrap_or_default().to_string();
                if let Some(since) = since {
                    // New rooms
                    match get_rooms_from_json(&r, &userid, &baseu) {
                        Ok(rs) => tx.send(BKResponse::NewRooms(rs)).unwrap(),
                        Err(err) => tx.send(BKResponse::SyncError(err)).unwrap(),
                    };

                    // Message events
                    match get_rooms_timeline_from_json(&baseu, &r, &tk, &since) {
                        Ok(msgs) => tx.send(BKResponse::RoomMessages(msgs)).unwrap(),
                        Err(err) => tx.send(BKResponse::RoomMessagesError(err)).unwrap(),
                    };
                    // Room notifications
                    get_rooms_notifies_from_json(&r)
                        .map(|notifies| {
                            notifies.iter().for_each(|&(ref r, n, h)| {
                                tx.send(BKResponse::RoomNotifications(r.clone(), n, h))
                                    .unwrap()
                            })
                        })
                        .unwrap_or_default();
                    // Other events
                    match parse_sync_events(&r) {
                        Err(err) => tx.send(BKResponse::SyncError(err)).unwrap(),
                        Ok(events) => {
                            for ev in events {
                                match ev.stype.as_ref() {
                                    "m.room.name" => {
                                        let name = ev.content["name"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_string();
                                        tx.send(BKResponse::RoomName(ev.room.clone(), name))
                                            .unwrap();
                                    }
                                    "m.room.topic" => {
                                        let t = ev.content["topic"]
                                            .as_str()
                                            .unwrap_or_default()
                                            .to_string();
                                        tx.send(BKResponse::RoomTopic(ev.room.clone(), t)).unwrap();
                                    }
                                    "m.room.avatar" => {
                                        tx.send(BKResponse::NewRoomAvatar(ev.room.clone()))
                                            .unwrap();
                                    }
                                    "m.room.member" => {
                                        tx.send(BKResponse::RoomMemberEvent(ev)).unwrap();
                                    }
                                    "m.sticker" => {
                                        // This event is managed in the room list
                                    }
                                    _ => {
                                        error!("EVENT NOT MANAGED: {:?}", ev);
                                    }
                                }
                            }
                        }
                    };
                } else {
                    data.lock().unwrap().m_direct = parse_m_direct(&r);

                    let rooms = match get_rooms_from_json(&r, &userid, &baseu) {
                        Ok(rs) => rs,
                        Err(err) => {
                            tx.send(BKResponse::SyncError(err)).unwrap();
                            vec![]
                        }
                    };

                    let jtr = data.lock().unwrap().join_to_room.clone();
                    let def = if !jtr.is_empty() {
                        rooms.iter().find(|x| x.id == jtr).cloned()
                    } else {
                        None
                    };
                    tx.send(BKResponse::Rooms(rooms, def)).unwrap();
                }

                tx.send(BKResponse::Sync(next_batch.clone())).unwrap();
                data.lock().unwrap().since = if !next_batch.is_empty() {
                    Some(next_batch)
                } else {
                    None
                }
            }
            Err(err) => {
                // we wait if there's an error to avoid 100% CPU
                error!("Sync Error, waiting 10 seconds to respond for the next sync");
                let ten_seconds = time::Duration::from_secs(10);
                thread::sleep(ten_seconds);

                tx.send(BKResponse::SyncError(err)).unwrap();
            }
        };
    });

    Ok(())
}

pub fn force_sync(bk: &Backend) -> Result<(), Error> {
    bk.data.lock().unwrap().since = None;
    sync(bk, None, true)
}
