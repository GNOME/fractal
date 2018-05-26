#[derive(Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub added_at: u64,
    pub medium: String,
    pub validated_at: u64,
    pub address: String,
}

impl Clone for UserInfo {
    fn clone(&self) -> UserInfo {
        UserInfo {
            added_at: self.added_at.clone(),
            medium: self.medium.clone(),
            validated_at: self.validated_at.clone(),
            address: self.address.clone(),
        }
    }
}
