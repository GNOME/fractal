use crate::r0::u64_is_10;
use crate::r0::AccessToken;
use matrix_sdk::identifiers::UserId;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub search_term: String,
    #[serde(skip_serializing_if = "u64_is_10")]
    pub limit: u64,
}

impl Default for Body {
    fn default() -> Self {
        Self {
            search_term: Default::default(),
            limit: 10,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub results: Vec<User>,
    pub limited: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct User {
    pub user_id: UserId,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

pub fn request(base: Url, params: &Parameters, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("_matrix/client/r0/user_directory/search")
        .expect("Malformed URL in user_directory");

    Client::new().post(url).query(params).json(body).build()
}
