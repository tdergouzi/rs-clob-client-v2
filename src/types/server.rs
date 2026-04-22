use serde::{Deserialize, Serialize};

/// Response body for `POST /v1/heartbeats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub heartbeat_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_msg: Option<String>,
}
