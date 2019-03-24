use super::{AuthenticationData, Identifier, Medium, UserIdentifier};
use crate::globals;
use serde::{de, Deserialize, Deserializer, Serialize};
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
#[serde(tag = "type")]
pub enum Auth {
    #[serde(rename = "m.login.password")]
    Password { password: String },
    #[serde(rename = "m.login.token")]
    Token { token: String },
}

impl LoginRequest {
    pub fn new(
        user: String,
        password: String,
        initial_device_display_name: Option<String>,
        device_id: Option<String>,
    ) -> Self {
        if globals::EMAIL_RE.is_match(&user) {
            Self {
                auth: Auth::Password { password },
                initial_device_display_name,
                identifier: Identifier::new(UserIdentifier::ThirdParty {
                    medium: Medium::Email,
                    address: user,
                }),
                device_id,
            }
        } else {
            Self {
                auth: Auth::Password { password },
                initial_device_display_name,
                identifier: Identifier::new(UserIdentifier::User { user }),
                device_id,
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

#[derive(Clone, Debug, Deserialize)]
#[serde(default, tag = "type")]
pub struct WellKnownResponse {
    #[serde(deserialize_with = "extract_base_url", rename = "m.homeserver")]
    pub homeserver: Option<String>,
    #[serde(deserialize_with = "extract_base_url", rename = "m.identity_server")]
    pub identity_server: Option<String>,
}

impl Default for WellKnownResponse {
    fn default() -> Self {
        // Identity server is usually vector.im if not specified
        Self {
            homeserver: None,
            identity_server: Some("https://vector.im".to_owned()),
        }
    }
}

fn extract_base_url<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct BaseUrlVisitor;

    impl<'de> de::Visitor<'de> for BaseUrlVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an object with a base_url key")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: de::MapAccess<'de>,
        {
            let res = map
                .next_entry::<String, String>()?
                .and_then(|(key, value)| if key == "base_url" { Some(value) } else { None });
            Ok(res)
        }
    }

    deserializer.deserialize_any(BaseUrlVisitor)
}
