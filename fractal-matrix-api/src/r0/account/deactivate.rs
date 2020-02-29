use super::AuthenticationData;
use crate::r0::AccessToken;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use serde::Serialize;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthenticationData>,
}

pub fn request(base: Url, params: &Parameters, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("_matrix/client/r0/account/deactivate")
        .expect("Malformed URL in deactivate");

    Client::new().post(url).query(params).json(body).build()
}
