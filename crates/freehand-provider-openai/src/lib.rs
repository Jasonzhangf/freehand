//! OpenAI-compatible provider adapter for Freehand.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader};
use std::time::Duration;

use freehand_blocks::{
    parse_tool_arguments_json, render_context_segments_as_text, render_tool_arguments_json,
};
use freehand_contracts::{
    ErrorClass, SearchSocialPlatform, TerminalStatus, TokenUsage, ToolCallContract, ToolCallId,
    ToolResultStatus,
};
use freehand_provider_core::{
    ProviderAdapterEvent, ProviderErrorHint, ProviderEventContext, ProviderExecutorConfig,
    ProviderExecutorErrorInfo, ProviderExecutorFactory, ProviderExecutorFactoryError,
    ProviderFamily, ProviderHostedSearchCandidate, ProviderHostedSearchDiscovery,
    ProviderHostedToolDefinition, ProviderInputAttachment, ProviderInputAttachmentKind,
    ProviderLiveExecutor, ProviderLiveExecutorError, ProviderProtocol, ProviderRawCapture,
    ProviderSemanticOutput, ProviderSemanticRequest, ProviderToolChoice, ProviderToolExchange,
    ProviderWebSearchCapability, ProviderWebSearchToolType, map_adapter_events,
    project_hosted_search_discovery,
};
use serde_json::{Value, json};
use thiserror::Error;

const PROVIDER_BLOCKING_CLIENT_TIMEOUT: Duration = Duration::from_secs(120);

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

#[derive(Debug, Clone, Default)]
pub struct OpenAiExecutorFactory;

impl ProviderExecutorFactory for OpenAiExecutorFactory {
    fn build_executor(
        &self,
        config: ProviderExecutorConfig,
    ) -> Result<Box<dyn ProviderLiveExecutor>, ProviderExecutorFactoryError> {
        if config.descriptor.family != ProviderFamily::OpenAiCompatible
            || (config.descriptor.protocol != ProviderProtocol::OpenAiResponses
                && config.descriptor.protocol != ProviderProtocol::OpenAiChatCompletions)
        {
            return Err(ProviderExecutorFactoryError::Unsupported {
                family: config.descriptor.family,
                protocol: config.descriptor.protocol,
            });
        }
        OpenAiExecutor::new(OpenAiExecutorConfig {
            base_url: config.base_url,
            api_key: config.api_key,
        })
        .map(|executor| Box::new(executor) as Box<dyn ProviderLiveExecutor>)
        .map_err(|err| {
            ProviderExecutorFactoryError::BuildFailed(classify_openai_executor_error(&err))
        })
    }
}

impl ProviderLiveExecutor for OpenAiExecutor {
    fn execute_once_with_raw(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        on_raw: &mut dyn FnMut(ProviderRawCapture<'_>) -> Result<(), String>,
    ) -> Result<Vec<ProviderSemanticOutput>, ProviderLiveExecutorError> {
        OpenAiExecutor::execute_once_with_raw(self, ctx, request, |raw| {
            on_raw(openai_raw_capture(raw)).map_err(OpenAiExecutorError::Callback)
        })
        .map_err(|err| ProviderLiveExecutorError::new(classify_openai_executor_error(&err)))
    }

    fn execute_stream_with_raw(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        on_raw: &mut dyn FnMut(ProviderRawCapture<'_>) -> Result<(), String>,
        on_outputs: &mut dyn FnMut(&[ProviderSemanticOutput]) -> Result<(), String>,
    ) -> Result<Vec<ProviderSemanticOutput>, ProviderLiveExecutorError> {
        OpenAiExecutor::execute_stream_with_raw(
            self,
            ctx,
            request,
            |raw| on_raw(openai_raw_capture(raw)).map_err(OpenAiExecutorError::Callback),
            |batch| on_outputs(batch).map_err(OpenAiExecutorError::Callback),
        )
        .map_err(|err| ProviderLiveExecutorError::new(classify_openai_executor_error(&err)))
    }
}

fn openai_raw_capture(raw: &OpenAiRawCapture) -> ProviderRawCapture<'_> {
    match raw {
        OpenAiRawCapture::ResponseBody { body } => ProviderRawCapture::Response {
            crate_name: "freehand-provider-openai",
            function: "OpenAiExecutor::execute_once_with_raw",
            body,
        },
        OpenAiRawCapture::HttpErrorBody { status, body } => ProviderRawCapture::HttpError {
            crate_name: "freehand-provider-openai",
            function: "OpenAiExecutor::send_rendered_request",
            status: *status,
            body,
        },
        OpenAiRawCapture::StreamEventBody {
            event_index,
            event_body,
        } => ProviderRawCapture::StreamEvent {
            crate_name: "freehand-provider-openai",
            function: "OpenAiExecutor::execute_stream_with_raw",
            event_index: *event_index,
            event_body,
        },
    }
}

pub fn classify_openai_executor_error(err: &OpenAiExecutorError) -> ProviderExecutorErrorInfo {
    match err {
        OpenAiExecutorError::HttpStatus { status, body } => ProviderExecutorErrorInfo {
            code: format!("openai_http_status_{status}"),
            message: body.clone(),
            retryable: *status == 408
                || *status == 409
                || *status == 425
                || *status == 429
                || *status >= 500,
            failover_eligible: true,
        },
        OpenAiExecutorError::Http(err) => ProviderExecutorErrorInfo {
            code: "openai_http_request_failed".to_owned(),
            message: err.to_string(),
            retryable: err.is_connect() || err.is_timeout() || err.is_request(),
            failover_eligible: true,
        },
        OpenAiExecutorError::StreamRead(err) => ProviderExecutorErrorInfo {
            code: "openai_stream_read_failed".to_owned(),
            message: err.to_string(),
            retryable: true,
            failover_eligible: true,
        },
        OpenAiExecutorError::Adapter(err) => ProviderExecutorErrorInfo {
            code: "openai_adapter_failed".to_owned(),
            message: err.to_string(),
            retryable: false,
            failover_eligible: false,
        },
        OpenAiExecutorError::InvalidConfig => ProviderExecutorErrorInfo {
            code: "openai_invalid_config".to_owned(),
            message: err.to_string(),
            retryable: false,
            failover_eligible: false,
        },
        OpenAiExecutorError::Callback(message) => ProviderExecutorErrorInfo {
            code: "openai_callback_failed".to_owned(),
            message: message.clone(),
            retryable: false,
            failover_eligible: false,
        },
    }
}

impl OpenAiExecutor {
    pub fn new(config: OpenAiExecutorConfig) -> Result<Self, OpenAiExecutorError> {
        if config.base_url.trim().is_empty() || config.api_key.trim().is_empty() {
            return Err(OpenAiExecutorError::InvalidConfig);
        }
        Ok(Self {
            config,
            client: reqwest::blocking::Client::builder()
                .timeout(PROVIDER_BLOCKING_CLIENT_TIMEOUT)
                .build()?,
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
                    "input": render_responses_input(&rendered_input, &request.input_attachments, &request.tool_exchanges)?,
                    "stream": stream,
                    });
                    let tools = openai_responses_tools(request);
                    if !tools.is_empty() {
                        body["tools"] = Value::Array(tools);
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
                    "messages": render_chat_messages(&rendered_input, &request.input_attachments, &request.tool_exchanges)?,
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
            ProviderProtocol::OpenAiResponses => self.parse_responses_body(ctx, &value)?,
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
            ProviderProtocol::OpenAiResponses => self.parse_responses_stream_event(ctx, &value)?,
            ProviderProtocol::OpenAiChatCompletions => self.parse_chat_stream_event(&value)?,
            other => return Err(OpenAiAdapterError::UnsupportedProtocol(other)),
        };
        Ok(map_adapter_events(ctx, events))
    }

    fn parse_responses_body(
        &mut self,
        ctx: &ProviderEventContext,
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
                    "web_search_call" => {
                        if let Some(discovery) = openai_hosted_search_discovery(ctx, item) {
                            events.push(ProviderAdapterEvent::SearchDiscovery(discovery));
                        }
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
        ctx: &ProviderEventContext,
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
            "response.output_item.added" | "response.output_item.done" => {
                if let Some(item) = value.get("item")
                    && item.get("type").and_then(Value::as_str) == Some("web_search_call")
                    && event_type == "response.output_item.done"
                    && let Some(discovery) = openai_hosted_search_discovery(ctx, item)
                {
                    events.push(ProviderAdapterEvent::SearchDiscovery(discovery));
                }
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
    attachments: &[ProviderInputAttachment],
    tool_exchanges: &[ProviderToolExchange],
) -> Result<Value, OpenAiAdapterError> {
    let mut content = vec![json!({
        "type": "input_text",
        "text": rendered_input,
    })];
    content.extend(attachments.iter().map(openai_responses_attachment_content));
    let mut items = vec![json!({
        "type": "message",
        "role": "user",
        "content": content,
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
    attachments: &[ProviderInputAttachment],
    tool_exchanges: &[ProviderToolExchange],
) -> Result<Value, OpenAiAdapterError> {
    let user_content = if attachments.is_empty() {
        json!(rendered_input)
    } else {
        let mut content = vec![json!({
            "type": "text",
            "text": rendered_input,
        })];
        content.extend(attachments.iter().map(openai_chat_attachment_content));
        Value::Array(content)
    };
    let mut messages = vec![json!({
        "role": "user",
        "content": user_content,
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

fn openai_responses_attachment_content(attachment: &ProviderInputAttachment) -> Value {
    match attachment.kind {
        ProviderInputAttachmentKind::Image => json!({
            "type": "input_image",
            "image_url": data_url(attachment),
        }),
    }
}

fn openai_chat_attachment_content(attachment: &ProviderInputAttachment) -> Value {
    match attachment.kind {
        ProviderInputAttachmentKind::Image => json!({
            "type": "image_url",
            "image_url": {
                "url": data_url(attachment),
            },
        }),
    }
}

fn data_url(attachment: &ProviderInputAttachment) -> String {
    format!(
        "data:{};base64,{}",
        attachment.media_type, attachment.data_base64
    )
}

fn openai_responses_tools(request: &ProviderSemanticRequest) -> Vec<Value> {
    let mut tools = request
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
        .collect::<Vec<_>>();
    tools.extend(
        request
            .hosted_tools
            .iter()
            .map(|tool| openai_responses_hosted_tool(tool, request)),
    );
    tools
}

fn openai_responses_hosted_tool(
    tool: &ProviderHostedToolDefinition,
    request: &ProviderSemanticRequest,
) -> Value {
    match tool {
        ProviderHostedToolDefinition::WebSearch {
            external_web_access,
            ..
        } => json!({
            "type": openai_responses_hosted_web_search_tool_type(request),
            "external_web_access": external_web_access,
        }),
    }
}

fn openai_responses_hosted_web_search_tool_type(request: &ProviderSemanticRequest) -> &'static str {
    match &request.descriptor.capabilities.web_search {
        ProviderWebSearchCapability::Hosted { wire_tool_type, .. } => wire_tool_type.as_str(),
        ProviderWebSearchCapability::Unsupported => ProviderWebSearchToolType::WebSearch.as_str(),
    }
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

fn openai_hosted_search_discovery(
    ctx: &ProviderEventContext,
    item: &Value,
) -> Option<freehand_contracts::SearchDiscoveryDelivery> {
    let domain_plan_ref = ctx.search_domain_plan_ref.clone();
    let action = item.get("action")?.as_object()?;
    let action_type = action.get("type").and_then(Value::as_str)?;
    let call_id = item
        .get("id")
        .or_else(|| item.get("call_id"))
        .and_then(Value::as_str)?;
    let status = item
        .get("status")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let (query, candidates) = match action_type {
        "search" => {
            let query = action
                .get("query")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    action
                        .get("queries")
                        .and_then(Value::as_array)
                        .map(|queries| {
                            queries
                                .iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                })
                .unwrap_or_else(|| "search".to_owned());
            let candidates = item
                .get("results")
                .and_then(Value::as_array)
                .map(|results| {
                    results
                        .iter()
                        .enumerate()
                        .filter_map(|(index, result)| {
                            let original_url =
                                result.get("url").and_then(Value::as_str)?.to_owned();
                            Some(ProviderHostedSearchCandidate {
                                candidate_id: result
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| {
                                        format!("{call_id}-candidate-{}", index + 1)
                                    }),
                                title: result
                                    .get("title")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_owned(),
                                original_url: Some(original_url),
                                snippet: result
                                    .get("snippet")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_owned(),
                                platform: Some(SearchSocialPlatform::Web),
                                source_weight: None,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            (query, candidates)
        }
        "open_page" | "find_in_page" => {
            let url = action.get("url").and_then(Value::as_str)?.to_owned();
            let query = if action_type == "find_in_page" {
                action
                    .get("pattern")
                    .and_then(Value::as_str)
                    .filter(|pattern| !pattern.trim().is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{action_type}:{url}"))
            } else {
                format!("{action_type}:{url}")
            };
            let candidates = vec![ProviderHostedSearchCandidate {
                candidate_id: format!("{call_id}-candidate-1"),
                title: String::new(),
                original_url: Some(url),
                snippet: String::new(),
                platform: Some(SearchSocialPlatform::Web),
                source_weight: None,
            }];
            (query, candidates)
        }
        _ => {
            // Unknown hosted web_search action: emit an observation with the actual
            // status and no candidates instead of silently dropping the activity.
            let query = format!("hosted web search ({action_type})");
            (query, Vec::new())
        }
    };
    let result_count = Some(candidates.len());
    Some(project_hosted_search_discovery(
        domain_plan_ref,
        format!("openai-{call_id}"),
        ProviderHostedSearchDiscovery {
            tool_call_id: Some(call_id.to_owned()),
            status,
            result_count,
            query,
            provider: "openai_responses".to_owned(),
            candidates,
        },
    ))
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
        ProviderCapabilities, ProviderDescriptor, ProviderFamily, ProviderHostedToolDefinition,
        ProviderInputAttachment, ProviderInputAttachmentKind, ProviderWebSearchCapability,
        ProviderWebSearchMode, ProviderWebSearchToolType, RawRetentionPolicy,
        build_semantic_request,
    };

    fn ctx() -> ProviderEventContext {
        ProviderEventContext {
            agent_id: AgentId::new("agent-1"),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("provider.openai-adapter"),
            search_domain_plan_ref: None,
        }
    }

    fn sourced_search_ctx() -> ProviderEventContext {
        ProviderEventContext {
            search_domain_plan_ref: Some("domain-1".to_owned()),
            ..ctx()
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
                    web_search: ProviderWebSearchCapability::hosted_live_with_functions(),
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
                    search_evidence: None,
                },
            },
        });
        request
    }

    fn hosted_web_search_request() -> ProviderSemanticRequest {
        let mut request = semantic_request(ProviderProtocol::OpenAiResponses);
        request
            .hosted_tools
            .push(ProviderHostedToolDefinition::WebSearch {
                mode: ProviderWebSearchMode::Live,
                external_web_access: true,
            });
        request
    }

    fn image_request(protocol: ProviderProtocol) -> ProviderSemanticRequest {
        let mut request = semantic_request(protocol);
        request.input_attachments.push(ProviderInputAttachment {
            attachment_id: "att-image-1".to_owned(),
            kind: ProviderInputAttachmentKind::Image,
            media_type: "image/png".to_owned(),
            name: "screen.png".to_owned(),
            size_bytes: Some(5),
            data_base64: "aW1hZ2U=".to_owned(),
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
    fn renders_responses_image_input_as_data_url() {
        let rendered = OpenAiAdapter::new()
            .render_request(&image_request(ProviderProtocol::OpenAiResponses), false)
            .expect("render");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        let content = body["input"][0]["content"].as_array().expect("content");

        assert_eq!(content[0]["type"], json!("input_text"));
        assert_eq!(content[1]["type"], json!("input_image"));
        assert_eq!(
            content[1]["image_url"],
            json!("data:image/png;base64,aW1hZ2U=")
        );
        assert!(!rendered.body.contains("att-image-1"));
        assert!(!rendered.body.contains("screen.png"));
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
    fn renders_responses_hosted_web_search_tool() {
        let adapter = OpenAiAdapter::new();
        let rendered = adapter
            .render_request(&hosted_web_search_request(), true)
            .expect("render");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        assert_eq!(body["tools"][0]["type"], json!("web_search"));
        assert_eq!(body["tools"][0]["external_web_access"], json!(true));
        assert!(body["tools"][0].get("name").is_none());
        assert!(body["tools"][0].get("parameters").is_none());
    }

    #[test]
    fn renders_responses_hosted_web_search_preview_tool_when_capability_declares_it() {
        let mut request = hosted_web_search_request();
        request.descriptor.capabilities.web_search =
            ProviderWebSearchCapability::hosted_live_with_functions()
                .with_wire_tool_type(ProviderWebSearchToolType::WebSearchPreview);
        let adapter = OpenAiAdapter::new();
        let rendered = adapter.render_request(&request, true).expect("render");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        assert_eq!(body["tools"][0]["type"], json!("web_search_preview"));
        assert_eq!(body["tools"][0]["external_web_access"], json!(true));
        assert!(body["tools"][0].get("name").is_none());
        assert!(body["tools"][0].get("parameters").is_none());
    }

    #[test]
    fn renders_responses_function_tools_with_hosted_web_search_tool() {
        let adapter = OpenAiAdapter::new();
        let mut request = tool_request(ProviderProtocol::OpenAiResponses);
        request
            .hosted_tools
            .push(ProviderHostedToolDefinition::WebSearch {
                mode: ProviderWebSearchMode::Live,
                external_web_access: true,
            });
        let rendered = adapter.render_request(&request, false).expect("render");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        let tools = body["tools"].as_array().expect("tools");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["type"], json!("function"));
        assert_eq!(tools[0]["name"], json!("read_file"));
        assert_eq!(tools[1]["type"], json!("web_search"));
        assert_eq!(tools[1]["external_web_access"], json!(true));
        assert!(tools[1].get("name").is_none());
        assert!(tools[1].get("parameters").is_none());
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
    fn renders_chat_completions_image_input_as_data_url() {
        let rendered = OpenAiAdapter::new()
            .render_request(
                &image_request(ProviderProtocol::OpenAiChatCompletions),
                false,
            )
            .expect("render");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        let content = body["messages"][0]["content"].as_array().expect("content");

        assert_eq!(content[0]["type"], json!("text"));
        assert_eq!(content[1]["type"], json!("image_url"));
        assert_eq!(
            content[1]["image_url"]["url"],
            json!("data:image/png;base64,aW1hZ2U=")
        );
        assert!(!rendered.body.contains("att-image-1"));
        assert!(!rendered.body.contains("screen.png"));
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
    fn parses_responses_web_search_call_as_observable_semantic_event() {
        let mut adapter = OpenAiAdapter::new();
        let outputs = adapter
            .parse_response(
                &sourced_search_ctx(),
                ProviderProtocol::OpenAiResponses,
                r#"{
                    "status":"completed",
                    "output":[
                        {
                            "type":"web_search_call",
                            "id":"ws-1",
                            "status":"completed",
                            "action":{
                                "type":"search",
                                "query":"OpenAI Responses web_search"
                            }
                        },
                        {"type":"message","content":[{"type":"output_text","text":"answer"}]}
                    ]
                }"#,
            )
            .expect("parsed");

        let discovery = outputs
            .iter()
            .find_map(|output| match output {
                ProviderSemanticOutput::SearchDiscovery(delivery) => Some(delivery),
                _ => None,
            })
            .expect("typed hosted-search discovery");
        let attempt = discovery.hosted_search_attempt.as_ref().expect("attempt");
        assert_eq!(attempt.query, "OpenAI Responses web_search");
        assert_eq!(attempt.provider, "openai_responses");
        assert_eq!(attempt.tool_call_id.as_deref(), Some("ws-1"));
        assert_eq!(attempt.status.as_deref(), Some("completed"));
        assert_eq!(attempt.result_count, Some(0));
        assert!(
            !outputs
                .iter()
                .any(|output| matches!(output, ProviderSemanticOutput::ToolCall(_))),
            "hosted provider search must not enter the local function-tool execution loop"
        );
        assert!(
            !outputs.iter().any(
                |output| matches!(output, ProviderSemanticOutput::SemanticEvent(event)
                    if event.kind == freehand_contracts::SemanticEventKind::Reasoning
                        && event.content.contains("provider-hosted web_search"))
            ),
            "hosted search must be a typed observation, not reasoning text"
        );
    }

    #[test]
    fn parses_responses_stream_web_search_call_as_observable_semantic_event() {
        let mut adapter = OpenAiAdapter::new();
        let outputs = adapter
            .parse_stream_event(
                &sourced_search_ctx(),
                ProviderProtocol::OpenAiResponses,
                r#"{
                    "type":"response.output_item.done",
                    "item":{
                        "type":"web_search_call",
                        "id":"ws-2",
                        "status":"completed",
                        "action":{
                            "type":"open_page",
                            "url":"https://example.com/source"
                        }
                    }
                }"#,
            )
            .expect("parsed");

        let discovery = outputs
            .iter()
            .find_map(|output| match output {
                ProviderSemanticOutput::SearchDiscovery(delivery) => Some(delivery),
                _ => None,
            })
            .expect("typed hosted-search discovery");
        let attempt = discovery.hosted_search_attempt.as_ref().expect("attempt");
        assert_eq!(attempt.tool_call_id.as_deref(), Some("ws-2"));
        assert_eq!(attempt.status.as_deref(), Some("completed"));
        assert_eq!(attempt.query, "open_page:https://example.com/source");
        assert_eq!(attempt.result_count, Some(1));
        assert_eq!(
            discovery.candidates[0].original_url.as_deref(),
            Some("https://example.com/source")
        );
    }

    #[test]
    fn projects_responses_open_page_url_as_typed_discovery() {
        let mut adapter = OpenAiAdapter::new();
        let outputs = adapter
            .parse_stream_event(
                &sourced_search_ctx(),
                ProviderProtocol::OpenAiResponses,
                r#"{
                    "type":"response.output_item.done",
                    "item":{
                        "type":"web_search_call",
                        "id":"ws-open",
                        "status":"completed",
                        "action":{"type":"open_page","url":"https://example.com/source"}
                    }
                }"#,
            )
            .expect("parsed");

        let discovery = outputs
            .iter()
            .find_map(|output| match output {
                ProviderSemanticOutput::SearchDiscovery(delivery) => Some(delivery),
                _ => None,
            })
            .expect("typed discovery");
        assert_eq!(discovery.domain_plan_ref.as_deref(), Some("domain-1"));
        assert_eq!(
            discovery.candidates[0].original_url.as_deref(),
            Some("https://example.com/source")
        );
        assert_eq!(
            discovery.candidates[0].status,
            freehand_contracts::SearchCandidateStatus::Usable
        );
    }

    #[test]
    fn projects_responses_find_in_page_url_without_parsing_observation_text() {
        let mut adapter = OpenAiAdapter::new();
        let outputs = adapter
            .parse_response(
                &sourced_search_ctx(),
                ProviderProtocol::OpenAiResponses,
                r#"{
                    "status":"completed",
                    "output":[{
                        "type":"web_search_call",
                        "id":"ws-find",
                        "status":"completed",
                        "action":{
                            "type":"find_in_page",
                            "url":"https://example.com/guide",
                            "pattern":"installation"
                        }
                    }]
                }"#,
            )
            .expect("parsed");

        assert!(outputs.iter().any(|output| matches!(
            output,
            ProviderSemanticOutput::SearchDiscovery(delivery)
                if delivery.domain_plan_ref.as_deref() == Some("domain-1")
                    && delivery.hosted_search_attempt.as_ref().is_some_and(|attempt| attempt.query == "installation")
                    && delivery.candidates[0].original_url.as_deref() == Some("https://example.com/guide")
        )));
    }

    #[test]
    fn sourced_search_promotes_search_action_as_typed_discovery_without_url_candidates() {
        let mut adapter = OpenAiAdapter::new();
        let outputs = adapter
            .parse_response(
                &sourced_search_ctx(),
                ProviderProtocol::OpenAiResponses,
                r#"{
                    "status":"completed",
                    "output":[{
                        "type":"web_search_call",
                        "id":"ws-search",
                        "status":"completed",
                        "action":{"type":"search","query":"Freehand"}
                    }]
                }"#,
            )
            .expect("parsed");

        let discovery = outputs
            .iter()
            .find_map(|output| match output {
                ProviderSemanticOutput::SearchDiscovery(delivery) => Some(delivery),
                _ => None,
            })
            .expect("typed hosted-search discovery for search action");
        let attempt = discovery.hosted_search_attempt.as_ref().expect("attempt");
        assert_eq!(attempt.tool_call_id.as_deref(), Some("ws-search"));
        assert_eq!(attempt.status.as_deref(), Some("completed"));
        assert_eq!(attempt.query, "Freehand");
        assert_eq!(attempt.result_count, Some(0));
        assert!(discovery.candidates.is_empty());
        assert!(
            !outputs
                .iter()
                .any(|output| matches!(output, ProviderSemanticOutput::ToolCall(_)))
        );
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
