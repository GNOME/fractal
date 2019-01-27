pub mod event;
pub mod fileinfo;
pub mod filter;
pub mod member;
pub mod message;
pub mod protocol;
pub mod register;
pub mod room;
pub mod stickers;
pub mod sync;
pub mod user;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Medium {
    #[serde(rename = "email")]
    Email,
    #[serde(rename = "msisdn")]
    MsIsdn,
}

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
pub struct ThreePIDCredentials {
    pub client_secret: String,
    pub id_server: String,
    pub sid: String,
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
    OAuth2 { uri: String },
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
