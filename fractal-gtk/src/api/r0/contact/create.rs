use crate::api::r0::AccessToken;
use crate::api::r0::ThreePIDCredentials;
use matrix_sdk::reqwest::Client;
use matrix_sdk::reqwest::Error;
use matrix_sdk::reqwest::Request;
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
        .join("_matrix/client/r0/account/3pid")
        .expect("Malformed URL in contact create");

    let data = serde_json::to_vec(body).unwrap();

    Client::new().post(url).query(params).body(data).build()
}
