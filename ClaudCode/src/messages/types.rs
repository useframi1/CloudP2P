use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // Election messages
    Election {
        from_id: u32,
        priority: f64,
    },
    Alive {
        from_id: u32,
    },
    Coordinator {
        leader_id: u32,
    },
    
    // Heartbeat
    Heartbeat {
        from_id: u32,
        timestamp: u64,
        load: f64,
    },
    
    // Work distribution
    WorkRequest {
        request_id: Uuid,
        request_type: RequestType,
        data: Vec<u8>,
        client_id: String,
    },
    WorkProposal {
        request_id: Uuid,
        from_id: u32,
        priority: f64,
    },
    WorkAssignment {
        request_id: Uuid,
        assigned_to: u32,
    },
    WorkResponse {
        request_id: Uuid,
        success: bool,
        data: Vec<u8>,
        error: Option<String>,
    },
    
    // Discovery service
    RegisterUser {
        user_id: String,
        username: String,
        connection_info: String,
    },
    UnregisterUser {
        user_id: String,
    },
    GetOnlineUsers,
    OnlineUsersResponse {
        users: Vec<UserInfo>,
    },
    
    // State synchronization
    Recovery {
        from_id: u32,
        last_known_timestamp: u64,
    },
    StateSync {
        state: StateSnapshot,
    },
    
    // Fault simulation
    SimulateFail {
        duration_secs: u64,
    },
    
    // Acknowledgments
    Ack {
        message_id: Uuid,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestType {
    Encryption,
    Decryption,
    Discovery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: String,
    pub username: String,
    pub connection_info: String,
    pub online_since: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub leader_id: Option<u32>,
    pub active_peers: Vec<u32>,
    pub discovery_table: HashMap<String, UserInfo>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionRequest {
    pub image_data: Vec<u8>,
    pub username: String,
    pub allowed_users: Vec<String>,
    pub view_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionResponse {
    pub encrypted_image_data: Vec<u8>,
    pub success: bool,
}

impl Message {
    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }
    
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}