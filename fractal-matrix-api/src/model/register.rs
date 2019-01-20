use serde::{Deserialize, Serialize};

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
