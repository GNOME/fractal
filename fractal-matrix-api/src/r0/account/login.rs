use super::Identifier;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    #[serde(flatten)]
    pub identifier: Identifier,
    #[serde(flatten)]
    pub auth: Auth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_device_display_name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum Auth {
    #[serde(rename = "m.login.password")]
    Password { password: String },
    #[serde(rename = "m.login.token")]
    Token { token: String },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub access_token: Option<String>,
    pub user_id: Option<String>,
    pub device_id: Option<String>,
}

pub fn request(base: Url, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/login")
        .expect("Malformed URL in login");

    Client::new().post(url).json(body).build()
}