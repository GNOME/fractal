use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Info {
    pub thumbnail_url: Option<String>,
    pub thumbnail_info: Option<JsonValue>,
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub size: u32,
    pub mimetype: String,
    pub orientation: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ExtraContent {
    pub info: Info,
}
