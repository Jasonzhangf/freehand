use crate::dto::*;
use crate::{
    SubscriptionSelector, UI_COMMAND_DESCRIPTORS, UiCommandFrameClass,
    is_public_adp_command_descriptor,
};
use freehand_contracts::{
    AgentId, ErrorErr01RuntimeClassified, ReasonReq04ToolCall, ReasonReq05ToolResultReentry,
    ReasonResp01SemanticEvent, ReasonResp02UsageEvent, ReasonResp03TerminalEvent, SessionId,
    TurnId,
};
use freehand_debug::DebugStateSnapshot;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum UiProjection {
    Turn(UiTurnProjection),
    NodeStatus(NodeStatusSnapshot),
    Progress(TaskProgressSnapshot),
    Debug(DebugStateSnapshot),
    Checkpoints(UiCheckpointSnapshot),
    TaskList(UiTaskListProjection),
    ErrorCenterEvents(UiErrorCenterEventListProjection),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSubscriptionEvent {
    pub projection: UiProjection,
    pub latest_active_turn_id: Option<TurnId>,
}

#[derive(Debug, Clone)]
pub struct TurnProjectionInput {
    pub source_agent_id: AgentId,
    pub source_node_id: String,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub created_at: Option<u64>,
    pub timing: Option<UiTurnTimingProjection>,
    pub cwd: Option<String>,
    pub user_text: Option<String>,
    pub semantic_events: Vec<ReasonResp01SemanticEvent>,
    pub tool_calls: Vec<ReasonReq04ToolCall>,
    pub tool_results: Vec<ReasonReq05ToolResultReentry>,
    pub usage_events: Vec<ReasonResp02UsageEvent>,
    pub terminal_event: Option<ReasonResp03TerminalEvent>,
    pub error_events: Vec<ErrorErr01RuntimeClassified>,
    pub slave_substream_card: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiQueryResult {
    Turn(Option<UiTurnProjection>),
    SessionList(UiSessionListProjection),
    SessionTurns(UiSessionTranscriptProjection),
    SessionTurnsPage(UiSessionTranscriptPageProjection),
    SessionSearch(UiSessionSearchProjection),
    NodeStatus(Option<NodeStatusSnapshot>),
    Progress(Option<TaskProgressSnapshot>),
    Debug(Option<DebugStateSnapshot>),
    Checkpoints(UiCheckpointSnapshot),
    TaskList(UiTaskListProjection),
    TaskBoard(UiTaskBoardProjection),
    AgentBoard(UiAgentBoardProjection),
    AgentLifecycle(UiAgentLifecycleProjection),
    EventInbox(UiTaskEventInboxProjection),
    MasterPoll(UiMasterPollProjection),
    WorkerControl(Box<UiWorkerControlProjection>),
    TaskHistory(UiTaskHistoryProjection),
    ErrorCenterEvents(UiErrorCenterEventListProjection),
    ConfigStatus(UiConfigStatusProjection),
    TimerList(UiTimerListProjection),
    ToolRegistry(UiToolRegistryProjection),
    Diagnostics(UiDiagnosticsProjection),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCommandIngressAck {
    pub command_kind: String,
    pub accepted: bool,
    pub status_text: String,
    pub mutation_authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiProtocolRejection {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCommandDispatchEnvelope {
    pub ingress: UiCommandIngressAck,
    pub command: UiCommand,
    pub target_feature_id: String,
    pub target_owner_module: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCommandDispatchReceipt {
    pub ingress: UiCommandIngressAck,
    pub target_feature_id: String,
    pub dispatch_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCommandDispatchFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAdpFailure {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

pub const UI_ADP_PROTOCOL_VERSION: u32 = 3;
pub const UI_ADP_HANDSHAKE_CAPABILITY: &str = "adp.v3.handshake";
pub const UI_ADP_INTERNAL_COMMAND_CAPABILITY: &str = "adp.v3.internal_command_ingress";

pub fn adp_internal_command_capability(token: &str) -> String {
    format!("{}:{}", UI_ADP_INTERNAL_COMMAND_CAPABILITY, token)
}

pub fn adp_internal_command_token_from_capability(capability: &str) -> Option<&str> {
    capability
        .strip_prefix(UI_ADP_INTERNAL_COMMAND_CAPABILITY)?
        .strip_prefix(':')
        .filter(|token| !token.trim().is_empty())
}

pub fn adp_protocol_version() -> u32 {
    UI_ADP_PROTOCOL_VERSION
}

pub fn adp_server_capabilities() -> Vec<String> {
    vec![UI_ADP_HANDSHAKE_CAPABILITY.to_owned()]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAdpProtocolManifest {
    pub protocol_version: u32,
    pub handshake_capability: String,
    pub request_kinds: Vec<String>,
    pub response_kinds: Vec<String>,
    pub commands: Vec<UiAdpCommandManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiAdpCommandManifestEntry {
    pub serde_name: String,
    pub semantic_kind: String,
    pub frame_class: UiCommandFrameClass,
    pub target_owner_feature: String,
}

pub fn adp_command_manifest_entries() -> Vec<UiAdpCommandManifestEntry> {
    UI_COMMAND_DESCRIPTORS
        .iter()
        .filter(|descriptor| is_public_adp_command_descriptor(descriptor))
        .map(|descriptor| UiAdpCommandManifestEntry {
            serde_name: descriptor.serde_name.to_owned(),
            semantic_kind: descriptor.semantic_kind.to_owned(),
            frame_class: descriptor.frame_class,
            target_owner_feature: descriptor.target_owner_feature.to_owned(),
        })
        .collect()
}

pub fn adp_protocol_manifest() -> UiAdpProtocolManifest {
    UiAdpProtocolManifest {
        protocol_version: UI_ADP_PROTOCOL_VERSION,
        handshake_capability: UI_ADP_HANDSHAKE_CAPABILITY.to_owned(),
        request_kinds: vec![
            "handshake".to_owned(),
            "command".to_owned(),
            "query".to_owned(),
            "subscribe".to_owned(),
        ],
        response_kinds: vec![
            "handshake_accepted".to_owned(),
            "command_receipt".to_owned(),
            "query_result".to_owned(),
            "subscription_event".to_owned(),
            "subscription_accepted".to_owned(),
            "failure".to_owned(),
        ],
        commands: adp_command_manifest_entries(),
    }
}

pub fn adp_protocol_manifest_json() -> String {
    let mut json = serde_json::to_string_pretty(&adp_protocol_manifest())
        .expect("ADP protocol manifest must serialize");
    json.push('\n');
    json
}

pub fn adp_protocol_webui_module() -> String {
    let manifest_json = serde_json::to_string_pretty(&adp_protocol_manifest())
        .expect("ADP protocol manifest must serialize");
    format!(
        r#"// @generated by `cargo run -p freehand-ui-protocol --bin export-adp-protocol -- --js <path>`; do not edit by hand.
export const ADP_PROTOCOL_MANIFEST = Object.freeze({manifest_json});

export const ADP_PROTOCOL_VERSION = ADP_PROTOCOL_MANIFEST.protocol_version;
export const ADP_HANDSHAKE_CAPABILITY = ADP_PROTOCOL_MANIFEST.handshake_capability;

const COMMANDS_BY_SERDE_NAME = new Map(
  ADP_PROTOCOL_MANIFEST.commands.map((entry) => [entry.serde_name, Object.freeze(entry)]),
);

function commandDescriptor(serdeName) {{
  const descriptor = COMMANDS_BY_SERDE_NAME.get(serdeName);
  if (!descriptor) {{
    throw new Error(`未知服务命令类型：${{serdeName}}`);
  }}
  return descriptor;
}}

function commandOf(frameClass, serdeName, payload) {{
  const descriptor = commandDescriptor(serdeName);
  if (descriptor.frame_class !== frameClass) {{
    throw new Error(`服务命令 ${{serdeName}} 不能走 ${{frameClass}} 通道`);
  }}
  if (payload === undefined || payload === null) {{
    return serdeName;
  }}
  return {{ [serdeName]: payload }};
}}

export function adpQueryOf(serdeName, payload) {{
  return commandOf('query', serdeName, payload);
}}

export function adpCommandOf(serdeName, payload) {{
  return commandOf('mutation', serdeName, payload);
}}

export function adpSubscribeOf(serdeName, payload) {{
  return commandOf('subscribe', serdeName, payload);
}}

export function adpCommandDescriptor(serdeName) {{
  return commandDescriptor(serdeName);
}}
"#
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAdpRequest {
    Handshake {
        request_id: String,
        client_name: String,
        capabilities: Vec<String>,
    },
    Command {
        request_id: String,
        command: UiCommand,
    },
    Query {
        request_id: String,
        query: UiCommand,
    },
    Subscribe {
        request_id: String,
        subscription: UiCommand,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAdpResponse {
    HandshakeAccepted {
        request_id: String,
        server_capabilities: Vec<String>,
    },
    CommandReceipt {
        request_id: String,
        receipt: UiCommandDispatchReceipt,
    },
    QueryResult {
        request_id: String,
        result: UiQueryResult,
    },
    SubscriptionEvent {
        request_id: String,
        event: UiSubscriptionEvent,
    },
    SubscriptionAccepted {
        request_id: String,
        selector: SubscriptionSelector,
    },
    Failure {
        request_id: String,
        failure: UiAdpFailure,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum UiAdpRequestWire {
    Handshake {
        request_id: String,
        client_name: String,
        capabilities: Vec<String>,
    },
    Command {
        request_id: String,
        command: UiCommand,
    },
    Query {
        request_id: String,
        query: UiCommand,
    },
    Subscribe {
        request_id: String,
        subscription: UiCommand,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum UiAdpResponseWire {
    HandshakeAccepted {
        request_id: String,
        server_capabilities: Vec<String>,
    },
    CommandReceipt {
        request_id: String,
        receipt: UiCommandDispatchReceipt,
    },
    QueryResult {
        request_id: String,
        result: UiQueryResult,
    },
    SubscriptionEvent {
        request_id: String,
        event: UiSubscriptionEvent,
    },
    SubscriptionAccepted {
        request_id: String,
        selector: SubscriptionSelector,
    },
    Failure {
        request_id: String,
        failure: UiAdpFailure,
    },
}

impl From<&UiAdpRequest> for UiAdpRequestWire {
    fn from(request: &UiAdpRequest) -> Self {
        match request {
            UiAdpRequest::Handshake {
                request_id,
                client_name,
                capabilities,
            } => Self::Handshake {
                request_id: request_id.clone(),
                client_name: client_name.clone(),
                capabilities: capabilities.clone(),
            },
            UiAdpRequest::Command {
                request_id,
                command,
            } => Self::Command {
                request_id: request_id.clone(),
                command: command.clone(),
            },
            UiAdpRequest::Query { request_id, query } => Self::Query {
                request_id: request_id.clone(),
                query: query.clone(),
            },
            UiAdpRequest::Subscribe {
                request_id,
                subscription,
            } => Self::Subscribe {
                request_id: request_id.clone(),
                subscription: subscription.clone(),
            },
        }
    }
}

impl From<UiAdpRequestWire> for UiAdpRequest {
    fn from(wire: UiAdpRequestWire) -> Self {
        match wire {
            UiAdpRequestWire::Handshake {
                request_id,
                client_name,
                capabilities,
            } => Self::Handshake {
                request_id,
                client_name,
                capabilities,
            },
            UiAdpRequestWire::Command {
                request_id,
                command,
            } => Self::Command {
                request_id,
                command,
            },
            UiAdpRequestWire::Query { request_id, query } => Self::Query { request_id, query },
            UiAdpRequestWire::Subscribe {
                request_id,
                subscription,
            } => Self::Subscribe {
                request_id,
                subscription,
            },
        }
    }
}

impl From<&UiAdpResponse> for UiAdpResponseWire {
    fn from(response: &UiAdpResponse) -> Self {
        match response {
            UiAdpResponse::HandshakeAccepted {
                request_id,
                server_capabilities,
            } => Self::HandshakeAccepted {
                request_id: request_id.clone(),
                server_capabilities: server_capabilities.clone(),
            },
            UiAdpResponse::CommandReceipt {
                request_id,
                receipt,
            } => Self::CommandReceipt {
                request_id: request_id.clone(),
                receipt: receipt.clone(),
            },
            UiAdpResponse::QueryResult { request_id, result } => Self::QueryResult {
                request_id: request_id.clone(),
                result: result.clone(),
            },
            UiAdpResponse::SubscriptionEvent { request_id, event } => Self::SubscriptionEvent {
                request_id: request_id.clone(),
                event: event.clone(),
            },
            UiAdpResponse::SubscriptionAccepted {
                request_id,
                selector,
            } => Self::SubscriptionAccepted {
                request_id: request_id.clone(),
                selector: selector.clone(),
            },
            UiAdpResponse::Failure {
                request_id,
                failure,
            } => Self::Failure {
                request_id: request_id.clone(),
                failure: failure.clone(),
            },
        }
    }
}

impl From<UiAdpResponseWire> for UiAdpResponse {
    fn from(wire: UiAdpResponseWire) -> Self {
        match wire {
            UiAdpResponseWire::HandshakeAccepted {
                request_id,
                server_capabilities,
            } => Self::HandshakeAccepted {
                request_id,
                server_capabilities,
            },
            UiAdpResponseWire::CommandReceipt {
                request_id,
                receipt,
            } => Self::CommandReceipt {
                request_id,
                receipt,
            },
            UiAdpResponseWire::QueryResult { request_id, result } => {
                Self::QueryResult { request_id, result }
            }
            UiAdpResponseWire::SubscriptionEvent { request_id, event } => {
                Self::SubscriptionEvent { request_id, event }
            }
            UiAdpResponseWire::SubscriptionAccepted {
                request_id,
                selector,
            } => Self::SubscriptionAccepted {
                request_id,
                selector,
            },
            UiAdpResponseWire::Failure {
                request_id,
                failure,
            } => Self::Failure {
                request_id,
                failure,
            },
        }
    }
}

fn adp_versioned_value<T: Serialize, E: serde::ser::Error>(
    wire: T,
) -> Result<serde_json::Value, E> {
    let mut value = serde_json::to_value(wire).map_err(E::custom)?;
    let Some(object) = value.as_object_mut() else {
        return Err(E::custom("ADP frame must serialize to JSON object"));
    };
    object.insert(
        "protocol_version".to_owned(),
        serde_json::Value::from(UI_ADP_PROTOCOL_VERSION),
    );
    Ok(value)
}

fn adp_checked_value<E: serde::de::Error>(
    mut value: serde_json::Value,
) -> Result<serde_json::Value, E> {
    let Some(object) = value.as_object_mut() else {
        return Err(E::custom("ADP frame must be a JSON object"));
    };
    let Some(protocol_version) = object.remove("protocol_version") else {
        return Err(E::custom("missing ADP protocol_version"));
    };
    let Some(protocol_version) = protocol_version.as_u64() else {
        return Err(E::custom("ADP protocol_version must be an integer"));
    };
    if protocol_version != u64::from(UI_ADP_PROTOCOL_VERSION) {
        return Err(E::custom(format!(
            "unsupported ADP protocol_version {protocol_version}; expected {UI_ADP_PROTOCOL_VERSION}"
        )));
    }
    Ok(value)
}

impl Serialize for UiAdpRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        adp_versioned_value::<_, S::Error>(UiAdpRequestWire::from(self))?.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UiAdpRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let value = adp_checked_value::<D::Error>(value)?;
        let wire = UiAdpRequestWire::deserialize(value).map_err(serde::de::Error::custom)?;
        Ok(wire.into())
    }
}

impl Serialize for UiAdpResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        adp_versioned_value::<_, S::Error>(UiAdpResponseWire::from(self))?.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for UiAdpResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let value = adp_checked_value::<D::Error>(value)?;
        let wire = UiAdpResponseWire::deserialize(value).map_err(serde::de::Error::custom)?;
        Ok(wire.into())
    }
}
