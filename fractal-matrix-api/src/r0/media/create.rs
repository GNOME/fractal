use crate::serde::url as serde_url;
use reqwest::header::{HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: String,
    pub filename: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    #[serde(with = "serde_url")]
    pub content_uri: Url,
}

pub fn request(
    base: Url,
    params: &Parameters,
    file: Vec<u8>,
    content_type: Option<HeaderValue>,
) -> Result<Request, Error> {
    let header = content_type
        .map(|mime| (CONTENT_TYPE, mime))
        .into_iter()
        .collect();

    let url = base
        .join("/_matrix/media/r0/upload")
        .expect("Malformed URL in upload");

    Client::new()
        .post(url)
        .query(params)
        .body(file)
        .headers(header)
        .build()
}
