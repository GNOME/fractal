use crate::client::ProxySettings;
use crate::error::{Error, StandardErrorResponse};
use crate::globals;
use crate::types::Event;
use crate::types::Member;
use crate::types::Message;
use crate::types::Room;
use crate::types::RoomMembership;
use crate::types::RoomTag;
use fractal_api::r0::filter::EventFilter;
use fractal_api::r0::filter::Filter;
use fractal_api::r0::filter::RoomEventFilter;
use fractal_api::r0::filter::RoomFilter;
use fractal_api::r0::sync::sync_events::request as sync_events;
use fractal_api::r0::sync::sync_events::IncludeState;
use fractal_api::r0::sync::sync_events::Parameters as SyncParameters;
use fractal_api::r0::sync::sync_events::Response as SyncResponse;
use fractal_api::r0::sync::sync_events::UnreadNotificationsCount;
use fractal_api::r0::AccessToken;

use fractal_api::identifiers::{EventId, RoomId, UserId};
use fractal_api::reqwest::blocking::{Client, Response};
use fractal_api::url::Url;
use log::error;
use serde::de::DeserializeOwned;
use serde_json::value::from_value;
use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    thread,
    time::{self, Duration},
};

pub enum RoomElement {
    Name(RoomId, String),
    Topic(RoomId, String),
    NewAvatar(RoomId),
    MemberEvent(Event),
    RemoveMessage(RoomId, EventId),
}

pub enum SyncRet {
    NoSince {
        rooms: Result<(Vec<Room>, Option<Room>), Error>,
        next_batch: String,
    },
    WithSince {
        update_rooms: Result<Vec<Room>, Error>,
        room_messages: Result<Vec<Message>, Error>,
        room_notifications: HashMap<RoomId, UnreadNotificationsCount>,
        update_rooms_2: Result<Vec<Room>, Error>,
        other: Result<Vec<RoomElement>, Error>,
        next_batch: String,
    },
}

pub fn sync(
    base: Url,
    access_token: AccessToken,
    user_id: UserId,
    join_to_room: Option<RoomId>,
    since: Option<String>,
    initial: bool,
    number_tries: u64,
) -> Result<SyncRet, (Error, u64)> {
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

    let client_builder_timeout =
        Client::builder().timeout(Some(Duration::from_secs(globals::TIMEOUT) + timeout));

    let query = ProxySettings::current().and_then(|proxy_settings| {
        let client = proxy_settings
            .apply_to_client_builder(client_builder_timeout)
            .build()?;
        let request = sync_events(base.clone(), &params)?;
        let response = client.execute(request)?;

        matrix_response::<SyncResponse>(response)
    });

    match query {
        Ok(response) => {
            if since.is_none() {
                let rooms = Room::from_sync_response(&response, user_id, base)
                    .map(|rooms| {
                        let def = join_to_room
                            .and_then(|jtr| rooms.iter().find(|x| x.id == jtr).cloned());
                        (rooms, def)
                    })
                    .map_err(Into::into);

                let next_batch = response.next_batch;

                Ok(SyncRet::NoSince { rooms, next_batch })
            } else {
                let join = &response.rooms.join;

                // New rooms
                let update_rooms =
                    Room::from_sync_response(&response, user_id.clone(), base).map_err(Into::into);

                // Message events
                let room_messages = join
                    .iter()
                    .try_fold(Vec::new(), |mut acum, (k, room)| {
                        let events = room.timeline.events.iter();
                        Message::from_json_events_iter(&k, events).map(|msgs| {
                            acum.extend(msgs);
                            acum
                        })
                    })
                    .map_err(Into::into);

                // Room notifications
                let room_notifications = join
                    .iter()
                    .map(|(k, room)| (k.clone(), room.unread_notifications.clone()))
                    .collect();

                // Typing notifications
                let update_rooms_2 = Ok(join
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
                    .collect());

                // Other events
                let other = join
                    .iter()
                    .flat_map(|(k, room)| {
                        room.timeline
                            .events
                            .iter()
                            .filter(|x| x["type"] != "m.room.message")
                            .map(move |ev| {
                                Ok(Event {
                                    room: k.clone(),
                                    sender: UserId::try_from(
                                        ev["sender"].as_str().unwrap_or_default(),
                                    )?,
                                    content: ev["content"].clone(),
                                    redacts: ev["redacts"]
                                        .as_str()
                                        .map(|r| r.try_into())
                                        .transpose()?,
                                    stype: ev["type"].as_str().map(Into::into).unwrap_or_default(),
                                    id: ev["id"].as_str().map(Into::into).unwrap_or_default(),
                                })
                            })
                    })
                    .filter_map(|ev| {
                        let ev = match ev {
                            Ok(ev) => ev,
                            Err(err) => return Some(Err(err)),
                        };

                        match ev.stype.as_ref() {
                            "m.room.name" => {
                                let name = ev.content["name"]
                                    .as_str()
                                    .map(Into::into)
                                    .unwrap_or_default();
                                Some(Ok(RoomElement::Name(ev.room.clone(), name)))
                            }
                            "m.room.topic" => {
                                let t = ev.content["topic"]
                                    .as_str()
                                    .map(Into::into)
                                    .unwrap_or_default();
                                Some(Ok(RoomElement::Topic(ev.room.clone(), t)))
                            }
                            "m.room.avatar" => Some(Ok(RoomElement::NewAvatar(ev.room.clone()))),
                            "m.room.member" => Some(Ok(RoomElement::MemberEvent(ev))),
                            "m.room.redaction" => Some(Ok(RoomElement::RemoveMessage(
                                ev.room.clone(),
                                ev.redacts.expect(
                                    "Events of type m.room.redaction should have a 'redacts' field",
                                ),
                            ))),
                            "m.sticker" => {
                                // This event is managed in the room list
                                None
                            }
                            _ => {
                                error!("EVENT NOT MANAGED: {:?}", ev);
                                None
                            }
                        }
                    })
                    .collect();

                let next_batch = response.next_batch;

                Ok(SyncRet::WithSince {
                    update_rooms,
                    room_messages,
                    room_notifications,
                    update_rooms_2,
                    other,
                    next_batch,
                })
            }
        }
        Err(err) => {
            // we wait if there's an error to avoid 100% CPU
            // we wait even longer, if it's a 429 (Too Many Requests) error
            let waiting_time = match err {
                Error::NetworkError(status) if status.as_u16() == 429 => {
                    10 * 2_u64.pow(
                        number_tries
                            .try_into()
                            .expect("The number of sync tries couldn't be transformed into a u32."),
                    )
                }
                _ => 10,
            };
            error!(
                "Sync Error, waiting {:?} seconds to respond for the next sync",
                waiting_time
            );
            thread::sleep(time::Duration::from_secs(waiting_time));

            Err((err, number_tries))
        }
    }
}

/// Returns the deserialized response to the given request. Handles Matrix errors.
fn matrix_response<T: DeserializeOwned>(response: Response) -> Result<T, Error> {
    if !response.status().is_success() {
        let status = response.status();
        return match response.json::<StandardErrorResponse>() {
            Ok(error_response) => Err(Error::from(error_response)),
            Err(_) => Err(Error::NetworkError(status)),
        };
    }

    response.json::<T>().map_err(Into::into)
}
