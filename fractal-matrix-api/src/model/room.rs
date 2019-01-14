use serde_json::Value as JsonValue;

use crate::model::member::Member;
use crate::model::member::MemberList;
use crate::model::message::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    pub fav: bool,
    pub left: bool,
    pub inv: bool,
    pub direct: bool,
    pub prev_batch: Option<String>,
    pub inv_sender: Option<Member>,

    /// Hashmap with the room users power levels
    /// the key will be the userid and the value will be the level
    pub power_levels: HashMap<String, i32>,
}

impl Room {
    pub fn new(id: String, name: Option<String>) -> Room {
        Room {
            id,
            name,
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
            fav: false,
            left: false,
            inv: false,
            direct: false,
            inv_sender: None,
            power_levels: HashMap::new(),
            prev_batch: None,
        }
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
                if let Some(r) = msg.id.clone().and_then(|id| receipts.get(&id)) {
                    msg.set_receipt(r.clone());
                }
            }
        }
    }

    pub fn add_receipt_from_fully_read(&mut self, uid: &str, evid: &str) {
        for msg in self
            .messages
            .iter_mut()
            .filter(|m| m.id == Some(evid.to_string()))
        {
            msg.receipt.insert(uid.to_string(), 0);
        }
    }
}

impl From<PublicRoomsChunk> for Room {
    fn from(input: PublicRoomsChunk) -> Self {
        let mut room = Self::new(input.room_id, input.name);
        room.alias = input.canonical_alias;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<Filter>,
    #[serde(flatten)]
    pub third_party_networks: ThirdPartyNetworks,
}

#[derive(Clone, Debug, Serialize)]
pub struct Filter {
    pub generic_search_term: Option<String>,
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
