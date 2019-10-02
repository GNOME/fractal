use serde::Serializer;
use std::time::Duration;
use url::Host;
use url::Url;

pub fn serialize_option_host<S>(host: &Option<Host>, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match host {
        Some(h) => ser.serialize_str(&h.to_string()),
        None => ser.serialize_none(),
    }
}

// TODO: use as_millis when duration_as_u128 is stable
pub fn serialize_duration_as_millis<S>(duration: &Duration, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    ser.serialize_u64(duration.as_secs() * 1000 + (duration.subsec_millis() as u64))
}

pub fn serialize_url<S>(url: &Url, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    ser.serialize_str(url.as_str())
}

pub fn serialize_option_url<S>(url: &Option<Url>, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match url {
        Some(u) => ser.serialize_str(u.as_str()),
        None => ser.serialize_none(),
    }
}
