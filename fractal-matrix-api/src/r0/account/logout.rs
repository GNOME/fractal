use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::Serialize;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: String,
}

pub fn request(base: Url, params: &Parameters) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/logout")
        .expect("Malformed URL in logout");

    Client::new().post(url).query(params).build()
}
