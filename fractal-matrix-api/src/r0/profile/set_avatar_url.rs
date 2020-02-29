use crate::r0::AccessToken;
use crate::serde::option_url;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::UserId;
use serde::Serialize;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    #[serde(with = "option_url")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<Url>,
}

pub fn request(
    base: Url,
    params: &Parameters,
    body: &Body,
    user_id: &UserId,
) -> Result<Request, Error> {
    let url = base
        .join(&format!("_matrix/client/r0/profile/{}/avatar_url", user_id))
        .expect("Malformed URL in set_avatar_url");

    Client::new().put(url).query(params).json(body).build()
}
