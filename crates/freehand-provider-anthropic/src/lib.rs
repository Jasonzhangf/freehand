//! Anthropic provider adapter for Freehand.

use std::collections::BTreeMap;

use freehand_blocks::{parse_tool_arguments_json, parse_tool_arguments_value};
use freehand_contracts::{ErrorClass, TerminalStatus, TokenUsage, ToolCallContract, ToolCallId};
use freehand_provider_core::{
    ProviderAdapterEvent, ProviderErrorHint, ProviderEventContext, ProviderProtocol,
    ProviderSemanticOutput, ProviderSemanticRequest, map_adapter_events,
};
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicAdapterConfig {
    pub max_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicRenderedRequest {
    pub path: &'static str,
    pub body: String,
}

#[derive(Debug)]
pub struct AnthropicAdapter {
    config: AnthropicAdapterConfig,
    partial_tool_calls: BTreeMap<String, PartialToolUseState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PartialToolUseState {
    tool_name: String,
    arguments_json: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AnthropicAdapterError {
    #[error("protocol `{0:?}` is not supported by Anthropic adapter")]
    UnsupportedProtocol(ProviderProtocol),
    #[error("invalid anthropic json payload: {0}")]
    InvalidJson(String),
    #[error("anthropic adapter max_tokens must be greater than zero")]
    InvalidMaxTokens,
    #[error("tool use block missing id")]
    MissingToolUseId,
    #[error("tool use block missing name")]
    MissingToolUseName,
    #[error("tool arguments invalid: {0}")]
    InvalidToolArguments(String),
}

impl AnthropicAdapter {
    pub fn new(config: AnthropicAdapterConfig) -> Result<Self, AnthropicAdapterError> {
        if config.max_tokens == 0 {
            return Err(AnthropicAdapterError::InvalidMaxTokens);
        }
        Ok(Self {
            config,
            partial_tool_calls: BTreeMap::new(),
        })
    }

    pub fn render_request(
        &self,
        request: &ProviderSemanticRequest,
        stream: bool,
    ) -> Result<AnthropicRenderedRequest, AnthropicAdapterError> {
        if request.descriptor.protocol != ProviderProtocol::AnthropicMessages {
            return Err(AnthropicAdapterError::UnsupportedProtocol(
                request.descriptor.protocol,
            ));
        }
        Ok(AnthropicRenderedRequest {
            path: "/v1/messages",
            body: json!({
                "model": request.descriptor.model,
                "max_tokens": self.config.max_tokens,
                "stream": stream,
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "text",
                                "text": request.payload.rendered_input,
                            }
                        ]
                    }
                ]
            })
            .to_string(),
        })
    }

    pub fn parse_response(
        &mut self,
        ctx: &ProviderEventContext,
        protocol: ProviderProtocol,
        body: &str,
    ) -> Result<Vec<ProviderSemanticOutput>, AnthropicAdapterError> {
        if protocol != ProviderProtocol::AnthropicMessages {
            return Err(AnthropicAdapterError::UnsupportedProtocol(protocol));
        }
        let value: Value = serde_json::from_str(body)
            .map_err(|err| AnthropicAdapterError::InvalidJson(err.to_string()))?;
        let events = self.parse_messages_body(&value)?;
        Ok(map_adapter_events(ctx, events))
    }

    pub fn parse_stream_event(
        &mut self,
        ctx: &ProviderEventContext,
        protocol: ProviderProtocol,
        event_body: &str,
    ) -> Result<Vec<ProviderSemanticOutput>, AnthropicAdapterError> {
        if protocol != ProviderProtocol::AnthropicMessages {
            return Err(AnthropicAdapterError::UnsupportedProtocol(protocol));
        }
        let value: Value = serde_json::from_str(event_body)
            .map_err(|err| AnthropicAdapterError::InvalidJson(err.to_string()))?;
        let events = self.parse_messages_stream_event(&value)?;
        Ok(map_adapter_events(ctx, events))
    }

    fn parse_messages_body(
        &mut self,
        value: &Value,
    ) -> Result<Vec<ProviderAdapterEvent>, AnthropicAdapterError> {
        let mut events = Vec::new();
        if let Some(content_blocks) = value.get("content").and_then(Value::as_array) {
            for block in content_blocks {
                let Some(kind) = block.get("type").and_then(Value::as_str) else {
                    continue;
                };
                match kind {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(Value::as_str)
                            && !text.is_empty()
                        {
                            events.push(ProviderAdapterEvent::TextDelta(text.to_owned()));
                        }
                    }
                    "tool_use" => {
                        events.push(self.parse_tool_use_block(block, true)?);
                    }
                    "thinking" | "redacted_thinking" => {
                        if let Some(text) = block.get("thinking").and_then(Value::as_str)
                            && !text.is_empty()
                        {
                            events.push(ProviderAdapterEvent::ReasoningDelta(text.to_owned()));
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(usage) = parse_anthropic_usage(
            value.get("usage"),
            value.get("stop_reason").and_then(Value::as_str),
        ) {
            events.push(ProviderAdapterEvent::Usage(usage));
        }
        if let Some(stop_reason) = value.get("stop_reason").and_then(Value::as_str) {
            events.push(terminal_event_from_stop_reason(stop_reason));
        }
        if let Some(error) = value.get("error") {
            events.push(ProviderAdapterEvent::Error(error_hint_from_value(error)));
        }
        Ok(events)
    }

    fn parse_messages_stream_event(
        &mut self,
        value: &Value,
    ) -> Result<Vec<ProviderAdapterEvent>, AnthropicAdapterError> {
        let mut events = Vec::new();
        let Some(event_type) = value.get("type").and_then(Value::as_str) else {
            return Ok(events);
        };
        match event_type {
            "message_start" => {
                if let Some(usage) = parse_anthropic_usage(
                    value
                        .get("message")
                        .and_then(|message| message.get("usage")),
                    None,
                ) {
                    events.push(ProviderAdapterEvent::Usage(usage));
                }
            }
            "content_block_start" => {
                if let Some(block) = value.get("content_block")
                    && let Some(kind) = block.get("type").and_then(Value::as_str)
                {
                    match kind {
                        "tool_use" => events.push(self.parse_tool_use_block(block, false)?),
                        "text" => {
                            if let Some(text) = block.get("text").and_then(Value::as_str)
                                && !text.is_empty()
                            {
                                events.push(ProviderAdapterEvent::TextDelta(text.to_owned()));
                            }
                        }
                        _ => {}
                    }
                }
            }
            "content_block_delta" => {
                if let Some(delta) = value.get("delta") {
                    match delta.get("type").and_then(Value::as_str) {
                        Some("text_delta") => {
                            if let Some(text) = delta.get("text").and_then(Value::as_str) {
                                events.push(ProviderAdapterEvent::TextDelta(text.to_owned()));
                            }
                        }
                        Some("thinking_delta") => {
                            if let Some(text) = delta.get("thinking").and_then(Value::as_str) {
                                events.push(ProviderAdapterEvent::ReasoningDelta(text.to_owned()));
                            }
                        }
                        Some("input_json_delta") => {
                            let id = value
                                .get("content_block")
                                .and_then(|block| block.get("id"))
                                .and_then(Value::as_str)
                                .or_else(|| value.get("id").and_then(Value::as_str))
                                .ok_or(AnthropicAdapterError::MissingToolUseId)?;
                            let name = value
                                .get("content_block")
                                .and_then(|block| block.get("name"))
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            let partial = delta
                                .get("partial_json")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            events.push(self.apply_partial_tool_delta(id, name, partial, false)?);
                        }
                        _ => {}
                    }
                }
            }
            "content_block_stop" => {
                if let Some(block) = value.get("content_block")
                    && matches!(block.get("type").and_then(Value::as_str), Some("tool_use"))
                {
                    let id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .ok_or(AnthropicAdapterError::MissingToolUseId)?;
                    let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                    let input = block
                        .get("input")
                        .map(Value::to_string)
                        .unwrap_or_else(|| "{}".to_owned());
                    events.push(self.apply_partial_tool_delta(id, name, &input, true)?);
                }
            }
            "message_delta" => {
                if let Some(delta) = value.get("delta")
                    && let Some(stop_reason) = delta.get("stop_reason").and_then(Value::as_str)
                {
                    events.push(terminal_event_from_stop_reason(stop_reason));
                }
                if let Some(usage) = parse_anthropic_usage(
                    value.get("usage"),
                    value
                        .get("delta")
                        .and_then(|delta| delta.get("stop_reason"))
                        .and_then(Value::as_str),
                ) {
                    events.push(ProviderAdapterEvent::Usage(usage));
                }
            }
            "message_stop" => {}
            "error" => {
                events.push(ProviderAdapterEvent::Error(error_hint_from_value(
                    value.get("error").unwrap_or(value),
                )));
            }
            _ => {}
        }
        Ok(events)
    }

    fn parse_tool_use_block(
        &mut self,
        block: &Value,
        is_complete: bool,
    ) -> Result<ProviderAdapterEvent, AnthropicAdapterError> {
        let id = block
            .get("id")
            .and_then(Value::as_str)
            .ok_or(AnthropicAdapterError::MissingToolUseId)?;
        let name = block
            .get("name")
            .and_then(Value::as_str)
            .ok_or(AnthropicAdapterError::MissingToolUseName)?;

        if is_complete {
            let arguments = parse_tool_arguments_value(block.get("input").unwrap_or(&json!({})))
                .map_err(|err| AnthropicAdapterError::InvalidToolArguments(err.to_string()))?;
            return Ok(ProviderAdapterEvent::ToolCall(ToolCallContract {
                tool_call_id: ToolCallId::new(id),
                tool_name: name.to_owned(),
                arguments,
                arguments_complete: true,
            }));
        }

        let input = block.get("input").map(Value::to_string).unwrap_or_default();
        self.apply_partial_tool_delta(id, name, &input, false)
    }

    fn apply_partial_tool_delta(
        &mut self,
        id: &str,
        tool_name: &str,
        delta: &str,
        is_complete: bool,
    ) -> Result<ProviderAdapterEvent, AnthropicAdapterError> {
        let state = self
            .partial_tool_calls
            .entry(id.to_owned())
            .or_insert_with(|| PartialToolUseState {
                tool_name: tool_name.to_owned(),
                arguments_json: String::new(),
            });
        if !tool_name.is_empty() {
            state.tool_name = tool_name.to_owned();
        }
        if is_complete && delta.trim_start().starts_with('{') && delta.trim_end().ends_with('}') {
            state.arguments_json = delta.to_owned();
        } else {
            state.arguments_json.push_str(delta);
        }

        let arguments = parse_tool_arguments_json(&state.arguments_json).unwrap_or_default();
        let event = ProviderAdapterEvent::ToolCall(ToolCallContract {
            tool_call_id: ToolCallId::new(id),
            tool_name: state.tool_name.clone(),
            arguments,
            arguments_complete: is_complete,
        });
        if is_complete {
            self.partial_tool_calls.remove(id);
        }
        Ok(event)
    }
}

fn parse_anthropic_usage(usage: Option<&Value>, finish_reason: Option<&str>) -> Option<TokenUsage> {
    let usage = usage?;
    Some(TokenUsage {
        input_tokens: usage
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        output_tokens: usage
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        total_tokens: usage
            .get("input_tokens")
            .and_then(Value::as_u64)
            .zip(usage.get("output_tokens").and_then(Value::as_u64))
            .map(|(input, output)| input + output),
        reasoning_tokens: usage.get("reasoning_tokens").and_then(Value::as_u64),
        cache_creation_tokens: usage
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        cache_read_tokens: usage
            .get("cache_read_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        finish_reason: finish_reason.map(ToOwned::to_owned),
    })
}

fn terminal_event_from_stop_reason(stop_reason: &str) -> ProviderAdapterEvent {
    let status = match stop_reason {
        "tool_use" => TerminalStatus::ToolPending,
        "max_tokens" => TerminalStatus::Interrupted,
        "refusal" => TerminalStatus::Failed,
        _ => TerminalStatus::Success,
    };
    ProviderAdapterEvent::Terminal {
        status,
        summary: stop_reason.to_owned(),
    }
}

fn error_hint_from_value(value: &Value) -> ProviderErrorHint {
    let error_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("anthropic_error")
        .to_owned();
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("anthropic request failed")
        .to_owned();
    let class = match error_type.as_str() {
        "authentication_error" | "permission_error" => ErrorClass::Auth,
        "rate_limit_error" => ErrorClass::RateLimit,
        "invalid_request_error" => ErrorClass::Protocol,
        "not_found_error" => ErrorClass::UserConfig,
        _ => ErrorClass::Upstream,
    };
    ProviderErrorHint {
        code: error_type,
        message,
        class,
        retry_after_seconds: value.get("retry_after").and_then(Value::as_u64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{
        AgentId, FeatureId, ReasonReq03ProviderPayload, SessionId, TraceId, TurnId,
    };
    use freehand_provider_core::{
        ProviderCapabilities, ProviderDescriptor, ProviderFamily, build_semantic_request,
    };

    fn ctx() -> ProviderEventContext {
        ProviderEventContext {
            agent_id: AgentId::new("agent-1"),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("provider.anthropic-adapter"),
        }
    }

    fn request() -> ProviderSemanticRequest {
        build_semantic_request(
            ProviderDescriptor {
                provider_name: "anthropic".to_owned(),
                family: ProviderFamily::Anthropic,
                protocol: ProviderProtocol::AnthropicMessages,
                model: "claude-test".to_owned(),
                capabilities: ProviderCapabilities {
                    web_search: false,
                    multimodal: true,
                    vision: true,
                    reasoning: true,
                },
            },
            ReasonReq03ProviderPayload {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("provider.anthropic-adapter"),
                agent_id: AgentId::new("agent-1"),
                model: "claude-test".to_owned(),
                rendered_input: "hello".to_owned(),
            },
            false,
        )
        .expect("request")
    }

    fn adapter() -> AnthropicAdapter {
        AnthropicAdapter::new(AnthropicAdapterConfig { max_tokens: 512 }).expect("adapter")
    }

    #[test]
    fn renders_messages_request() {
        let adapter = adapter();
        let rendered = adapter.render_request(&request(), true).expect("rendered");
        assert_eq!(rendered.path, "/v1/messages");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        assert_eq!(body.get("max_tokens").and_then(Value::as_u64), Some(512));
        assert_eq!(body.get("stream").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn parses_messages_response_text_tool_usage_and_terminal() {
        let mut adapter = adapter();
        let outputs = adapter
            .parse_response(
                &ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{
                    "content":[
                        {"type":"text","text":"answer"},
                        {"type":"tool_use","id":"toolu_1","name":"search","input":{"query":"rust"}}
                    ],
                    "usage":{"input_tokens":10,"output_tokens":4,"cache_read_input_tokens":2},
                    "stop_reason":"tool_use"
                }"#,
            )
            .expect("parsed");
        assert_eq!(outputs.len(), 4);
    }

    #[test]
    fn parses_streaming_partial_tool_use() {
        let mut adapter = adapter();
        let first = adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{"type":"content_block_start","content_block":{"type":"tool_use","id":"toolu_1","name":"search"}}"#,
            )
            .expect("first");
        match &first[0] {
            ProviderSemanticOutput::ToolCall(call) => {
                assert!(!call.tool_call.arguments_complete);
            }
            other => panic!("unexpected output: {other:?}"),
        }

        let second = adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{"type":"content_block_delta","id":"toolu_1","delta":{"type":"input_json_delta","partial_json":"{\"query\":\"ru"}}"#,
            )
            .expect("second");
        match &second[0] {
            ProviderSemanticOutput::ToolCall(call) => {
                assert!(!call.tool_call.arguments_complete);
                assert!(call.tool_call.arguments.is_empty());
            }
            other => panic!("unexpected output: {other:?}"),
        }
    }

    #[test]
    fn parses_streaming_tool_use_completion_and_text() {
        let mut adapter = adapter();
        adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{"type":"content_block_start","content_block":{"type":"tool_use","id":"toolu_1","name":"search"}}"#,
            )
            .expect("start");
        adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{"type":"content_block_delta","id":"toolu_1","delta":{"type":"input_json_delta","partial_json":"{\"query\":\"rust\"}"}}"#,
            )
            .expect("delta");
        let outputs = adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{"type":"content_block_stop","content_block":{"type":"tool_use","id":"toolu_1","name":"search","input":{"query":"rust"}}}"#,
            )
            .expect("stop");
        assert!(matches!(
            &outputs[0],
            ProviderSemanticOutput::ToolCall(call) if call.tool_call.arguments_complete && call.tool_call.arguments.len() == 1
        ));
    }

    #[test]
    fn rejects_zero_max_tokens_config() {
        let err =
            AnthropicAdapter::new(AnthropicAdapterConfig { max_tokens: 0 }).expect_err("must fail");
        assert_eq!(err, AnthropicAdapterError::InvalidMaxTokens);
    }
}
