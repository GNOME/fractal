use backend::types::BKResponse;
use backend::types::Backend;
use error::Error;
use globals;
use std::{thread, time};
use types::Room;
use util::get_rooms_from_json;
use util::get_rooms_notifies_from_json;
use util::get_rooms_timeline_from_json;
use util::json_q;
use util::parse_m_direct;
use util::parse_sync_events;

pub fn sync(bk: &Backend, new_since: Option<String>, initial: bool) -> Result<(), Error> {
    let tk = bk.data.lock().unwrap().access_token.clone();
    if tk.is_empty() {
        return Err(Error::BackendError);
    }

    let since = bk.data.lock().unwrap().since.clone().or(new_since);
    let userid = bk.data.lock().unwrap().user_id.clone();

    let mut params: Vec<(&str, String)> = vec![];
    params.push(("full_state", String::from("false")));

    let timeout;

    if let Some(since) = since.clone() {
        params.push(("since", since));
    }

    if !initial {
        params.push(("timeout", String::from("30000")));
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
        params.push(("timeout", String::from("0")));
        timeout = 0;
    }

    let baseu = bk.get_base_url();
    let url = bk.url("sync", &params)?;

    let tx = bk.tx.clone();
    let data = bk.data.clone();

    let attrs = json!(null);

    thread::spawn(move || {
        match json_q("get", &url, &attrs, timeout) {
            Ok(r) => {
                let next_batch = String::from(r["next_batch"].as_str().unwrap_or(""));
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
                    match get_rooms_notifies_from_json(&r) {
                        Ok(notifies) => {
                            for (r, n, h) in notifies {
                                tx.send(BKResponse::RoomNotifications(r.clone(), n, h))
                                    .unwrap();
                            }
                        }
                        Err(_) => {}
                    };
                    // Other events
                    match parse_sync_events(&r) {
                        Err(err) => tx.send(BKResponse::SyncError(err)).unwrap(),
                        Ok(events) => {
                            for ev in events {
                                match ev.stype.as_ref() {
                                    "m.room.name" => {
                                        let name =
                                            String::from(ev.content["name"].as_str().unwrap_or(""));
                                        tx.send(BKResponse::RoomName(ev.room.clone(), name))
                                            .unwrap();
                                    }
                                    "m.room.topic" => {
                                        let t = String::from(
                                            ev.content["topic"].as_str().unwrap_or(""),
                                        );
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

                    let mut def: Option<Room> = None;
                    let jtr = data.lock().unwrap().join_to_room.clone();
                    if !jtr.is_empty() {
                        if let Some(r) = rooms.iter().find(|x| x.id == jtr) {
                            def = Some(r.clone());
                        }
                    }
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
                let ten_seconds = time::Duration::from_millis(10000);
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
