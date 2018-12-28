use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub added_at: u64,
    pub medium: String,
    pub validated_at: u64,
    pub address: String,
}
