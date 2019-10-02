pub mod change_password;
pub mod deactivate;
pub mod login;
pub mod logout;
pub mod register;

use crate::r0::{Medium, ThreePIDCredentials};
use crate::ser::serialize_url;
use serde::Serialize;
use url::Url;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum UserIdentifier {
    #[serde(rename = "m.id.user")]
    User { user: String },
    #[serde(rename = "m.id.thirdparty")]
    ThirdParty { medium: Medium, address: String },
    #[serde(rename = "m.id.phone")]
    Phone { country: String, phone: String },
}

#[derive(Clone, Debug, Serialize)]
enum LegacyMedium {
    #[serde(rename = "email")]
    Email,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
enum LegacyIdentifier {
    User {
        user: String,
    },
    Email {
        medium: LegacyMedium,
        address: String,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct Identifier {
    identifier: UserIdentifier,
    #[serde(flatten)]
    legacy_identifier: Option<LegacyIdentifier>,
}

impl Identifier {
    pub fn new(identifier: UserIdentifier) -> Self {
        Self {
            identifier: identifier.clone(),
            legacy_identifier: match identifier {
                UserIdentifier::User { user } => Some(LegacyIdentifier::User { user }),
                UserIdentifier::ThirdParty { medium: _, address } => {
                    Some(LegacyIdentifier::Email {
                        medium: LegacyMedium::Email,
                        address,
                    })
                }
                UserIdentifier::Phone { .. } => None,
            },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum AuthenticationData {
    #[serde(rename = "m.login.password")]
    Password {
        #[serde(flatten)]
        identifier: Identifier,
        password: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },
    #[serde(rename = "m.login.recaptcha")]
    Recaptcha {
        response: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },
    #[serde(rename = "m.login.token")]
    Token {
        token: String,
        txn_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },
    #[serde(rename = "m.login.oauth2")]
    OAuth2 {
        #[serde(serialize_with = "serialize_url")]
        uri: Url,
    },
    #[serde(rename = "m.login.email.identity")]
    Email {
        threepid_creds: ThreePIDCredentials,
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },
    #[serde(rename = "m.login.dummy")]
    Dummy {
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },
}
