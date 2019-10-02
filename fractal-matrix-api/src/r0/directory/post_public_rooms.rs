use crate::de::option_url;
use crate::ser::serialize_option_host;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use serde::{Deserialize, Serialize};
use url::Host;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: String,
    #[serde(serialize_with = "serialize_option_host")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<Host<String>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    // This field doesn't follow the spec but for some reason
    // it fails with matrix.org if it's not set this way
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<Filter>,
    #[serde(flatten)]
    pub third_party_networks: ThirdPartyNetworks,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "include_all_networks", content = "third_party_instance_id")]
pub enum ThirdPartyNetworks {
    #[serde(rename = "false")]
    None,
    #[serde(rename = "false")]
    Only(String),
    #[serde(rename = "true")]
    All,
}

impl Default for ThirdPartyNetworks {
    fn default() -> Self {
        ThirdPartyNetworks::None
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Filter {
    pub generic_search_term: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    pub chunk: Vec<Chunk>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
    pub total_room_count_estimate: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Chunk {
    pub aliases: Option<Vec<String>>,
    #[serde(deserialize_with = "option_url::deserialize")]
    #[serde(default)]
    pub avatar_url: Option<Url>,
    pub canonical_alias: Option<String>,
    pub guest_can_join: bool,
    pub name: Option<String>,
    pub num_joined_members: i32,
    pub room_id: String,
    pub topic: Option<String>,
    pub world_readable: bool,
}

pub fn request(base: Url, params: &Parameters, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("/_matrix/client/r0/publicRooms")
        .expect("Malformed URL in post_public_rooms");

    Client::new().post(url).query(params).json(body).build()
}
