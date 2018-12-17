use serde_json::Value as JsonValue;

use model::member::Member;
use model::member::MemberList;
use model::message::Message;
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
    pub fn new(id: String, name: Option<String>) -> Self {
        Self {
            id: id,
            name: name,
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
            .iter()
            .flat_map(|x| x.iter())
            .filter_map(|(mid, obj)| {
                let receipts = obj["m.read"].as_object().map(|read| {
                    read.iter()
                        .map(|(uid, ts)| (uid.to_string(), ts["ts"].as_i64().unwrap()))
                        .collect()
                })?;
                Some((mid.to_string(), receipts))
            })
            .collect::<HashMap<String, HashMap<String, i64>>>();

        self.messages.iter_mut().for_each(|msg| {
            msg.id
                .clone()
                .and_then(|id| receipts.get(&id))
                .map(|r| msg.set_receipt(r.clone()));
        });
    }

    pub fn add_receipt_from_fully_read(&mut self, uid: &str, evid: &str) {
        self.messages
            .iter_mut()
            .filter(|m| m.id == Some(evid.to_string()))
            .for_each(|msg| {
                msg.receipt.insert(uid.to_string(), 0);
            })
    }
}

impl PartialEq for Room {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

pub type RoomList = HashMap<String, Room>;
