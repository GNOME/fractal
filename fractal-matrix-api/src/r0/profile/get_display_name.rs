use crate::r0::AccessToken;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::UserId;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub displayname: Option<String>,
}

pub fn request(base: Url, params: &Parameters, user_id: &UserId) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "_matrix/client/r0/profile/{}/displayname",
            user_id
        ))
        .expect("Malformed URL in get_display_name");

    Client::new().get(url).query(params).build()
}
