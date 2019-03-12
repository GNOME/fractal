use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub sid: String,
    pub client_secret: String,
    pub token: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub success: bool,
}

pub fn request(base: Url, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/identity/api/v1/validate/msisdn/submitToken")
        .expect("Malformed URL in msisdn submit_token");

    Client::new().post(url).json(body).build()
}
