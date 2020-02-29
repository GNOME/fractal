use crate::r0::AccessToken;
use crate::serde::url as serde_url;
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
    pub filename: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    #[serde(with = "serde_url")]
    pub content_uri: Url,
}

pub fn request(base: Url, params: &Parameters, contents: Vec<u8>) -> Result<Request, Error> {
    let (mime, _) = gio::content_type_guess(None, &contents);

    let url = base
        .join("_matrix/media/r0/upload")
        .expect("Malformed URL in upload");

    Client::new()
        .post(url)
        .query(params)
        .body(contents)
        .header(CONTENT_TYPE, mime.to_string())
        .build()
}
