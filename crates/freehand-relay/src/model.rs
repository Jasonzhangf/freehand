use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub access_token: String,
    pub account_id: String,
    pub username: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Master,
    Worker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWorkStatus {
    Idle,
    Running,
    Waiting,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentHeartbeat {
    pub agent_id: String,
    pub display_name: String,
    pub node_id: String,
    pub role: AgentRole,
    pub status: AgentWorkStatus,
    pub active_session_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelayControlInFrame {
    AgentIdentity {
        agent_id: String,
        display_name: String,
        node_id: String,
        role: AgentRole,
        status: AgentWorkStatus,
        active_session_count: u32,
    },
    PresenceHeartbeat {
        status: AgentWorkStatus,
        active_session_count: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelayControlOutFrame {
    IdentityAccepted {
        protocol_version: u16,
        agent_id: String,
        control_generation: u64,
    },
}

pub const RELAY_TUNNEL_PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayDataProtocol {
    Http,
    Adp,
    WebSocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayDataFrameKind {
    Text,
    Binary,
    Ping,
    Pong,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelayDataOutFrame {
    RequestOpen {
        exchange_id: String,
        protocol: RelayDataProtocol,
        method: Option<String>,
        path_and_query: String,
        headers: Vec<(String, Vec<u8>)>,
    },
    RequestChunk {
        exchange_id: String,
        frame_kind: Option<RelayDataFrameKind>,
        bytes: Vec<u8>,
    },
    RequestEnd {
        exchange_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelayDataInFrame {
    ResponseOpen {
        exchange_id: String,
        status: Option<u16>,
        headers: Vec<(String, Vec<u8>)>,
    },
    ResponseChunk {
        exchange_id: String,
        frame_kind: Option<RelayDataFrameKind>,
        bytes: Vec<u8>,
    },
    ResponseEnd {
        exchange_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelayErrorInFrame {
    TunnelFailure {
        exchange_id: Option<String>,
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelayErrorOutFrame {
    CorrelatedFailure {
        exchange_id: Option<String>,
        code: String,
        message: String,
    },
    Terminal {
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentPresence {
    pub agent_id: String,
    pub display_name: String,
    pub node_id: String,
    pub role: AgentRole,
    pub status: AgentWorkStatus,
    pub active_session_count: u32,
    pub last_seen_unix: u64,
    pub online: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDirectory {
    pub account_id: String,
    pub generated_at_unix: u64,
    pub agents: Vec<AgentPresence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelayDirectoryOutFrame {
    Snapshot { directory: AgentDirectory },
    Terminal { code: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}
