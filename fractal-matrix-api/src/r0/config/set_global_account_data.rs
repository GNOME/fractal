use crate::r0::AccessToken;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::UserId;
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
    user_id: &UserId,
    event_type: &str, // TODO: Use EventType
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "_matrix/client/r0/user/{}/account_data/{}",
            user_id, event_type,
        ))
        .expect("Malformed URL in set_global_account_data");

    Client::new().put(url).query(params).json(body).build()
}
