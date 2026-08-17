//! Anthropic provider adapter for Freehand.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader};
use std::time::Duration;

use freehand_blocks::{
    parse_tool_arguments_json, parse_tool_arguments_value, render_context_segments_as_text,
    render_tool_arguments_json,
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
    map_adapter_events, project_hosted_search_discovery,
};
use serde_json::{Value, json};
use thiserror::Error;

pub const DEFAULT_ANTHROPIC_MAX_TOKENS: u64 = 8192;
const PROVIDER_BLOCKING_CLIENT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicAdapterConfig {
    pub max_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicExecutorConfig {
    pub base_url: String,
    pub api_key: String,
    pub anthropic_version: String,
    pub adapter: AnthropicAdapterConfig,
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
    partial_tool_call_indexes: BTreeMap<u64, String>,
    hosted_search_queries: BTreeMap<String, String>,
    hosted_search_query_json: BTreeMap<String, String>,
    hosted_search_indexes: BTreeMap<u64, String>,
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

#[derive(Debug, Error)]
pub enum AnthropicExecutorError {
    #[error("anthropic executor base_url, api_key, and anthropic_version must be non-empty")]
    InvalidConfig,
    #[error(transparent)]
    Adapter(#[from] AnthropicAdapterError),
    #[error("anthropic http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("anthropic http status `{status}` returned body `{body}`")]
    HttpStatus { status: u16, body: String },
    #[error("anthropic stream read failed: {0}")]
    StreamRead(#[from] io::Error),
    #[error("anthropic stream callback failed: {0}")]
    Callback(String),
}

pub struct AnthropicExecutor {
    config: AnthropicExecutorConfig,
    client: reqwest::blocking::Client,
    adapter: AnthropicAdapter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnthropicRawCapture {
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
pub struct AnthropicExecutorFactory;

impl ProviderExecutorFactory for AnthropicExecutorFactory {
    fn build_executor(
        &self,
        config: ProviderExecutorConfig,
    ) -> Result<Box<dyn ProviderLiveExecutor>, ProviderExecutorFactoryError> {
        if config.descriptor.family != ProviderFamily::Anthropic
            || config.descriptor.protocol != ProviderProtocol::AnthropicMessages
        {
            return Err(ProviderExecutorFactoryError::Unsupported {
                family: config.descriptor.family,
                protocol: config.descriptor.protocol,
            });
        }
        AnthropicExecutor::new(AnthropicExecutorConfig {
            base_url: config.base_url,
            api_key: config.api_key,
            anthropic_version: "2023-06-01".to_owned(),
            adapter: AnthropicAdapterConfig {
                max_tokens: DEFAULT_ANTHROPIC_MAX_TOKENS,
            },
        })
        .map(|executor| Box::new(executor) as Box<dyn ProviderLiveExecutor>)
        .map_err(|err| {
            ProviderExecutorFactoryError::BuildFailed(classify_anthropic_executor_error(&err))
        })
    }
}

impl ProviderLiveExecutor for AnthropicExecutor {
    fn execute_once_with_raw(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        on_raw: &mut dyn FnMut(ProviderRawCapture<'_>) -> Result<(), String>,
    ) -> Result<Vec<ProviderSemanticOutput>, ProviderLiveExecutorError> {
        AnthropicExecutor::execute_once_with_raw(self, ctx, request, |raw| {
            on_raw(anthropic_raw_capture(raw)).map_err(AnthropicExecutorError::Callback)
        })
        .map_err(|err| ProviderLiveExecutorError::new(classify_anthropic_executor_error(&err)))
    }

    fn execute_stream_with_raw(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        on_raw: &mut dyn FnMut(ProviderRawCapture<'_>) -> Result<(), String>,
        on_outputs: &mut dyn FnMut(&[ProviderSemanticOutput]) -> Result<(), String>,
    ) -> Result<Vec<ProviderSemanticOutput>, ProviderLiveExecutorError> {
        AnthropicExecutor::execute_stream_with_raw(
            self,
            ctx,
            request,
            |raw| on_raw(anthropic_raw_capture(raw)).map_err(AnthropicExecutorError::Callback),
            |batch| on_outputs(batch).map_err(AnthropicExecutorError::Callback),
        )
        .map_err(|err| ProviderLiveExecutorError::new(classify_anthropic_executor_error(&err)))
    }
}

fn anthropic_raw_capture(raw: &AnthropicRawCapture) -> ProviderRawCapture<'_> {
    match raw {
        AnthropicRawCapture::ResponseBody { body } => ProviderRawCapture::Response {
            crate_name: "freehand-provider-anthropic",
            function: "AnthropicExecutor::execute_once_with_raw",
            body,
        },
        AnthropicRawCapture::HttpErrorBody { status, body } => ProviderRawCapture::HttpError {
            crate_name: "freehand-provider-anthropic",
            function: "AnthropicExecutor::send_rendered_request",
            status: *status,
            body,
        },
        AnthropicRawCapture::StreamEventBody {
            event_index,
            event_body,
        } => ProviderRawCapture::StreamEvent {
            crate_name: "freehand-provider-anthropic",
            function: "AnthropicExecutor::execute_stream_with_raw",
            event_index: *event_index,
            event_body,
        },
    }
}

pub fn classify_anthropic_executor_error(
    err: &AnthropicExecutorError,
) -> ProviderExecutorErrorInfo {
    match err {
        AnthropicExecutorError::HttpStatus { status, body } => ProviderExecutorErrorInfo {
            code: format!("anthropic_http_status_{status}"),
            message: body.clone(),
            retryable: *status == 408
                || *status == 409
                || *status == 425
                || *status == 429
                || *status >= 500,
            failover_eligible: true,
        },
        AnthropicExecutorError::Http(err) => ProviderExecutorErrorInfo {
            code: "anthropic_http_request_failed".to_owned(),
            message: err.to_string(),
            retryable: err.is_connect() || err.is_timeout() || err.is_request(),
            failover_eligible: true,
        },
        AnthropicExecutorError::StreamRead(err) => ProviderExecutorErrorInfo {
            code: "anthropic_stream_read_failed".to_owned(),
            message: err.to_string(),
            retryable: true,
            failover_eligible: true,
        },
        AnthropicExecutorError::Adapter(err) => ProviderExecutorErrorInfo {
            code: "anthropic_adapter_failed".to_owned(),
            message: err.to_string(),
            retryable: false,
            failover_eligible: false,
        },
        AnthropicExecutorError::InvalidConfig => ProviderExecutorErrorInfo {
            code: "anthropic_invalid_config".to_owned(),
            message: err.to_string(),
            retryable: false,
            failover_eligible: false,
        },
        AnthropicExecutorError::Callback(message) => ProviderExecutorErrorInfo {
            code: "anthropic_callback_failed".to_owned(),
            message: message.clone(),
            retryable: false,
            failover_eligible: false,
        },
    }
}

impl AnthropicExecutor {
    pub fn new(config: AnthropicExecutorConfig) -> Result<Self, AnthropicExecutorError> {
        if config.base_url.trim().is_empty()
            || config.api_key.trim().is_empty()
            || config.anthropic_version.trim().is_empty()
        {
            return Err(AnthropicExecutorError::InvalidConfig);
        }
        Ok(Self {
            adapter: AnthropicAdapter::new(config.adapter.clone())?,
            client: reqwest::blocking::Client::builder()
                .timeout(PROVIDER_BLOCKING_CLIENT_TIMEOUT)
                .build()?,
            config,
        })
    }

    pub fn execute_once(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
    ) -> Result<Vec<ProviderSemanticOutput>, AnthropicExecutorError> {
        self.execute_once_with_raw(ctx, request, |_| Ok(()))
    }

    pub fn execute_once_with_raw<F>(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        mut on_raw: F,
    ) -> Result<Vec<ProviderSemanticOutput>, AnthropicExecutorError>
    where
        F: FnMut(&AnthropicRawCapture) -> Result<(), AnthropicExecutorError>,
    {
        let rendered = self.adapter.render_request(request, false)?;
        let response = match self.send_rendered_request(&rendered) {
            Ok(response) => response,
            Err(AnthropicExecutorError::HttpStatus { status, body }) => {
                on_raw(&AnthropicRawCapture::HttpErrorBody {
                    status,
                    body: body.clone(),
                })?;
                return Err(AnthropicExecutorError::HttpStatus { status, body });
            }
            Err(other) => return Err(other),
        };
        let body = response.text()?;
        on_raw(&AnthropicRawCapture::ResponseBody { body: body.clone() })?;
        Ok(self
            .adapter
            .parse_response(ctx, request.descriptor.protocol, &body)?)
    }

    pub fn execute_stream(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
    ) -> Result<Vec<ProviderSemanticOutput>, AnthropicExecutorError> {
        self.execute_stream_with(ctx, request, |_| Ok(()))
    }

    pub fn execute_stream_with<F>(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        mut on_outputs: F,
    ) -> Result<Vec<ProviderSemanticOutput>, AnthropicExecutorError>
    where
        F: FnMut(&[ProviderSemanticOutput]) -> Result<(), AnthropicExecutorError>,
    {
        self.execute_stream_with_raw(ctx, request, |_| Ok(()), |batch| on_outputs(batch))
    }

    pub fn execute_stream_with_raw<FR, FO>(
        &mut self,
        ctx: &ProviderEventContext,
        request: &ProviderSemanticRequest,
        mut on_raw: FR,
        mut on_outputs: FO,
    ) -> Result<Vec<ProviderSemanticOutput>, AnthropicExecutorError>
    where
        FR: FnMut(&AnthropicRawCapture) -> Result<(), AnthropicExecutorError>,
        FO: FnMut(&[ProviderSemanticOutput]) -> Result<(), AnthropicExecutorError>,
    {
        let rendered = self.adapter.render_request(request, true)?;
        let response = match self.send_rendered_request(&rendered) {
            Ok(response) => response,
            Err(AnthropicExecutorError::HttpStatus { status, body }) => {
                on_raw(&AnthropicRawCapture::HttpErrorBody {
                    status,
                    body: body.clone(),
                })?;
                return Err(AnthropicExecutorError::HttpStatus { status, body });
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
            on_raw(&AnthropicRawCapture::StreamEventBody {
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
            on_raw(&AnthropicRawCapture::StreamEventBody {
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
        rendered: &AnthropicRenderedRequest,
    ) -> Result<reqwest::blocking::Response, AnthropicExecutorError> {
        let response = self
            .client
            .post(join_base_url_path(&self.config.base_url, rendered.path))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", &self.config.anthropic_version)
            .header("content-type", "application/json")
            .body(rendered.body.clone())
            .send()?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text()?;
            return Err(AnthropicExecutorError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }
        Ok(response)
    }
}

impl AnthropicAdapter {
    pub fn new(config: AnthropicAdapterConfig) -> Result<Self, AnthropicAdapterError> {
        if config.max_tokens == 0 {
            return Err(AnthropicAdapterError::InvalidMaxTokens);
        }
        Ok(Self {
            config,
            partial_tool_calls: BTreeMap::new(),
            partial_tool_call_indexes: BTreeMap::new(),
            hosted_search_queries: BTreeMap::new(),
            hosted_search_query_json: BTreeMap::new(),
            hosted_search_indexes: BTreeMap::new(),
        })
    }

    pub fn render_request(
        &self,
        request: &ProviderSemanticRequest,
        stream: bool,
    ) -> Result<AnthropicRenderedRequest, AnthropicAdapterError> {
        let rendered_input = render_context_segments_as_text(&request.payload.input_segments);
        if request.descriptor.protocol != ProviderProtocol::AnthropicMessages {
            return Err(AnthropicAdapterError::UnsupportedProtocol(
                request.descriptor.protocol,
            ));
        }
        let mut body = json!({
            "model": request.descriptor.model,
            "max_tokens": self.config.max_tokens,
            "stream": stream,
            "messages": render_messages(&rendered_input, &request.input_attachments, &request.tool_exchanges)?,
        });
        let tools = anthropic_messages_tools(request);
        if !tools.is_empty() {
            body["tools"] = Value::Array(tools);
        }
        if let Some(choice) = &request.tool_choice {
            body["tool_choice"] = match choice {
                ProviderToolChoice::Auto => json!({"type":"auto"}),
                ProviderToolChoice::Required { name } => json!({"type":"tool","name":name}),
            };
        }
        Ok(AnthropicRenderedRequest {
            path: "/v1/messages",
            body: body.to_string(),
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
        let events = self.parse_messages_body(ctx, &value)?;
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
        let events = self.parse_messages_stream_event(ctx, &value)?;
        Ok(map_adapter_events(ctx, events))
    }

    fn parse_messages_body(
        &mut self,
        ctx: &ProviderEventContext,
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
                    "server_tool_use" => {
                        self.track_hosted_search_start(None, block);
                    }
                    "web_search_tool_result" => {
                        if let Some(discovery) = self.hosted_search_discovery(ctx, block) {
                            events.push(ProviderAdapterEvent::SearchDiscovery(discovery));
                        }
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
        ctx: &ProviderEventContext,
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
                        "tool_use" => events.push(self.parse_indexed_tool_use_block(value, block)?),
                        "server_tool_use" => {
                            self.track_hosted_search_start(
                                value.get("index").and_then(Value::as_u64),
                                block,
                            );
                        }
                        "web_search_tool_result" => {
                            if let Some(discovery) = self.hosted_search_discovery(ctx, block) {
                                events.push(ProviderAdapterEvent::SearchDiscovery(discovery));
                            }
                        }
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
                            let id = self
                                .stream_tool_use_id(value)
                                .ok_or(AnthropicAdapterError::MissingToolUseId)?
                                .to_owned();
                            let name = value
                                .get("content_block")
                                .and_then(|block| block.get("name"))
                                .and_then(Value::as_str)
                                .or_else(|| {
                                    self.partial_tool_calls
                                        .get(id.as_str())
                                        .map(|state| state.tool_name.as_str())
                                })
                                .unwrap_or("")
                                .to_owned();
                            let partial = delta
                                .get("partial_json")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            if self
                                .hosted_search_indexes
                                .values()
                                .any(|hosted_id| hosted_id == &id)
                            {
                                self.apply_hosted_search_query_delta(&id, partial);
                            } else {
                                events.push(self.apply_partial_tool_delta(
                                    id.as_str(),
                                    name.as_str(),
                                    partial,
                                    false,
                                )?);
                            }
                        }
                        _ => {}
                    }
                }
            }
            "content_block_stop" => {
                if let Some(id) = self.stream_tool_use_id(value).map(ToOwned::to_owned) {
                    if self
                        .hosted_search_indexes
                        .values()
                        .any(|hosted_id| hosted_id == &id)
                    {
                        if let Some(input) = value
                            .get("content_block")
                            .and_then(|block| block.get("input"))
                        {
                            self.apply_hosted_search_query_delta(&id, &input.to_string());
                        }
                        return Ok(events);
                    }
                    let name = value
                        .get("content_block")
                        .and_then(|block| block.get("name"))
                        .and_then(Value::as_str)
                        .or_else(|| {
                            self.partial_tool_calls
                                .get(id.as_str())
                                .map(|state| state.tool_name.as_str())
                        })
                        .unwrap_or("")
                        .to_owned();
                    let input = value
                        .get("content_block")
                        .and_then(|block| block.get("input"))
                        .map(Value::to_string)
                        .unwrap_or_default();
                    events.push(self.apply_partial_tool_delta(
                        id.as_str(),
                        name.as_str(),
                        input.as_str(),
                        true,
                    )?);
                    if let Some(index) = value.get("index").and_then(Value::as_u64) {
                        self.partial_tool_call_indexes.remove(&index);
                    }
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

    fn stream_tool_use_id<'a>(&'a self, value: &'a Value) -> Option<&'a str> {
        value
            .get("content_block")
            .and_then(|block| block.get("id"))
            .and_then(Value::as_str)
            .or_else(|| value.get("id").and_then(Value::as_str))
            .or_else(|| {
                value
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|index| self.partial_tool_call_indexes.get(&index))
                    .map(String::as_str)
            })
            .or_else(|| {
                value
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|index| self.hosted_search_indexes.get(&index))
                    .map(String::as_str)
            })
    }

    fn track_hosted_search_start(&mut self, index: Option<u64>, block: &Value) {
        if block.get("name").and_then(Value::as_str) != Some("web_search") {
            return;
        }
        let Some(id) = block.get("id").and_then(Value::as_str) else {
            return;
        };
        if let Some(index) = index {
            self.hosted_search_indexes.insert(index, id.to_owned());
        }
        if let Some(query) = block
            .get("input")
            .and_then(|input| input.get("query"))
            .and_then(Value::as_str)
            .filter(|query| !query.trim().is_empty())
        {
            self.hosted_search_queries
                .insert(id.to_owned(), query.to_owned());
        }
    }

    fn apply_hosted_search_query_delta(&mut self, id: &str, delta: &str) {
        let buffer = self
            .hosted_search_query_json
            .entry(id.to_owned())
            .or_default();
        buffer.push_str(delta);
        if let Ok(value) = serde_json::from_str::<Value>(buffer)
            && let Some(query) = value
                .get("query")
                .and_then(Value::as_str)
                .filter(|query| !query.trim().is_empty())
        {
            self.hosted_search_queries
                .insert(id.to_owned(), query.to_owned());
        }
    }

    fn hosted_search_discovery(
        &mut self,
        ctx: &ProviderEventContext,
        block: &Value,
    ) -> Option<freehand_contracts::SearchDiscoveryDelivery> {
        let domain_plan_ref = ctx.search_domain_plan_ref.clone();
        let tool_use_id = block.get("tool_use_id").and_then(Value::as_str)?;
        // A web_search tool result must never be silently dropped. If the query
        // was not correlated (unexpected ordering), fall back to a generic query
        // so the UI still projects the hosted search activity.
        let query = self
            .hosted_search_queries
            .remove(tool_use_id)
            .unwrap_or_else(|| "hosted web search".to_owned());
        self.hosted_search_query_json.remove(tool_use_id);
        self.hosted_search_indexes
            .retain(|_, hosted_id| hosted_id != tool_use_id);
        anthropic_hosted_search_discovery(domain_plan_ref, query, block)
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

        let input = block
            .get("input")
            .filter(|input| !input.as_object().is_some_and(serde_json::Map::is_empty))
            .map(Value::to_string)
            .unwrap_or_default();
        self.apply_partial_tool_delta(id, name, &input, false)
    }

    fn parse_indexed_tool_use_block(
        &mut self,
        value: &Value,
        block: &Value,
    ) -> Result<ProviderAdapterEvent, AnthropicAdapterError> {
        let id = block
            .get("id")
            .and_then(Value::as_str)
            .ok_or(AnthropicAdapterError::MissingToolUseId)?;
        if let Some(index) = value.get("index").and_then(Value::as_u64) {
            self.partial_tool_call_indexes.insert(index, id.to_owned());
        }
        self.parse_tool_use_block(block, false)
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

fn join_base_url_path(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn render_messages(
    rendered_input: &str,
    attachments: &[ProviderInputAttachment],
    tool_exchanges: &[ProviderToolExchange],
) -> Result<Value, AnthropicAdapterError> {
    let mut user_content = vec![json!({
        "type": "text",
        "text": rendered_input,
    })];
    user_content.extend(attachments.iter().map(anthropic_attachment_content));
    let mut messages = vec![json!({
        "role": "user",
        "content": user_content
    })];
    if !tool_exchanges.is_empty() {
        messages.push(json!({
            "role": "assistant",
            "content": tool_exchanges
                .iter()
                .map(|exchange| {
                    let input_json =
                        render_tool_arguments_json(&exchange.tool_call.tool_call.arguments)
                            .map_err(|err| {
                                AnthropicAdapterError::InvalidToolArguments(err.to_string())
                            })?;
                    let input: Value = serde_json::from_str(&input_json).map_err(|err| {
                        AnthropicAdapterError::InvalidToolArguments(err.to_string())
                    })?;
                    Ok(json!({
                        "type": "tool_use",
                        "id": exchange.tool_call.tool_call.tool_call_id.as_str(),
                        "name": exchange.tool_call.tool_call.tool_name,
                        "input": input,
                    }))
                })
                .collect::<Result<Vec<_>, AnthropicAdapterError>>()?,
        }));
        messages.push(json!({
            "role": "user",
            "content": tool_exchanges
                .iter()
                .map(|exchange| {
                    json!({
                        "type": "tool_result",
                        "tool_use_id": exchange.tool_result.tool_result.tool_call_id.as_str(),
                        "content": exchange.tool_result.tool_result.output,
                        "is_error": exchange.tool_result.tool_result.status == ToolResultStatus::Failed,
                    })
                })
                .collect::<Vec<_>>(),
        }));
    }
    Ok(Value::Array(messages))
}

fn anthropic_attachment_content(attachment: &ProviderInputAttachment) -> Value {
    match attachment.kind {
        ProviderInputAttachmentKind::Image => json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": attachment.media_type,
                "data": attachment.data_base64,
            },
        }),
    }
}

fn anthropic_messages_tools(request: &ProviderSemanticRequest) -> Vec<Value> {
    let mut tools = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
            })
        })
        .collect::<Vec<_>>();
    tools.extend(
        request
            .hosted_tools
            .iter()
            .map(anthropic_messages_hosted_tool),
    );
    tools
}

fn anthropic_messages_hosted_tool(tool: &ProviderHostedToolDefinition) -> Value {
    match tool {
        ProviderHostedToolDefinition::WebSearch { .. } => json!({
            "type": "web_search_20250305",
            "name": "web_search",
            "max_uses": 5,
        }),
    }
}

fn anthropic_hosted_search_discovery(
    domain_plan_ref: Option<String>,
    query: String,
    block: &Value,
) -> Option<freehand_contracts::SearchDiscoveryDelivery> {
    let tool_use_id = block
        .get("tool_use_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let items = block
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let status = block
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let result_count = items.len();
    let candidates = items
        .iter()
        .enumerate()
        .map(|(index, item)| ProviderHostedSearchCandidate {
            candidate_id: item
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("{tool_use_id}-candidate-{}", index + 1)),
            title: item
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            original_url: item.get("url").and_then(Value::as_str).map(str::to_owned),
            snippet: item
                .get("snippet")
                .or_else(|| item.get("page_age"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            platform: Some(SearchSocialPlatform::Web),
            source_weight: None,
        })
        .collect();
    Some(project_hosted_search_discovery(
        domain_plan_ref,
        format!("anthropic-{tool_use_id}"),
        ProviderHostedSearchDiscovery {
            tool_call_id: Some(tool_use_id.to_owned()),
            status: Some(status),
            result_count: Some(result_count),
            query,
            provider: "anthropic_messages".to_owned(),
            candidates,
        },
    ))
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
fn sse_data_events(raw_sse: &str) -> Vec<String> {
    let mut collector = SseEventCollector::default();
    let mut events = Vec::new();
    for line in raw_sse.lines() {
        if let Some(event) = collector.push_line(line) {
            events.push(event);
        }
    }
    if let Some(event) = collector.finish() {
        events.push(event);
    }
    events
}

fn parse_anthropic_usage(usage: Option<&Value>, finish_reason: Option<&str>) -> Option<TokenUsage> {
    let usage = usage?;
    let uncached_input_tokens = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_creation_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let input_tokens = uncached_input_tokens
        .saturating_add(cache_creation_tokens)
        .saturating_add(cache_read_tokens);
    Some(TokenUsage {
        input_tokens,
        output_tokens,
        total_tokens: Some(input_tokens.saturating_add(output_tokens)),
        reasoning_tokens: usage.get("reasoning_tokens").and_then(Value::as_u64),
        cache_creation_tokens,
        cache_read_tokens,
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
        AgentId, FeatureId, ReasonReq03ProviderPayload, ReasonReq04ToolCall,
        ReasonReq05ToolResultReentry, SessionId, ToolArgument, ToolResultContract, TraceId, TurnId,
    };
    use freehand_provider_core::{
        ProviderCapabilities, ProviderDescriptor, ProviderFamily, ProviderInputAttachment,
        ProviderInputAttachmentKind, ProviderToolChoice, ProviderToolDefinition,
        ProviderToolExchange, ProviderWebSearchCapability, build_semantic_request,
    };
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn ctx() -> ProviderEventContext {
        ProviderEventContext {
            agent_id: AgentId::new("agent-1"),
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("provider.anthropic-adapter"),
            search_domain_plan_ref: None,
        }
    }

    fn sourced_search_ctx() -> ProviderEventContext {
        ProviderEventContext {
            search_domain_plan_ref: Some("domain-1".to_owned()),
            ..ctx()
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
                    web_search: ProviderWebSearchCapability::Unsupported,
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
            false,
        )
        .expect("request")
    }

    fn adapter() -> AnthropicAdapter {
        AnthropicAdapter::new(AnthropicAdapterConfig {
            max_tokens: DEFAULT_ANTHROPIC_MAX_TOKENS,
        })
        .expect("adapter")
    }

    fn executor(base_url: String) -> AnthropicExecutor {
        AnthropicExecutor::new(AnthropicExecutorConfig {
            base_url,
            api_key: "test-api-key".to_owned(),
            anthropic_version: "2023-06-01".to_owned(),
            adapter: AnthropicAdapterConfig {
                max_tokens: DEFAULT_ANTHROPIC_MAX_TOKENS,
            },
        })
        .expect("executor")
    }

    fn spawn_mock_server(
        status: u16,
        content_type: &'static str,
        response_body: &'static str,
    ) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let (tx, rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("timeout");
            let mut raw = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("read request");
                if read == 0 {
                    break;
                }
                raw.extend_from_slice(&buffer[..read]);
                if request_is_complete(&raw) {
                    break;
                }
            }
            let request = String::from_utf8(raw).expect("request utf8");
            tx.send(request).expect("send request");
            let response = format!(
                "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
                response_body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });
        (base_url, rx, handle)
    }

    fn spawn_incremental_stream_server(
        first_chunk: &'static str,
        remaining_chunks: &'static str,
    ) -> (
        String,
        mpsc::Receiver<String>,
        mpsc::Receiver<bool>,
        mpsc::Sender<()>,
        thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind incremental mock server");
        let base_url = format!("http://{}", listener.local_addr().expect("addr"));
        let (request_tx, request_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (continue_tx, continue_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("timeout");
            let mut raw = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("read request");
                if read == 0 {
                    break;
                }
                raw.extend_from_slice(&buffer[..read]);
                if request_is_complete(&raw) {
                    break;
                }
            }
            request_tx
                .send(String::from_utf8(raw).expect("request utf8"))
                .expect("send request");

            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
                )
                .expect("write headers");
            stream
                .write_all(first_chunk.as_bytes())
                .expect("write first chunk");
            stream.flush().expect("flush first chunk");

            let released = continue_rx.recv_timeout(Duration::from_secs(2)).is_ok();
            release_tx.send(released).expect("send released");
            if released {
                stream
                    .write_all(remaining_chunks.as_bytes())
                    .expect("write remaining chunks");
                stream.flush().expect("flush remaining chunks");
            }
        });
        (base_url, request_rx, release_rx, continue_tx, handle)
    }

    fn request_is_complete(raw: &[u8]) -> bool {
        let text = String::from_utf8_lossy(raw);
        let Some(header_end) = text.find("\r\n\r\n") else {
            return false;
        };
        let content_length = text[..header_end]
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length: ")
                    .or_else(|| line.strip_prefix("Content-Length: "))
            })
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        raw.len() >= header_end + 4 + content_length
    }

    #[test]
    fn renders_messages_request() {
        let adapter = adapter();
        let rendered = adapter.render_request(&request(), true).expect("rendered");
        assert_eq!(rendered.path, "/v1/messages");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        assert_eq!(
            body.get("max_tokens").and_then(Value::as_u64),
            Some(DEFAULT_ANTHROPIC_MAX_TOKENS)
        );
        assert_eq!(body.get("stream").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn renders_messages_image_input_as_base64_source() {
        let mut request = request();
        request.input_attachments.push(ProviderInputAttachment {
            attachment_id: "att-image-1".to_owned(),
            kind: ProviderInputAttachmentKind::Image,
            media_type: "image/png".to_owned(),
            name: "screen.png".to_owned(),
            size_bytes: Some(5),
            data_base64: "aW1hZ2U=".to_owned(),
        });

        let rendered = adapter().render_request(&request, false).expect("rendered");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");
        let content = body["messages"][0]["content"].as_array().expect("content");

        assert_eq!(content[0]["type"], json!("text"));
        assert_eq!(content[1]["type"], json!("image"));
        assert_eq!(content[1]["source"]["type"], json!("base64"));
        assert_eq!(content[1]["source"]["media_type"], json!("image/png"));
        assert_eq!(content[1]["source"]["data"], json!("aW1hZ2U="));
        assert!(!rendered.body.contains("att-image-1"));
        assert!(!rendered.body.contains("screen.png"));
    }

    #[test]
    fn renders_tool_schema_and_tool_result_exchange_as_messages_protocol() {
        let adapter = adapter();
        let mut request = request();
        request.tools = vec![ProviderToolDefinition {
            name: "echo_json".to_owned(),
            description: "echo input".to_owned(),
            input_schema: json!({"type":"object"}),
        }];
        request.tool_choice = Some(ProviderToolChoice::Required {
            name: "echo_json".to_owned(),
        });
        request.tool_exchanges = vec![ProviderToolExchange {
            tool_call: ReasonReq04ToolCall {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("provider.anthropic-adapter"),
                agent_id: AgentId::new("agent-1"),
                tool_call: ToolCallContract {
                    tool_call_id: ToolCallId::new("toolu_1"),
                    tool_name: "echo_json".to_owned(),
                    arguments: vec![ToolArgument {
                        name: "message".to_owned(),
                        value: json!("pong"),
                    }],
                    arguments_complete: true,
                },
            },
            tool_result: ReasonReq05ToolResultReentry {
                session_id: SessionId::new("session-1"),
                turn_id: TurnId::new("turn-1"),
                trace_id: TraceId::new("trace-1"),
                feature_id: FeatureId::new("provider.anthropic-adapter"),
                agent_id: AgentId::new("agent-1"),
                tool_result: ToolResultContract {
                    tool_call_id: ToolCallId::new("toolu_1"),
                    status: freehand_contracts::ToolResultStatus::Success,
                    output: r#"{"status":"ok"}"#.to_owned(),
                    search_evidence: None,
                },
            },
        }];

        let rendered = adapter.render_request(&request, false).expect("rendered");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");

        assert_eq!(body["tools"][0]["name"], "echo_json");
        assert_eq!(
            body["tool_choice"],
            json!({"type":"tool","name":"echo_json"})
        );
        assert_eq!(body["messages"][1]["role"], "assistant");
        assert_eq!(body["messages"][1]["content"][0]["type"], "tool_use");
        assert_eq!(
            body["messages"][1]["content"][0]["input"]["message"],
            "pong"
        );
        assert_eq!(body["messages"][2]["role"], "user");
        assert_eq!(body["messages"][2]["content"][0]["type"], "tool_result");
        assert_eq!(body["messages"][2]["content"][0]["tool_use_id"], "toolu_1");
    }

    #[test]
    fn renders_messages_hosted_web_search_server_tool() {
        let adapter = adapter();
        let mut request = request();
        request
            .hosted_tools
            .push(ProviderHostedToolDefinition::WebSearch {
                mode: freehand_provider_core::ProviderWebSearchMode::Live,
                external_web_access: true,
            });

        let rendered = adapter.render_request(&request, false).expect("rendered");
        let body: Value = serde_json::from_str(&rendered.body).expect("json");

        assert_eq!(body["tools"][0]["type"], "web_search_20250305");
        assert_eq!(body["tools"][0]["name"], "web_search");
        assert_eq!(body["tools"][0]["max_uses"], 5);
        assert!(body["tools"][0].get("input_schema").is_none());
    }

    #[test]
    fn parses_messages_hosted_web_search_blocks_as_observations() {
        let mut adapter = adapter();
        let outputs = adapter
            .parse_response(
                &sourced_search_ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{
                    "content":[
                        {"type":"server_tool_use","id":"srv_1","name":"web_search","input":{"query":"Freehand hosted search"}},
                        {"type":"web_search_tool_result","tool_use_id":"srv_1","status":"completed","content":[{"title":"Freehand","url":"https://example.test"}]}
                    ],
                    "stop_reason":"end_turn"
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
        let attempt = discovery
            .hosted_search_attempt
            .as_ref()
            .expect("hosted search attempt");
        assert_eq!(attempt.query, "Freehand hosted search");
        assert_eq!(attempt.provider, "anthropic_messages");
        assert_eq!(attempt.tool_call_id.as_deref(), Some("srv_1"));
        assert_eq!(attempt.status.as_deref(), Some("completed"));
        assert_eq!(attempt.result_count, Some(1));
        assert!(
            !outputs
                .iter()
                .any(|output| matches!(output, ProviderSemanticOutput::ToolCall(_)))
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
    fn projects_messages_hosted_results_with_typed_query_and_domain_plan() {
        let mut adapter = adapter();
        let outputs = adapter
            .parse_response(
                &sourced_search_ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{
                    "content":[
                        {"type":"server_tool_use","id":"srv_1","name":"web_search","input":{"query":"Freehand evidence pipeline"}},
                        {"type":"web_search_tool_result","tool_use_id":"srv_1","content":[
                            {"title":"Freehand","url":"https://example.test/source","snippet":"verified later"},
                            {"title":"Missing URL","snippet":"unusable"}
                        ]}
                    ],
                    "stop_reason":"end_turn"
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
            discovery
                .hosted_search_attempt
                .as_ref()
                .map(|attempt| attempt.query.as_str()),
            Some("Freehand evidence pipeline")
        );
        let attempt = discovery.hosted_search_attempt.as_ref().expect("attempt");
        assert_eq!(attempt.tool_call_id.as_deref(), Some("srv_1"));
        assert_eq!(attempt.status.as_deref(), Some("unknown"));
        assert_eq!(attempt.result_count, Some(2));
        assert_eq!(
            discovery.candidates[0].status,
            freehand_contracts::SearchCandidateStatus::Usable
        );
        assert_eq!(
            discovery.candidates[1].status,
            freehand_contracts::SearchCandidateStatus::UnusableMissingUrl
        );
    }

    #[test]
    fn projects_streamed_messages_hosted_results_only_after_query_correlation() {
        let mut adapter = adapter();
        for event in [
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"server_tool_use","id":"srv_stream","name":"web_search","input":{}}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"query\":\"streamed query\"}"}}"#,
        ] {
            let outputs = adapter
                .parse_stream_event(
                    &sourced_search_ctx(),
                    ProviderProtocol::AnthropicMessages,
                    event,
                )
                .expect("stream event");
            assert!(
                !outputs
                    .iter()
                    .any(|output| matches!(output, ProviderSemanticOutput::SearchDiscovery(_)))
            );
        }

        let outputs = adapter
            .parse_stream_event(
                &sourced_search_ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{"type":"content_block_start","index":1,"content_block":{"type":"web_search_tool_result","tool_use_id":"srv_stream","content":[{"title":"Source","url":"https://example.test/stream"}]}}"#,
            )
            .expect("result event");
        assert!(outputs.iter().any(|output| matches!(
            output,
            ProviderSemanticOutput::SearchDiscovery(delivery)
                if delivery.hosted_search_attempt.as_ref().is_some_and(|attempt| attempt.query == "streamed query")
        )));
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
    fn replays_minimonth_single_shot_fixture() {
        let mut adapter = adapter();
        let outputs = adapter
            .parse_response(
                &ctx(),
                ProviderProtocol::AnthropicMessages,
                include_str!("../fixtures/minimonth_messages_single.json"),
            )
            .expect("parsed");

        assert!(outputs.iter().any(|output| {
            matches!(
                output,
                ProviderSemanticOutput::SemanticEvent(event)
                    if event.kind == freehand_contracts::SemanticEventKind::Reasoning
                        && event.content.contains("reply exactly pong")
            )
        }));
        assert!(outputs.iter().any(|output| {
            matches!(
                output,
                ProviderSemanticOutput::SemanticEvent(event)
                    if event.kind == freehand_contracts::SemanticEventKind::Text
                        && event.content == "pong"
            )
        }));
        assert!(outputs.iter().any(|output| {
            matches!(
                output,
                ProviderSemanticOutput::Usage(usage)
                    if usage.usage.input_tokens == 46
                        && usage.usage.output_tokens == 82
                        && usage.usage.cache_read_tokens == 32
                        && usage.usage.total_input_tokens() == 46
                        && (usage.usage.cache_hit_rate() - 32.0 / 46.0).abs() < f64::EPSILON
                        && usage.usage.resolved_total_tokens() == 128
                        && usage.usage.finish_reason.as_deref() == Some("end_turn")
            )
        }));
        assert!(outputs.iter().any(|output| {
            matches!(
                output,
                ProviderSemanticOutput::Terminal(terminal)
                    if terminal.status == TerminalStatus::Success
                        && terminal.summary == "end_turn"
            )
        }));
    }

    #[test]
    fn replays_minimonth_stream_fixture() {
        let mut adapter = adapter();
        let mut outputs = Vec::new();
        for event_body in sse_data_events(include_str!("../fixtures/minimonth_messages_stream.sse"))
        {
            outputs.extend(
                adapter
                    .parse_stream_event(&ctx(), ProviderProtocol::AnthropicMessages, &event_body)
                    .expect("parsed stream event"),
            );
        }

        let reasoning = outputs
            .iter()
            .filter_map(|output| match output {
                ProviderSemanticOutput::SemanticEvent(event)
                    if event.kind == freehand_contracts::SemanticEventKind::Reasoning =>
                {
                    Some(event.content.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
        let text = outputs
            .iter()
            .filter_map(|output| match output {
                ProviderSemanticOutput::SemanticEvent(event)
                    if event.kind == freehand_contracts::SemanticEventKind::Text =>
                {
                    Some(event.content.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        assert!(reasoning.contains("reply exactly pong"));
        assert_eq!(text.trim(), "pong");
        assert!(outputs.iter().any(|output| {
            matches!(
                output,
                ProviderSemanticOutput::Usage(usage)
                    if usage.usage.input_tokens == 46
                        && usage.usage.output_tokens == 82
                        && usage.usage.cache_read_tokens == 32
                        && usage.usage.total_input_tokens() == 46
                        && (usage.usage.cache_hit_rate() - 32.0 / 46.0).abs() < f64::EPSILON
                        && usage.usage.resolved_total_tokens() == 128
                        && usage.usage.finish_reason.as_deref() == Some("end_turn")
            )
        }));
        assert!(outputs.iter().any(|output| {
            matches!(
                output,
                ProviderSemanticOutput::Terminal(terminal)
                    if terminal.status == TerminalStatus::Success
                        && terminal.summary == "end_turn"
            )
        }));
    }

    #[test]
    fn executor_posts_single_shot_request_and_parses_response() {
        let (base_url, rx, handle) = spawn_mock_server(
            200,
            "application/json",
            include_str!("../fixtures/minimonth_messages_single.json"),
        );
        let mut executor = executor(base_url);

        let outputs = executor
            .execute_once(&ctx(), &request())
            .expect("execute once");
        let raw_request = rx.recv().expect("request");
        handle.join().expect("server thread");

        assert!(raw_request.starts_with("POST /v1/messages HTTP/1.1"));
        assert!(raw_request.contains("x-api-key: test-api-key"));
        assert!(raw_request.contains("anthropic-version: 2023-06-01"));
        assert!(raw_request.contains("\"stream\":false"));
        assert!(outputs.iter().any(|output| {
            matches!(
                output,
                ProviderSemanticOutput::SemanticEvent(event)
                    if event.kind == freehand_contracts::SemanticEventKind::Text
                        && event.content == "pong"
            )
        }));
    }

    #[test]
    fn executor_posts_stream_request_and_parses_sse_response() {
        let (base_url, rx, handle) = spawn_mock_server(
            200,
            "text/event-stream",
            include_str!("../fixtures/minimonth_messages_stream.sse"),
        );
        let mut executor = executor(base_url);

        let outputs = executor
            .execute_stream(&ctx(), &request())
            .expect("execute stream");
        let raw_request = rx.recv().expect("request");
        handle.join().expect("server thread");

        assert!(raw_request.starts_with("POST /v1/messages HTTP/1.1"));
        assert!(raw_request.contains("\"stream\":true"));
        assert!(outputs.iter().any(|output| {
            matches!(
                output,
                ProviderSemanticOutput::Usage(usage)
                    if usage.usage.finish_reason.as_deref() == Some("end_turn")
            )
        }));
        assert!(outputs.iter().any(|output| {
            matches!(
                output,
                ProviderSemanticOutput::Terminal(terminal)
                    if terminal.status == TerminalStatus::Success
            )
        }));
    }

    #[test]
    fn executor_stream_callback_runs_before_response_finishes() {
        let first_chunk = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"pong\"}}\n\n"
        );
        let remaining_chunks = concat!(
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"pong\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":14,\"output_tokens\":82}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );
        let (base_url, rx, released_rx, continue_tx, handle) =
            spawn_incremental_stream_server(first_chunk, remaining_chunks);
        let mut executor = executor(base_url);

        let outputs = executor
            .execute_stream_with(&ctx(), &request(), |batch| {
                if batch.iter().any(|output| {
                    matches!(
                        output,
                        ProviderSemanticOutput::SemanticEvent(event)
                            if event.kind == freehand_contracts::SemanticEventKind::Reasoning
                    )
                }) {
                    let _ = continue_tx.send(());
                }
                Ok(())
            })
            .expect("execute stream with callback");
        let raw_request = rx.recv().expect("request");
        let released = released_rx.recv().expect("release status");
        handle.join().expect("server thread");

        assert!(raw_request.starts_with("POST /v1/messages HTTP/1.1"));
        assert!(
            released,
            "callback did not fire before stream completion gate"
        );
        assert!(outputs.iter().any(|output| {
            matches!(
                output,
                ProviderSemanticOutput::Usage(usage)
                    if usage.usage.finish_reason.as_deref() == Some("end_turn")
            )
        }));
    }

    #[test]
    fn executor_raw_callback_sees_single_response_body_before_parse_failure() {
        let (base_url, _rx, handle) = spawn_mock_server(200, "application/json", "{\"type\":");
        let mut executor = executor(base_url);
        let mut raw = Vec::<AnthropicRawCapture>::new();

        let err = executor
            .execute_once_with_raw(&ctx(), &request(), |capture| {
                raw.push(capture.clone());
                Ok(())
            })
            .expect_err("parse failure");
        handle.join().expect("server thread");

        assert!(matches!(
            err,
            AnthropicExecutorError::Adapter(AnthropicAdapterError::InvalidJson(_))
        ));
        assert_eq!(
            raw,
            vec![AnthropicRawCapture::ResponseBody {
                body: "{\"type\":".to_owned()
            }]
        );
    }

    #[test]
    fn executor_raw_callback_sees_http_error_body() {
        let (base_url, _rx, handle) =
            spawn_mock_server(401, "application/json", "{\"error\":\"expired\"}");
        let mut executor = executor(base_url);
        let mut raw = Vec::<AnthropicRawCapture>::new();

        let err = executor
            .execute_once_with_raw(&ctx(), &request(), |capture| {
                raw.push(capture.clone());
                Ok(())
            })
            .expect_err("http failure");
        handle.join().expect("server thread");

        assert!(matches!(
            err,
            AnthropicExecutorError::HttpStatus { status: 401, body }
                if body.contains("expired")
        ));
        assert_eq!(
            raw,
            vec![AnthropicRawCapture::HttpErrorBody {
                status: 401,
                body: "{\"error\":\"expired\"}".to_owned()
            }]
        );
    }

    #[test]
    fn executor_raw_callback_sees_stream_event_bodies() {
        let raw_sse = concat!(
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"pong\"}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );
        let (base_url, _rx, handle) = spawn_mock_server(200, "text/event-stream", raw_sse);
        let mut executor = executor(base_url);
        let mut raw = Vec::<AnthropicRawCapture>::new();

        let outputs = executor
            .execute_stream_with_raw(
                &ctx(),
                &request(),
                |capture| {
                    raw.push(capture.clone());
                    Ok(())
                },
                |_| Ok(()),
            )
            .expect("execute stream");
        handle.join().expect("server thread");

        assert!(!outputs.is_empty());
        assert_eq!(
            raw,
            vec![
                AnthropicRawCapture::StreamEventBody {
                    event_index: 1,
                    event_body:
                        "{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}"
                            .to_owned()
                },
                AnthropicRawCapture::StreamEventBody {
                    event_index: 2,
                    event_body:
                        "{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"pong\"}}"
                            .to_owned()
                },
                AnthropicRawCapture::StreamEventBody {
                    event_index: 3,
                    event_body: "{\"type\":\"message_stop\"}".to_owned()
                }
            ]
        );
    }

    #[test]
    fn executor_rejects_non_success_http_status() {
        let (base_url, rx, handle) = spawn_mock_server(
            401,
            "application/json",
            r#"{"error":{"message":"expired"}}"#,
        );
        let mut executor = executor(base_url);

        let err = executor
            .execute_once(&ctx(), &request())
            .expect_err("must fail");
        let raw_request = rx.recv().expect("request");
        handle.join().expect("server thread");

        assert!(raw_request.contains("POST /v1/messages"));
        assert!(matches!(
            err,
            AnthropicExecutorError::HttpStatus { status: 401, body }
                if body.contains("expired")
        ));
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
    fn parses_indexed_streaming_tool_use_delta_without_repeated_id() {
        let mut adapter = adapter();
        adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"call_function_s889op96nzbw_1","name":"task","input":{}}}"#,
            )
            .expect("start");

        let delta = adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"op\":\"list_agents\"}"}}"#,
            )
            .expect("delta");

        match &delta[0] {
            ProviderSemanticOutput::ToolCall(call) => {
                assert_eq!(
                    call.tool_call.tool_call_id.as_str(),
                    "call_function_s889op96nzbw_1"
                );
                assert_eq!(call.tool_call.tool_name, "task");
                assert!(!call.tool_call.arguments_complete);
                assert_eq!(call.tool_call.arguments.len(), 1);
                assert_eq!(call.tool_call.arguments[0].name, "op");
                assert_eq!(call.tool_call.arguments[0].value, json!("list_agents"));
            }
            other => panic!("unexpected output: {other:?}"),
        }

        let stop = adapter
            .parse_stream_event(
                &ctx(),
                ProviderProtocol::AnthropicMessages,
                r#"{"type":"content_block_stop","index":1}"#,
            )
            .expect("stop");
        match &stop[0] {
            ProviderSemanticOutput::ToolCall(call) => {
                assert_eq!(call.tool_call.tool_name, "task");
                assert!(call.tool_call.arguments_complete);
                assert_eq!(call.tool_call.arguments.len(), 1);
                assert_eq!(call.tool_call.arguments[0].name, "op");
                assert_eq!(call.tool_call.arguments[0].value, json!("list_agents"));
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
