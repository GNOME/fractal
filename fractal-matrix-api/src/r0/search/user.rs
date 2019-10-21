use crate::serde::option_url;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: String,
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
    pub user_id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(with = "option_url")]
    #[serde(default)]
    pub avatar_url: Option<Url>,
}

fn u64_is_10(number: &u64) -> bool {
    number == &10
}

pub fn request(base: Url, params: &Parameters, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/user_directory/search")
        .expect("Malformed URL in user_directory");

    Client::new().post(url).query(params).json(body).build()
}
