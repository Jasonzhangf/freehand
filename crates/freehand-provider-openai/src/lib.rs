//! OpenAI-compatible provider adapter for Freehand.

use std::collections::BTreeMap;

use freehand_blocks::parse_tool_arguments_json;
use freehand_contracts::{ErrorClass, TerminalStatus, TokenUsage, ToolCallContract, ToolCallId};
use freehand_provider_core::{
    ProviderAdapterEvent, ProviderErrorHint, ProviderEventContext, ProviderProtocol,
    ProviderSemanticOutput, ProviderSemanticRequest, map_adapter_events,
};
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiRenderedRequest {
    pub path: &'static str,
    pub body: String,
}

#[derive(Debug, Default)]
pub struct OpenAiAdapter {
    partial_tool_calls: BTreeMap<String, PartialToolCallState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PartialToolCallState {
    tool_name: String,
    arguments_json: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OpenAiAdapterError {
    #[error("protocol `{0:?}` is not supported by OpenAI adapter")]
    UnsupportedProtocol(ProviderProtocol),
    #[error("invalid openai json payload: {0}")]
    InvalidJson(String),
    #[error("responses output item missing call id")]
    MissingCallId,
    #[error("responses output item missing tool name")]
    MissingToolName,
    #[error("chat tool call missing function payload")]
    MissingChatFunctionPayload,
    #[error("tool arguments invalid: {0}")]
    InvalidToolArguments(String),
}

impl OpenAiAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render_request(
        &self,
        request: &ProviderSemanticRequest,
        stream: bool,
    ) -> Result<OpenAiRenderedRequest, OpenAiAdapterError> {
        match request.descriptor.protocol {
            ProviderProtocol::OpenAiResponses => Ok(OpenAiRenderedRequest {
                path: "/responses",
                body: json!({
                    "model": request.descriptor.model,
                    "input": request.payload.rendered_input,
                    "stream": stream,
                })
                .to_string(),
            }),
            ProviderProtocol::OpenAiChatCompletions => Ok(OpenAiRenderedRequest {
                path: "/chat/completions",
                body: json!({
                    "model": request.descriptor.model,
                    "messages": [
                        {
                            "role": "user",
                            "content": request.payload.rendered_input,
                        }
                    ],
                    "stream": stream,
                })
                .to_string(),
            }),
            other => Err(OpenAiAdapterError::UnsupportedProtocol(other)),
        }
    }

    pub fn parse_response(
        &mut self,
        ctx: &ProviderEventContext,
        protocol: ProviderProtocol,
        body: &str,
    ) -> Result<Vec<ProviderSemanticOutput>, OpenAiAdapterError> {
        let value: Value = serde_json::from_str(body)
            .map_err(|err| OpenAiAdapterError::InvalidJson(err.to_string()))?;
        let events = match protocol {
            ProviderProtocol::OpenAiResponses => self.parse_responses_body(&value)?,
            ProviderProtocol::OpenAiChatCompletions => self.parse_chat_body(&value)?,
            other => return Err(OpenAiAdapterError::UnsupportedProtocol(other)),
        };
        Ok(map_adapter_events(ctx, events))
    }

    pub fn parse_stream_event(
        &mut self,
        ctx: &ProviderEventContext,
        protocol: ProviderProtocol,
        event_body: &str,
    ) -> Result<Vec<ProviderSemanticOutput>, OpenAiAdapterError> {
        if event_body.trim() == "[DONE]" {
            return Ok(Vec::new());
        }
        let value: Value = serde_json::from_str(event_body)
            .map_err(|err| OpenAiAdapterError::InvalidJson(err.to_string()))?;
        let events = match protocol {
            ProviderProtocol::OpenAiResponses => self.parse_responses_stream_event(&value)?,
            ProviderProtocol::OpenAiChatCompletions => self.parse_chat_stream_event(&value)?,
            other => return Err(OpenAiAdapterError::UnsupportedProtocol(other)),
        };
        Ok(map_adapter_events(ctx, events))
    }

    fn parse_responses_body(
        &mut self,
        value: &Value,
    ) -> Result<Vec<ProviderAdapterEvent>, OpenAiAdapterError> {
        let mut events = Vec::new();
        if let Some(output_items) = value.get("output").and_then(Value::as_array) {
            for item in output_items {
                let Some(kind) = item.get("type").and_then(Value::as_str) else {
                    continue;
                };
                match kind {
                    "message" => {
                        if let Some(content_items) = item.get("content").and_then(Value::as_array) {
                            for content in content_items {
                                if matches!(
                                    content.get("type").and_then(Value::as_str),
                                    Some("output_text" | "text")
                                ) && let Some(text) = content.get("text").and_then(Value::as_str)
                                    && !text.is_empty()
                                {
                                    events.push(ProviderAdapterEvent::TextDelta(text.to_owned()));
                                }
                            }
                        }
                    }
                    "reasoning" => {
                        if let Some(summary) = item.get("summary").and_then(Value::as_array) {
                            for entry in summary {
                                if let Some(text) = entry.get("text").and_then(Value::as_str)
                                    && !text.is_empty()
                                {
                                    events.push(ProviderAdapterEvent::ReasoningDelta(
                                        text.to_owned(),
                                    ));
                                }
                            }
                        }
                    }
                    "function_call" => {
                        let call_id = item
                            .get("call_id")
                            .or_else(|| item.get("id"))
                            .and_then(Value::as_str)
                            .ok_or(OpenAiAdapterError::MissingCallId)?;
                        let tool_name = item
                            .get("name")
                            .and_then(Value::as_str)
                            .ok_or(OpenAiAdapterError::MissingToolName)?;
                        let arguments = item
                            .get("arguments")
                            .and_then(Value::as_str)
                            .unwrap_or("{}");
                        events.push(ProviderAdapterEvent::ToolCall(ToolCallContract {
                            tool_call_id: ToolCallId::new(call_id),
                            tool_name: tool_name.to_owned(),
                            arguments: parse_tool_arguments_json(arguments).map_err(|err| {
                                OpenAiAdapterError::InvalidToolArguments(err.to_string())
                            })?,
                            arguments_complete: true,
                        }));
                    }
                    _ => {}
                }
            }
        }

        if let Some(usage) =
            parse_openai_usage(value.get("usage"), terminal_reason_from_responses(value))
        {
            events.push(ProviderAdapterEvent::Usage(usage));
        }

        if let Some(status) = value.get("status").and_then(Value::as_str)
            && matches!(status, "completed" | "failed" | "incomplete")
        {
            events.push(terminal_event_from_reason(status));
        }

        if let Some(error) = value.get("error") {
            events.push(ProviderAdapterEvent::Error(error_hint_from_value(error)));
        }
        Ok(events)
    }

    fn parse_chat_body(
        &mut self,
        value: &Value,
    ) -> Result<Vec<ProviderAdapterEvent>, OpenAiAdapterError> {
        let mut events = Vec::new();
        let mut finish_reason = None;
        if let Some(choices) = value.get("choices").and_then(Value::as_array) {
            for choice in choices {
                if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                    finish_reason = Some(reason.to_owned());
                }
                if let Some(message) = choice.get("message") {
                    if let Some(content) = message.get("content").and_then(Value::as_str)
                        && !content.is_empty()
                    {
                        events.push(ProviderAdapterEvent::TextDelta(content.to_owned()));
                    }
                    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
                        for tool_call in tool_calls {
                            events.push(self.parse_chat_tool_call(tool_call, true)?);
                        }
                    }
                }
            }
        }
        if let Some(usage) = parse_openai_usage(value.get("usage"), finish_reason.clone()) {
            events.push(ProviderAdapterEvent::Usage(usage));
        }
        if let Some(reason) = finish_reason {
            events.push(terminal_event_from_reason(&reason));
        }
        if let Some(error) = value.get("error") {
            events.push(ProviderAdapterEvent::Error(error_hint_from_value(error)));
        }
        Ok(events)
    }

    fn parse_responses_stream_event(
        &mut self,
        value: &Value,
    ) -> Result<Vec<ProviderAdapterEvent>, OpenAiAdapterError> {
        let mut events = Vec::new();
        let Some(event_type) = value.get("type").and_then(Value::as_str) else {
            return Ok(events);
        };
        match event_type {
            "response.output_text.delta" => {
                if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                    events.push(ProviderAdapterEvent::TextDelta(delta.to_owned()));
                }
            }
            "response.reasoning.delta" | "response.reasoning_summary_text.delta" => {
                if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                    events.push(ProviderAdapterEvent::ReasoningDelta(delta.to_owned()));
                }
            }
            "response.function_call_arguments.delta" => {
                events.push(
                    self.apply_partial_tool_delta(
                        value
                            .get("call_id")
                            .or_else(|| value.get("item_id"))
                            .and_then(Value::as_str)
                            .ok_or(OpenAiAdapterError::MissingCallId)?,
                        value.get("name").and_then(Value::as_str).unwrap_or(""),
                        value.get("delta").and_then(Value::as_str).unwrap_or(""),
                        false,
                    )?,
                );
            }
            "response.function_call_arguments.done" => {
                events.push(
                    self.apply_partial_tool_delta(
                        value
                            .get("call_id")
                            .or_else(|| value.get("item_id"))
                            .and_then(Value::as_str)
                            .ok_or(OpenAiAdapterError::MissingCallId)?,
                        value.get("name").and_then(Value::as_str).unwrap_or(""),
                        value.get("arguments").and_then(Value::as_str).unwrap_or(""),
                        true,
                    )?,
                );
            }
            "response.completed" => {
                if let Some(usage) = parse_openai_usage(
                    value.get("response").and_then(|v| v.get("usage")),
                    Some("completed".to_owned()),
                ) {
                    events.push(ProviderAdapterEvent::Usage(usage));
                }
                events.push(terminal_event_from_reason("completed"));
            }
            "response.failed" => {
                events.push(ProviderAdapterEvent::Error(error_hint_from_value(
                    value.get("error").unwrap_or(value),
                )));
                events.push(terminal_event_from_reason("failed"));
            }
            _ => {}
        }
        Ok(events)
    }

    fn parse_chat_stream_event(
        &mut self,
        value: &Value,
    ) -> Result<Vec<ProviderAdapterEvent>, OpenAiAdapterError> {
        let mut events = Vec::new();
        let usage = parse_openai_usage(value.get("usage"), None);
        if let Some(choices) = value.get("choices").and_then(Value::as_array) {
            for choice in choices {
                let finish_reason = choice
                    .get("finish_reason")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                if let Some(delta) = choice.get("delta") {
                    if let Some(content) = delta.get("content").and_then(Value::as_str)
                        && !content.is_empty()
                    {
                        events.push(ProviderAdapterEvent::TextDelta(content.to_owned()));
                    }
                    if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                        for tool_call in tool_calls {
                            events.push(
                                self.parse_chat_tool_call(tool_call, finish_reason.is_some())?,
                            );
                        }
                    }
                }
                if let Some(reason) = finish_reason.as_deref() {
                    events.push(terminal_event_from_reason(reason));
                }
            }
        }
        if let Some(usage) = usage {
            events.push(ProviderAdapterEvent::Usage(usage));
        }
        Ok(events)
    }

    fn parse_chat_tool_call(
        &mut self,
        value: &Value,
        is_complete: bool,
    ) -> Result<ProviderAdapterEvent, OpenAiAdapterError> {
        let call_id = value
            .get("id")
            .or_else(|| value.get("tool_call_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| {
                value
                    .get("index")
                    .and_then(Value::as_u64)
                    .map(|index| format!("index-{index}"))
            })
            .ok_or(OpenAiAdapterError::MissingCallId)?;
        let function = value
            .get("function")
            .ok_or(OpenAiAdapterError::MissingChatFunctionPayload)?;
        let tool_name = function.get("name").and_then(Value::as_str).unwrap_or("");
        let arguments_delta = function
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("");
        self.apply_partial_tool_delta(&call_id, tool_name, arguments_delta, is_complete)
    }

    fn apply_partial_tool_delta(
        &mut self,
        call_id: &str,
        tool_name: &str,
        delta: &str,
        is_complete: bool,
    ) -> Result<ProviderAdapterEvent, OpenAiAdapterError> {
        let state = self
            .partial_tool_calls
            .entry(call_id.to_owned())
            .or_insert_with(|| PartialToolCallState {
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
            tool_call_id: ToolCallId::new(call_id),
            tool_name: state.tool_name.clone(),
            arguments,
            arguments_complete: is_complete,
        });
        if is_complete {
            self.partial_tool_calls.remove(call_id);
        }
        Ok(event)
    }
}

fn terminal_reason_from_responses(value: &Value) -> Option<String> {
    value
        .get("status")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn terminal_event_from_reason(reason: &str) -> ProviderAdapterEvent {
    let status = match reason {
        "tool_calls" | "tool_use" => TerminalStatus::ToolPending,
        "failed" => TerminalStatus::Failed,
        "incomplete" | "length" => TerminalStatus::Interrupted,
        "cancelled" => TerminalStatus::Cancelled,
        _ => TerminalStatus::Success,
    };
    ProviderAdapterEvent::Terminal {
        status,
        summary: reason.to_owned(),
    }
}

fn parse_openai_usage(usage: Option<&Value>, finish_reason: Option<String>) -> Option<TokenUsage> {
    let usage = usage?;
    let input_tokens = usage
        .get("input_tokens")
        .or_else(|| usage.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .or_else(|| usage.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage.get("total_tokens").and_then(Value::as_u64);
    let reasoning_tokens = usage
        .get("output_tokens_details")
        .and_then(|details| details.get("reasoning_tokens"))
        .or_else(|| {
            usage
                .get("completion_tokens_details")
                .and_then(|details| details.get("reasoning_tokens"))
        })
        .and_then(Value::as_u64);
    let cache_creation_tokens = usage
        .get("input_tokens_details")
        .and_then(|details| details.get("cache_creation_tokens"))
        .or_else(|| usage.get("cache_creation_input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read_tokens = usage
        .get("input_tokens_details")
        .and_then(|details| details.get("cached_tokens"))
        .or_else(|| usage.get("cache_read_input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Some(TokenUsage {
        input_tokens,
        output_tokens,
        total_tokens,
        reasoning_tokens,
        cache_creation_tokens,
        cache_read_tokens,
        finish_reason,
    })
}

fn error_hint_from_value(value: &Value) -> ProviderErrorHint {
    let code = value
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("openai_error")
        .to_owned();
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("openai request failed")
        .to_owned();
    let class = match code.as_str() {
        "invalid_api_key" | "authentication_error" => ErrorClass::Auth,
        "rate_limit_exceeded" | "rate_limit" => ErrorClass::RateLimit,
        "invalid_request_error" | "json_validation_error" => ErrorClass::Protocol,
        "unsupported_protocol" | "unsupported_feature" => ErrorClass::Unsupported,
        _ => ErrorClass::Upstream,
    };
    let retry_after_seconds = value
        .get("retry_after")
        .and_then(Value::as_u64)
        .or_else(|| value.get("retry_after_seconds").and_then(Value::as_u64));
    ProviderErrorHint {
        code,
        message,
        class,
        retry_after_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{
        AgentId, FeatureId, ReasonReq03ProviderPayload, SessionId, TraceId, TurnId,
    };
    use freehand_provider_core::{
        ProviderCapabilities, ProviderDescriptor, ProviderFamily, RawRetentionPolicy,
        build_semantic_request,
    };

    fn ctx() -> ProviderEventContext {
        ProviderEventContext {
            agent_id: AgentId::new("agent-1"),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("provider.openai-adapter"),
        }
    }

    fn semantic_request(protocol: ProviderProtocol) -> ProviderSemanticRequest {
        build_semantic_request(
            ProviderDescriptor {
                provider_name: "openai".to_owned(),
                family: ProviderFamily::OpenAiCompatible,
                protocol,
                model: "gpt-test".to_owned(),
                capabilities: ProviderCapabilities {
                    web_search: true,
                    multimodal: false,
                    vision: true,
                    reasoning: true,
                },
            },
            ReasonReq03ProviderPayload {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("provider.openai-adapter"),
                agent_id: AgentId::new("agent-1"),
                model: "gpt-test".to_owned(),
                rendered_input: "hello".to_owned(),
            },
            true,
        )
        .expect("request")
    }

    #[test]
    fn renders_responses_request() {
        let adapter = OpenAiAdapter::new();
        let rendered = adapter
            .render_request(&semantic_request(ProviderProtocol::OpenAiResponses), true)
            .expect("render");
        assert_eq!(rendered.path, "/responses");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        assert_eq!(body.get("input").and_then(Value::as_str), Some("hello"));
        assert_eq!(body.get("stream").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn renders_chat_completions_request() {
        let adapter = OpenAiAdapter::new();
        let rendered = adapter
            .render_request(
                &semantic_request(ProviderProtocol::OpenAiChatCompletions),
                false,
            )
            .expect("render");
        assert_eq!(rendered.path, "/chat/completions");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        assert_eq!(
            body.get("messages")
                .and_then(Value::as_array)
                .and_then(|messages| messages.first())
                .and_then(|message| message.get("role"))
                .and_then(Value::as_str),
            Some("user")
        );
    }

    #[test]
    fn parses_responses_single_shot_with_reasoning_tool_and_usage() {
        let mut adapter = OpenAiAdapter::new();
        let outputs = adapter
            .parse_response(
                &ctx(),
                ProviderProtocol::OpenAiResponses,
                r#"{
                    "status":"completed",
                    "output":[
                        {"type":"reasoning","summary":[{"text":"thinking"}]},
                        {"type":"message","content":[{"type":"output_text","text":"answer"}]},
                        {"type":"function_call","call_id":"call-1","name":"search","arguments":"{\"query\":\"rust\"}"}
                    ],
                    "usage":{
                        "input_tokens":10,
                        "output_tokens":4,
                        "total_tokens":14,
                        "output_tokens_details":{"reasoning_tokens":2},
                        "input_tokens_details":{"cached_tokens":3}
                    }
                }"#,
            )
            .expect("parsed");
        assert_eq!(outputs.len(), 5);
    }

    #[test]
    fn parses_chat_completions_stream_with_partial_tool_call() {
        let mut adapter = OpenAiAdapter::new();
        let first = adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::OpenAiChatCompletions,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"search","arguments":"{\"query\":\"ru"}}]}}]}"#,
            )
            .expect("first");
        match &first[0] {
            ProviderSemanticOutput::ToolCall(call) => {
                assert!(!call.tool_call.arguments_complete);
                assert!(call.tool_call.arguments.is_empty());
            }
            other => panic!("unexpected output: {other:?}"),
        }

        let second = adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::OpenAiChatCompletions,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"arguments":"st\"}"}}],"content":"ok"},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":8,"completion_tokens":2,"total_tokens":10}}"#,
            )
            .expect("second");
        assert!(second.iter().any(|output| matches!(output, ProviderSemanticOutput::SemanticEvent(event) if event.content == "ok")));
        assert!(second.iter().any(|output| matches!(output, ProviderSemanticOutput::ToolCall(call) if call.tool_call.arguments_complete && call.tool_call.arguments.len() == 1)));
    }

    #[test]
    fn parses_responses_stream_events() {
        let mut adapter = OpenAiAdapter::new();
        let outputs = adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::OpenAiResponses,
                r#"{"type":"response.output_text.delta","delta":"hello"}"#,
            )
            .expect("parsed");
        assert!(matches!(
            &outputs[0],
            ProviderSemanticOutput::SemanticEvent(event) if event.content == "hello"
        ));
    }

    #[test]
    fn build_request_keeps_debug_retention_in_core() {
        let request = semantic_request(ProviderProtocol::OpenAiResponses);
        assert_eq!(request.raw_retention, RawRetentionPolicy::DebugOnly);
    }
}
