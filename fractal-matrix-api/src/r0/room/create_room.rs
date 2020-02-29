use super::Visibility;
use crate::r0::{AccessToken, HostAndPort, Medium};
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::{RoomId, UserId};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::ops::Not;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Body {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_alias_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub invite: Vec<UserId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub invite3pid: Vec<InviteThreePID>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_content: Option<JsonValue>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub initial_state: Vec<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<RoomPreset>,
    #[serde(skip_serializing_if = "Not::not")]
    pub is_direct: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_level_content_override: Option<JsonValue>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InviteThreePID {
    pub id_server: HostAndPort<String>,
    pub id_access_token: AccessToken,
    pub medium: Medium, // TODO: Use enum
    pub address: String,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomPreset {
    PrivateChat,
    PublicChat,
    TrustedPrivateChat,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub room_id: RoomId,
}

pub fn request(base: Url, params: &Parameters, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("_matrix/client/r0/createRoom")
        .expect("Malformed URL in create_room");

    Client::new().post(url).query(params).json(body).build()
}
