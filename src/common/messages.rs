//! # Message Protocol
//!
//! Defines all message types used in the CloudP2P distributed system for:
//! - Leader election (Modified Bully Algorithm)
//! - Peer-to-peer heartbeat monitoring
//! - Client-server task submission and responses
//! - Fault tolerance and task history tracking
//!
//! Messages are serialized to JSON and sent over TCP with a 4-byte length prefix.

use serde::{Deserialize, Serialize};

// ============================================================================
// MESSAGE TYPES - Protocol for Modified Bully Election and Task Distribution
// ============================================================================

/// Core message enum for all communication in the CloudP2P system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // ========== LEADER ELECTION MESSAGES ==========
    /// **Election Message**
    ///
    /// Sent when a server initiates a leader election process.
    ///
    /// # Fields
    /// - `from_id`: ID of the server starting the election
    /// - `priority`: The server's calculated priority score (LOWER = BETTER candidate)
    ///
    /// # Modified Bully Algorithm
    /// Unlike classic Bully Algorithm which uses static server IDs, this implementation
    /// uses dynamic load-based priority where lower values indicate less-loaded servers.
    Election { from_id: u32, priority: f64 },

    /// **Alive Message**
    ///
    /// Response to an Election message indicating the responding server has higher
    /// priority (lower load) and should be considered for leadership.
    ///
    /// # Fields
    /// - `from_id`: ID of the responding server
    Alive { from_id: u32 },

    /// **Coordinator Message**
    ///
    /// Broadcast by the election winner to announce itself as the new leader.
    ///
    /// # Fields
    /// - `leader_id`: ID of the server that won the election
    Coordinator { leader_id: u32 },

    /// **Heartbeat Message**
    ///
    /// Periodic message sent by all servers to indicate they are alive and share
    /// their current load metrics.
    ///
    /// # Fields
    /// - `from_id`: ID of the server sending the heartbeat
    /// - `timestamp`: Unix timestamp when heartbeat was sent (seconds since epoch)
    /// - `load`: Current load score (0.0 = no load, 100.0 = maximum load)
    ///
    /// # Fault Detection
    /// Servers that don't send heartbeats within the configured timeout are
    /// considered failed, triggering orphaned task cleanup and potential re-election.
    Heartbeat {
        from_id: u32,
        timestamp: u64,
        load: f64,
    },

    // ========== CLIENT-SERVER COMMUNICATION ==========
    /// **Leader Query**
    ///
    /// Sent by clients to discover which server is currently the leader.
    /// Any server can respond with the current leader information.
    LeaderQuery,

    /// **Leader Response**
    ///
    /// Response to LeaderQuery containing the current leader's ID.
    ///
    /// # Fields
    /// - `leader_id`: ID of the current leader server
    LeaderResponse { leader_id: u32 },

    /// **Task Assignment Request**
    ///
    /// Sent by clients to the leader to request which server should process their task.
    /// The leader performs load balancing and returns the least-loaded server.
    ///
    /// # Fields
    /// - `client_name`: Name/identifier of the requesting client
    /// - `request_id`: Unique ID for this request (for tracking and idempotency)
    TaskAssignmentRequest {
        client_name: String,
        request_id: u64,
    },

    /// **Task Assignment Response**
    ///
    /// Leader's response indicating which server the client should send their task to.
    ///
    /// # Fields
    /// - `request_id`: ID of the request this answers
    /// - `assigned_server_id`: ID of the server that should process the task
    /// - `assigned_server_address`: IP:port address of the assigned server
    TaskAssignmentResponse {
        request_id: u64,
        assigned_server_id: u32,
        assigned_server_address: String,
    },

    /// **Task Request**
    ///
    /// Sent by clients to assigned servers to perform steganography encryption.
    ///
    /// # Fields
    /// - `client_name`: Name of the client submitting the task
    /// - `request_id`: Unique ID for tracking
    /// - `secret_image_data`: Raw bytes of the secret image to hide in the server's carrier image
    /// - `assigned_by_leader`: ID of the leader that assigned this task (for validation)
    TaskRequest {
        client_name: String,
        request_id: u64,
        secret_image_data: Vec<u8>,
        assigned_by_leader: u32,
    },

    /// **Task Response**
    ///
    /// Server's response after processing a task request.
    ///
    /// # Fields
    /// - `request_id`: ID of the request being answered
    /// - `encrypted_image_data`: Carrier image bytes with embedded secret image (PNG format)
    /// - `success`: Whether the encryption succeeded
    /// - `error_message`: Error details if success is false
    TaskResponse {
        request_id: u64,
        encrypted_image_data: Vec<u8>,
        success: bool,
        error_message: Option<String>,
    },

    /// **Task Acknowledgment**
    ///
    /// Sent by clients after successfully receiving a TaskResponse to confirm receipt.
    /// This ensures the server knows the client got the result before removing it from
    /// task history, preventing orphaned work if the response is lost in transit.
    ///
    /// # Fields
    /// - `client_name`: Client that received the response
    /// - `request_id`: ID of the completed task
    TaskAck {
        client_name: String,
        request_id: u64,
    },

    /// **Task Status Query**
    ///
    /// Sent by clients (via broadcast) to check the status of a task when the originally
    /// assigned server fails. Used for server-side failover - client polls to discover
    /// if the task has been reassigned to a different server.
    ///
    /// # Fields
    /// - `client_name`: Client asking about the task
    /// - `request_id`: ID of the task to check
    TaskStatusQuery {
        client_name: String,
        request_id: u64,
    },

    /// **Task Status Response**
    ///
    /// Response to TaskStatusQuery, indicating the current assignment status of a task.
    /// Any server can respond by checking the shared task history.
    ///
    /// # Fields
    /// - `request_id`: ID of the task being queried
    /// - `assigned_server_id`: Current server assigned to process this task
    /// - `assigned_server_address`: Network address of the assigned server
    TaskStatusResponse {
        request_id: u64,
        assigned_server_id: u32,
        assigned_server_address: String,
    },

    // ========== FAULT TOLERANCE MESSAGES ==========
    /// **History Add**
    ///
    /// Broadcast by the leader when assigning a task to track which server is
    /// responsible. Used for orphaned task cleanup when servers fail.
    ///
    /// # Fields
    /// - `client_name`: Client that submitted the task
    /// - `request_id`: ID of the task
    /// - `assigned_server_id`: Server responsible for this task
    /// - `timestamp`: When the assignment was made
    HistoryAdd {
        client_name: String,
        request_id: u64,
        assigned_server_id: u32,
        timestamp: u64,
    },

    /// **History Remove**
    ///
    /// Sent by servers after successfully completing a task to remove it from
    /// the tracked history.
    ///
    /// # Fields
    /// - `client_name`: Client that submitted the task
    /// - `request_id`: ID of the completed task
    HistoryRemove {
        client_name: String,
        request_id: u64,
    },

    /// **History Sync Request**
    ///
    /// Sent by a newly elected leader to all peers to request their task history.
    /// Used to build a complete view of all active tasks across the cluster.
    ///
    /// # Fields
    /// - `from_server_id`: ID of the server requesting history (the new leader)
    HistorySyncRequest { from_server_id: u32 },

    /// **History Sync Response**
    ///
    /// Response to HistorySyncRequest containing all task history entries from a peer.
    /// The new leader merges these responses to build complete cluster state.
    ///
    /// # Fields
    /// - `from_server_id`: ID of the server responding
    /// - `history_entries`: List of (client_name, request_id, assigned_server_id, timestamp) tuples
    HistorySyncResponse {
        from_server_id: u32,
        history_entries: Vec<(String, u64, u32, u64)>,
    },
}

impl Message {
    /// Serialize a message to JSON bytes for transmission over the network.
    ///
    /// # Returns
    /// - `Ok(Vec<u8>)`: JSON-encoded message bytes
    /// - `Err`: Serialization error
    ///
    /// # Example
    /// ```ignore
    /// let msg = Message::Heartbeat { from_id: 1, timestamp: 12345, load: 0.5 };
    /// let bytes = msg.to_bytes()?;
    /// ```
    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize a message from JSON bytes received from the network.
    ///
    /// # Arguments
    /// - `bytes`: JSON-encoded message data
    ///
    /// # Returns
    /// - `Ok(Message)`: Deserialized message
    /// - `Err`: Deserialization error
    ///
    /// # Example
    /// ```ignore
    /// let msg = Message::from_bytes(&received_bytes)?;
    /// match msg {
    ///     Message::Heartbeat { from_id, .. } => println!("Got heartbeat from {}", from_id),
    ///     _ => {}
    /// }
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Get the current Unix timestamp in seconds since January 1, 1970.
///
/// Used for timestamping heartbeat messages and task history entries.
///
/// # Returns
/// Current time as Unix timestamp (u64 seconds)
///
/// # Example
/// ```ignore
/// let now = current_timestamp();
/// let msg = Message::Heartbeat { from_id: 1, timestamp: now, load: 0.3 };
/// ```
#[allow(dead_code)]
pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
