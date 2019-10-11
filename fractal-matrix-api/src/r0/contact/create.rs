use crate::r0::AccessToken;
use crate::r0::ThreePIDCredentials;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::Serialize;
use std::ops::Not;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub three_pid_creds: ThreePIDCredentials,
    #[serde(skip_serializing_if = "Not::not")]
    pub bind: bool,
}

pub fn request(base: Url, params: &Parameters, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/account/3pid")
        .expect("Malformed URL in contact create");

    Client::new().post(url).query(params).json(body).build()
}
