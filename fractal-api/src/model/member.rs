use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Member {
    pub alias: Option<String>,
    pub uid: String,
    pub avatar: Option<String>,
}

impl Clone for Member {
    fn clone(&self) -> Member {
        Member {
            alias: self.alias.clone(),
            uid: self.uid.clone(),
            avatar: self.avatar.clone(),
        }
    }
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
