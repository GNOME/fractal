pub mod account;
pub mod contact;
pub mod directory;
pub mod filter;
pub mod media;
pub mod profile;
pub mod search;
pub mod server;
pub mod sync;
pub mod thirdparty;

use serde::{Deserialize, Serialize, Serializer};
use std::convert::TryFrom;
use std::fmt::{self, Display, Formatter};
use url::Host;
use url::ParseError;
use url::Url;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "lowercase")]
pub enum Medium {
    Email,
    MsIsdn,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThreePIDCredentials {
    pub client_secret: String,
    pub id_server: HostAndPort<String>,
    pub sid: String,
}

#[derive(Clone, Debug)]
pub struct HostAndPort<T> {
    pub host: Host<T>,
    pub port: Option<u16>,
}

impl TryFrom<Url> for HostAndPort<String> {
    type Error = ParseError;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        Ok(Self {
            host: url
                .host()
                .ok_or(ParseError::SetHostOnCannotBeABaseUrl)?
                .to_owned(),
            port: url.port(),
        })
    }
}

impl<T: AsRef<str>> Display for HostAndPort<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if let Some(port) = self.port {
            write!(f, "{}:{}", self.host, port)
        } else {
            write!(f, "{}", self.host)
        }
    }
}

impl<T: AsRef<str>> Serialize for HostAndPort<T> {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ser.serialize_str(&self.to_string())
    }
}
