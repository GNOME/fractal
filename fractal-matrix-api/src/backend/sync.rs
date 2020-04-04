use crate::backend::types::BKResponse;
use crate::backend::types::Backend;
use crate::client::ProxySettings;
use crate::globals;
use crate::r0::filter::EventFilter;
use crate::r0::filter::Filter;
use crate::r0::filter::RoomEventFilter;
use crate::r0::filter::RoomFilter;
use crate::r0::sync::sync_events::request as sync_events;
use crate::r0::sync::sync_events::IncludeState;
use crate::r0::sync::sync_events::Parameters as SyncParameters;
use crate::r0::sync::sync_events::Response as SyncResponse;
use crate::r0::sync::sync_events::UnreadNotificationsCount;
use crate::r0::AccessToken;
use crate::types::Event;
use crate::types::Member;
use crate::types::Message;
use crate::types::Room;
use crate::types::RoomMembership;
use crate::types::RoomTag;
use crate::util::parse_m_direct;
use crate::util::ResultExpectLog;

use log::error;
use reqwest::blocking::Client;
use ruma_identifiers::UserId;
use serde_json::value::from_value;
use std::{
    convert::TryFrom,
    thread,
    time::{self, Duration},
};
use url::Url;

pub fn sync(
    bk: &Backend,
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    since: Option<String>,
    initial: bool,
) {
    let tx = bk.tx.clone();
    let data = bk.data.clone();

    let (timeout, filter) = if !initial {
        (time::Duration::from_secs(30), Default::default())
    } else {
        let filter = Filter {
            room: Some(RoomFilter {
                state: Some(RoomEventFilter {
                    lazy_load_members: true,
                    types: Some(vec!["m.room.*"]),
                    ..Default::default()
                }),
                timeline: Some(RoomEventFilter {
                    types: Some(vec!["m.room.message", "m.sticker"]),
                    not_types: vec!["m.call.*"],
                    limit: Some(globals::PAGE_LIMIT),
                    ..Default::default()
                }),
                ephemeral: Some(RoomEventFilter {
                    types: Some(vec![]),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            presence: Some(EventFilter {
                types: Some(vec![]),
                ..Default::default()
            }),
            event_fields: Some(vec![
                "type",
                "content",
                "sender",
                "origin_server_ts",
                "event_id",
                "unsigned",
            ]),
            ..Default::default()
        };

        (Default::default(), filter)
    };

    let params = SyncParameters {
        access_token,
        filter,
        include_state: IncludeState::Changed {
            since: since.clone().unwrap_or_default(),
            timeout,
        },
        set_presence: Default::default(),
    };

    thread::spawn(move || {
        let client_builder_timeout =
            Client::builder().timeout(Some(Duration::from_secs(globals::TIMEOUT) + timeout));

        let query = ProxySettings::current().and_then(|proxy_settings| {
            let client = proxy_settings
                .apply_to_client_builder(client_builder_timeout)?
                .build()?;
            let request = sync_events(base.clone(), &params)?;
            client
                .execute(request)?
                .json::<SyncResponse>()
                .map_err(Into::into)
        });

        match query {
            Ok(response) => {
                if since.is_some() {
                    let join = &response.rooms.join;

                    // New rooms
                    let rs = Room::from_sync_response(&response, user_id.clone(), base)
                        .map_err(Into::into);
                    tx.send(BKResponse::UpdateRooms(rs))
                        .expect_log("Connection closed");

                    // Message events
                    let msgs = join
                        .iter()
                        .try_fold(Vec::new(), |mut acum, (k, room)| {
                            let events = room.timeline.events.iter();
                            Message::from_json_events_iter(&k, events).map(|msgs| {
                                acum.extend(msgs);
                                acum
                            })
                        })
                        .map_err(Into::into);
                    tx.send(BKResponse::RoomMessages(msgs))
                        .expect_log("Connection closed");

                    // Room notifications
                    for (k, room) in join.iter() {
                        let UnreadNotificationsCount {
                            highlight_count: h,
                            notification_count: n,
                        } = room.unread_notifications;
                        tx.send(BKResponse::RoomNotifications(k.clone(), n, h))
                            .expect_log("Connection closed");
                    }

                    // Typing notifications
                    let rooms: Vec<Room> = join
                        .iter()
                        .map(|(k, room)| {
                            let ephemerals = &room.ephemeral.events;
                            let typing: Vec<Member> = ephemerals.iter()
                                .flat_map(|event| {
                                    event
                                        .get("content")
                                        .and_then(|x| x.get("user_ids"))
                                        .and_then(|x| x.as_array())
                                        .unwrap_or(&vec![])
                                        .to_owned()
                                })
                                .filter_map(|user| from_value(user).ok())
                                // ignoring the user typing notifications
                                .filter(|user| *user != user_id)
                                .map(|uid| {
                                    Member {
                                        uid,
                                        alias: None,
                                        avatar: None,
                                    }
                                })
                                .collect();

                            Room {
                                typing_users: typing,
                                ..Room::new(k.clone(), RoomMembership::Joined(RoomTag::None))
                            }
                        })
                        .collect();
                    tx.send(BKResponse::UpdateRooms(Ok(rooms)))
                        .expect_log("Connection closed");

                    // Other events
                    join.iter()
                        .flat_map(|(k, room)| {
                            room.timeline
                                .events
                                .iter()
                                .filter(|x| x["type"] != "m.room.message")
                                .map(move |ev| Event {
                                    room: k.clone(),
                                    sender: UserId::try_from(
                                        ev["sender"].as_str().unwrap_or_default(),
                                    )
                                    .unwrap(),
                                    // TODO: Correct error management is too hard here,
                                    //       needs refactoring, but this workaround
                                    //       is enough
                                    content: ev["content"].clone(),
                                    redacts: ev["redacts"]
                                        .as_str()
                                        .map(Into::into)
                                        .unwrap_or_default(),
                                    stype: ev["type"].as_str().map(Into::into).unwrap_or_default(),
                                    id: ev["id"].as_str().map(Into::into).unwrap_or_default(),
                                })
                        })
                        .for_each(|ev| {
                            match ev.stype.as_ref() {
                                "m.room.name" => {
                                    let name = ev.content["name"]
                                        .as_str()
                                        .map(Into::into)
                                        .unwrap_or_default();
                                    tx.send(BKResponse::RoomName(ev.room.clone(), name))
                                        .expect_log("Connection closed");
                                }
                                "m.room.topic" => {
                                    let t = ev.content["topic"]
                                        .as_str()
                                        .map(Into::into)
                                        .unwrap_or_default();
                                    tx.send(BKResponse::RoomTopic(ev.room.clone(), t))
                                        .expect_log("Connection closed");
                                }
                                "m.room.avatar" => {
                                    tx.send(BKResponse::NewRoomAvatar(ev.room.clone()))
                                        .expect_log("Connection closed");
                                }
                                "m.room.member" => {
                                    tx.send(BKResponse::RoomMemberEvent(ev))
                                        .expect_log("Connection closed");
                                }
                                "m.sticker" => {
                                    // This event is managed in the room list
                                }
                                "m.room.redaction" => {
                                    let _ = tx.send(BKResponse::RemoveMessage(Ok((
                                        ev.room.clone(),
                                        ev.redacts,
                                    ))));
                                }
                                _ => {
                                    error!("EVENT NOT MANAGED: {:?}", ev);
                                }
                            }
                        });
                } else {
                    data.lock().unwrap().m_direct = parse_m_direct(&response.account_data.events);

                    let rooms_def =
                        Room::from_sync_response(&response, user_id, base)
                            .map(|rooms| {
                                let def =
                                    data.lock().unwrap().join_to_room.as_ref().and_then(|jtr| {
                                        rooms.iter().find(|x| x.id == *jtr).cloned()
                                    });
                                (rooms, def)
                            })
                            .map_err(Into::into);
                    tx.send(BKResponse::Rooms(rooms_def))
                        .expect_log("Connection closed");
                }

                let next_batch = response.next_batch;
                tx.send(BKResponse::Sync(Ok(next_batch)))
                    .expect_log("Connection closed");
            }
            Err(err) => {
                // we wait if there's an error to avoid 100% CPU
                error!("Sync Error, waiting 10 seconds to respond for the next sync");
                thread::sleep(time::Duration::from_secs(10));

                tx.send(BKResponse::Sync(Err(err)))
                    .expect_log("Connection closed");
            }
        }
    });
}
