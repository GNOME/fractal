use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Info {
    pub thumbnail_url: Option<Url>,
    pub thumbnail_info: Option<JsonValue>,
    pub w: Option<u32>,
    pub h: Option<u32>,
    pub size: u32,
    pub mimetype: String,
    pub orientation: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtraContent {
    pub info: Info,
}
