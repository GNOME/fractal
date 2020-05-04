use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
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
    pub base_url: Url,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IDServerInfo {
    pub base_url: Url,
}

pub fn request(base: Url) -> Result<Request, Error> {
    let url = base
        .join(".well-known/matrix/client")
        .expect("Malformed URL in domain_info");

    Client::new().get(url).build()
}
