pub mod account;
pub mod contact;
pub mod directory;
pub mod filter;
pub mod media;
pub mod profile;
pub mod search;
pub mod sync;
pub mod thirdparty;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "lowercase")]
pub enum Medium {
    Email,
    MsIsdn,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThreePIDCredentials {
    pub client_secret: String,
    pub id_server: String,
    pub sid: String,
}
