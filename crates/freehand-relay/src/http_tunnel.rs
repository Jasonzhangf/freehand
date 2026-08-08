use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::model::{RelayDataOutFrame, RelayDataProtocol};
use crate::service::{RelayState, authenticated_account, error_response, raw_agent_route_path};
use crate::store::RelayStoreError;
use crate::tunnel::{RelayErrorTunnelSender, RelayExchangeAdmissionError, RelayRoutableExchange};
use crate::tunnel::{RelayPendingResponse, RelayResponsePart, RelayTunnelIdentity};

const RESPONSE_OPEN_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn proxy_http_root(
    Path(agent_id): Path<String>,
    State(state): State<RelayState>,
    request: Request,
) -> Response {
    if let Some(location) = canonical_root_redirect(request.uri()) {
        return Redirect::permanent(&location).into_response();
    }
    let path = match raw_http_agent_path(&agent_id, request.uri()) {
        Ok(path) => path,
        Err(error) => return error_response(error),
    };
    proxy_http(state, agent_id, path, request).await
}

fn canonical_root_redirect(uri: &axum::http::Uri) -> Option<String> {
    if uri.path().ends_with('/') {
        return None;
    }
    let mut location = format!("{}/", uri.path());
    if let Some(query) = uri.query() {
        location.push('?');
        location.push_str(query);
    }
    Some(location)
}

pub(crate) async fn proxy_http_path(
    Path((agent_id, _path)): Path<(String, String)>,
    State(state): State<RelayState>,
    request: Request,
) -> Response {
    let path = match raw_http_agent_path(&agent_id, request.uri()) {
        Ok(path) => path,
        Err(error) => return error_response(error),
    };
    proxy_http(state, agent_id, path, request).await
}

async fn proxy_http(
    state: RelayState,
    agent_id: String,
    path: String,
    request: Request,
) -> Response {
    match proxy_http_inner(state, agent_id, path, request).await {
        Ok(response) => response,
        Err(error) => error_response(error),
    }
}

async fn proxy_http_inner(
    state: RelayState,
    agent_id: String,
    path: String,
    request: Request,
) -> Result<Response, RelayStoreError> {
    let account_id = authenticated_account(&state, request.headers()).await?;
    let identity = RelayTunnelIdentity {
        account_id,
        agent_id: agent_id.clone(),
    };
    let exchange_id = format!(
        "http-{}",
        state.exchange_sequence.fetch_add(1, Ordering::Relaxed)
    );
    let RelayRoutableExchange {
        data_sender: sender,
        error_sender,
        pending,
    } = open_http_exchange(&state, &identity, &exchange_id)?;
    let method = request.method().as_str().to_owned();
    let path_and_query = path_and_query(&path, request.uri().query());
    let headers = match request_headers(request.headers()) {
        Ok(headers) => headers,
        Err(error) => {
            return cancel_active_exchange(&state, &identity, &exchange_id, error).await;
        }
    };
    if let Err(error) = sender
        .send(RelayDataOutFrame::RequestOpen {
            exchange_id: exchange_id.clone(),
            protocol: RelayDataProtocol::Http,
            method: Some(method),
            path_and_query,
            headers,
            access_scope: None,
        })
        .await
    {
        return cancel_active_exchange(
            &state,
            &identity,
            &exchange_id,
            RelayStoreError::Upstream(error),
        )
        .await;
    }

    let mut body = request.into_body().into_data_stream();
    while let Some(chunk) = body.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                return cancel_active_exchange(
                    &state,
                    &identity,
                    &exchange_id,
                    RelayStoreError::Io(error.to_string()),
                )
                .await;
            }
        };
        if let Err(error) = sender
            .send(RelayDataOutFrame::RequestChunk {
                exchange_id: exchange_id.clone(),
                frame_kind: None,
                bytes: chunk.to_vec(),
            })
            .await
        {
            return cancel_active_exchange(
                &state,
                &identity,
                &exchange_id,
                RelayStoreError::Upstream(error),
            )
            .await;
        }
    }
    if let Err(error) = sender
        .send(RelayDataOutFrame::RequestEnd {
            exchange_id: exchange_id.clone(),
        })
        .await
    {
        return cancel_active_exchange(
            &state,
            &identity,
            &exchange_id,
            RelayStoreError::Upstream(error),
        )
        .await;
    }

    build_http_response(&state, &identity, &exchange_id, error_sender, pending).await
}

fn open_http_exchange(
    state: &RelayState,
    identity: &RelayTunnelIdentity,
    exchange_id: &str,
) -> Result<RelayRoutableExchange, RelayStoreError> {
    let mut tunnels = state
        .tunnels
        .lock()
        .map_err(|_| RelayStoreError::Io("Relay tunnel registry lock poisoned".to_owned()))?;
    tunnels
        .open_routable_exchange(identity.clone(), exchange_id.to_owned())
        .map_err(|error| match error {
            RelayExchangeAdmissionError::DataTunnelUnavailable => RelayStoreError::AgentNotFound,
            RelayExchangeAdmissionError::ErrorTunnelUnavailable
            | RelayExchangeAdmissionError::Invalid(_) => {
                RelayStoreError::Upstream(error.to_string())
            }
        })
}

async fn build_http_response(
    state: &RelayState,
    identity: &RelayTunnelIdentity,
    exchange_id: &str,
    error_sender: RelayErrorTunnelSender,
    pending: RelayPendingResponse,
) -> Result<Response, RelayStoreError> {
    let open = match tokio::time::timeout(RESPONSE_OPEN_TIMEOUT, pending.open).await {
        Err(_) => {
            return cancel_active_exchange(
                state,
                identity,
                exchange_id,
                RelayStoreError::Upstream("Relay response-open timed out".to_owned()),
            )
            .await;
        }
        Ok(Err(_)) => {
            return Err(RelayStoreError::Upstream(
                "Relay response-open channel closed".to_owned(),
            ));
        }
        Ok(Ok(Err(error))) => return Err(RelayStoreError::Upstream(error)),
        Ok(Ok(Ok(open))) => open,
    };
    let status = match open.status {
        Some(status) => match StatusCode::from_u16(status) {
            Ok(status) => status,
            Err(error) => {
                return cancel_active_exchange(
                    state,
                    identity,
                    exchange_id,
                    RelayStoreError::Upstream(error.to_string()),
                )
                .await;
            }
        },
        None => {
            return cancel_active_exchange(
                state,
                identity,
                exchange_id,
                RelayStoreError::Upstream("HTTP response is missing status".to_owned()),
            )
            .await;
        }
    };
    let headers = match response_headers(open.headers) {
        Ok(headers) => headers,
        Err(error) => return cancel_active_exchange(state, identity, exchange_id, error).await,
    };
    let (body_sender, body_receiver) = mpsc::channel(8);
    let cancellation_state = state.clone();
    let cancellation_identity = identity.clone();
    let cancellation_exchange_id = exchange_id.to_owned();
    tokio::spawn(async move {
        if let Err(error) = pump_streamed_response(
            pending.parts,
            body_sender,
            cancellation_state,
            cancellation_identity,
            cancellation_exchange_id,
            error_sender,
        )
        .await
        {
            eprintln!("Relay streamed HTTP response pump failed: {error}");
        }
    });
    let stream = futures_util::stream::unfold(body_receiver, |mut receiver| async move {
        receiver.recv().await.map(|item| (item, receiver))
    });
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = status;
    *response.headers_mut() = headers;
    Ok(response)
}

async fn pump_streamed_response(
    mut parts: mpsc::Receiver<Result<RelayResponsePart, String>>,
    body_sender: mpsc::Sender<Result<Bytes, std::io::Error>>,
    state: RelayState,
    identity: RelayTunnelIdentity,
    exchange_id: String,
    error_sender: crate::tunnel::RelayErrorTunnelSender,
) -> Result<(), String> {
    loop {
        let part = tokio::select! {
            _ = body_sender.closed() => {
                return cancel_streamed_exchange(
                    &state,
                    &identity,
                    &exchange_id,
                    &error_sender,
                    "Relay HTTP response client disconnected",
                ).await;
            }
            part = parts.recv() => part,
        };
        match part {
            Some(Ok(RelayResponsePart::Chunk {
                frame_kind: None,
                bytes,
            })) => {
                if body_sender.send(Ok(Bytes::from(bytes))).await.is_err() {
                    return cancel_streamed_exchange(
                        &state,
                        &identity,
                        &exchange_id,
                        &error_sender,
                        "Relay HTTP response client disconnected",
                    )
                    .await;
                }
            }
            Some(Ok(RelayResponsePart::End)) => return Ok(()),
            Some(Ok(RelayResponsePart::Chunk {
                frame_kind: Some(_),
                ..
            })) => {
                let message = "HTTP exchange received a WebSocket frame kind";
                let body_delivery = body_sender
                    .send(Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        message,
                    )))
                    .await
                    .map_err(|_| "Relay HTTP response body receiver closed".to_owned());
                let cancellation = cancel_streamed_exchange(
                    &state,
                    &identity,
                    &exchange_id,
                    &error_sender,
                    message,
                )
                .await;
                body_delivery?;
                return cancellation;
            }
            Some(Err(error)) => {
                body_sender
                    .send(Err(std::io::Error::other(error)))
                    .await
                    .map_err(|_| "Relay HTTP response body receiver closed".to_owned())?;
                return Ok(());
            }
            None => {
                body_sender
                    .send(Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Relay HTTP response closed before response-end",
                    )))
                    .await
                    .map_err(|_| "Relay HTTP response body receiver closed".to_owned())?;
                return Ok(());
            }
        }
    }
}

async fn cancel_streamed_exchange(
    state: &RelayState,
    identity: &RelayTunnelIdentity,
    exchange_id: &str,
    error_sender: &crate::tunnel::RelayErrorTunnelSender,
    message: &str,
) -> Result<(), String> {
    let cancelled = state
        .tunnels
        .lock()
        .map_err(|_| "Relay tunnel registry lock poisoned".to_owned())?
        .cancel_exchange(identity, exchange_id)?;
    if !cancelled {
        return Ok(());
    }
    error_sender
        .send(crate::model::RelayErrorOutFrame::CorrelatedFailure {
            exchange_id: Some(exchange_id.to_owned()),
            code: "relay_client_cancelled".to_owned(),
            message: message.to_owned(),
        })
        .await
}

async fn cancel_active_exchange<T>(
    state: &RelayState,
    identity: &RelayTunnelIdentity,
    exchange_id: &str,
    cause: RelayStoreError,
) -> Result<T, RelayStoreError> {
    let cause_message = cause.to_string();
    let delivery = state
        .tunnels
        .lock()
        .map_err(|_| RelayStoreError::Io("Relay tunnel registry lock poisoned".to_owned()))?
        .fail_exchange(identity, exchange_id, cause_message)
        .map_err(|cleanup| {
            RelayStoreError::Upstream(format!("{cause}; Relay exchange cleanup failed: {cleanup}"))
        })?;
    if let Some(delivery) = delivery {
        delivery.deliver().await.map_err(|cleanup| {
            RelayStoreError::Upstream(format!("{cause}; Relay exchange cleanup failed: {cleanup}"))
        })?;
    }
    Err(cause)
}

fn request_headers(headers: &HeaderMap) -> Result<Vec<(String, Vec<u8>)>, RelayStoreError> {
    let mut output = headers
        .iter()
        .filter(|(name, _)| forward_request_header(name))
        .map(|(name, value)| Ok((name.as_str().to_owned(), value.as_bytes().to_vec())))
        .collect::<Result<Vec<_>, RelayStoreError>>()?;
    if let Some(cookie) = upstream_cookie(headers) {
        output.push((header::COOKIE.as_str().to_owned(), cookie.into_bytes()));
    }
    Ok(output)
}

fn upstream_cookie(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::COOKIE)?.to_str().ok()?;
    let filtered = value
        .split(';')
        .map(str::trim)
        .filter(|part| {
            !part.is_empty()
                && !part.starts_with("freehand_relay_session=")
                && !part.starts_with("freehand_adp_auth=")
        })
        .collect::<Vec<_>>()
        .join("; ");
    (!filtered.is_empty()).then_some(filtered)
}

fn response_headers(values: Vec<(String, Vec<u8>)>) -> Result<HeaderMap, RelayStoreError> {
    let mut headers = HeaderMap::new();
    for (name, value) in values {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|error| RelayStoreError::Upstream(error.to_string()))?;
        if !forward_response_header(&name) {
            continue;
        }
        let value = HeaderValue::from_bytes(&value)
            .map_err(|error| RelayStoreError::Upstream(error.to_string()))?;
        headers.append(name, value);
    }
    Ok(headers)
}

fn path_and_query(path: &str, query: Option<&str>) -> String {
    let path = format!("/{}", path.trim_start_matches('/'));
    match query {
        Some(query) => format!("{path}?{query}"),
        None => path,
    }
}

fn raw_http_agent_path(agent_id: &str, uri: &axum::http::Uri) -> Result<String, RelayStoreError> {
    raw_agent_route_path(agent_id, uri, None)
}

fn forward_request_header(name: &HeaderName) -> bool {
    !matches!(
        name,
        &header::HOST
            | &header::AUTHORIZATION
            | &header::COOKIE
            | &header::CONNECTION
            | &header::CONTENT_LENGTH
    )
}

fn forward_response_header(name: &HeaderName) -> bool {
    !matches!(
        name,
        &header::CONNECTION
            | &header::CONTENT_LENGTH
            | &header::SET_COOKIE
            | &header::TRANSFER_ENCODING
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use tokio::sync::{mpsc, oneshot, watch};

    use crate::service::RelayServiceConfig;
    use crate::store::RelayStore;

    #[test]
    fn raw_http_route_matches_encoded_agent_identity_and_rejects_mismatch() {
        let uri: axum::http::Uri = "/relay/agents/studio%20one/files/a%3Fb"
            .parse()
            .expect("URI");
        assert_eq!(
            raw_http_agent_path("studio one", &uri).expect("encoded Agent route"),
            "/files/a%3Fb"
        );
        assert!(raw_http_agent_path("different-agent", &uri).is_err());
    }

    #[test]
    fn relay_agent_root_redirect_preserves_query_and_canonical_slash() {
        let root: axum::http::Uri = "/relay/agents/studio?client=android-webview"
            .parse()
            .expect("root URI");
        assert_eq!(
            canonical_root_redirect(&root).as_deref(),
            Some("/relay/agents/studio/?client=android-webview")
        );

        let canonical: axum::http::Uri = "/relay/agents/studio/?client=android-webview"
            .parse()
            .expect("canonical URI");
        assert_eq!(canonical_root_redirect(&canonical), None);
    }
    use crate::tunnel::{RelayResponseOpen, RelayTunnelRegistry};

    #[tokio::test]
    async fn incomplete_http_response_is_not_projected_as_success() {
        let temp = tempfile::tempdir().expect("temporary store");
        let store_path = temp.path().join("relay.json");
        RelayStore::initialize(&store_path).expect("initialize store");
        let store = RelayStore::load(&store_path).expect("load store");
        let (presence_updates, _) = watch::channel(0);
        let state = RelayState {
            store: Arc::new(Mutex::new(store)),
            tunnels: Arc::new(Mutex::new(RelayTunnelRegistry::default())),
            exchange_sequence: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            presence_updates,
            config: RelayServiceConfig {
                presence_lease_seconds: 45,
                secure_cookie: false,
            },
        };
        let identity = RelayTunnelIdentity {
            account_id: "account-1".to_owned(),
            agent_id: "agent-1".to_owned(),
        };
        let (open_tx, open_rx) = oneshot::channel();
        let (parts_tx, parts_rx) = mpsc::channel(1);
        let (error_tx, _error_rx) = mpsc::channel(1);
        state
            .tunnels
            .lock()
            .expect("registry")
            .attach_control(identity.clone())
            .map(|admission| admission.generation)
            .expect("control tunnel");
        state
            .tunnels
            .lock()
            .expect("registry")
            .admit_error(
                identity.clone(),
                crate::tunnel::RelayErrorTunnelSender::new(error_tx),
            )
            .expect("error tunnel");
        let error_sender = state
            .tunnels
            .lock()
            .expect("registry")
            .error_sender(&identity)
            .expect("error sender");
        open_tx
            .send(Ok(RelayResponseOpen {
                status: Some(200),
                headers: Vec::new(),
            }))
            .expect("response-open receiver");
        parts_tx
            .send(Ok(RelayResponsePart::Chunk {
                frame_kind: None,
                bytes: b"truncated".to_vec(),
            }))
            .await
            .expect("response chunk");
        drop(parts_tx);

        let response = build_http_response(
            &state,
            &identity,
            "http-1",
            error_sender,
            RelayPendingResponse {
                open: open_rx,
                parts: parts_rx,
            },
        )
        .await
        .expect("response-open creates streaming response");
        let error = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect_err("missing response-end must fail the response body");
        assert!(
            error
                .to_string()
                .contains("Relay HTTP response closed before response-end")
        );
    }

    #[tokio::test]
    async fn dropped_stream_cancels_only_its_pending_exchange() {
        let temp = tempfile::tempdir().expect("temporary store");
        let store_path = temp.path().join("relay.json");
        RelayStore::initialize(&store_path).expect("initialize store");
        let store = RelayStore::load(&store_path).expect("load store");
        let (presence_updates, _) = watch::channel(0);
        let state = RelayState {
            store: Arc::new(Mutex::new(store)),
            tunnels: Arc::new(Mutex::new(RelayTunnelRegistry::default())),
            exchange_sequence: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            presence_updates,
            config: RelayServiceConfig {
                presence_lease_seconds: 45,
                secure_cookie: false,
            },
        };
        let identity = RelayTunnelIdentity {
            account_id: "account-1".to_owned(),
            agent_id: "agent-1".to_owned(),
        };
        let (pending, open) = {
            let mut active = state.tunnels.lock().expect("registry");
            let pending = active
                .open_exchange_for_test(identity.clone(), "http-stream".to_owned())
                .expect("stream exchange");
            let _other = active
                .open_exchange_for_test(identity.clone(), "http-other".to_owned())
                .expect("other exchange");
            let open = active
                .accept_data(
                    &identity,
                    crate::model::RelayDataInFrame::ResponseOpen {
                        exchange_id: "http-stream".to_owned(),
                        status: Some(200),
                        headers: vec![(
                            "content-type".to_owned(),
                            b"application/octet-stream".to_vec(),
                        )],
                    },
                )
                .expect("response open");
            (pending, open)
        };
        open.deliver().await.expect("response open delivery");
        let (error_tx, mut error_rx) = mpsc::channel(1);
        state
            .tunnels
            .lock()
            .expect("registry")
            .attach_control(identity.clone())
            .map(|admission| admission.generation)
            .expect("control tunnel");
        state
            .tunnels
            .lock()
            .expect("registry")
            .admit_error(
                identity.clone(),
                crate::tunnel::RelayErrorTunnelSender::new(error_tx),
            )
            .expect("error tunnel");
        let error_sender = state
            .tunnels
            .lock()
            .expect("registry")
            .error_sender(&identity)
            .expect("error sender");
        let response = build_http_response(&state, &identity, "http-stream", error_sender, pending)
            .await
            .expect("stream response");
        drop(response);

        assert!(matches!(
            tokio::time::timeout(std::time::Duration::from_secs(1), error_rx.recv())
                .await
                .expect("cancel timeout"),
            Some(crate::model::RelayErrorOutFrame::CorrelatedFailure {
                exchange_id: Some(exchange_id),
                code,
                ..
            }) if exchange_id == "http-stream" && code == "relay_client_cancelled"
        ));
        let mut registry = state.tunnels.lock().expect("registry");
        assert!(registry.has_pending_exchange("http-stream"));
        assert!(registry.has_pending_exchange("http-other"));
        assert!(matches!(
            registry
                .accept_data(
                    &identity,
                    crate::model::RelayDataInFrame::ResponseEnd {
                        exchange_id: "http-stream".to_owned(),
                    },
                )
                .expect("cancel response end"),
            crate::tunnel::RelayDataDelivery::Cancelled
        ));
        assert!(!registry.has_pending_exchange("http-stream"));
        assert!(registry.has_pending_exchange("http-other"));
    }
}
