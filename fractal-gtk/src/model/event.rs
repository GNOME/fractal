use fractal_api::identifiers::{EventId, RoomId, UserId};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone)]
pub struct Event {
    pub sender: UserId,
    pub stype: String,
    pub room: RoomId,
    pub id: String,
    pub redacts: Option<EventId>,
    pub content: JsonValue,
}

impl PartialEq for Event {
    fn eq(&self, other: &Event) -> bool {
        self.id == other.id
    }
}
