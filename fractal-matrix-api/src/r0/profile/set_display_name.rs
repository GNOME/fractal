use crate::r0::AccessToken;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::Serialize;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub displayname: Option<String>,
}

pub fn request(
    base: Url,
    params: &Parameters,
    body: &Body,
    user_id: &str,
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "/_matrix/client/r0/profile/{}/displayname",
            user_id
        ))
        .expect("Malformed URL in set_display_name");

    Client::new().put(url).query(params).json(body).build()
}
