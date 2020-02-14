use crate::r0::AccessToken;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::RoomId;
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

pub fn request(
    base: Url,
    user_id: &str,
    room_id: &RoomId,
    tag: &str,
    params: &Parameters,
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "/_matrix/client/r0/user/{}/rooms/{}/tags/{}",
            user_id, room_id, tag
        ))
        .expect("Malformed URL in delete_tag");

    Client::new().delete(url).query(params).build()
}
