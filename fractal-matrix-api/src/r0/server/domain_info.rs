use crate::de::url as serde_url;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::Deserialize;
use url::Url;

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    #[serde(rename = "m.homeserver")]
    pub homeserver: HomeserverInfo,
    #[serde(rename = "m.identity_server")]
    pub identity_server: Option<IDServerInfo>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HomeserverInfo {
    #[serde(deserialize_with = "serde_url::deserialize")]
    pub base_url: Url,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IDServerInfo {
    #[serde(deserialize_with = "serde_url::deserialize")]
    pub base_url: Url,
}

pub fn request(base: Url) -> Result<Request, Error> {
    let url = base
        .join("/.well-known/matrix/client")
        .expect("Malformed URL in domain_info");

    Client::new().post(url).build()
}
