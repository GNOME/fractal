use crate::r0::search::user::User;
use crate::r0::sync::get_joined_members::RoomMember;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

// TODO: Make this non-(de)serializable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    // The mxid is either inside the json object, or outside of it.
    // Since we don't know, we always have to populate it manually
    #[serde(default)]
    pub uid: String,
    #[serde(rename = "display_name")]
    pub alias: Option<String>,
    #[serde(rename = "avatar_url")]
    pub avatar: Option<String>,
}

impl Member {
    pub fn get_alias(&self) -> String {
        if let Some(ref alias) = self.alias {
            if !alias.is_empty() {
                return alias.clone();
            }
        }
        self.uid.clone()
    }
}

impl PartialEq for Member {
    fn eq(&self, other: &Member) -> bool {
        self.uid == other.uid
    }
}

impl From<User> for Member {
    fn from(user: User) -> Self {
        Self {
            uid: user.user_id,
            alias: user.display_name,
            avatar: user.avatar_url.map(Url::into_string),
        }
    }
}

impl From<(String, RoomMember)> for Member {
    fn from(uid_roommember: (String, RoomMember)) -> Self {
        Member {
            uid: uid_roommember.0,
            alias: uid_roommember.1.display_name,
            avatar: uid_roommember
                .1
                .avatar_url
                .as_ref()
                .map(ToString::to_string),
        }
    }
}

// hashmap userid -> Member
pub type MemberList = HashMap<String, Member>;
