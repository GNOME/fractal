use fractal_api::identifiers::UserId;
use fractal_api::r0::search::user::User;
use fractal_api::r0::sync::get_joined_members::RoomMember;
use fractal_api::url::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// TODO: Make this non-(de)serializable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub uid: UserId,
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
        self.uid.to_string()
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

impl From<(UserId, RoomMember)> for Member {
    fn from((uid, roommember): (UserId, RoomMember)) -> Self {
        Member {
            uid,
            alias: roommember.display_name,
            avatar: roommember.avatar_url.as_ref().map(ToString::to_string),
        }
    }
}

// hashmap userid -> Member
pub type MemberList = HashMap<UserId, Member>;
