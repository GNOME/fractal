use crate::r0::AccessToken;
use crate::serde::{option_host, option_url};
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::RoomAliasId;
use ruma_identifiers::RoomId;
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use url::Host;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
    #[serde(with = "option_host")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<Host<String>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Body {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<Filter>,
    #[serde(flatten)]
    pub third_party_networks: ThirdPartyNetworks,
}

#[derive(Clone, Debug)]
pub enum ThirdPartyNetworks {
    None,
    Only(String),
    All,
}

impl Serialize for ThirdPartyNetworks {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ThirdPartyNetworks::None => {
                let mut serialized_map = ser.serialize_map(Some(1))?;
                serialized_map.serialize_entry("include_all_networks", &false)?;
                serialized_map.end()
            }
            ThirdPartyNetworks::Only(network) => {
                let mut serialized_map = ser.serialize_map(Some(2))?;

                serialized_map.serialize_entry("include_all_networks", &false)?;
                serialized_map.serialize_entry("third_party_instance_id", &network)?;
                serialized_map.end()
            }
            ThirdPartyNetworks::All => {
                let mut serialized_map = ser.serialize_map(Some(1))?;
                serialized_map.serialize_entry("include_all_networks", &true)?;
                serialized_map.end()
            }
        }
    }
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
    pub aliases: Option<Vec<RoomAliasId>>, // TODO: Change Vec to Set?
    #[serde(with = "option_url")]
    #[serde(default)]
    pub avatar_url: Option<Url>,
    pub canonical_alias: Option<RoomAliasId>,
    pub guest_can_join: bool,
    pub name: Option<String>,
    pub num_joined_members: i32,
    pub room_id: RoomId,
    pub topic: Option<String>,
    pub world_readable: bool,
}

pub fn request(base: Url, params: &Parameters, body: &Body) -> Result<Request, Error> {
    let url = base
        .join("_matrix/client/r0/publicRooms")
        .expect("Malformed URL in post_public_rooms");

    Client::new().post(url).query(params).json(body).build()
}
