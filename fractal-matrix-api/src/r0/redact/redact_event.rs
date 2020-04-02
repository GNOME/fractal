use crate::r0::AccessToken;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::RoomId;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub event_id: Option<String>,
}

pub fn request(
    base: Url,
    params: &Parameters,
    body: &Body,
    room_id: &RoomId,
    event_type: &str, // TODO: Use EventType
    txn_id: &str,
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "/_matrix/client/r0/rooms/{}/redact/{}/{}",
            room_id, event_type, txn_id,
        ))
        .expect("Malformed URL in redact_event");

    Client::new().put(url).query(params).json(body).build()
}
