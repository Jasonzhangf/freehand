//! Provider-neutral request, event, and response semantics for Freehand.

use freehand_contracts::{
    AgentId, ErrorClass, ErrorContract, ErrorErr01RuntimeClassified, FeatureId,
    ReasonReq03ProviderPayload, ReasonReq04ToolCall, ReasonReq05ToolResultReentry,
    ReasonResp01SemanticEvent, ReasonResp02UsageEvent, ReasonResp03TerminalEvent, RecoveryPolicy,
    SemanticEventKind, SessionId, TerminalStatus, TokenUsage, ToolCallContract, TraceId, TurnId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderFamily {
    OpenAiCompatible,
    Anthropic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderProtocol {
    OpenAiResponses,
    AnthropicMessages,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub web_search: bool,
    pub multimodal: bool,
    pub vision: bool,
    pub reasoning: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDescriptor {
    pub provider_name: String,
    pub family: ProviderFamily,
    pub protocol: ProviderProtocol,
    pub model: String,
    pub capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RawRetentionPolicy {
    DebugOnly,
    DoNotRetain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSemanticRequest {
    pub descriptor: ProviderDescriptor,
    pub payload: ReasonReq03ProviderPayload,
    pub raw_retention: RawRetentionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderAdapterEvent {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCall(ToolCallContract),
    ToolResultReentry(ReasonReq05ToolResultReentry),
    Usage(TokenUsage),
    Terminal {
        status: TerminalStatus,
        summary: String,
    },
    Error(ProviderErrorHint),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderErrorHint {
    pub code: String,
    pub message: String,
    pub class: ErrorClass,
    pub retry_after_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderSemanticOutput {
    SemanticEvent(ReasonResp01SemanticEvent),
    ToolCall(ReasonReq04ToolCall),
    ToolResultReentry(ReasonReq05ToolResultReentry),
    Usage(ReasonResp02UsageEvent),
    Terminal(ReasonResp03TerminalEvent),
    Error(ErrorErr01RuntimeClassified),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEventContext {
    pub agent_id: AgentId,
    pub session_id: SessionId,
    pub turn_id: TurnId,
    pub trace_id: TraceId,
    pub feature_id: FeatureId,
}

#[derive(Debug, Error)]
pub enum ProviderSemanticError {
    #[error("provider descriptor `{provider}` does not support protocol `{protocol:?}`")]
    UnsupportedProtocol {
        provider: String,
        protocol: ProviderProtocol,
    },
}

pub fn build_semantic_request(
    descriptor: ProviderDescriptor,
    payload: ReasonReq03ProviderPayload,
    debug: bool,
) -> Result<ProviderSemanticRequest, ProviderSemanticError> {
    match descriptor.protocol {
        ProviderProtocol::OpenAiResponses | ProviderProtocol::AnthropicMessages => {
            Ok(ProviderSemanticRequest {
                descriptor,
                payload,
                raw_retention: if debug {
                    RawRetentionPolicy::DebugOnly
                } else {
                    RawRetentionPolicy::DoNotRetain
                },
            })
        }
    }
}

pub fn classify_provider_error(hint: ProviderErrorHint) -> ErrorContract {
    let recovery = match hint.retry_after_seconds {
        Some(seconds) => RecoveryPolicy::PeriodicRecoverable {
            retry_after_seconds: seconds,
        },
        None => match hint.class {
            ErrorClass::Auth
            | ErrorClass::Unsupported
            | ErrorClass::UserConfig
            | ErrorClass::Contract => RecoveryPolicy::Unrecoverable,
            ErrorClass::RateLimit
            | ErrorClass::Upstream
            | ErrorClass::Protocol
            | ErrorClass::Stream => RecoveryPolicy::Recoverable,
        },
    };
    ErrorContract {
        code: hint.code,
        class: hint.class,
        recovery,
        message: hint.message,
    }
}

pub fn map_adapter_event(
    ctx: &ProviderEventContext,
    event: ProviderAdapterEvent,
) -> ProviderSemanticOutput {
    match event {
        ProviderAdapterEvent::TextDelta(content) => {
            ProviderSemanticOutput::SemanticEvent(ReasonResp01SemanticEvent {
                session_id: ctx.session_id.clone(),
                turn_id: ctx.turn_id.clone(),
                trace_id: ctx.trace_id.clone(),
                feature_id: ctx.feature_id.clone(),
                agent_id: ctx.agent_id.clone(),
                kind: SemanticEventKind::Text,
                content,
            })
        }
        ProviderAdapterEvent::ReasoningDelta(content) => {
            ProviderSemanticOutput::SemanticEvent(ReasonResp01SemanticEvent {
                session_id: ctx.session_id.clone(),
                turn_id: ctx.turn_id.clone(),
                trace_id: ctx.trace_id.clone(),
                feature_id: ctx.feature_id.clone(),
                agent_id: ctx.agent_id.clone(),
                kind: SemanticEventKind::Reasoning,
                content,
            })
        }
        ProviderAdapterEvent::ToolCall(tool_call) => {
            ProviderSemanticOutput::ToolCall(ReasonReq04ToolCall {
                session_id: ctx.session_id.clone(),
                turn_id: ctx.turn_id.clone(),
                trace_id: ctx.trace_id.clone(),
                feature_id: ctx.feature_id.clone(),
                agent_id: ctx.agent_id.clone(),
                tool_call,
            })
        }
        ProviderAdapterEvent::ToolResultReentry(tool_result) => {
            ProviderSemanticOutput::ToolResultReentry(tool_result)
        }
        ProviderAdapterEvent::Usage(usage) => {
            ProviderSemanticOutput::Usage(ReasonResp02UsageEvent {
                session_id: ctx.session_id.clone(),
                turn_id: ctx.turn_id.clone(),
                trace_id: ctx.trace_id.clone(),
                feature_id: ctx.feature_id.clone(),
                agent_id: ctx.agent_id.clone(),
                usage,
            })
        }
        ProviderAdapterEvent::Terminal { status, summary } => {
            ProviderSemanticOutput::Terminal(ReasonResp03TerminalEvent {
                session_id: ctx.session_id.clone(),
                turn_id: ctx.turn_id.clone(),
                trace_id: ctx.trace_id.clone(),
                feature_id: ctx.feature_id.clone(),
                agent_id: ctx.agent_id.clone(),
                status,
                summary,
            })
        }
        ProviderAdapterEvent::Error(hint) => {
            ProviderSemanticOutput::Error(ErrorErr01RuntimeClassified {
                session_id: Some(ctx.session_id.clone()),
                turn_id: Some(ctx.turn_id.clone()),
                trace_id: ctx.trace_id.clone(),
                feature_id: ctx.feature_id.clone(),
                agent_id: Some(ctx.agent_id.clone()),
                error: classify_provider_error(hint),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{
        FeatureId, ReasonReq03ProviderPayload, SessionId, ToolArgument, ToolCallId,
    };

    fn descriptor() -> ProviderDescriptor {
        ProviderDescriptor {
            provider_name: "openai".to_owned(),
            family: ProviderFamily::OpenAiCompatible,
            protocol: ProviderProtocol::OpenAiResponses,
            model: "gpt-test".to_owned(),
            capabilities: ProviderCapabilities {
                web_search: true,
                multimodal: false,
                vision: true,
                reasoning: true,
            },
        }
    }

    fn payload() -> ReasonReq03ProviderPayload {
        ReasonReq03ProviderPayload {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("provider.semantic"),
            agent_id: AgentId::new("agent-1"),
            model: "gpt-test".to_owned(),
            rendered_input: "hello".to_owned(),
        }
    }

    fn ctx() -> ProviderEventContext {
        ProviderEventContext {
            agent_id: AgentId::new("agent-1"),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("provider.semantic"),
        }
    }

    #[test]
    fn builds_openai_responses_semantic_request_with_debug_retention() {
        let request = build_semantic_request(descriptor(), payload(), true).expect("build request");
        assert_eq!(
            request.descriptor.protocol,
            ProviderProtocol::OpenAiResponses
        );
        assert_eq!(request.raw_retention, RawRetentionPolicy::DebugOnly);
    }

    #[test]
    fn maps_reasoning_and_text_events_into_semantic_output() {
        let mapped = map_adapter_event(
            &ctx(),
            ProviderAdapterEvent::ReasoningDelta("thinking".to_owned()),
        );
        match mapped {
            ProviderSemanticOutput::SemanticEvent(event) => {
                assert_eq!(event.kind, SemanticEventKind::Reasoning);
                assert_eq!(event.content, "thinking");
            }
            other => panic!("unexpected output: {other:?}"),
        }

        let mapped =
            map_adapter_event(&ctx(), ProviderAdapterEvent::TextDelta("answer".to_owned()));
        match mapped {
            ProviderSemanticOutput::SemanticEvent(event) => {
                assert_eq!(event.kind, SemanticEventKind::Text);
                assert_eq!(event.content, "answer");
            }
            other => panic!("unexpected output: {other:?}"),
        }
    }

    #[test]
    fn maps_partial_tool_call_from_responses_protocol() {
        let mapped = map_adapter_event(
            &ctx(),
            ProviderAdapterEvent::ToolCall(ToolCallContract {
                tool_call_id: ToolCallId::new("tool-1"),
                tool_name: "web_search".to_owned(),
                arguments: vec![ToolArgument {
                    name: "query".to_owned(),
                    value: "rust".to_owned(),
                }],
                arguments_complete: false,
            }),
        );
        match mapped {
            ProviderSemanticOutput::ToolCall(event) => {
                assert!(!event.tool_call.arguments_complete);
                assert_eq!(event.tool_call.tool_name, "web_search");
            }
            other => panic!("unexpected output: {other:?}"),
        }
    }

    #[test]
    fn classifies_periodic_recoverable_error_with_provider_window() {
        let classified = classify_provider_error(ProviderErrorHint {
            code: "rate_limit".to_owned(),
            message: "wait".to_owned(),
            class: ErrorClass::RateLimit,
            retry_after_seconds: Some(1800),
        });
        assert_eq!(classified.class, ErrorClass::RateLimit);
        assert_eq!(
            classified.recovery,
            RecoveryPolicy::PeriodicRecoverable {
                retry_after_seconds: 1800
            }
        );
    }

    #[test]
    fn maps_usage_event_and_preserves_cache_hit_rate() {
        let mapped = map_adapter_event(
            &ctx(),
            ProviderAdapterEvent::Usage(TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_tokens: 20,
                cache_read_tokens: 80,
            }),
        );
        match mapped {
            ProviderSemanticOutput::Usage(event) => {
                assert!((event.usage.cache_hit_rate() - 0.8).abs() < f64::EPSILON);
            }
            other => panic!("unexpected output: {other:?}"),
        }
    }
}
