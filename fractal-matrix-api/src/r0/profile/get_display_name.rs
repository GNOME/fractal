use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use serde::Deserialize;
use url::Url;

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub displayname: Option<String>,
}

pub fn request(base: Url, user_id: &str) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "/_matrix/client/r0/profile/{}/displayname",
            user_id
        ))
        .expect("Malformed URL in get_display_name");

    Client::new().get(url).build()
}
