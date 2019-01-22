use serde::Serialize;
use std::ops::Not;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Filter<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_fields: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_format: Option<EventFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence: Option<EventFilter<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_data: Option<EventFilter<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<RoomFilter<'a>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EventFormat {
    Client,
    Federation,
}

impl Default for EventFormat {
    fn default() -> Self {
        EventFormat::Client
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct EventFilter<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_senders: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_types: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub senders: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<&'a str>>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RoomFilter<'a> {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_rooms: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rooms: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral: Option<RoomEventFilter<'a>>,
    #[serde(skip_serializing_if = "Not::not")]
    pub include_leave: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<RoomEventFilter<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<RoomEventFilter<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_data: Option<RoomEventFilter<'a>>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RoomEventFilter<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_senders: Vec<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_types: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub senders: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_rooms: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rooms: Option<Vec<&'a str>>,
    #[serde(skip_serializing_if = "Not::not")]
    pub contains_url: bool,
}
