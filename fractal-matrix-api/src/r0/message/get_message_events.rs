use crate::r0::filter::serialize_room_event_filter_as_str;
use crate::r0::filter::RoomEventFilter;
use crate::r0::u64_is_10;
use crate::r0::AccessToken;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::RoomId;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters<'a> {
    pub access_token: AccessToken,
    pub from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub dir: Direction,
    #[serde(skip_serializing_if = "u64_is_10")]
    pub limit: u64,
    #[serde(serialize_with = "serialize_room_event_filter_as_str")]
    #[serde(skip_serializing_if = "RoomEventFilter::is_default")]
    pub filter: RoomEventFilter<'a>,
}

#[derive(Clone, Debug, Serialize)]
pub enum Direction {
    #[serde(rename = "b")]
    Backward,
    #[serde(rename = "f")]
    Forward,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub start: Option<String>,
    pub end: Option<String>,
    #[serde(default)]
    pub chunk: Vec<JsonValue>,
    #[serde(default)]
    pub state: Vec<JsonValue>,
}

pub fn request(base: Url, params: &Parameters, room_id: &RoomId) -> Result<Request, Error> {
    let url = base
        .join(&format!("/_matrix/client/r0/rooms/{}/messages", room_id))
        .expect("Malformed URL in user_directory");

    Client::new().get(url).query(params).build()
}
