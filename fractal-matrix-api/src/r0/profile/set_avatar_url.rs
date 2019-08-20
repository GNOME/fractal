use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::Serialize;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub avatar_url: Option<String>,
}

pub fn request(
    base: Url,
    params: &Parameters,
    body: &Body,
    user_id: &str,
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "/_matrix/client/r0/profile/{}/avatar_url",
            user_id
        ))
        .expect("Malformed URL in set_avatar_url");

    Client::new().put(url).query(params).json(body).build()
}
