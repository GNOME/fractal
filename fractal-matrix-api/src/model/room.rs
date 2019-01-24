use serde_json::Value as JsonValue;

use crate::model::member::Member;
use crate::model::member::MemberList;
use crate::model::message::Message;
use crate::types::SyncResponse;
use crate::util::get_user_avatar;
use crate::util::parse_m_direct;
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
    Left,
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
        self == &RoomMembership::Left
    }

    pub fn match_joined_tag(&self, tag: RoomTag) -> bool {
        if let RoomMembership::Joined(this_tag) = self {
            this_tag == &tag
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoomTag {
    None,
    Favourite,
    LowPriority,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub avatar: Option<String>,
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

    /// Hashmap with the room users power levels
    /// the key will be the userid and the value will be the level
    pub power_levels: HashMap<String, i32>,
}

impl Room {
    pub fn new(id: String, membership: RoomMembership) -> Room {
        Room {
            id,
            name: None,
            avatar: None,
            topic: None,
            alias: None,
            guest_can_join: true,
            world_readable: true,
            n_members: 0,
            notifications: 0,
            highlight: 0,
            messages: vec![],
            members: HashMap::new(),
            membership,
            direct: false,
            power_levels: HashMap::new(),
            prev_batch: None,
        }
    }

    pub fn from_sync_response(r: &SyncResponse, userid: &str, baseu: &Url) -> Vec<Self> {
        let join = r.rooms.join.clone();
        let leave = r.rooms.leave.clone();
        let invite = r.rooms.invite.clone();

        // getting the list of direct rooms
        let direct: HashSet<String> = parse_m_direct(&r.account_data.events)
            .values()
            .flatten()
            .cloned()
            .collect();

        let mut rooms: Vec<Self> = vec![];
        for (k, room) in join.iter() {
            let stevents = &room.state.events;
            let timeline = &room.timeline;
            let ephemeral = &room.ephemeral;
            let dataevs = &room.account_data.events;
            let name = calculate_room_name(stevents, userid);
            let mut room_tag = RoomTag::None;
            for tag in dataevs.iter().filter(|x| x["type"] == "m.tag") {
                if tag["content"]["tags"]["m.favourite"].as_object().is_some() {
                    room_tag = RoomTag::Favourite;
                }
            }
            let mut r = Self::new(k.clone(), RoomMembership::Joined(room_tag));

            r.name = name;
            r.avatar = Some(evc(stevents, "m.room.avatar", "url"));
            r.alias = Some(evc(stevents, "m.room.canonical_alias", "alias"));
            r.topic = Some(evc(stevents, "m.room.topic", "topic"));
            r.direct = direct.contains(k);
            r.notifications = room.unread_notifications.notification_count;
            r.highlight = room.unread_notifications.highlight_count;

            r.prev_batch = timeline.prev_batch.clone();

            let ms = Message::from_json_events_iter(&k, timeline.events.iter());
            r.messages.extend(ms);

            r.add_receipt_from_json(
                ephemeral
                    .events
                    .iter()
                    .filter(|ev| ev["type"] == "m.receipt")
                    .collect(),
            );
            // Adding fully read to the receipts events
            if let Some(fread) = dataevs.into_iter().find(|x| x["type"] == "m.fully_read") {
                if let Some(ev) = fread["content"]["event_id"].as_str() {
                    r.add_receipt_from_fully_read(userid, ev);
                }
            }

            let mevents = stevents.iter().filter(|x| x["type"] == "m.room.member");

            for ev in mevents {
                let member = parse_room_member(ev);
                if let Some(m) = member {
                    r.members.insert(m.uid.clone(), m.clone());
                }
            }

            // power levels info
            r.power_levels = get_admins(stevents);

            rooms.push(r);
        }

        // left rooms
        for k in leave.keys() {
            let r = Self::new(k.clone(), RoomMembership::Left);
            rooms.push(r);
        }

        // invitations
        for (k, room) in invite.iter() {
            let stevents = &room.invite_state.events;
            let name = calculate_room_name(stevents, userid);

            if let Some(ev) = stevents
                .iter()
                .find(|x| x["membership"] == "invite" && x["state_key"] == userid)
            {
                if let Ok((alias, avatar)) =
                    get_user_avatar(baseu, ev["sender"].as_str().unwrap_or_default())
                {
                    let inv_sender = Member {
                        alias: Some(alias),
                        avatar: Some(avatar),
                        uid: String::from(userid),
                    };
                    let mut r = Self::new(k.clone(), RoomMembership::Invited(inv_sender));
                    r.name = name;

                    r.avatar = Some(evc(stevents, "m.room.avatar", "url"));
                    r.alias = Some(evc(stevents, "m.room.canonical_alias", "alias"));
                    r.topic = Some(evc(stevents, "m.room.topic", "topic"));
                    r.direct = direct.contains(k);

                    rooms.push(r);
                }
            }
        }

        rooms
    }

    pub fn add_receipt_from_json(&mut self, mut events: Vec<&JsonValue>) {
        let receipts = events
            .pop()
            .and_then(|ev| ev["content"].as_object())
            .and_then(|content| {
                let mut msgs: HashMap<String, HashMap<String, i64>> = HashMap::new();

                for (mid, obj) in content.iter() {
                    if let Some(reads) = obj["m.read"].as_object() {
                        let mut receipts: HashMap<String, i64> = HashMap::new();

                        for (uid, ts) in reads.iter() {
                            receipts.insert(uid.to_string(), ts["ts"].as_i64().unwrap());
                        }

                        msgs.insert(mid.to_string(), receipts);
                    }
                }

                Some(msgs)
            });

        if let Some(receipts) = receipts.clone() {
            for msg in self.messages.iter_mut() {
                if let Some(r) = receipts.get(&msg.id) {
                    msg.set_receipt(r.clone());
                }
            }
        }
    }

    pub fn add_receipt_from_fully_read(&mut self, uid: &str, evid: &str) {
        for msg in self
            .messages
            .iter_mut()
            .filter(|m| m.id == evid.to_string())
        {
            msg.receipt.insert(uid.to_string(), 0);
        }
    }
}

impl From<PublicRoomsChunk> for Room {
    fn from(input: PublicRoomsChunk) -> Self {
        let mut room = Self::new(input.room_id, RoomMembership::None);
        room.alias = input.canonical_alias;
        room.name = input.name;
        room.avatar = input.avatar_url;
        room.topic = input.topic;
        room.n_members = input.num_joined_members;
        room.world_readable = input.world_readable;
        room.guest_can_join = input.guest_can_join;

        room
    }
}

impl PartialEq for Room {
    fn eq(&self, other: &Room) -> bool {
        self.id == other.id
    }
}

pub type RoomList = HashMap<String, Room>;

#[derive(Clone, Debug, Serialize)]
pub struct PublicRoomsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    // This field doesn't follow the spec but for some reason
    // it fails with matrix.org if it's not set this way
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(flatten)]
    pub third_party_networks: ThirdPartyNetworks,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "include_all_networks", content = "third_party_instance_id")]
pub enum ThirdPartyNetworks {
    #[serde(rename = "false")]
    None,
    #[serde(rename = "false")]
    Only(String),
    #[serde(rename = "true")]
    All,
}

impl Default for ThirdPartyNetworks {
    fn default() -> Self {
        ThirdPartyNetworks::None
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct PublicRoomsResponse {
    pub chunk: Vec<PublicRoomsChunk>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
    pub total_room_count_estimate: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PublicRoomsChunk {
    pub aliases: Option<Vec<String>>,
    pub avatar_url: Option<String>,
    pub canonical_alias: Option<String>,
    pub guest_can_join: bool,
    pub name: Option<String>,
    pub num_joined_members: i32,
    pub room_id: String,
    pub topic: Option<String>,
    pub world_readable: bool,
}

fn evc(events: &Vec<JsonValue>, t: &str, field: &str) -> String {
    events
        .iter()
        .find(|x| x["type"] == t)
        .and_then(|js| js["content"][field].as_str())
        .map(Into::into)
        .unwrap_or_default()
}

fn get_admins(stevents: &Vec<JsonValue>) -> HashMap<String, i32> {
    let mut admins = HashMap::new();

    let plevents = stevents
        .iter()
        .filter(|x| x["type"] == "m.room.power_levels");

    for ev in plevents {
        if let Some(users) = ev["content"]["users"].as_object() {
            for u in users.keys() {
                let level = users[u].as_i64().unwrap_or_default();
                admins.insert(u.to_string(), level as i32);
            }
        }
    }

    admins
}

fn calculate_room_name(events: &Vec<JsonValue>, userid: &str) -> Option<String> {
    // looking for "m.room.name" event
    if let Some(name) = events.iter().find(|x| x["type"] == "m.room.name") {
        if let Some(name) = name["content"]["name"].as_str() {
            if !name.to_string().is_empty() {
                return Some(name.to_string());
            }
        }
    }

    // looking for "m.room.canonical_alias" event
    if let Some(name) = events
        .iter()
        .find(|x| x["type"] == "m.room.canonical_alias")
    {
        if let Some(name) = name["content"]["alias"].as_str() {
            return Some(name.to_string());
        }
    }

    // we look for members that aren't me
    let filter = |x: &&JsonValue| {
        (x["type"] == "m.room.member"
            && ((x["content"]["membership"] == "join" && x["sender"] != userid)
                || (x["content"]["membership"] == "invite" && x["state_key"] != userid)))
    };
    let c = events.iter().filter(&filter);
    let members = events.iter().filter(&filter);
    let mut members2 = events.iter().filter(&filter);

    if c.count() == 0 {
        // we don't have information to calculate the name
        return None;
    }

    let m1 = match members2.next() {
        Some(m) => {
            let sender = m["sender"].as_str().unwrap_or("NONAMED");
            m["content"]["displayname"].as_str().unwrap_or(sender)
        }
        None => "",
    };
    let m2 = match members2.next() {
        Some(m) => {
            let sender = m["sender"].as_str().unwrap_or("NONAMED");
            m["content"]["displayname"].as_str().unwrap_or(sender)
        }
        None => "",
    };

    let name = match members.count() {
        0 => String::from("EMPTY ROOM"),
        1 => String::from(m1),
        2 => format!("{} and {}", m1, m2),
        _ => format!("{} and Others", m1),
    };

    Some(name)
}

fn parse_room_member(msg: &JsonValue) -> Option<Member> {
    let sender = msg["sender"].as_str().unwrap_or_default();

    let c = &msg["content"];

    let membership = c["membership"].as_str();
    if membership.is_none() || membership.unwrap() != "join" {
        return None;
    }

    let displayname = match c["displayname"].as_str() {
        None => None,
        Some(s) => Some(String::from(s)),
    };
    let avatar_url = match c["avatar_url"].as_str() {
        None => None,
        Some(s) => Some(String::from(s)),
    };

    Some(Member {
        uid: String::from(sender),
        alias: displayname,
        avatar: avatar_url,
    })
}
