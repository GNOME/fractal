use crate::model::member::Member;
use crate::model::member::MemberList;
use crate::model::message::Message;
use crate::model::message_list::MessageList;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use either::Either;
use log::{debug, info};
use matrix_sdk::directory::PublicRoomsChunk;
use matrix_sdk::events::{
    room::member::{MemberEventContent, MembershipState},
    AnyBasicEvent, AnyBasicEventContent, AnyMessageEventContent, AnyStrippedStateEvent,
    AnySyncEphemeralRoomEvent, AnySyncRoomEvent, AnySyncStateEvent, SyncStateEvent,
};
use matrix_sdk::identifiers::{EventId, RoomAliasId, RoomId, UserId};
use matrix_sdk::{deserialized_responses::SyncResponse, identifiers::MxcUri};
use serde::{Deserialize, Serialize};
use serde_json::value::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::{TryFrom, TryInto};

#[derive(Debug, Clone, PartialEq)]
pub enum RoomMembership {
    // If the user hasn't yet joined a room, e.g. in the room directory
    None,
    Joined(RoomTag),
    // An invite is send by some other user
    Invited(UserId),
    Left(Reason),
}

// This needs to opt-out of the lint to keep consistency
#[allow(dead_code)]
impl RoomMembership {
    pub fn is_joined(&self) -> bool {
        if let RoomMembership::Joined(_) = self {
            true
        } else {
            false
        }
    }

    pub fn is_invited(&self) -> bool {
        if let RoomMembership::Invited(_) = self {
            true
        } else {
            false
        }
    }

    pub fn is_left(&self) -> bool {
        if let RoomMembership::Left(_) = self {
            true
        } else {
            false
        }
    }

    pub fn match_joined_tag(&self, tag: RoomTag) -> bool {
        if let RoomMembership::Joined(this_tag) = self {
            this_tag == &tag
        } else {
            false
        }
    }
}

impl Default for RoomMembership {
    fn default() -> Self {
        RoomMembership::None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Reason {
    None,
    Kicked(String, UserId),
}

// This needs to opt-out of the lint to keep consistency
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum RoomTag {
    None,
    Favourite,
    LowPriority,
    Custom(String),
}

#[derive(Deserialize, Serialize)]
#[serde(try_from = "&str")]
struct DirectType;

impl TryFrom<&str> for DirectType {
    type Error = anyhow::Error;

    fn try_from(event_type: &str) -> Result<Self, Self::Error> {
        match event_type {
            "m.direct" => Ok(Self),
            _ => Err(anyhow!("not a m.direct event")),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct CustomDirectEvent {
    content: BTreeMap<String, Vec<RoomId>>,
    #[serde(rename = "type")]
    _type: DirectType,
}

#[derive(Debug, Clone)]
pub struct Room {
    pub id: RoomId,
    pub avatar: Option<MxcUri>,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub alias: Option<RoomAliasId>,
    pub guest_can_join: bool,
    pub world_readable: bool,
    pub n_members: u64,
    pub members: MemberList,
    pub notifications: u64,
    pub highlight: u64,
    pub messages: MessageList,
    pub membership: RoomMembership,
    pub direct: bool,
    pub prev_batch: Option<String>,
    pub typing_users: Vec<Member>,
    pub language: Option<String>,

    /// Hashmap with the room users power levels
    /// the key will be the userid and the value will be the level
    pub admins: HashMap<UserId, i64>,
    pub default_power_level: i64,
}

impl Room {
    pub fn new(id: RoomId, membership: RoomMembership) -> Room {
        Room {
            id,
            membership,
            guest_can_join: true,
            world_readable: true,
            avatar: Default::default(),
            name: Default::default(),
            topic: Default::default(),
            alias: Default::default(),
            n_members: Default::default(),
            members: Default::default(),
            notifications: Default::default(),
            highlight: Default::default(),
            messages: Default::default(),
            direct: Default::default(),
            prev_batch: Default::default(),
            typing_users: Default::default(),
            language: Default::default(),
            admins: Default::default(),
            default_power_level: -1,
        }
    }

    pub fn from_sync_response(response: &SyncResponse, user_id: UserId) -> Vec<Self> {
        // getting the list of direct rooms
        let direct: HashSet<RoomId> = response
            .account_data
            .events
            .iter()
            .filter_map(|event| {
                if let AnyBasicEventContent::Direct(content) = event.content() {
                    Some(content.values().flatten().cloned().collect::<Vec<RoomId>>())
                } else {
                    None
                }
            })
            .flatten()
            .collect();

        let joined_rooms = response.rooms.join.iter().map(|(k, room)| {
            let stevents: Vec<_> = room.state.events.iter().collect();
            let dataevs: Vec<_> = room.account_data.events.iter().collect();
            let room_tag = dataevs
                .iter()
                .find_map(|event| match event {
                    AnyBasicEvent::Tag(ev) => ev.content.tags.get("m.favourite"),
                    _ => None,
                })
                .and(Some(RoomTag::Favourite))
                .unwrap_or(RoomTag::None);

            let members: MemberList = stevents
                .iter()
                .filter_map(|event| match event {
                    AnySyncStateEvent::RoomMember(ev) => parse_room_member(ev),
                    _ => None,
                })
                .map(|m| (m.uid.clone(), m))
                .collect();

            let mut r = Self {
                name: stevents
                    .iter()
                    .filter_map(|event| match event {
                        AnySyncStateEvent::RoomName(ev) => {
                            ev.content.name().filter(|name| !name.is_empty()).map(Err)
                        }
                        AnySyncStateEvent::RoomCanonicalAlias(ev) => ev
                            .content
                            .alias
                            .as_ref()
                            .map(|r_alias| Ok(r_alias.as_str())),
                        _ => None,
                    })
                    .try_fold(None, |_, alias_name| alias_name.map(Some))
                    .unwrap_or_else(Some)
                    .map(Into::into)
                    .or_else(|| {
                        let members: Vec<_> = members
                            .values()
                            .map(|m| m.alias.as_deref().unwrap_or_else(|| m.uid.as_str()))
                            .filter(|&uid| uid == user_id.as_str())
                            .collect();
                        room_name_from_members(&members)
                    }),
                avatar: stevents
                    .iter()
                    .find_map(|event| match event {
                        AnySyncStateEvent::RoomAvatar(ev) => Some(ev.content.url.clone()),
                        _ => None,
                    })
                    .flatten(),
                alias: stevents
                    .iter()
                    .find_map(|event| match event {
                        AnySyncStateEvent::RoomCanonicalAlias(ev) => Some(ev.content.alias.clone()),
                        _ => None,
                    })
                    .flatten(),
                topic: stevents.iter().find_map(|event| match event {
                    AnySyncStateEvent::RoomTopic(ev) => Some(ev.content.topic.clone()),
                    _ => None,
                }),
                direct: direct.contains(&k),
                notifications: room.unread_notifications.notification_count,
                highlight: room.unread_notifications.highlight_count,
                prev_batch: room.timeline.prev_batch.clone(),
                messages: room
                    .timeline
                    .events
                    .iter()
                    .filter_map(|event| (k.clone(), event.clone()).try_into().ok())
                    .collect(),
                admins: stevents
                    .iter()
                    .filter_map(|event| match event {
                        AnySyncStateEvent::RoomPowerLevels(ev) => Some(ev.content.users.clone()),
                        _ => None,
                    })
                    .flatten()
                    .map(|(uid, level)| (uid, level.into()))
                    .collect(),
                default_power_level: stevents
                    .iter()
                    .filter_map(|event| match event {
                        AnySyncStateEvent::RoomPowerLevels(ev) => {
                            Some(ev.content.users_default.clone().into())
                        }
                        _ => None,
                    })
                    .last()
                    .unwrap_or(-1),
                members,
                language: dataevs
                    .iter()
                    .find_map(|event| match event {
                        AnyBasicEvent::Custom(ev)
                            if ev.content.event_type == "org.gnome.fractal.language" =>
                        {
                            ev.content
                                .data
                                .get("input_language")
                                .map(|v| v.as_str())
                                .flatten()
                        }
                        _ => None,
                    })
                    .map(String::from),
                ..Self::new(k.clone(), RoomMembership::Joined(room_tag))
            };

            let receipts: HashMap<EventId, HashMap<UserId, i64>> = room
                .ephemeral
                .events
                .iter()
                .filter_map(|event| match event {
                    AnySyncEphemeralRoomEvent::Receipt(ev) => Some(ev.content.0.clone()),
                    _ => None,
                })
                .take(1)
                .flatten()
                .map(|(event_id, receipts)| {
                    let receipts = receipts
                        .read
                        .into_iter()
                        .flatten()
                        .map(|(uid, receipt)| {
                            let ts = receipt
                                .ts
                                .map(DateTime::<Utc>::from)
                                .map(|time| time.timestamp())
                                .unwrap_or_default();
                            (uid, ts)
                        })
                        .inspect(|(_, ts)| {
                            debug!("Value of timestamp 'ts': {:?}", ts);
                            if *ts == 0 {
                                info!(
                                    "Possibly malformed timestamp, working around synapse bug 4898"
                                );
                            };
                        })
                        .collect();

                    (event_id, receipts)
                })
                .collect();

            let changed_msgs: Vec<_> = r
                .messages
                .iter()
                .filter_map(|msg| {
                    let receipt = msg.id.as_ref().and_then(|evid| receipts.get(evid))?;
                    Some((msg.clone(), receipt.clone()))
                })
                .collect();
            for (mut msg, receipt) in changed_msgs {
                msg.set_receipt(receipt);
                r.take_new_message(msg);
            }

            if let Some(event_id) = room.ephemeral.events.iter().find_map(|event| match event {
                AnySyncEphemeralRoomEvent::FullyRead(ev) => Some(ev.content.event_id.clone()),
                _ => None,
            }) {
                if let Some(mut msg) = r.messages.get(&event_id).cloned() {
                    msg.receipt.insert(user_id.clone(), 0);
                    r.take_new_message(msg);
                }
            }

            r
        });

        let left_rooms = response.rooms.leave.iter().map(|(k, room)| {
            // TODO: The spec doesn't explain where to get the reason
            //       for the kicking from, so matrix-sdk doesn't support
            //       that.
            if let Some(last_event) = room.timeline.events.last() {
                if let AnySyncRoomEvent::Message(message) = last_event {
                    let kicker = message.sender().clone();
                    if kicker != user_id {
                        if let AnyMessageEventContent::Custom(message) = message.content() {
                            if let Some(Value::String(kick_reason)) = &message.data.get("reason") {
                                let reason = Reason::Kicked(kick_reason.clone(), kicker);
                                return Self::new(k.clone(), RoomMembership::Left(reason));
                            }
                        }
                    }
                }
            };

            Self::new(k.clone(), RoomMembership::Left(Reason::None))
        });

        let invited_rooms = response.rooms.invite.iter().filter_map(|(k, room)| {
            let stevents: Vec<_> = room.invite_state.events.iter().collect();
            let inv_sender = stevents
                .iter()
                .find_map(|event| match event {
                    AnyStrippedStateEvent::RoomMember(ev)
                        if ev.content.membership == MembershipState::Invite
                            && ev.state_key == user_id =>
                    {
                        Some(ev)
                    }
                    _ => None,
                })
                .map(|ev| ev.sender.clone());
            if let Some(inv_sender) = inv_sender {
                Some(Self {
                    name: stevents
                        .iter()
                        .filter_map(|event| match event {
                            AnyStrippedStateEvent::RoomName(ev) => {
                                ev.content.name().filter(|name| !name.is_empty()).map(Err)
                            }
                            AnyStrippedStateEvent::RoomCanonicalAlias(ev) => ev
                                .content
                                .alias
                                .as_ref()
                                .map(|r_alias| Ok(r_alias.as_str())),
                            _ => None,
                        })
                        .try_fold(None, |_, alias_name| alias_name.map(Some))
                        .unwrap_or_else(Some)
                        .map(Into::into)
                        .or_else(|| {
                            let members: Vec<_> = stevents
                                .iter()
                                .filter_map(|event| member_from_stripped_event(event, &user_id))
                                .take(3)
                                .map(Into::into)
                                .collect();
                            room_name_from_members(&members)
                        }),
                    avatar: stevents
                        .iter()
                        .find_map(|event| match event {
                            AnyStrippedStateEvent::RoomAvatar(ev) => Some(ev.content.url.clone()),
                            _ => None,
                        })
                        .flatten(),
                    alias: stevents
                        .iter()
                        .find_map(|event| match event {
                            AnyStrippedStateEvent::RoomCanonicalAlias(ev) => {
                                Some(ev.content.alias.clone())
                            }
                            _ => None,
                        })
                        .flatten(),
                    topic: stevents.iter().find_map(|event| match event {
                        AnyStrippedStateEvent::RoomTopic(ev) => Some(ev.content.topic.clone()),
                        _ => None,
                    }),
                    direct: direct.contains(&k),
                    ..Self::new(k.clone(), RoomMembership::Invited(inv_sender))
                })
            } else {
                None
            }
        });

        joined_rooms
            .chain(left_rooms)
            .chain(invited_rooms)
            .collect()
    }

    /// Inserts the given message into the room.
    pub fn take_new_message(&mut self, msg: Message) {
        self.messages.add(msg);
    }
}

impl From<PublicRoomsChunk> for Room {
    fn from(input: PublicRoomsChunk) -> Self {
        Self {
            alias: input.canonical_alias,
            name: input.name,
            avatar: input.avatar_url,
            topic: input.topic,
            n_members: input.num_joined_members.into(),
            world_readable: input.world_readable,
            guest_can_join: input.guest_can_join,
            ..Self::new(input.room_id, RoomMembership::None)
        }
    }
}

impl PartialEq for Room {
    fn eq(&self, other: &Room) -> bool {
        self.id == other.id
    }
}

pub type RoomList = HashMap<RoomId, Room>;

fn member_from_stripped_event<'a>(
    event: &'a AnyStrippedStateEvent,
    user_id: &UserId,
) -> Option<&'a str> {
    match event {
        AnyStrippedStateEvent::RoomMember(ev) => match ev.content.membership {
            MembershipState::Join if ev.sender.as_str() != user_id.as_str() => Some(
                ev.content
                    .displayname
                    .as_ref()
                    .map(String::as_str)
                    .unwrap_or_else(|| ev.sender.as_str()),
            ),
            MembershipState::Invite if ev.state_key.as_str() != user_id.as_str() => Some(
                ev.content
                    .displayname
                    .as_ref()
                    .map(String::as_str)
                    .unwrap_or_else(|| ev.state_key.as_str()),
            ),
            _ => None,
        },
        _ => None,
    }
}

fn room_name_from_members(members: &[&str]) -> Option<String> {
    match members.len() {
        0 => None,
        1 => Some(members[0].to_owned()),
        2 => Some(format!("{} and {}", members[0], members[1])),
        _ => Some(format!("{} and Others", members[0])),
    }
}

fn parse_room_member(msg: &SyncStateEvent<MemberEventContent>) -> Option<Member> {
    if msg.content.membership == MembershipState::Join {
        Some(Member {
            uid: msg.sender.clone(),
            alias: msg.content.displayname.clone(),
            avatar: msg.content.avatar_url.clone().map(Either::Left),
        })
    } else {
        None
    }
}
