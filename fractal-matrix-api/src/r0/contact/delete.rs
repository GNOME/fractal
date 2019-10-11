use crate::r0::AccessToken;
use crate::r0::Medium;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub address: String,
    pub medium: Medium,
}

pub fn request(base: Url, params: &Parameters, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/account/3pid/delete")
        .expect("Malformed URL in contact delete");

    Client::new().post(url).query(params).json(body).build()
}
