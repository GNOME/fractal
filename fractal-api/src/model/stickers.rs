extern crate serde_json;
use self::serde_json::Value as JsonValue;


#[derive(Debug)]
#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct Sticker {
    pub name: String,
    pub description: String,
    pub body: String,
    pub thumbnail: String,
    pub url: String,
    pub size: (i32, i32),
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Serialize, Deserialize)]
pub struct StickerGroup {
    pub name: String,
    pub description: String,
    pub price: i64,
    pub purchased: bool,
    pub thumbnail: String,
    pub stickers: Vec<Sticker>,
}

impl StickerGroup {
    pub fn from_json(js: &JsonValue) -> StickerGroup {
        let mut stickers = vec![];
        let d = &js["data"];

        let purchased = js["purchased"].as_bool().unwrap_or_default();
        let name = d["name"].as_str().unwrap_or_default().to_string();
        let description = d["description"].as_str().unwrap_or_default().to_string();
        let price = d["price"].as_i64().unwrap_or_default();
        let thumbnail = d["thumbnail"].as_str().unwrap_or_default().to_string();

        for img in d["images"].as_array().unwrap_or(&vec![]).iter() {
            let c = &img["content"];
            let w = c["info"]["h"].as_i64().unwrap_or_default();
            let h = c["info"]["h"].as_i64().unwrap_or_default();
            stickers.push(Sticker {
                name: img["name"].as_str().unwrap_or_default().to_string(),
                description: img["description"].as_str().unwrap_or_default().to_string(),
                body: c["body"].as_str().unwrap_or_default().to_string(),
                url: c["url"].as_str().unwrap_or_default().to_string(),
                thumbnail: c["info"]["thumbnail_url"].as_str().unwrap_or_default().to_string(),
                size: (w as i32, h as i32),
            });
        }

        StickerGroup {
            name,
            description,
            price,
            purchased,
            thumbnail,
            stickers,
        }
    }
}
