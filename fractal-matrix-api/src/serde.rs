pub mod url {
    use serde::de::{Error, Visitor};
    use serde::Deserializer;
    use serde::Serializer;
    use std::fmt::{self, Formatter};
    use url::Url;

    pub(super) struct UrlVisitor;

    impl<'de> Visitor<'de> for UrlVisitor {
        type Value = Url;

        fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
            write!(formatter, "a valid URL")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Url::parse(v).map_err(E::custom)
        }
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Url, D::Error>
    where
        D: Deserializer<'de>,
    {
        de.deserialize_str(UrlVisitor)
    }

    pub fn serialize<S>(url: &Url, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ser.serialize_str(url.as_str())
    }
}

pub mod option_url {
    use super::url as serde_url;
    use serde::de::{Error, Visitor};
    use serde::Deserializer;
    use serde::Serializer;
    use std::fmt::{self, Formatter};
    use url::Url;

    struct OptionUrlVisitor;

    impl<'de> Visitor<'de> for OptionUrlVisitor {
        type Value = Option<Url>;

        fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
            serde_url::UrlVisitor.expecting(formatter)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(None)
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, de: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            // serde_url::deserialize(de).map(Some)
            //
            // Using an alternative way because Synapse sends empty strings
            // and junk data as url (see #606 "relative URL without a base" error)

            Ok(serde_url::deserialize(de).ok())
        }
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Option<Url>, D::Error>
    where
        D: Deserializer<'de>,
    {
        de.deserialize_option(OptionUrlVisitor)
    }

    pub fn serialize<S>(url: &Option<Url>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match url {
            Some(u) => ser.serialize_str(u.as_str()),
            None => ser.serialize_none(),
        }
    }
}

pub mod option_host {
    use serde::Serializer;
    use url::Host;

    pub fn serialize<S>(host: &Option<Host>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match host {
            Some(h) => ser.serialize_str(&h.to_string()),
            None => ser.serialize_none(),
        }
    }
}

pub mod host_list {
    use serde::ser::Serializer;
    use url::Host;

    pub fn serialize<S>(host_list: &[Host], ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ser.collect_seq(host_list.iter().map(ToString::to_string))
    }
}
