use crate::de::option_url;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct Parameters {
    pub access_token: String,
}

pub type Response = BTreeMap<String, Protocol>;

#[derive(Debug, Clone, Deserialize)]
pub struct Protocol {
    pub user_fields: Vec<String>,
    pub location_fields: Vec<String>,
    // This field is documented as "required",
    // but for some reason matrix.org does not send this
    #[serde(deserialize_with = "option_url::deserialize")]
    #[serde(default)]
    pub icon: Option<Url>,
    pub field_types: BTreeMap<String, FieldType>,
    pub instances: Vec<ProtocolInstance>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldType {
    pub regexp: String, // TODO: Change type to Regex
    pub placeholder: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolInstance {
    // TODO: Avoid this rename
    #[serde(rename = "network_id")]
    pub id: String,
    pub desc: String,
    #[serde(deserialize_with = "option_url::deserialize")]
    #[serde(default)]
    pub icon: Option<Url>,
    pub fields: JsonValue,
}

pub fn request(base: Url, params: &Parameters) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/thirdparty/protocols")
        .expect("Wrong URL in get_supported_protocols");

    Client::new().get(url).query(params).build()
}
