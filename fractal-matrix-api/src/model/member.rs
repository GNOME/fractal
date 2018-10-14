use std::collections::HashMap;

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
        match self.alias {
            ref a if a.is_none() || a.clone().unwrap().is_empty() => self.uid.clone(),
            ref a => a.as_ref().unwrap().clone(),
        }
    }
}

impl PartialEq for Member {
    fn eq(&self, other: &Member) -> bool {
        self.uid == other.uid
    }
}

// hashmap userid -> Member
pub type MemberList = HashMap<String, Member>;
