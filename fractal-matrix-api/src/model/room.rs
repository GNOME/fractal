use serde_json::Value as JsonValue;

use crate::model::member::Member;
use crate::model::member::MemberList;
use crate::model::message::Message;
use crate::r0::directory::post_public_rooms::Chunk as PublicRoomsChunk;
use crate::r0::sync::sync_events::Response as SyncResponse;
use crate::util::get_user_avatar;
use crate::util::parse_m_direct;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoomMembership {
    // If the user hasn't yet joined a room, e.g. in the room directory
    None,
    Joined(RoomTag),
    // An invite is send by some other user
    Invited(Member),
    Left(Reason),
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Reason {
    None,
    Kicked(String, Member),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoomTag {
    None,
    Favourite,
    LowPriority,
    Custom(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub avatar: Option<String>, // TODO: Use Option<Url>
    pub name: Option<String>,
    pub topic: Option<String>,
    pub alias: Option<String>,
    pub guest_can_join: bool,
    pub world_readable: bool,
    pub n_members: i32,
    pub members: MemberList,
    pub notifications: i32,
    pub highlight: i32,
    pub messages: Vec<Message>,
    pub membership: RoomMembership,
    pub direct: bool,
    pub prev_batch: Option<String>,
    pub typing_users: Vec<Member>,

    /// Hashmap with the room users power levels
    /// the key will be the userid and the value will be the level
    pub admins: HashMap<String, i32>,
    pub power_levels: HashMap<String, i32>,
}

impl Room {
    pub fn new(id: String, membership: RoomMembership) -> Room {
        Room {
            id,
            membership,
            guest_can_join: true,
            world_readable: true,
            ..Default::default()
        }
    }

    pub fn from_sync_response(response: &SyncResponse, userid: &str, baseu: &Url) -> Vec<Self> {
        // getting the list of direct rooms
        let direct: HashSet<String> = parse_m_direct(&response.account_data.events)
            .values()
            .flatten()
            .cloned()
            .collect();

        let joined_rooms = response.rooms.join.iter().map(|(k, room)| {
            let stevents = &room.state.events;
            let timeline = &room.timeline;
            let ephemeral = &room.ephemeral;
            let dataevs = &room.account_data.events;
            let room_tag = dataevs
                .iter()
                .filter(|x| x["type"] == "m.tag")
                .find_map(|tag| tag["content"]["tags"]["m.favourite"].as_object())
                .and(Some(RoomTag::Favourite))
                .unwrap_or(RoomTag::None);

            let mut r = Self {
                name: calculate_room_name(stevents, userid),
                avatar: evc(stevents, "m.room.avatar", "url"),
                alias: evc(stevents, "m.room.canonical_alias", "alias"),
                topic: evc(stevents, "m.room.topic", "topic"),
                direct: direct.contains(k),
                notifications: room.unread_notifications.notification_count,
                highlight: room.unread_notifications.highlight_count,
                prev_batch: timeline.prev_batch.clone(),
                messages: Message::from_json_events_iter(&k, timeline.events.iter()),
                admins: get_admins(stevents),
                power_levels: get_power_levels(stevents),
                members: stevents
                    .iter()
                    .filter(|x| x["type"] == "m.room.member")
                    .filter_map(parse_room_member)
                    .map(|m| (m.uid.clone(), m))
                    .collect(),
                ..Self::new(k.clone(), RoomMembership::Joined(room_tag))
            };

            r.add_receipt_from_json(
                ephemeral
                    .events
                    .iter()
                    .filter(|ev| ev["type"] == "m.receipt")
                    .collect(),
            );
            // Adding fully read to the receipts events
            if let Some(ev) = dataevs
                .into_iter()
                .find(|x| x["type"] == "m.fully_read")
                .and_then(|fread| fread["content"]["event_id"].as_str())
            {
                r.add_receipt_from_fully_read(userid, ev);
            }

            r
        });

        let left_rooms = response.rooms.leave.iter().map(|(k, room)| {
            if let Some(last_event) = room.timeline.events.last() {
                let leave_id = &last_event["sender"];
                if leave_id != userid {
                    let kick_reason = &last_event["content"]["reason"];
                    if let Some((kicker_alias, kicker_avatar)) =
                        get_user_avatar(baseu, leave_id.as_str().unwrap_or_default()).ok()
                    {
                        let kicker = Member {
                            alias: Some(kicker_alias),
                            avatar: Some(kicker_avatar),
                            uid: String::from(leave_id.as_str().unwrap_or_default()),
                        };
                        let reason = Reason::Kicked(
                            String::from(kick_reason.as_str().unwrap_or_default()),
                            kicker,
                        );
                        Self::new(k.clone(), RoomMembership::Left(reason))
                    } else {
                        Self::new(k.clone(), RoomMembership::Left(Reason::None))
                    }
                } else {
                    Self::new(k.clone(), RoomMembership::Left(Reason::None))
                }
            } else {
                Self::new(k.clone(), RoomMembership::Left(Reason::None))
            }
        });

        let invited_rooms = response.rooms.invite.iter().filter_map(|(k, room)| {
            let stevents = &room.invite_state.events;
            if let Some((alias, avatar)) = stevents
                .iter()
                .find(|x| x["content"]["membership"] == "invite" && x["state_key"] == userid)
                .and_then(|ev| {
                    get_user_avatar(baseu, ev["sender"].as_str().unwrap_or_default()).ok()
                })
            {
                let inv_sender = Member {
                    alias: Some(alias),
                    avatar: Some(avatar),
                    uid: String::from(userid),
                };

                Some(Self {
                    name: calculate_room_name(stevents, userid),
                    avatar: evc(stevents, "m.room.avatar", "url"),
                    alias: evc(stevents, "m.room.canonical_alias", "alias"),
                    topic: evc(stevents, "m.room.topic", "topic"),
                    direct: direct.contains(k),
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

    pub fn add_receipt_from_json(&mut self, mut events: Vec<&JsonValue>) {
        let receipts: HashMap<String, HashMap<String, i64>> = events
            .pop()
            .and_then(|ev| ev["content"].as_object())
            .into_iter()
            .flatten()
            .filter_map(|(mid, obj)| {
                let receipts = obj["m.read"]
                    .as_object()?
                    .iter()
                    .map(|(uid, ts)| {
                        debug!("Value of timestamp 'ts': {}", ts);
                        let ts = ts["ts"].as_i64().unwrap_or(0);
                        if ts == 0 {
                            info!("Possibly malformed timestamp, working around synapse bug 4898");
                        };
                        (uid.to_string(), ts)
                    })
                    .collect();

                Some((mid.to_string(), receipts))
            })
            .collect();

        if !receipts.is_empty() {
            for msg in self.messages.iter_mut() {
                if let Some(r) = receipts.get(&msg.id) {
                    msg.set_receipt(r.clone());
                }
            }
        }
    }

    pub fn add_receipt_from_fully_read(&mut self, uid: &str, evid: &str) {
        let _ = self
            .messages
            .iter_mut()
            .filter(|msg| msg.id == evid.to_string())
            .map(|msg| msg.receipt.insert(uid.to_string(), 0));
    }
}

impl From<PublicRoomsChunk> for Room {
    fn from(input: PublicRoomsChunk) -> Self {
        Self {
            alias: input.canonical_alias.as_ref().map(ToString::to_string),
            name: input.name,
            avatar: input.avatar_url.map(Url::into_string),
            topic: input.topic,
            n_members: input.num_joined_members,
            world_readable: input.world_readable,
            guest_can_join: input.guest_can_join,
            ..Self::new(input.room_id.to_string(), RoomMembership::None)
        }
    }
}

impl PartialEq for Room {
    fn eq(&self, other: &Room) -> bool {
        self.id == other.id
    }
}

pub type RoomList = HashMap<String, Room>;

fn evc(events: &Vec<JsonValue>, t: &str, field: &str) -> Option<String> {
    events
        .iter()
        .find(|x| x["type"] == t)
        .and_then(|js| js["content"][field].as_str())
        .map(Into::into)
}

fn get_admins(stevents: &Vec<JsonValue>) -> HashMap<String, i32> {
    stevents
        .iter()
        .filter(|x| x["type"] == "m.room.power_levels")
        .filter_map(|ev| ev["content"]["users"].as_object())
        .flatten()
        .map(|(k, v)| (k.clone(), v.as_i64().map(|v| v as i32).unwrap_or_default()))
        .collect()
}

fn get_power_levels(stevents: &Vec<JsonValue>) -> HashMap<String, i32> {
    stevents
        .iter()
        .filter(|x| x["type"] == "m.room.power_levels")
        .filter_map(|ev| ev["content"].as_object())
        .flatten()
        .map(|(k, v)| (k.clone(), v.as_i64().map(|v| v as i32).unwrap_or_default()))
        .collect()
}

fn calculate_room_name(events: &Vec<JsonValue>, userid: &str) -> Option<String> {
    // looking for "m.room.name" event
    if let Some(name) = events
        .iter()
        .find(|x| x["type"] == "m.room.name")
        .and_then(|name| name["content"]["name"].as_str())
        .filter(|name| !name.is_empty())
        .map(Into::into)
    {
        return Some(name);
    }

    // looking for "m.room.canonical_alias" event
    if let Some(name) = events
        .iter()
        .find(|x| x["type"] == "m.room.canonical_alias")
        .and_then(|name| name["content"]["alias"].as_str())
        .map(Into::into)
    {
        return Some(name);
    }

    // we look for members that aren't me
    let members: Vec<&str> = events
        .iter()
        .filter(|x| {
            (x["type"] == "m.room.member"
                && ((x["content"]["membership"] == "join" && x["sender"] != userid)
                    || (x["content"]["membership"] == "invite" && x["state_key"] != userid)))
        })
        .take(3)
        .map(|m| {
            let sender = m["sender"].as_str().unwrap_or("NONAMED");
            m["content"]["displayname"].as_str().unwrap_or(sender)
        })
        .collect();

    match members.len() {
        // we don't have information to calculate the name
        0 => None,
        1 => Some(members[0].to_string()),
        2 => Some(format!("{} and {}", members[0], members[1])),
        _ => Some(format!("{} and Others", members[0])),
    }
}

fn parse_room_member(msg: &JsonValue) -> Option<Member> {
    let c = &msg["content"];
    let _ = c["membership"].as_str().filter(|&m| m == "join")?;

    Some(Member {
        uid: msg["sender"].as_str().map(Into::into)?,
        alias: c["displayname"].as_str().map(Into::into),
        avatar: c["avatar_url"].as_str().map(Into::into),
    })
}
