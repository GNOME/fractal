use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Member {
    pub alias: String,
    pub uid: String,
    pub avatar: String,
}

impl Member {
    pub fn get_alias(&self) -> String {
        match self.alias {
            ref a if a.is_empty() => self.uid.clone(),
            ref a => a.clone(),
        }
    }
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

// hashmap userid -> Member
pub type MemberList = HashMap<String, Member>;
