use serde::{Deserialize, Serialize};

// ============================================================================
// MESSAGE TYPES - Only what's needed for Modified Bully Election
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // ELECTION MESSAGE
    // Sent when a server starts an election
    // from_id: which server is starting the election
    // priority: the server's calculated priority score (higher = better candidate)
    Election {
        from_id: u32,
        priority: f64,
    },

    // ALIVE MESSAGE
    // Sent in response to an Election message
    // Means "I'm still here and I have higher priority than you"
    // from_id: which server is responding
    Alive {
        from_id: u32,
    },

    // COORDINATOR MESSAGE
    // Sent by the winner of the election to announce "I am the leader now"
    // leader_id: which server won the election
    Coordinator {
        leader_id: u32,
    },

    // HEARTBEAT MESSAGE
    // Sent periodically by all servers to say "I'm still alive"
    // from_id: which server is sending the heartbeat
    // timestamp: when this heartbeat was sent (unix timestamp)
    // load: current load of the server (0.0 to 1.0)
    Heartbeat {
        from_id: u32,
        timestamp: u64,
        load: f64,
    },

    // NEW: Client asks "who is the leader?"
    LeaderQuery,

    // NEW: Server responds with leader ID
    LeaderResponse {
        leader_id: u32,
    },

    // NEW: Client sends a task to process
    TaskRequest {
        task_id: u64,
        processing_time_ms: u64,
        load_impact: f64,
    },

    // NEW: Server acknowledges task received
    TaskAck {
        task_id: u64,
    },

    TaskDelegate {
        task_id: u64,
        processing_time_ms: u64,
        load_impact: f64,
    },
}

impl Message {
    // Convert a message to bytes so we can send it over the network
    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    // Convert bytes received from network back into a Message
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

// Get current time as unix timestamp (seconds since Jan 1, 1970)
pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
