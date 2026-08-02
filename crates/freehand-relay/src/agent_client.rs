use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt, stream};
use reqwest::{Body, Client, Method, Url};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};

use crate::model::{
    AgentHeartbeat, RelayControlInFrame, RelayControlOutFrame, RelayDataFrameKind,
    RelayDataInFrame, RelayDataOutFrame, RelayDataProtocol, RelayErrorInFrame, RelayErrorOutFrame,
};

#[derive(Debug, Clone)]
pub struct RelayAgentClientConfig {
    pub relay_base_url: String,
    pub access_token: String,
    pub heartbeat: AgentHeartbeat,
    pub local_daemon_addr: SocketAddr,
    pub local_adp_token: Option<String>,
    pub heartbeat_interval: Duration,
}

pub struct RelayAgentClient {
    config: RelayAgentClientConfig,
    http: Client,
}

struct HttpExchange {
    method: String,
    path_and_query: String,
    headers: Vec<(String, Vec<u8>)>,
    body: mpsc::Receiver<Vec<u8>>,
}

struct WebSocketExchange {
    protocol: RelayDataProtocol,
    path_and_query: String,
    inbound: mpsc::Receiver<RelayDataOutFrame>,
}

enum AgentExchange {
    Http {
        sender: Option<mpsc::Sender<Vec<u8>>>,
        task: tokio::task::AbortHandle,
    },
    WebSocket {
        sender: mpsc::Sender<RelayDataOutFrame>,
        task: tokio::task::AbortHandle,
    },
}

impl RelayAgentClient {
    pub fn new(config: RelayAgentClientConfig) -> Result<Self, String> {
        if config.access_token.trim().is_empty() {
            return Err("Relay Agent access token is empty".to_owned());
        }
        let relay_url = Url::parse(&config.relay_base_url).map_err(|error| error.to_string())?;
        if relay_url.query().is_some() || relay_url.fragment().is_some() {
            return Err("Relay Agent URL cannot contain a query or fragment".to_owned());
        }
        Ok(Self {
            config,
            http: Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|error| error.to_string())?,
        })
    }

    pub async fn run(self) -> Result<(), String> {
        let control = connect_channel(&self.config, "control").await?;
        let (mut control_sink, mut control_stream) = control.split();
        send_control_identity(&mut control_sink, &self.config.heartbeat).await?;
        let accepted = tokio::time::timeout(Duration::from_secs(10), control_stream.next())
            .await
            .map_err(|_| "Relay control identity admission timed out".to_owned())?
            .ok_or_else(|| "Relay control tunnel closed before identity admission".to_owned())?
            .map_err(|error| error.to_string())?;
        let Message::Text(accepted) = accepted else {
            return Err("Relay control identity admission returned a non-text frame".to_owned());
        };
        let accepted: RelayControlOutFrame = serde_json::from_str(accepted.as_str())
            .map_err(|error| format!("Relay control admission frame is invalid: {error}"))?;
        if accepted
            != (RelayControlOutFrame::IdentityAccepted {
                agent_id: self.config.heartbeat.agent_id.clone(),
            })
        {
            return Err("Relay control identity admission does not match the Agent".to_owned());
        }

        // The Relay only admits data/error channels after the control channel
        // has authenticated the Agent identity. Keep this ordering explicit:
        // the three sockets are separate typed channels, but admission is
        // established by the control channel first.
        let data = connect_channel(&self.config, "data").await?;
        let error = connect_channel(&self.config, "error").await?;
        let (mut data_sink, mut data_stream) = data.split();
        let (mut error_sink, mut error_stream) = error.split();

        let heartbeat = self.config.heartbeat.clone();
        let heartbeat_interval = self.config.heartbeat_interval;
        let mut channel_tasks: JoinSet<Result<(), String>> = JoinSet::new();
        channel_tasks.spawn(async move {
            let mut interval = tokio::time::interval(heartbeat_interval);
            loop {
                interval.tick().await;
                let frame = RelayControlInFrame::PresenceHeartbeat {
                    status: heartbeat.status,
                    active_session_count: heartbeat.active_session_count,
                };
                let text = serde_json::to_string(&frame).map_err(|error| error.to_string())?;
                control_sink
                    .send(Message::Text(text.into()))
                    .await
                    .map_err(|error| error.to_string())?;
            }
        });

        let (responses_tx, mut responses_rx) = mpsc::channel::<RelayDataInFrame>(64);
        let (errors_tx, mut errors_rx) = mpsc::channel::<RelayErrorInFrame>(32);
        let (cancellations_tx, mut cancellations_rx) = mpsc::channel::<String>(32);
        channel_tasks.spawn(async move {
            loop {
                tokio::select! {
                    frame = errors_rx.recv() => {
                        let frame = frame.ok_or_else(|| "Relay Agent error-report channel closed".to_owned())?;
                        let text = serde_json::to_string(&frame).map_err(|error| error.to_string())?;
                        error_sink.send(Message::Text(text.into())).await.map_err(|error| error.to_string())?;
                    }
                    incoming = error_stream.next() => {
                        let message = incoming
                            .ok_or_else(|| "Relay Agent error tunnel closed".to_owned())?
                            .map_err(|error| error.to_string())?;
                        let Message::Text(text) = message else {
                            return Err("Relay Agent error tunnel received a non-text frame".to_owned());
                        };
                        match serde_json::from_str::<RelayErrorOutFrame>(text.as_str())
                            .map_err(|error| format!("Relay error frame is invalid: {error}"))?
                        {
                            RelayErrorOutFrame::CorrelatedFailure { exchange_id: Some(exchange_id), .. } => {
                                cancellations_tx.send(exchange_id).await.map_err(|_| "Relay cancellation channel closed".to_owned())?;
                            }
                            RelayErrorOutFrame::CorrelatedFailure { exchange_id: None, .. } => {
                                return Err("Relay correlated failure is missing exchange id".to_owned());
                            }
                            RelayErrorOutFrame::Terminal { code, message } => {
                                return Err(format!("{code}: {message}"));
                            }
                        }
                    }
                }
            }
        });

        let mut exchanges = BTreeMap::<String, AgentExchange>::new();
        let mut completed_exchanges = BTreeSet::<String>::new();
        let mut exchange_tasks = JoinSet::<(String, Result<(), String>)>::new();
        loop {
            tokio::select! {
                task = channel_tasks.join_next() => {
                    let result = task
                        .ok_or_else(|| "Relay Agent channel task set closed".to_owned())?
                        .map_err(|error| format!("Relay Agent channel task failed to join: {error}"))?;
                    return match result {
                        Ok(()) => Err("Relay Agent channel task stopped unexpectedly".to_owned()),
                        Err(error) => Err(error),
                    };
                }
                task = exchange_tasks.join_next(), if !exchange_tasks.is_empty() => {
                    match task {
                        Some(Ok((exchange_id, Ok(())))) => {
                            exchanges.remove(&exchange_id);
                            completed_exchanges.insert(exchange_id);
                        }
                        Some(Ok((_exchange_id, Err(error)))) => return Err(error),
                        Some(Err(error)) if error.is_cancelled() => {}
                        Some(Err(error)) => {
                            return Err(format!("Relay Agent exchange task failed to join: {error}"));
                        }
                        None => return Err("Relay Agent exchange task set closed".to_owned()),
                    }
                }
                response = responses_rx.recv() => {
                    let Some(response) = response else {
                        return Err("Relay Agent response channel closed".to_owned());
                    };
                    let text = serde_json::to_string(&response).map_err(|error| error.to_string())?;
                    data_sink.send(Message::Text(text.into())).await.map_err(|error| error.to_string())?;
                }
                cancellation = cancellations_rx.recv() => {
                    let exchange_id = cancellation.ok_or_else(|| "Relay cancellation channel closed".to_owned())?;
                    let active = cancel_local_exchange(
                        &exchange_id,
                        &mut exchanges,
                        &mut completed_exchanges,
                    )?;
                    if active {
                        responses_tx.send(RelayDataInFrame::ResponseEnd { exchange_id })
                            .await
                            .map_err(|_| "Relay Agent response channel closed".to_owned())?;
                    }
                }
                incoming = data_stream.next() => {
                    let message = incoming
                        .ok_or_else(|| "Relay data tunnel closed".to_owned())?
                        .map_err(|error| error.to_string())?;
                    let Message::Text(text) = message else {
                        return Err("Relay data tunnel received a non-text contract frame".to_owned());
                    };
                    let frame: RelayDataOutFrame = serde_json::from_str(text.as_str())
                        .map_err(|error| format!("Relay data frame is invalid: {error}"))?;
                    handle_data_frame(
                        frame,
                        &mut exchanges,
                        &mut completed_exchanges,
                        self.http.clone(),
                        self.config.local_daemon_addr,
                        self.config.local_adp_token.clone(),
                        responses_tx.clone(),
                        errors_tx.clone(),
                        &mut exchange_tasks,
                    ).await?;
                }
            }
        }
    }
}

fn cancel_local_exchange(
    exchange_id: &str,
    exchanges: &mut BTreeMap<String, AgentExchange>,
    completed_exchanges: &mut BTreeSet<String>,
) -> Result<bool, String> {
    if let Some(exchange) = exchanges.remove(exchange_id) {
        match exchange {
            AgentExchange::Http { task, .. } | AgentExchange::WebSocket { task, .. } => {
                task.abort();
            }
        }
        return Ok(true);
    }
    if completed_exchanges.remove(exchange_id) {
        return Ok(false);
    }
    Err("Relay cancellation references an unknown exchange".to_owned())
}

async fn connect_channel(
    config: &RelayAgentClientConfig,
    channel: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
> {
    let url = relay_channel_url(&config.relay_base_url, channel, &config.heartbeat.agent_id)?;
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|error| error.to_string())?;
    request.headers_mut().insert(
        AUTHORIZATION,
        format!("Bearer {}", config.access_token)
            .parse()
            .map_err(|error| format!("Relay authorization header is invalid: {error}"))?,
    );
    connect_async(request)
        .await
        .map(|(socket, _)| socket)
        .map_err(|error| error.to_string())
}

fn relay_channel_url(base_url: &str, channel: &str, agent_id: &str) -> Result<Url, String> {
    let mut url = Url::parse(base_url).map_err(|error| error.to_string())?;
    let scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        _ => return Err("Relay URL must use http or https".to_owned()),
    };
    url.set_scheme(scheme)
        .map_err(|_| "Relay WebSocket scheme is invalid".to_owned())?;
    let already_mounted = url.path().trim_end_matches('/').ends_with("/relay");
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| "Relay URL cannot be a base".to_owned())?;
    segments.pop_if_empty();
    if !already_mounted {
        segments.push("relay");
    }
    segments.extend(["tunnel", channel, agent_id]);
    drop(segments);
    Ok(url)
}

async fn send_control_identity<S>(sink: &mut S, heartbeat: &AgentHeartbeat) -> Result<(), String>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let frame = RelayControlInFrame::AgentIdentity {
        agent_id: heartbeat.agent_id.clone(),
        display_name: heartbeat.display_name.clone(),
        node_id: heartbeat.node_id.clone(),
        role: heartbeat.role,
        status: heartbeat.status,
        active_session_count: heartbeat.active_session_count,
    };
    let text = serde_json::to_string(&frame).map_err(|error| error.to_string())?;
    sink.send(Message::Text(text.into()))
        .await
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
async fn handle_data_frame(
    frame: RelayDataOutFrame,
    exchanges: &mut BTreeMap<String, AgentExchange>,
    completed_exchanges: &mut BTreeSet<String>,
    http: Client,
    local_addr: SocketAddr,
    local_adp_token: Option<String>,
    responses: mpsc::Sender<RelayDataInFrame>,
    errors: mpsc::Sender<RelayErrorInFrame>,
    exchange_tasks: &mut JoinSet<(String, Result<(), String>)>,
) -> Result<(), String> {
    if let RelayDataOutFrame::RequestOpen { exchange_id, .. } = &frame
        && exchanges.contains_key(exchange_id)
    {
        return Err("Relay Agent exchange opened twice".to_owned());
    }
    match frame {
        RelayDataOutFrame::RequestOpen {
            exchange_id,
            protocol: RelayDataProtocol::Http,
            method,
            path_and_query,
            headers,
        } => {
            let method =
                method.ok_or_else(|| "HTTP tunnel request is missing method".to_owned())?;
            let (body_sender, body_receiver) = mpsc::channel(32);
            let task_exchange_id = exchange_id.clone();
            let task = exchange_tasks.spawn(async move {
                let result = run_http_exchange(
                    task_exchange_id.clone(),
                    HttpExchange {
                        method,
                        path_and_query,
                        headers,
                        body: body_receiver,
                    },
                    http,
                    local_addr,
                    responses,
                    errors,
                )
                .await;
                (task_exchange_id, result)
            });
            exchanges.insert(
                exchange_id,
                AgentExchange::Http {
                    sender: Some(body_sender),
                    task,
                },
            );
        }
        RelayDataOutFrame::RequestOpen {
            exchange_id,
            protocol,
            method,
            path_and_query,
            headers,
        } if matches!(
            protocol,
            RelayDataProtocol::Adp | RelayDataProtocol::WebSocket
        ) =>
        {
            if method.is_some() || !headers.is_empty() {
                return Err("WebSocket tunnel open contains HTTP request semantics".to_owned());
            }
            if !valid_local_websocket_path(&path_and_query)
                || (protocol == RelayDataProtocol::Adp && path_and_query != "/adp")
            {
                return Err("WebSocket tunnel target path is invalid".to_owned());
            }
            let (sender, receiver) = mpsc::channel(32);
            let task_exchange_id = exchange_id.clone();
            let task = exchange_tasks.spawn(async move {
                let result = run_websocket_exchange(
                    task_exchange_id.clone(),
                    local_addr,
                    local_adp_token,
                    WebSocketExchange {
                        protocol,
                        path_and_query,
                        inbound: receiver,
                    },
                    responses,
                    errors,
                )
                .await;
                (task_exchange_id, result)
            });
            exchanges.insert(exchange_id, AgentExchange::WebSocket { sender, task });
        }
        RelayDataOutFrame::RequestOpen { protocol, .. } => {
            return Err(format!(
                "Relay Agent does not support request-open protocol {protocol:?}"
            ));
        }
        RelayDataOutFrame::RequestChunk {
            exchange_id,
            frame_kind,
            bytes,
        } => match exchanges.get_mut(&exchange_id) {
            Some(AgentExchange::Http {
                sender: Some(sender),
                ..
            }) if frame_kind.is_none() => {
                sender
                    .send(bytes)
                    .await
                    .map_err(|_| "local HTTP exchange closed".to_owned())?;
            }
            Some(AgentExchange::WebSocket { sender, .. }) if frame_kind.is_some() => {
                sender
                    .send(RelayDataOutFrame::RequestChunk {
                        exchange_id,
                        frame_kind,
                        bytes,
                    })
                    .await
                    .map_err(|_| "local WebSocket exchange closed".to_owned())?;
            }
            _ => return Err("Relay Agent chunk does not match an open exchange".to_owned()),
        },
        RelayDataOutFrame::RequestEnd { exchange_id } => {
            if completed_exchanges.remove(&exchange_id) {
                return Ok(());
            }
            let mut remove_exchange = false;
            let exchange = exchanges
                .get_mut(&exchange_id)
                .ok_or_else(|| "Relay Agent end references an unknown exchange".to_owned())?;
            match exchange {
                AgentExchange::Http { sender, .. } => {
                    sender.take();
                }
                AgentExchange::WebSocket { sender, .. } => {
                    if !sender.is_closed() {
                        sender
                            .send(RelayDataOutFrame::RequestEnd {
                                exchange_id: exchange_id.clone(),
                            })
                            .await
                            .map_err(|_| {
                                "local WebSocket exchange closed during request end".to_owned()
                            })?;
                    }
                    remove_exchange = true;
                }
            }
            if remove_exchange {
                exchanges.remove(&exchange_id);
            }
        }
    }
    Ok(())
}

async fn run_http_exchange(
    exchange_id: String,
    exchange: HttpExchange,
    http: Client,
    local_addr: SocketAddr,
    responses: mpsc::Sender<RelayDataInFrame>,
    errors: mpsc::Sender<RelayErrorInFrame>,
) -> Result<(), String> {
    let result = execute_http_exchange(&exchange_id, exchange, http, local_addr, &responses).await;
    if let Err(message) = result {
        errors
            .send(RelayErrorInFrame::TunnelFailure {
                exchange_id: Some(exchange_id),
                code: "local_http_bridge_failed".to_owned(),
                message,
            })
            .await
            .map_err(|_| "Relay Agent error-report channel closed".to_owned())?;
    }
    Ok(())
}

async fn execute_http_exchange(
    exchange_id: &str,
    exchange: HttpExchange,
    http: Client,
    local_addr: SocketAddr,
    responses: &mpsc::Sender<RelayDataInFrame>,
) -> Result<(), String> {
    let method =
        Method::from_bytes(exchange.method.as_bytes()).map_err(|error| error.to_string())?;
    let url = format!("http://{local_addr}{}", exchange.path_and_query);
    let mut request = http.request(method, url);
    for (name, value) in exchange.headers {
        request = request.header(name, value);
    }
    let body = Body::wrap_stream(stream::unfold(exchange.body, |mut body| async move {
        body.recv()
            .await
            .map(|bytes| (Ok::<Vec<u8>, std::io::Error>(bytes), body))
    }));
    let response = request
        .body(body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
        .collect();
    responses
        .send(RelayDataInFrame::ResponseOpen {
            exchange_id: exchange_id.to_owned(),
            status: Some(status),
            headers,
        })
        .await
        .map_err(|_| "Relay data response channel closed".to_owned())?;
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        responses
            .send(RelayDataInFrame::ResponseChunk {
                exchange_id: exchange_id.to_owned(),
                frame_kind: None,
                bytes: chunk.map_err(|error| error.to_string())?.to_vec(),
            })
            .await
            .map_err(|_| "Relay data response channel closed".to_owned())?;
    }
    responses
        .send(RelayDataInFrame::ResponseEnd {
            exchange_id: exchange_id.to_owned(),
        })
        .await
        .map_err(|_| "Relay data response channel closed".to_owned())
}

fn valid_local_websocket_path(path: &str) -> bool {
    path.starts_with('/') && !path.contains("://") && !path.split('/').any(|part| part == "..")
}

async fn run_websocket_exchange(
    exchange_id: String,
    local_addr: SocketAddr,
    local_adp_token: Option<String>,
    mut exchange: WebSocketExchange,
    responses: mpsc::Sender<RelayDataInFrame>,
    errors: mpsc::Sender<RelayErrorInFrame>,
) -> Result<(), String> {
    let result = execute_websocket_exchange(
        &exchange_id,
        local_addr,
        local_adp_token,
        exchange.protocol,
        &exchange.path_and_query,
        &mut exchange.inbound,
        &responses,
    )
    .await;
    if let Err(message) = result {
        errors
            .send(RelayErrorInFrame::TunnelFailure {
                exchange_id: Some(exchange_id),
                code: "local_websocket_bridge_failed".to_owned(),
                message,
            })
            .await
            .map_err(|_| "Relay Agent error-report channel closed".to_owned())?;
    }
    Ok(())
}

async fn execute_websocket_exchange(
    exchange_id: &str,
    local_addr: SocketAddr,
    local_adp_token: Option<String>,
    protocol: RelayDataProtocol,
    path_and_query: &str,
    inbound: &mut mpsc::Receiver<RelayDataOutFrame>,
    responses: &mpsc::Sender<RelayDataInFrame>,
) -> Result<(), String> {
    let request = local_websocket_request(local_addr, protocol, local_adp_token, path_and_query)?;
    let (socket, _) = connect_async(request)
        .await
        .map_err(|error| error.to_string())?;
    responses
        .send(RelayDataInFrame::ResponseOpen {
            exchange_id: exchange_id.to_owned(),
            status: None,
            headers: Vec::new(),
        })
        .await
        .map_err(|_| "Relay data response channel closed".to_owned())?;
    let (mut sink, mut stream) = socket.split();
    loop {
        tokio::select! {
            request = inbound.recv() => match request {
                Some(RelayDataOutFrame::RequestChunk { frame_kind: Some(kind), bytes, .. }) => {
                    let message = decode_message(kind, bytes)?;
                    sink.send(message).await.map_err(|error| error.to_string())?;
                }
                Some(RelayDataOutFrame::RequestEnd { .. }) | None => break,
                _ => return Err("local WebSocket bridge received an invalid data node".to_owned()),
            },
            response = stream.next() => match response {
                Some(Ok(message)) => {
                    let terminal = matches!(message, Message::Close(_));
                    let (kind, bytes) = encode_message(message)?;
                    responses.send(RelayDataInFrame::ResponseChunk {
                        exchange_id: exchange_id.to_owned(),
                        frame_kind: Some(kind),
                        bytes,
                    }).await.map_err(|_| "Relay data response channel closed".to_owned())?;
                    if terminal {
                        break;
                    }
                }
                Some(Err(error)) => return Err(error.to_string()),
                None => break,
            }
        }
    }
    responses
        .send(RelayDataInFrame::ResponseEnd {
            exchange_id: exchange_id.to_owned(),
        })
        .await
        .map_err(|_| "Relay data response channel closed".to_owned())
}

fn local_websocket_request(
    local_addr: SocketAddr,
    protocol: RelayDataProtocol,
    local_adp_token: Option<String>,
    path_and_query: &str,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>, String> {
    let mut request = format!("ws://{local_addr}{path_and_query}")
        .into_client_request()
        .map_err(|error| error.to_string())?;
    if protocol == RelayDataProtocol::Adp
        && let Some(token) = local_adp_token.filter(|token| !token.trim().is_empty())
    {
        request.headers_mut().insert(
            AUTHORIZATION,
            format!("Bearer {token}").parse().map_err(|error| {
                format!("local WebSocket authorization header is invalid: {error}")
            })?,
        );
    }
    Ok(request)
}

fn encode_message(message: Message) -> Result<(RelayDataFrameKind, Vec<u8>), String> {
    Ok(match message {
        Message::Text(value) => (RelayDataFrameKind::Text, value.as_str().as_bytes().to_vec()),
        Message::Binary(value) => (RelayDataFrameKind::Binary, value.to_vec()),
        Message::Ping(value) => (RelayDataFrameKind::Ping, value.to_vec()),
        Message::Pong(value) => (RelayDataFrameKind::Pong, value.to_vec()),
        Message::Close(frame) => (
            RelayDataFrameKind::Close,
            frame.map_or_else(Vec::new, |frame| {
                let code: u16 = frame.code.into();
                let mut bytes = code.to_be_bytes().to_vec();
                bytes.extend_from_slice(frame.reason.as_bytes());
                bytes
            }),
        ),
        Message::Frame(_) => return Err("local WebSocket bridge received a raw frame".to_owned()),
    })
}

fn decode_message(kind: RelayDataFrameKind, bytes: Vec<u8>) -> Result<Message, String> {
    Ok(match kind {
        RelayDataFrameKind::Text => Message::Text(
            String::from_utf8(bytes)
                .map_err(|_| "Relay Agent received invalid UTF-8 text-frame bytes".to_owned())?
                .into(),
        ),
        RelayDataFrameKind::Binary => Message::Binary(bytes.into()),
        RelayDataFrameKind::Ping => Message::Ping(bytes.into()),
        RelayDataFrameKind::Pong => Message::Pong(bytes.into()),
        RelayDataFrameKind::Close => {
            let frame = match bytes.len() {
                0 => None,
                1 => return Err("Relay Agent received an invalid close-frame payload".to_owned()),
                _ => Some(CloseFrame {
                    code: CloseCode::from(u16::from_be_bytes([bytes[0], bytes[1]])),
                    reason: String::from_utf8(bytes[2..].to_vec())
                        .map_err(|_| {
                            "Relay Agent received invalid UTF-8 close-frame reason bytes".to_owned()
                        })?
                        .into(),
                }),
            };
            Message::Close(frame)
        }
    })
}

#[cfg(test)]
mod url_tests {
    use super::*;

    #[test]
    fn relay_client_rejects_query_and_fragment_base_urls() {
        let base = RelayAgentClientConfig {
            relay_base_url: "https://relay.example/freehand?tenant=one".to_owned(),
            access_token: "token".to_owned(),
            heartbeat: AgentHeartbeat {
                agent_id: "studio".to_owned(),
                display_name: "Studio".to_owned(),
                node_id: "node-studio".to_owned(),
                role: crate::model::AgentRole::Master,
                status: crate::model::AgentWorkStatus::Running,
                active_session_count: 1,
            },
            local_daemon_addr: "127.0.0.1:4042".parse().expect("local address"),
            local_adp_token: None,
            heartbeat_interval: Duration::from_secs(15),
        };
        assert_eq!(
            RelayAgentClient::new(base).err().as_deref(),
            Some("Relay Agent URL cannot contain a query or fragment")
        );
    }

    #[test]
    fn relay_channel_urls_mount_the_api_root_exactly_once() {
        assert_eq!(
            relay_channel_url("https://relay.example", "control", "studio")
                .expect("origin URL")
                .as_str(),
            "wss://relay.example/relay/tunnel/control/studio"
        );
        assert_eq!(
            relay_channel_url("https://relay.example/freehand/", "data", "studio")
                .expect("prefixed URL")
                .as_str(),
            "wss://relay.example/freehand/relay/tunnel/data/studio"
        );
        assert_eq!(
            relay_channel_url("https://relay.example/freehand/relay/", "error", "studio")
                .expect("mounted URL")
                .as_str(),
            "wss://relay.example/freehand/relay/tunnel/error/studio"
        );
    }

    #[test]
    fn local_adp_token_is_never_attached_to_generic_websocket_requests() {
        let local_addr = "127.0.0.1:4042".parse().expect("local address");
        let adp = local_websocket_request(
            local_addr,
            RelayDataProtocol::Adp,
            Some("local-secret".to_owned()),
            "/adp",
        )
        .expect("ADP request");
        assert_eq!(
            adp.headers()
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer local-secret")
        );

        let generic = local_websocket_request(
            local_addr,
            RelayDataProtocol::WebSocket,
            Some("local-secret".to_owned()),
            "/echo",
        )
        .expect("generic request");
        assert!(generic.headers().get(AUTHORIZATION).is_none());
    }

    #[test]
    fn websocket_frame_decode_preserves_valid_bytes_and_rejects_malformed_text() {
        let text = decode_message(RelayDataFrameKind::Text, b"relay text".to_vec())
            .expect("valid text frame");
        assert_eq!(text, Message::Text("relay text".into()));
        assert_eq!(
            decode_message(RelayDataFrameKind::Text, vec![0xff]).unwrap_err(),
            "Relay Agent received invalid UTF-8 text-frame bytes"
        );
        assert_eq!(
            decode_message(RelayDataFrameKind::Close, vec![0x03]).unwrap_err(),
            "Relay Agent received an invalid close-frame payload"
        );
        assert_eq!(
            decode_message(RelayDataFrameKind::Close, vec![0x03, 0xe8, 0xff]).unwrap_err(),
            "Relay Agent received invalid UTF-8 close-frame reason bytes"
        );
    }

    #[tokio::test]
    async fn duplicate_websocket_open_is_rejected_before_replacing_the_active_exchange() {
        let mut exchanges = BTreeMap::new();
        let mut completed_exchanges = BTreeSet::new();
        let mut exchange_tasks = JoinSet::new();
        let (responses, _response_rx) = mpsc::channel(4);
        let (errors, _error_rx) = mpsc::channel(4);
        let open = || RelayDataOutFrame::RequestOpen {
            exchange_id: "duplicate-websocket".to_owned(),
            protocol: RelayDataProtocol::WebSocket,
            method: None,
            path_and_query: "/echo".to_owned(),
            headers: Vec::new(),
        };

        handle_data_frame(
            open(),
            &mut exchanges,
            &mut completed_exchanges,
            Client::new(),
            "127.0.0.1:9".parse().expect("local address"),
            None,
            responses.clone(),
            errors.clone(),
            &mut exchange_tasks,
        )
        .await
        .expect("first WebSocket open");
        let first_sender = match exchanges
            .get("duplicate-websocket")
            .expect("active exchange")
        {
            AgentExchange::WebSocket { sender, .. } => sender.clone(),
            AgentExchange::Http { .. } => panic!("expected WebSocket exchange"),
        };

        let error = handle_data_frame(
            open(),
            &mut exchanges,
            &mut completed_exchanges,
            Client::new(),
            "127.0.0.1:9".parse().expect("local address"),
            None,
            responses,
            errors,
            &mut exchange_tasks,
        )
        .await
        .expect_err("duplicate WebSocket open must fail");
        assert_eq!(error, "Relay Agent exchange opened twice");
        match exchanges
            .get("duplicate-websocket")
            .expect("original exchange remains active")
        {
            AgentExchange::WebSocket { sender, .. } => {
                assert!(sender.same_channel(&first_sender));
            }
            AgentExchange::Http { .. } => panic!("expected WebSocket exchange"),
        }
        exchange_tasks.abort_all();
    }

    #[tokio::test]
    async fn cancellation_after_local_completion_is_idempotent_but_unknown_id_fails() {
        let mut exchanges = BTreeMap::new();
        let mut completed_exchanges = BTreeSet::from(["completed-http".to_owned()]);

        cancel_local_exchange("completed-http", &mut exchanges, &mut completed_exchanges)
            .expect("known completed exchange cancellation");
        assert!(!completed_exchanges.contains("completed-http"));
        assert_eq!(
            cancel_local_exchange("never-opened", &mut exchanges, &mut completed_exchanges),
            Err("Relay cancellation references an unknown exchange".to_owned())
        );
    }
}
