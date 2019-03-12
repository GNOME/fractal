use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::Deserialize;
use url::Url;

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub avatar_url: Option<String>,
    pub displayname: Option<String>,
}

pub fn request(base: Url, user_id: &str) -> Result<Request, Error> {
    let url = base
        .join(&format!("/_matrix/client/r0/profile/{}", user_id))
        .expect("Malformed URL in get_profile_avatar");

    Client::new().get(url).build()
}
