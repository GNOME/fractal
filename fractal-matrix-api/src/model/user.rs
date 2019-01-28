use super::{AuthenticationData, Medium, ThreePIDCredentials};
use serde::{Deserialize, Serialize};
use std::ops::Not;

#[derive(Clone, Debug, Deserialize)]
pub struct GetDisplayNameResponse {
    pub displayname: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PutDisplayNameRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displayname: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThirdPartyIDResponse {
    pub threepids: Vec<ThirdPartyIdentifier>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThirdPartyIdentifier {
    pub added_at: u64,
    pub medium: Medium,
    pub validated_at: u64,
    pub address: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct EmailTokenRequest {
    pub client_secret: String,
    pub email: String,
    pub id_server: String,
    pub send_attempt: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_link: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhoneTokenRequest {
    pub client_secret: String,
    pub phone_number: String,
    pub country: String,
    pub id_server: String,
    pub send_attempt: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_link: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ThirdPartyTokenResponse {
    pub sid: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct AddThreePIDRequest {
    pub three_pid_creds: ThreePIDCredentials,
    #[serde(skip_serializing_if = "Not::not")]
    pub bind: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SubmitPhoneTokenRequest {
    pub sid: String,
    pub client_secret: String,
    pub token: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SubmitPhoneTokenResponse {
    pub success: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeleteThreePIDRequest {
    pub medium: Medium,
    pub address: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChangePasswordRequest {
    pub new_password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthenticationData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeactivateAccountRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthenticationData>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchUserRequest {
    pub search_term: String,
    #[serde(skip_serializing_if = "u64_is_10")]
    pub limit: u64,
}

impl Default for SearchUserRequest {
    fn default() -> Self {
        Self {
            search_term: Default::default(),
            limit: 10,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SearchUserResponse {
    pub results: Vec<User>,
    pub limited: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct User {
    pub user_id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

fn u64_is_10(number: &u64) -> bool {
    number == &10
}
