use ruma_identifiers::{RoomId, UserId};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone)]
pub struct Event {
    pub sender: UserId,
    pub stype: String,
    pub room: RoomId,
    pub id: String,
    pub redacts: String,
    pub content: JsonValue,
}

impl PartialEq for Event {
    fn eq(&self, other: &Event) -> bool {
        self.id == other.id
    }
}
