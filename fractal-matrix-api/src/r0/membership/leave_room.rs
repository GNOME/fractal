use crate::r0::AccessToken;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::RoomId;
use serde::Serialize;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

pub fn request(base: Url, room_id: &RoomId, params: &Parameters) -> Result<Request, Error> {
    let url = base
        .join(&format!("/_matrix/client/r0/rooms/{}/leave", room_id))
        .expect("Malformed URL in leave_room");

    Client::new().post(url).query(params).build()
}
