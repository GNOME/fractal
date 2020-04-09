use crate::serde::option_url;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::UserId;
use serde::Deserialize;
use url::Url;

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    #[serde(with = "option_url")]
    #[serde(default)]
    pub avatar_url: Option<Url>,
    pub displayname: Option<String>,
}

pub fn request(base: Url, user_id: &UserId) -> Result<Request, Error> {
    let url = base
        .join(&format!("_matrix/client/r0/profile/{}", user_id))
        .expect("Malformed URL in get_profile_avatar");

    Client::new().get(url).build()
}
