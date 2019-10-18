use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub client_secret: String,
    pub email: String,
    pub id_server: String,
    pub send_attempt: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_link: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(untagged)]
pub enum Response {
    Passed(InfoPassed),
    Failed(InfoFailed),
}

#[derive(Clone, Debug, Deserialize)]
pub struct InfoPassed {
    pub sid: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct InfoFailed {
    pub errcode: String,
}

pub fn request(base: Url, params: &Parameters, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/account/3pid/email/requestToken")
        .expect("Malformed URL in request_verification_token_email");

    Client::new().post(url).query(params).json(body).build()
}
