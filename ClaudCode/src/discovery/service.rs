use tokio::sync::RwLock;
use std::collections::HashMap;
use crate::messages::UserInfo;
use crate::utils::current_timestamp;

pub struct DiscoveryService {
    users: RwLock<HashMap<String, UserInfo>>,
}

impl DiscoveryService {
    pub fn new() -> Self {
        Self {
            users: RwLock::new(HashMap::new()),
        }
    }
    
    pub async fn register_user(&self, user_id: String, username: String, connection_info: String) {
        let user_info = UserInfo {
            user_id: user_id.clone(),
            username,
            connection_info,
            online_since: current_timestamp(),
        };
        
        self.users.write().await.insert(user_id, user_info);
    }
    
    pub async fn unregister_user(&self, user_id: &str) {
        self.users.write().await.remove(user_id);
    }
    
    pub async fn get_all_users(&self) -> HashMap<String, UserInfo> {
        self.users.read().await.clone()
    }
    
    pub async fn apply_snapshot(&self, snapshot: HashMap<String, UserInfo>) {
        *self.users.write().await = snapshot;
    }
}