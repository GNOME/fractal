use crate::JsonValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sticker {
    pub name: String,
    pub description: String,
    pub body: String,
    pub thumbnail: String,
    pub url: String,
    pub size: (i32, i32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerGroup {
    pub name: String,
    pub asset: String,
    pub description: String,
    pub price: i64,
    pub purchased: bool,
    pub thumbnail: String,
    pub stickers: Vec<Sticker>,
}

impl StickerGroup {
    pub fn from_json(js: &JsonValue) -> Self {
        let d = &js["data"];

        Self {
            name: d["name"].as_str().unwrap_or_default().to_string(),
            asset: js["asset_type"].as_str().unwrap_or_default().to_string(),
            description: d["description"].as_str().unwrap_or_default().to_string(),
            price: d["price"].as_i64().unwrap_or_default(),
            purchased: js["purchased"].as_bool().unwrap_or_default(),
            thumbnail: d["thumbnail"].as_str().unwrap_or_default().to_string(),
            stickers: d["images"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|img| {
                    let c = &img["content"];
                    let w = c["info"]["h"].as_i64().unwrap_or_default() as i32;
                    let h = c["info"]["h"].as_i64().unwrap_or_default() as i32;
                    Sticker {
                        name: img["name"].as_str().unwrap_or_default().to_string(),
                        description: img["description"].as_str().unwrap_or_default().to_string(),
                        body: c["body"].as_str().unwrap_or_default().to_string(),
                        url: c["url"].as_str().unwrap_or_default().to_string(),
                        thumbnail: c["info"]["thumbnail_url"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                        size: (w, h),
                    }
                })
                .collect(),
        }
    }
}
