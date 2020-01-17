use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use ruma_identifiers::UserId;
use serde::Deserialize;
use url::Url;

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub displayname: Option<String>,
}

pub fn request(base: Url, user_id: &UserId) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "/_matrix/client/r0/profile/{}/displayname",
            user_id
        ))
        .expect("Malformed URL in get_display_name");

    Client::new().get(url).build()
}
