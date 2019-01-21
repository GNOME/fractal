use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize)]
pub struct SyncResponse {
    pub next_batch: String,
    #[serde(default)]
    pub rooms: Rooms,
    pub presence: Option<Presence>,
    #[serde(default)]
    pub account_data: AccountData,
    pub to_device: Option<ToDevice>,
    pub device_lists: Option<DeviceLists>,
    #[serde(default)]
    pub device_one_time_keys_count: HashMap<String, u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Rooms {
    #[serde(default)]
    pub leave: HashMap<String, LeftRoom>,
    #[serde(default)]
    pub join: HashMap<String, JoinedRoom>,
    #[serde(default)]
    pub invite: HashMap<String, InvitedRoom>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JoinedRoom {
    #[serde(default)]
    pub unread_notifications: UnreadNotificationsCount,
    #[serde(default)]
    pub timeline: Timeline,
    #[serde(default)]
    pub state: State,
    #[serde(default)]
    pub account_data: AccountData,
    #[serde(default)]
    pub ephemeral: Ephemeral,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Ephemeral {
    // TODO: Implement Event
    #[serde(default)]
    pub events: Vec<JsonValue>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct UnreadNotificationsCount {
    #[serde(default)]
    pub highlight_count: i32,
    #[serde(default)]
    pub notification_count: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct InvitedRoom {
    #[serde(default)]
    pub invite_state: InviteState,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct InviteState {
    // TODO: Implement StrippedState
    #[serde(default)]
    pub events: Vec<JsonValue>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LeftRoom {
    #[serde(default)]
    pub timeline: Timeline,
    #[serde(default)]
    pub state: State,
    #[serde(default)]
    pub account_data: AccountData,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct State {
    // TODO: Implement StateEvent
    #[serde(default)]
    pub events: Vec<JsonValue>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Timeline {
    #[serde(default)]
    pub limited: bool,
    pub prev_batch: Option<String>,
    // TODO: Implement RoomEvent
    #[serde(default)]
    pub events: Vec<JsonValue>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Presence {
    // TODO: Implement Event
    #[serde(default)]
    pub events: Vec<JsonValue>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AccountData {
    // TODO: Implement Event
    #[serde(default)]
    pub events: Vec<JsonValue>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToDevice {
    // TODO: Implement Event
    #[serde(default)]
    pub events: Vec<JsonValue>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DeviceLists {
    #[serde(default)]
    pub changed: Vec<String>,
    #[serde(default)]
    pub left: Vec<String>,
}
