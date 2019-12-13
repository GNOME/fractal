use crate::r0::AccessToken;
use crate::serde::option_url;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use ruma_identifiers::RoomId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    #[serde(default)]
    pub joined: HashMap<String, RoomMember>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RoomMember {
    pub display_name: Option<String>,
    #[serde(with = "option_url")]
    pub avatar_url: Option<Url>,
}

pub fn request(base: Url, room_id: &RoomId, params: &Parameters) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "/_matrix/client/r0/rooms/{}/joined_members",
            room_id
        ))
        .expect("Malformed URL in get_joined_members");

    Client::new().get(url).query(params).build()
}
