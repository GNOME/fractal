use crate::r0::AccessToken;
use crate::r0::Medium;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Response {
    #[serde(default)]
    pub threepids: Vec<ThirdPartyIdentifier>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThirdPartyIdentifier {
    pub added_at: u64,
    pub medium: Medium,
    pub validated_at: u64,
    pub address: String,
}

pub fn request(base: Url, params: &Parameters) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/account/3pid")
        .expect("Malformed URL in get_identifiers");

    Client::new().get(url).query(params).build()
}
