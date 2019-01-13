use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

pub type SupportedProtocols = BTreeMap<String, Protocol>;

#[derive(Debug, Clone, Deserialize)]
pub struct Protocol {
    pub user_fields: Vec<String>,
    pub location_fields: Vec<String>,
    // This field is documented as "required",
    // but for some reason matrix.org does not send this
    pub icon: Option<String>,
    pub field_types: BTreeMap<String, FieldType>,
    pub instances: Vec<ProtocolInstance>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldType {
    pub regexp: String,
    pub placeholder: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolInstance {
    #[serde(rename = "network_id")]
    pub id: String,
    pub desc: String,
    pub icon: Option<String>,
    pub fields: JsonValue,
}
