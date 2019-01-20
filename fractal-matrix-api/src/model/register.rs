use crate::globals;
use serde::{Deserialize, Serialize};
use std::ops::Not;

#[derive(Clone, Debug, Serialize)]
pub struct LoginRequest {
    #[serde(flatten)]
    pub identifier: Identifier,
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

#[derive(Clone, Debug, Serialize)]
enum LegacyMedium {
    #[serde(rename = "email")]
    Email,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
enum LegacyIdentifier {
    User {
        user: String,
    },
    Email {
        medium: LegacyMedium,
        address: String,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct Identifier {
    identifier: UserIdentifier,
    #[serde(flatten)]
    legacy_identifier: Option<LegacyIdentifier>,
}

impl Identifier {
    pub fn new(identifier: UserIdentifier) -> Self {
        Self {
            identifier: identifier.clone(),
            legacy_identifier: match identifier {
                UserIdentifier::User { user } => Some(LegacyIdentifier::User { user: user }),
                UserIdentifier::ThirdParty { medium: _, address } => {
                    Some(LegacyIdentifier::Email {
                        medium: LegacyMedium::Email,
                        address,
                    })
                }
                UserIdentifier::Phone { .. } => None,
            },
        }
    }
}

impl LoginRequest {
    pub fn new(user: String, password: String) -> Self {
        if globals::EMAIL_RE.is_match(&user) {
            Self {
                auth: Auth::Password { password },
                initial_device_display_name: Some(String::from("Fractal")),
                identifier: Identifier::new(UserIdentifier::ThirdParty {
                    medium: Medium::Email,
                    address: user,
                }),
                device_id: None,
            }
        } else {
            Self {
                auth: Auth::Password { password },
                initial_device_display_name: Some(String::from("Fractal")),
                identifier: Identifier::new(UserIdentifier::User { user }),
                device_id: None,
            }
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
