use serde::{Deserialize, Serialize};
use std::ops::Not;

#[derive(Clone, Debug, Default, Serialize)]
pub struct LoginRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<Medium>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    pub identifier: UserIdentifier,
    #[serde(flatten)]
    pub auth: Auth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_device_display_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoginResponse {
    pub access_token: Option<String>,
    pub home_server: Option<String>,
    pub user_id: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub enum Medium {
    #[serde(rename = "email")]
    Email,
    #[serde(rename = "msisdn")]
    MsIsdn,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum Auth {
    #[serde(rename = "m.login.password")]
    Password { password: String },
    #[serde(rename = "m.login.token")]
    Token { token: String },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum UserIdentifier {
    #[serde(rename = "m.id.user")]
    User { user: String },
    #[serde(rename = "m.id.thirdparty")]
    ThirdParty { medium: Medium, address: String },
    #[serde(rename = "m.id.phone")]
    Phone { country: String, phone: String },
}

impl Default for Auth {
    fn default() -> Self {
        Auth::Password {
            password: Default::default(),
        }
    }
}

impl Default for UserIdentifier {
    fn default() -> Self {
        UserIdentifier::User {
            user: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct RegisterRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthenticationData>,
    #[serde(skip_serializing_if = "Not::not")]
    pub bind_email: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_device_display_name: Option<String>,
    #[serde(skip_serializing_if = "Not::not")]
    pub inhibit_login: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegisterResponse {
    pub user_id: String,
    pub home_server: Option<String>,
    pub access_token: Option<String>,
    pub device_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthenticationData {
    #[serde(rename = "type")]
    pub kind: AuthenticationKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub enum AuthenticationKind {
    #[serde(rename = "m.login.password")]
    Password,
    #[serde(rename = "m.login.recaptcha")]
    Recaptcha,
    #[serde(rename = "m.login.oauth2")]
    OAuth2,
    #[serde(rename = "m.login.email.identity")]
    Email,
    #[serde(rename = "m.login.token")]
    Token,
    #[serde(rename = "m.login.dummy")]
    Dummy,
}
