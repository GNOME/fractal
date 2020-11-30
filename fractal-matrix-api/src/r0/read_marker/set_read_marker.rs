use crate::r0::AccessToken;
use matrix_sdk::identifiers::{EventId, RoomId};
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use serde::Serialize;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    #[serde(rename = "m.fully_read")]
    pub fully_read: EventId,
    #[serde(rename = "m.read")]
    pub read: Option<EventId>,
}

pub fn request(
    base: Url,
    params: &Parameters,
    body: &Body,
    room_id: &RoomId,
) -> Result<Request, Error> {
    let url = base
        .join(&format!("_matrix/client/r0/rooms/{}/read_markers", room_id))
        .expect("Malformed URL in set_read_marker");

    Client::new().post(url).query(params).json(body).build()
}
