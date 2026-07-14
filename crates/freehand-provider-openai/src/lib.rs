//! OpenAI-compatible provider adapter for Freehand.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader};

use freehand_blocks::{
    parse_tool_arguments_json, render_context_segments_as_text, render_tool_arguments_json,
};
use freehand_contracts::{
    ErrorClass, TerminalStatus, TokenUsage, ToolCallContract, ToolCallId, ToolResultStatus,
};
use freehand_provider_core::{
    ProviderAdapterEvent, ProviderErrorHint, ProviderEventContext, ProviderProtocol,
    ProviderSemanticOutput, ProviderSemanticRequest, ProviderToolChoice, ProviderToolExchange,
    map_adapter_events,
};
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiRenderedRequest {
    pub path: &'static str,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiExecutorConfig {
    pub base_url: String,
    pub api_key: String,
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

#[derive(Debug, Error)]
pub enum OpenAiExecutorError {
    #[error("openai executor base_url and api_key must be non-empty")]
    InvalidConfig,
    #[error(transparent)]
    Adapter(#[from] OpenAiAdapterError),
    #[error("openai http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("openai http status `{status}` returned body `{body}`")]
    HttpStatus { status: u16, body: String },
    #[error("openai stream read failed: {0}")]
    StreamRead(#[from] io::Error),
    #[error("openai stream callback failed: {0}")]
    Callback(String),
}

pub struct OpenAiExecutor {
    config: OpenAiExecutorConfig,
    client: reqwest::blocking::Client,
    adapter: OpenAiAdapter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAiRawCapture {
    ResponseBody {
        body: String,
    },
    HttpErrorBody {
        status: u16,
        body: String,
    },
    StreamEventBody {
        event_index: usize,
        event_body: String,
    },
}

impl OpenAiExecutor {
    pub fn new(config: OpenAiExecutorConfig) -> Result<Self, OpenAiExecutorError> {
        if config.base_url.trim().is_empty() || config.api_key.trim().is_empty() {
            return Err(OpenAiExecutorError::InvalidConfig);
        }
        Ok(Self {
            config,
            client: reqwest::blocking::Client::new(),
            adapter: OpenAiAdapter::new(),
        })
    }

    pub fn execute_once(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
    ) -> Result<Vec<ProviderSemanticOutput>, OpenAiExecutorError> {
        self.execute_once_with_raw(ctx, request, |_| Ok(()))
    }

    pub fn execute_once_with_raw<F>(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        mut on_raw: F,
    ) -> Result<Vec<ProviderSemanticOutput>, OpenAiExecutorError>
    where
        F: FnMut(&OpenAiRawCapture) -> Result<(), OpenAiExecutorError>,
    {
        let rendered = self.adapter.render_request(request, false)?;
        let response = match self.send_rendered_request(&rendered) {
            Ok(response) => response,
            Err(OpenAiExecutorError::HttpStatus { status, body }) => {
                on_raw(&OpenAiRawCapture::HttpErrorBody {
                    status,
                    body: body.clone(),
                })?;
                return Err(OpenAiExecutorError::HttpStatus { status, body });
            }
            Err(other) => return Err(other),
        };
        let body = response.text()?;
        on_raw(&OpenAiRawCapture::ResponseBody { body: body.clone() })?;
        Ok(self
            .adapter
            .parse_response(ctx, request.descriptor.protocol, &body)?)
    }

    pub fn execute_stream(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
    ) -> Result<Vec<ProviderSemanticOutput>, OpenAiExecutorError> {
        self.execute_stream_with(ctx, request, |_| Ok(()))
    }

    pub fn execute_stream_with<F>(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        mut on_outputs: F,
    ) -> Result<Vec<ProviderSemanticOutput>, OpenAiExecutorError>
    where
        F: FnMut(&[ProviderSemanticOutput]) -> Result<(), OpenAiExecutorError>,
    {
        self.execute_stream_with_raw(ctx, request, |_| Ok(()), |batch| on_outputs(batch))
    }

    pub fn execute_stream_with_raw<FR, FO>(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        mut on_raw: FR,
        mut on_outputs: FO,
    ) -> Result<Vec<ProviderSemanticOutput>, OpenAiExecutorError>
    where
        FR: FnMut(&OpenAiRawCapture) -> Result<(), OpenAiExecutorError>,
        FO: FnMut(&[ProviderSemanticOutput]) -> Result<(), OpenAiExecutorError>,
    {
        let rendered = self.adapter.render_request(request, true)?;
        let response = match self.send_rendered_request(&rendered) {
            Ok(response) => response,
            Err(OpenAiExecutorError::HttpStatus { status, body }) => {
                on_raw(&OpenAiRawCapture::HttpErrorBody {
                    status,
                    body: body.clone(),
                })?;
                return Err(OpenAiExecutorError::HttpStatus { status, body });
            }
            Err(other) => return Err(other),
        };
        let mut reader = BufReader::new(response);
        let mut outputs = Vec::new();
        let mut collector = SseEventCollector::default();
        let mut line = String::new();
        let mut event_index = 0usize;
        loop {
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            let Some(event_body) = collector.push_line(&line) else {
                continue;
            };
            event_index = event_index.saturating_add(1);
            on_raw(&OpenAiRawCapture::StreamEventBody {
                event_index,
                event_body: event_body.clone(),
            })?;
            let batch =
                self.adapter
                    .parse_stream_event(ctx, request.descriptor.protocol, &event_body)?;
            on_outputs(&batch)?;
            outputs.extend(batch);
        }
        if let Some(event_body) = collector.finish() {
            event_index = event_index.saturating_add(1);
            on_raw(&OpenAiRawCapture::StreamEventBody {
                event_index,
                event_body: event_body.clone(),
            })?;
            let batch =
                self.adapter
                    .parse_stream_event(ctx, request.descriptor.protocol, &event_body)?;
            on_outputs(&batch)?;
            outputs.extend(batch);
        }
        Ok(outputs)
    }

    fn send_rendered_request(
        &self,
        rendered: &OpenAiRenderedRequest,
    ) -> Result<reqwest::blocking::Response, OpenAiExecutorError> {
        let response = self
            .client
            .post(join_base_url_path(&self.config.base_url, rendered.path))
            .bearer_auth(&self.config.api_key)
            .header("content-type", "application/json")
            .body(rendered.body.clone())
            .send()?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text()?;
            return Err(OpenAiExecutorError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }
        Ok(response)
    }
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
        let rendered_input = render_context_segments_as_text(&request.payload.input_segments);
        match request.descriptor.protocol {
            ProviderProtocol::OpenAiResponses => Ok(OpenAiRenderedRequest {
                path: "/responses",
                body: {
                    let mut body = json!({
                    "model": request.descriptor.model,
                    "input": render_responses_input(&rendered_input, &request.tool_exchanges)?,
                    "stream": stream,
                    });
                    if !request.tools.is_empty() {
                        body["tools"] = Value::Array(
                            request
                                .tools
                                .iter()
                                .map(|tool| {
                                    json!({
                                        "type": "function",
                                        "name": tool.name,
                                        "description": tool.description,
                                        "parameters": tool.input_schema,
                                    })
                                })
                                .collect(),
                        );
                    }
                    if let Some(choice) = &request.tool_choice {
                        body["tool_choice"] = openai_tool_choice(choice);
                    }
                    body.to_string()
                },
            }),
            ProviderProtocol::OpenAiChatCompletions => Ok(OpenAiRenderedRequest {
                path: "/chat/completions",
                body: {
                    let mut body = json!({
                    "model": request.descriptor.model,
                    "messages": render_chat_messages(&rendered_input, &request.tool_exchanges)?,
                    "stream": stream,
                    });
                    if !request.tools.is_empty() {
                        body["tools"] = Value::Array(
                            request
                                .tools
                                .iter()
                                .map(|tool| {
                                    json!({
                                        "type": "function",
                                        "function": {
                                            "name": tool.name,
                                            "description": tool.description,
                                            "parameters": tool.input_schema,
                                        },
                                    })
                                })
                                .collect(),
                        );
                    }
                    if let Some(choice) = &request.tool_choice {
                        body["tool_choice"] = openai_tool_choice(choice);
                    }
                    body.to_string()
                },
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

        if let Some(error) = value.get("error").filter(|error| !error.is_null()) {
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
        if let Some(error) = value.get("error").filter(|error| !error.is_null()) {
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

fn render_responses_input(
    rendered_input: &str,
    tool_exchanges: &[ProviderToolExchange],
) -> Result<Value, OpenAiAdapterError> {
    let mut items = vec![json!({
        "type": "message",
        "role": "user",
        "content": [
            {
                "type": "input_text",
                "text": rendered_input,
            }
        ],
    })];
    for exchange in tool_exchanges {
        let arguments = render_tool_arguments_json(&exchange.tool_call.tool_call.arguments)
            .map_err(|err| OpenAiAdapterError::InvalidToolArguments(err.to_string()))?;
        items.push(json!({
            "type": "function_call",
            "call_id": exchange.tool_call.tool_call.tool_call_id.as_str(),
            "name": exchange.tool_call.tool_call.tool_name,
            "arguments": arguments,
        }));
        items.push(json!({
            "type": "function_call_output",
            "call_id": exchange.tool_result.tool_result.tool_call_id.as_str(),
            "output": exchange.tool_result.tool_result.output,
        }));
    }
    Ok(Value::Array(items))
}

fn render_chat_messages(
    rendered_input: &str,
    tool_exchanges: &[ProviderToolExchange],
) -> Result<Value, OpenAiAdapterError> {
    let mut messages = vec![json!({
        "role": "user",
        "content": rendered_input,
    })];
    if !tool_exchanges.is_empty() {
        messages.push(json!({
            "role": "assistant",
            "content": null,
            "tool_calls": tool_exchanges
                .iter()
                .map(|exchange| {
                    let arguments =
                        render_tool_arguments_json(&exchange.tool_call.tool_call.arguments)
                            .map_err(|err| {
                                OpenAiAdapterError::InvalidToolArguments(err.to_string())
                            })?;
                    Ok(json!({
                        "id": exchange.tool_call.tool_call.tool_call_id.as_str(),
                        "type": "function",
                        "function": {
                            "name": exchange.tool_call.tool_call.tool_name,
                            "arguments": arguments,
                        },
                    }))
                })
                .collect::<Result<Vec<_>, OpenAiAdapterError>>()?,
        }));
        for exchange in tool_exchanges {
            let content = if exchange.tool_result.tool_result.status == ToolResultStatus::Failed {
                format!("ERROR: {}", exchange.tool_result.tool_result.output)
            } else {
                exchange.tool_result.tool_result.output.clone()
            };
            messages.push(json!({
                "role": "tool",
                "tool_call_id": exchange.tool_result.tool_result.tool_call_id.as_str(),
                "content": content,
            }));
        }
    }
    Ok(Value::Array(messages))
}

fn openai_tool_choice(choice: &ProviderToolChoice) -> Value {
    match choice {
        ProviderToolChoice::Auto => json!("auto"),
        ProviderToolChoice::Required { name } => json!({
            "type": "function",
            "function": {"name": name},
        }),
    }
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

fn join_base_url_path(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

#[derive(Debug, Default)]
struct SseEventCollector {
    data_lines: Vec<String>,
}

impl SseEventCollector {
    fn push_line(&mut self, raw_line: &str) -> Option<String> {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return self.finish();
        }
        if let Some(data) = line.strip_prefix("data:") {
            self.data_lines
                .push(data.strip_prefix(' ').unwrap_or(data).to_owned());
        }
        None
    }

    fn finish(&mut self) -> Option<String> {
        if self.data_lines.is_empty() {
            return None;
        }
        Some(std::mem::take(&mut self.data_lines).join("\n"))
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
                input_segments: vec![freehand_contracts::ContextSegment {
                    segment_id: freehand_contracts::ContextSegmentId::new("segment-user"),
                    kind: freehand_contracts::ContextSegmentKind::UserTurnInput,
                    stability: freehand_contracts::ContextStability::TurnVolatile,
                    cache_policy: freehand_contracts::ContextCachePolicy::NoCache,
                    role: freehand_contracts::ContextRole::User,
                    content: "hello".to_owned(),
                    token_budget: 64,
                    provenance: freehand_contracts::ContextProvenance {
                        source: "turn_input".to_owned(),
                        reference: None,
                    },
                }],
            },
            true,
        )
        .expect("request")
    }

    fn tool_request(protocol: ProviderProtocol) -> ProviderSemanticRequest {
        let mut request = semantic_request(protocol);
        request
            .tools
            .push(freehand_provider_core::ProviderToolDefinition {
                name: "read_file".to_owned(),
                description: "Read one UTF-8 file inside the locked workspace.".to_owned(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }),
            });
        request.tool_choice = Some(ProviderToolChoice::Auto);
        request.tool_exchanges.push(ProviderToolExchange {
            tool_call: freehand_contracts::ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("provider.openai-adapter"),
                agent_id: AgentId::new("agent-1"),
                tool_call: ToolCallContract {
                    tool_call_id: ToolCallId::new("call-1"),
                    tool_name: "read_file".to_owned(),
                    arguments: vec![freehand_contracts::ToolArgument {
                        name: "path".to_owned(),
                        value: json!("README.md"),
                    }],
                    arguments_complete: true,
                },
            },
            tool_result: freehand_contracts::ReasonReq05ToolResultReentry {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("provider.openai-adapter"),
                agent_id: AgentId::new("agent-1"),
                tool_result: freehand_contracts::ToolResultContract {
                    tool_call_id: ToolCallId::new("call-1"),
                    status: ToolResultStatus::Success,
                    output: "file contents".to_owned(),
                },
            },
        });
        request
    }

    #[test]
    fn renders_responses_request() {
        let adapter = OpenAiAdapter::new();
        let rendered = adapter
            .render_request(&semantic_request(ProviderProtocol::OpenAiResponses), true)
            .expect("render");
        assert_eq!(rendered.path, "/responses");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        let input = body.get("input").and_then(Value::as_array).expect("input");
        let text = input[0]["content"][0]["text"].as_str().expect("input text");
        assert!(text.contains("kind=\"user_turn_input\""));
        assert!(text.contains("\nhello\n"));
        assert_eq!(body.get("stream").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn renders_responses_tools_and_tool_result_reentry() {
        let adapter = OpenAiAdapter::new();
        let rendered = adapter
            .render_request(&tool_request(ProviderProtocol::OpenAiResponses), true)
            .expect("render");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        assert_eq!(body["tools"][0]["type"], json!("function"));
        assert_eq!(body["tools"][0]["name"], json!("read_file"));
        assert_eq!(body["tools"][0]["parameters"]["required"][0], json!("path"));
        assert_eq!(body["tool_choice"], json!("auto"));
        let input = body["input"].as_array().expect("input");
        assert!(
            input
                .iter()
                .any(|item| item["type"] == json!("function_call")
                    && item["name"] == json!("read_file")
                    && item["arguments"] == json!(r#"{"path":"README.md"}"#))
        );
        assert!(
            input
                .iter()
                .any(|item| item["type"] == json!("function_call_output")
                    && item["call_id"] == json!("call-1")
                    && item["output"] == json!("file contents"))
        );
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
        assert!(
            body.get("messages")
                .and_then(Value::as_array)
                .and_then(|messages| messages.first())
                .and_then(|message| message.get("content"))
                .and_then(Value::as_str)
                .is_some_and(|content| content.contains("kind=\"user_turn_input\""))
        );
    }

    #[test]
    fn renders_chat_completions_tools_and_tool_result_reentry() {
        let adapter = OpenAiAdapter::new();
        let rendered = adapter
            .render_request(
                &tool_request(ProviderProtocol::OpenAiChatCompletions),
                false,
            )
            .expect("render");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        assert_eq!(body["tools"][0]["type"], json!("function"));
        assert_eq!(body["tools"][0]["function"]["name"], json!("read_file"));
        let messages = body["messages"].as_array().expect("messages");
        assert!(
            messages
                .iter()
                .any(|message| message["role"] == json!("assistant")
                    && message["tool_calls"][0]["function"]["name"] == json!("read_file")
                    && message["tool_calls"][0]["function"]["arguments"]
                        == json!(r#"{"path":"README.md"}"#))
        );
        assert!(
            messages
                .iter()
                .any(|message| message["role"] == json!("tool")
                    && message["tool_call_id"] == json!("call-1")
                    && message["content"] == json!("file contents"))
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
    fn responses_success_with_null_error_does_not_emit_error_semantics() {
        let mut adapter = OpenAiAdapter::new();
        let outputs = adapter
            .parse_response(
                &ctx(),
                ProviderProtocol::OpenAiResponses,
                r#"{
                    "status":"completed",
                    "error":null,
                    "output":[
                        {"type":"message","content":[{"type":"output_text","text":"done"}]}
                    ]
                }"#,
            )
            .expect("parsed");

        assert!(outputs.iter().any(|output| matches!(
            output,
            ProviderSemanticOutput::SemanticEvent(event) if event.content == "done"
        )));
        assert!(outputs.iter().any(|output| matches!(
            output,
            ProviderSemanticOutput::Terminal(event) if event.status == TerminalStatus::Success
        )));
        assert!(
            !outputs
                .iter()
                .any(|output| matches!(output, ProviderSemanticOutput::Error(_)))
        );
    }

    #[test]
    fn responses_non_null_error_still_emits_error_semantics() {
        let mut adapter = OpenAiAdapter::new();
        let outputs = adapter
            .parse_response(
                &ctx(),
                ProviderProtocol::OpenAiResponses,
                r#"{
                    "status":"failed",
                    "error":{"code":"rate_limit_exceeded","message":"retry later"},
                    "output":[]
                }"#,
            )
            .expect("parsed");

        assert!(outputs.iter().any(|output| matches!(
            output,
            ProviderSemanticOutput::Error(event)
                if event.error.code == "rate_limit_exceeded"
                    && event.error.message == "retry later"
        )));
    }

    #[test]
    fn chat_success_with_null_error_does_not_emit_error_semantics() {
        let mut adapter = OpenAiAdapter::new();
        let outputs = adapter
            .parse_response(
                &ctx(),
                ProviderProtocol::OpenAiChatCompletions,
                r#"{
                    "choices":[{
                        "message":{"content":"done"},
                        "finish_reason":"stop"
                    }],
                    "error":null
                }"#,
            )
            .expect("parsed");

        assert!(
            !outputs
                .iter()
                .any(|output| matches!(output, ProviderSemanticOutput::Error(_)))
        );
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
