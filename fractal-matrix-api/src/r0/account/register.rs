use super::AuthenticationData;
use crate::r0::AccessToken;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use ruma_identifiers::DeviceId;
use serde::{Deserialize, Serialize};
use std::ops::Not;
use url::Url;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Parameters {
    #[serde(skip_serializing_if = "RegistrationKind::is_default")]
    pub kind: RegistrationKind,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum RegistrationKind {
    #[serde(rename = "guest")]
    Guest,
    #[serde(rename = "user")]
    User,
}

impl Default for RegistrationKind {
    fn default() -> Self {
        RegistrationKind::User
    }
}

impl RegistrationKind {
    pub fn is_default(&self) -> bool {
        *self == Default::default()
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Body {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthenticationData>,
    #[serde(skip_serializing_if = "Not::not")]
    pub bind_email: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<DeviceId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_device_display_name: Option<String>,
    #[serde(skip_serializing_if = "Not::not")]
    pub inhibit_login: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub user_id: String,
    pub access_token: Option<AccessToken>,
    pub device_id: Option<DeviceId>,
}

pub fn request(base: Url, params: &Parameters, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/register")
        .expect("Malformed URL in register");

    Client::new().post(url).query(params).json(body).build()
}
