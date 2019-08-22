use serde_json::Value as JsonValue;

#[derive(Debug, Clone)]
pub struct Event {
    pub sender: String,
    pub stype: String,
    pub room: String,
    pub id: String,
    pub redacts: String,
    pub content: JsonValue,
}

impl PartialEq for Event {
    fn eq(&self, other: &Event) -> bool {
        self.id == other.id
    }
}
