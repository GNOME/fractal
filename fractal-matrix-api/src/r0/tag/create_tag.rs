use crate::r0::AccessToken;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use ruma_identifiers::{RoomId, UserId};
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    // TODO: Restrict values to the range [0.0, 1.0]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<f64>,
}

pub fn request(
    base: Url,
    user_id: &UserId,
    room_id: &RoomId,
    tag: &str,
    params: &Parameters,
    body: &Body,
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "_matrix/client/r0/user/{}/rooms/{}/tags/{}",
            user_id, room_id, tag
        ))
        .expect("Malformed URL in create_tag");

    Client::new().put(url).query(params).json(body).build()
}
