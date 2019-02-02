use crate::r0::filter::{serialize_filter_as_str, Filter};
use crate::ser::serialize_duration_as_millis;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Parameters<'a> {
    pub access_token: String,
    #[serde(serialize_with = "serialize_filter_as_str")]
    #[serde(skip_serializing_if = "Filter::is_default")]
    pub filter: Filter<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(flatten)]
    pub include_state: IncludeState,
    #[serde(skip_serializing_if = "MarkPresence::is_default")]
    pub set_presence: MarkPresence,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "full_state", content = "timeout")]
pub enum IncludeState {
    #[serde(rename = "false")]
    #[serde(serialize_with = "serialize_duration_as_millis")]
    Changed(Duration),
    #[serde(rename = "true")]
    Full,
}

impl Default for IncludeState {
    fn default() -> Self {
        IncludeState::Changed(Default::default())
    }
}

impl IncludeState {
    pub fn is_default(&self) -> bool {
        *self == Default::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkPresence {
    Offline,
    Unavailable,
    Online,
}

impl Default for MarkPresence {
    fn default() -> Self {
        MarkPresence::Online
    }
}

impl MarkPresence {
    pub fn is_default(&self) -> bool {
        *self == Default::default()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
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

pub fn request(base: Url, params: &Parameters) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/sync")
        .expect("Malformed URL in sync_events");

    Client::new().get(url).query(params).build()
}
