use crate::r0::AccessToken;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::RoomId;
use serde::Serialize;
use serde_json::Value as JsonValue;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

pub fn request(
    base: Url,
    params: &Parameters,
    body: &JsonValue,
    room_id: &RoomId,
    // event_type: &EventType,  TODO: Use this parameter
    state_keys: &str,
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "_matrix/client/r0/rooms/{}/state/{}/",
            room_id, state_keys,
        ))
        .expect("Malformed URL in get_state_events_for_key");

    Client::new().put(url).query(params).json(body).build()
}
