use std::time::Duration;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;

use crate::model::{AgentPresence, RelayDirectoryOutFrame};
use crate::service::{RelayState, authenticated_account, error_response, project_directory};

pub(crate) async fn directory_subscription(
    State(state): State<RelayState>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    let account_id = match authenticated_account(&state, &headers).await {
        Ok(account_id) => account_id,
        Err(error) => return error_response(error),
    };
    let updates = state.presence_updates.subscribe();
    upgrade
        .on_upgrade(move |socket| run_directory_socket(socket, state, account_id, updates))
        .into_response()
}

async fn run_directory_socket(
    mut socket: WebSocket,
    state: RelayState,
    account_id: String,
    mut updates: tokio::sync::watch::Receiver<u64>,
) {
    let mut previous_agents: Option<Vec<AgentPresence>> = None;
    let mut lease_tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        if let Err(error) =
            send_snapshot_if_changed(&mut socket, &state, &account_id, &mut previous_agents).await
        {
            send_terminal_or_log(&mut socket, error).await;
            return;
        }
        tokio::select! {
            changed = updates.changed() => {
                if changed.is_err() {
                    send_terminal_or_log(
                        &mut socket,
                        "Relay presence projection owner stopped".to_owned(),
                    ).await;
                    return;
                }
            }
            _ = lease_tick.tick() => {}
            incoming = socket.next() => match incoming {
                Some(Ok(Message::Close(_))) | None => return,
                Some(Ok(_)) => {
                    send_terminal_or_log(
                        &mut socket,
                        "Relay directory subscription is server-output-only".to_owned(),
                    ).await;
                    return;
                }
                Some(Err(error)) => {
                    eprintln!("Relay directory subscriber socket failed: {error}");
                    return;
                }
            }
        }
    }
}

async fn send_terminal_or_log(socket: &mut WebSocket, message: String) {
    if let Err(error) = send_terminal(socket, message).await {
        eprintln!("Relay directory terminal frame failed: {error}");
    }
}

async fn send_snapshot_if_changed(
    socket: &mut WebSocket,
    state: &RelayState,
    account_id: &str,
    previous_agents: &mut Option<Vec<AgentPresence>>,
) -> Result<(), String> {
    let directory = project_directory(state, account_id.to_owned())
        .await
        .map_err(|error| error.to_string())?;
    if previous_agents.as_ref() == Some(&directory.agents) {
        return Ok(());
    }
    *previous_agents = Some(directory.agents.clone());
    let frame = RelayDirectoryOutFrame::Snapshot { directory };
    let text = serde_json::to_string(&frame).map_err(|error| error.to_string())?;
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|error| error.to_string())
}

async fn send_terminal(socket: &mut WebSocket, message: String) -> Result<(), String> {
    let frame = RelayDirectoryOutFrame::Terminal {
        code: "relay_directory_subscription_terminal".to_owned(),
        message,
    };
    let text = serde_json::to_string(&frame).map_err(|error| error.to_string())?;
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|error| error.to_string())
}
