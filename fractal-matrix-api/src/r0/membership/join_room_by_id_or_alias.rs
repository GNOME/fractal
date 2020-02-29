use crate::r0::AccessToken;
use crate::serde::host_list;
use reqwest::Client;
use reqwest::Error;
use reqwest::Request;
use ruma_identifiers::RoomIdOrAliasId;
use serde::Serialize;
use url::Host;
use url::Url;

#[derive(Clone, Debug, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
    #[serde(with = "host_list")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub server_name: Vec<Host>,
}

// TODO: Implement Body

pub fn request(
    base: Url,
    room_id_or_alias: &RoomIdOrAliasId,
    params: &Parameters,
) -> Result<Request, Error> {
    let url = base
        .join(&format!("_matrix/client/r0/join/{}", room_id_or_alias))
        .expect("Malformed URL in join_room_by_id_or_alias");

    Client::new().post(url).query(params).build()
}
