use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use serde::Serialize;
use url::{Host, Url};

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub width: u64,
    pub height: u64,
    pub method: Option<Method>,
    #[serde(skip_serializing_if = "bool::clone")]
    pub allow_remote: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Crop,
    Scale,
}

pub fn request(
    base: Url,
    params: &Parameters,
    server: &Host<String>,
    media_id: &str,
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "/_matrix/media/r0/thumbnail/{}/{}",
            server, media_id,
        ))
        .expect("Malformed URL in get_content_thumbnail");

    Client::new().get(url).query(params).build()
}
