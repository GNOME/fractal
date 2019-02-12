use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Serialize, Deserialize)]
pub struct Info {
    pub thumbnail_url: Option<String>,
    pub thumbnail_info: Option<JsonValue>,
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub size: Option<u32>,
    pub mimetype: String,
    pub orientation: Option<i32>,
}
