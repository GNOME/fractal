use crate::r0::AccessToken;
use reqwest::blocking::Client;
use reqwest::blocking::Request;
use reqwest::Error;
use ruma_identifiers::{RoomId, UserId};
use serde::ser::SerializeMap;
use serde::Serialize;
use serde::Serializer;
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct Parameters {
    pub access_token: AccessToken,
}

#[derive(Clone, Debug)]
pub enum Body {
    StopTyping,
    Typing(Duration),
}

impl Serialize for Body {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Body::StopTyping => {
                let mut serialized_map = ser.serialize_map(Some(1))?;
                serialized_map.serialize_entry("typing", &false)?;
                serialized_map.end()
            }
            Body::Typing(dur) => {
                let mut serialized_map = ser.serialize_map(Some(2))?;
                serialized_map.serialize_entry("typing", &true)?;
                serialized_map.serialize_entry("timeout", &dur.as_millis())?;
                serialized_map.end()
            }
        }
    }
}

pub fn request(
    base: Url,
    room_id: &RoomId,
    user_id: &UserId,
    params: &Parameters,
    body: &Body,
) -> Result<Request, Error> {
    let url = base
        .join(&format!(
            "/_matrix/client/r0/rooms/{}/typing/{}",
            room_id, user_id,
        ))
        .expect("Malformed URL in typing");

    Client::new().put(url).query(params).json(body).build()
}
