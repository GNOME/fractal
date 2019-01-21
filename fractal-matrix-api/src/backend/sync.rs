use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::error::Error;
use crate::globals;
use crate::types::SyncResponse;
use crate::types::UnreadNotificationsCount;
use crate::util::get_rooms_from_sync_response;
use crate::util::get_rooms_timeline_from_joined_rooms;
use crate::util::json_q;
use crate::util::parse_m_direct;
use crate::util::parse_sync_events;
use log::error;
use serde_json::json;
use serde_json::Value as JsonValue;
use std::{thread, time};

pub fn sync(bk: &Backend, new_since: Option<String>, initial: bool) -> Result<(), Error> {
    let tk = bk.data.lock().unwrap().access_token.clone();
    if tk.is_empty() {
        return Err(Error::BackendError);
    }

    let since = bk.data.lock().unwrap().since.clone().or(new_since);
    let userid = bk.data.lock().unwrap().user_id.clone();

    let mut params = vec![("full_state", String::from("false"))];

    if let Some(since) = since.clone() {
        params.push(("since", since));
    }

    let timeout = if !initial {
        time::Duration::from_secs(30)
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

        Default::default()
    };

    params.push(("timeout", timeout.as_secs().to_string()));

    let baseu = bk.get_base_url();
    let url = bk.url("sync", params)?;

    let tx = bk.tx.clone();
    let data = bk.data.clone();

    let attrs = json!(null);

    get!(
        &url,
        &attrs,
        |r: JsonValue| {
            if let Ok(response) = serde_json::from_value::<SyncResponse>(r) {
                if since.is_some() {
                    // New rooms
                    let rs = get_rooms_from_sync_response(&response, &userid, &baseu);
                    tx.send(BKResponse::NewRooms(rs)).unwrap();

                    // Message events
                    let msgs = get_rooms_timeline_from_joined_rooms(&response.rooms.join);
                    tx.send(BKResponse::RoomMessages(msgs)).unwrap();

                    // Room notifications
                    for (k, room) in response.rooms.join.iter() {
                        let UnreadNotificationsCount {
                            highlight_count: h,
                            notification_count: n,
                        } = room.unread_notifications;
                        tx.send(BKResponse::RoomNotifications(k.clone(), n, h))
                            .unwrap();
                    }
                    // Other events
                    for ev in parse_sync_events(&response.rooms.join) {
                        match ev.stype.as_ref() {
                            "m.room.name" => {
                                let name = ev.content["name"]
                                    .as_str()
                                    .map(Into::into)
                                    .unwrap_or_default();
                                tx.send(BKResponse::RoomName(ev.room.clone(), name))
                                    .unwrap();
                            }
                            "m.room.topic" => {
                                let t = ev.content["topic"]
                                    .as_str()
                                    .map(Into::into)
                                    .unwrap_or_default();
                                tx.send(BKResponse::RoomTopic(ev.room.clone(), t)).unwrap();
                            }
                            "m.room.avatar" => {
                                tx.send(BKResponse::NewRoomAvatar(ev.room.clone())).unwrap();
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
                } else {
                    data.lock().unwrap().m_direct = parse_m_direct(&response.account_data.events);

                    let rooms = get_rooms_from_sync_response(&response, &userid, &baseu);
                    let jtr = data.lock().unwrap().join_to_room.clone();
                    let def = if !jtr.is_empty() {
                        rooms.iter().find(|x| x.id == jtr).cloned()
                    } else {
                        None
                    };
                    tx.send(BKResponse::Rooms(rooms, def)).unwrap();
                }

                let next_batch = response.next_batch;
                tx.send(BKResponse::Sync(next_batch.clone())).unwrap();
                data.lock().unwrap().since = Some(next_batch).filter(|s| !s.is_empty());
            } else {
                tx.send(BKResponse::SyncError(Error::BackendError)).unwrap();
            }
        },
        |err| {
            // we wait if there's an error to avoid 100% CPU
            error!("Sync Error, waiting 10 seconds to respond for the next sync");
            thread::sleep(time::Duration::from_secs(10));

            tx.send(BKResponse::SyncError(err)).unwrap();
        },
        timeout.as_secs()
    );

    Ok(())
}

pub fn force_sync(bk: &Backend) -> Result<(), Error> {
    bk.data.lock().unwrap().since = None;
    sync(bk, None, true)
}
