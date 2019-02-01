use serde::Serializer;
use url::Host;

pub fn serialize_option_host<S>(host: &Option<Host>, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match host {
        Some(h) => ser.serialize_str(&h.to_string()),
        None => ser.serialize_none(),
    }
}
