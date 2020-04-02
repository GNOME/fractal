use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use serde::Serialize;
use url::{Host, Url};

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    #[serde(skip_serializing_if = "bool::clone")]
    pub allow_remote: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Self { allow_remote: true }
    }
}

pub fn request(
    base: Url,
    params: &Parameters,
    server: &Host<String>,
    media_id: &str,
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "/_matrix/media/r0/download/{}/{}",
            server, media_id,
        ))
        .expect("Malformed URL in get_content");

    Client::new().get(url).query(params).build()
}
