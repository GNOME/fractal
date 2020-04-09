use crate::r0::AccessToken;
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

pub fn request(
    base: Url,
    params: &Parameters,
    user_id: &UserId,
    event_type: &str, // TODO: Use EventType
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "_matrix/client/r0/user/{}/account_data/{}",
            user_id, event_type,
        ))
        .expect("Malformed URL in get_global_account_data");

    Client::new().get(url).query(params).build()
}
