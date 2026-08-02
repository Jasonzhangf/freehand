use std::path::Path;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::OriginalUri;
use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use freehand_relay::{
    AgentDirectory, AgentHeartbeat, AgentRole, AgentWorkStatus, AuthResponse, RelayAgentClient,
    RelayAgentClientConfig, RelayDirectoryOutFrame, RelayService, RelayServiceConfig, RelayStore,
};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

struct TestServer {
    base_url: String,
    task: JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn spawn_upstream() -> TestServer {
    async fn home(headers: HeaderMap) -> impl IntoResponse {
        assert!(headers.get("authorization").is_none());
        assert!(
            matches!(
                headers.get("cookie").and_then(|value| value.to_str().ok()),
                None | Some("theme=dark")
            ),
            "Relay or local-ADP authentication cookie reached the Agent HTTP bridge"
        );
        (
            [(
                "set-cookie",
                "freehand_adp_auth=upstream-token; Path=/; HttpOnly",
            )],
            Html(r#"<script src="/assets/app.js"></script><script>new WebSocket("/adp")</script>"#),
        )
    }

    async fn asset() -> &'static str {
        "window.freehandRelayBlackbox = true;"
    }

    async fn invalid_html() -> Response {
        Response::builder()
            .header("content-type", "text/html")
            .body(Body::from(vec![0xff, 0xfe]))
            .expect("invalid html response")
    }

    async fn echo_socket(upgrade: WebSocketUpgrade) -> Response {
        upgrade
            .on_upgrade(|mut socket| async move {
                while let Some(Ok(message)) = socket.recv().await {
                    match message {
                        Message::Text(text) => {
                            if socket.send(Message::Text(text)).await.is_err() {
                                break;
                            }
                        }
                        Message::Binary(bytes) => {
                            if socket.send(Message::Binary(bytes)).await.is_err() {
                                break;
                            }
                        }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
            })
            .into_response()
    }

    async fn adp(
        OriginalUri(uri): OriginalUri,
        headers: HeaderMap,
        upgrade: WebSocketUpgrade,
    ) -> impl IntoResponse {
        if uri.path() == "/echo" {
            assert_eq!(uri.query(), Some("mode=opaque"));
            assert!(headers.get("authorization").is_none());
            return echo_socket(upgrade).await;
        }
        let authorized_by_cookie = headers.get("cookie").and_then(|value| value.to_str().ok())
            == Some("freehand_adp_auth=upstream-token");
        let authorized_by_bearer = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            == Some("Bearer upstream-token");
        if !authorized_by_cookie && !authorized_by_bearer {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        echo_socket(upgrade).await
    }

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream bind");
    let address = listener.local_addr().expect("upstream address");
    let router = Router::new()
        .route("/", get(home))
        .route("/assets/app.js", get(asset))
        .route("/invalid-html", get(invalid_html))
        .route("/adp", get(adp))
        .route("/echo", get(adp));
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("upstream serve");
    });
    TestServer {
        base_url: format!("http://{address}"),
        task,
    }
}

async fn spawn_relay(store_path: &Path, lease_seconds: u64) -> TestServer {
    spawn_relay_with_cookie_policy(store_path, lease_seconds, false).await
}

async fn spawn_relay_with_cookie_policy(
    store_path: &Path,
    lease_seconds: u64,
    secure_cookie: bool,
) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("relay bind");
    let address = listener.local_addr().expect("relay address");
    RelayStore::initialize(store_path).expect("initialize relay store");
    let store = RelayStore::load(store_path).expect("relay store");
    let service = RelayService::new(
        store,
        RelayServiceConfig {
            presence_lease_seconds: lease_seconds,
            secure_cookie,
        },
    )
    .expect("relay service");
    let task = tokio::spawn(async move {
        service.serve(listener).await.expect("relay serve");
    });
    TestServer {
        base_url: format!("http://{address}"),
        task,
    }
}

#[tokio::test]
async fn session_cookie_policy_is_explicit_and_http_cookie_authenticates() {
    let temp = TempDir::new().expect("tempdir");
    let http_relay =
        spawn_relay_with_cookie_policy(&temp.path().join("http-relay.json"), 45, false).await;
    let client = Client::new();
    let response = client
        .post(format!("{}/relay/api/auth/register", http_relay.base_url))
        .json(&serde_json::json!({
            "username": "http-cookie",
            "password": "relay-password-123"
        }))
        .send()
        .await
        .expect("HTTP-mode register");
    assert_eq!(response.status(), StatusCode::CREATED);
    let set_cookie = response
        .headers()
        .get("set-cookie")
        .expect("session cookie")
        .to_str()
        .expect("session cookie text")
        .to_owned();
    assert!(set_cookie.contains("HttpOnly"));
    assert!(!set_cookie.contains("; Secure"));
    let cookie_pair = set_cookie.split(';').next().expect("session cookie pair");
    let directory = client
        .get(format!("{}/relay/api/agents", http_relay.base_url))
        .header("cookie", cookie_pair)
        .send()
        .await
        .expect("cookie-only directory");
    assert_eq!(directory.status(), StatusCode::OK);

    let secure_relay =
        spawn_relay_with_cookie_policy(&temp.path().join("secure-relay.json"), 45, true).await;
    let secure_response = client
        .post(format!("{}/relay/api/auth/register", secure_relay.base_url))
        .json(&serde_json::json!({
            "username": "secure-cookie",
            "password": "relay-password-123"
        }))
        .send()
        .await
        .expect("TLS-mode register");
    let secure_set_cookie = secure_response
        .headers()
        .get("set-cookie")
        .expect("secure session cookie")
        .to_str()
        .expect("secure session cookie text");
    assert!(secure_set_cookie.contains("; Secure"));
}

async fn register(client: &Client, relay: &TestServer, username: &str) -> AuthResponse {
    let response = client
        .post(format!("{}/relay/api/auth/register", relay.base_url))
        .json(&serde_json::json!({
            "username": username,
            "password": "relay-password-123"
        }))
        .send()
        .await
        .expect("register request");
    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.expect("register response")
}

async fn login(client: &Client, relay: &TestServer, username: &str) -> AuthResponse {
    client
        .post(format!("{}/relay/api/auth/login", relay.base_url))
        .json(&serde_json::json!({
            "username": username,
            "password": "relay-password-123"
        }))
        .send()
        .await
        .expect("login request")
        .error_for_status()
        .expect("login status")
        .json()
        .await
        .expect("login response")
}

async fn heartbeat(
    client: &Client,
    relay: &TestServer,
    auth: &AuthResponse,
    _upstream: &TestServer,
) {
    let response = client
        .post(format!("{}/relay/api/agents/heartbeat", relay.base_url))
        .bearer_auth(&auth.access_token)
        .json(&AgentHeartbeat {
            agent_id: "studio".to_owned(),
            display_name: "Studio Master".to_owned(),
            node_id: "node-studio".to_owned(),
            role: AgentRole::Master,
            status: AgentWorkStatus::Running,
            active_session_count: 3,
        })
        .send()
        .await
        .expect("heartbeat request");
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

async fn start_agent(
    client: &Client,
    relay: &TestServer,
    auth: &AuthResponse,
    local: &TestServer,
) -> JoinHandle<()> {
    let local_addr = local
        .base_url
        .strip_prefix("http://")
        .expect("local daemon URL")
        .parse()
        .expect("local daemon address");
    let client_task = RelayAgentClient::new(RelayAgentClientConfig {
        relay_base_url: relay.base_url.clone(),
        access_token: auth.access_token.clone(),
        heartbeat: AgentHeartbeat {
            agent_id: "studio".to_owned(),
            display_name: "Studio Master".to_owned(),
            node_id: "node-studio".to_owned(),
            role: AgentRole::Master,
            status: AgentWorkStatus::Running,
            active_session_count: 3,
        },
        local_daemon_addr: local_addr,
        local_adp_token: Some("upstream-token".to_owned()),
        heartbeat_interval: Duration::from_millis(100),
    })
    .expect("Agent client");
    let task = tokio::spawn(async move {
        client_task.run().await.expect("Agent tunnel");
    });
    for _ in 0..50 {
        let response = client
            .get(format!("{}/relay/api/agents", relay.base_url))
            .bearer_auth(&auth.access_token)
            .send()
            .await;
        let directory = match response {
            Ok(response) => response.json::<AgentDirectory>().await.ok(),
            Err(_) => None,
        };
        let online = directory
            .as_ref()
            .and_then(|directory| directory.agents.first())
            .is_some_and(|agent| agent.online);
        if online {
            let proxy_ready = client
                .get(format!("{}/relay/agents/studio/", relay.base_url))
                .bearer_auth(&auth.access_token)
                .header("cookie", "freehand_adp_auth=upstream-token")
                .send()
                .await
                .is_ok_and(|response| response.status() == StatusCode::OK);
            if proxy_ready {
                return task;
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    task.abort();
    panic!("Agent outbound tunnel did not become online");
}

#[tokio::test]
async fn authenticated_directory_http_and_adp_proxy_are_account_isolated() {
    let temp = TempDir::new().expect("tempdir");
    let upstream = spawn_upstream().await;
    let relay = spawn_relay(&temp.path().join("relay.json"), 45).await;
    let client = Client::new();
    let first = register(&client, &relay, "first").await;
    let second = register(&client, &relay, "second").await;
    let logged_in = login(&client, &relay, "first").await;
    assert_eq!(logged_in.account_id, first.account_id);
    assert_ne!(logged_in.access_token, first.access_token);

    let mut directory_request = format!(
        "{}/relay/api/agents/subscribe",
        relay.base_url.replace("http://", "ws://")
    )
    .into_client_request()
    .expect("directory subscription request");
    directory_request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", logged_in.access_token)
            .parse()
            .expect("directory authorization"),
    );
    let (mut directory_socket, _) = connect_async(directory_request)
        .await
        .expect("directory subscription");
    let initial = directory_socket
        .next()
        .await
        .expect("initial directory frame")
        .expect("initial directory result")
        .into_text()
        .expect("initial directory text");
    let initial: RelayDirectoryOutFrame =
        serde_json::from_str(&initial).expect("initial directory json");
    assert!(matches!(
        initial,
        RelayDirectoryOutFrame::Snapshot { directory } if directory.agents.is_empty()
    ));
    let mut second_directory_request = format!(
        "{}/relay/api/agents/subscribe",
        relay.base_url.replace("http://", "ws://")
    )
    .into_client_request()
    .expect("second directory subscription request");
    second_directory_request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", second.access_token)
            .parse()
            .expect("second directory authorization"),
    );
    let (mut second_directory_socket, _) = connect_async(second_directory_request)
        .await
        .expect("second directory subscription");
    let second_initial = second_directory_socket
        .next()
        .await
        .expect("second initial directory frame")
        .expect("second initial directory result")
        .into_text()
        .expect("second initial directory text");
    let second_initial: RelayDirectoryOutFrame =
        serde_json::from_str(&second_initial).expect("second initial directory json");
    assert!(matches!(
        second_initial,
        RelayDirectoryOutFrame::Snapshot { directory } if directory.agents.is_empty()
    ));
    heartbeat(&client, &relay, &first, &upstream).await;
    let agent_task = start_agent(&client, &relay, &first, &upstream).await;

    let online = tokio::time::timeout(Duration::from_secs(2), directory_socket.next())
        .await
        .expect("online directory timeout")
        .expect("online directory frame")
        .expect("online directory result")
        .into_text()
        .expect("online directory text");
    let online: RelayDirectoryOutFrame =
        serde_json::from_str(&online).expect("online directory json");
    assert!(matches!(
        online,
        RelayDirectoryOutFrame::Snapshot { directory }
            if directory.agents.len() == 1 && directory.agents[0].online
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(200), second_directory_socket.next())
            .await
            .is_err(),
        "foreign Agent presence leaked into another account subscription"
    );

    let directory: AgentDirectory = client
        .get(format!("{}/relay/api/agents", relay.base_url))
        .bearer_auth(&first.access_token)
        .send()
        .await
        .expect("directory")
        .error_for_status()
        .expect("directory status")
        .json()
        .await
        .expect("directory body");
    assert_eq!(directory.agents.len(), 1);
    assert_eq!(directory.agents[0].agent_id, "studio");
    assert_eq!(directory.agents[0].active_session_count, 3);

    let html = client
        .get(format!("{}/relay/agents/studio/", relay.base_url))
        .bearer_auth(&first.access_token)
        .header(
            "cookie",
            format!(
                "freehand_relay_session={}; freehand_adp_auth=upstream-token; theme=dark",
                first.access_token
            ),
        )
        .send()
        .await
        .expect("proxy root")
        .error_for_status()
        .expect("proxy status");
    assert!(
        html.headers().get_all("set-cookie").iter().next().is_none(),
        "Agent-local cookies crossed into the Relay origin"
    );
    let html = html.text().await.expect("proxy body");
    assert!(html.contains("/assets/app.js"));
    assert!(html.contains("/adp"));
    assert!(!html.contains("/relay/agents/studio/assets/app.js"));

    let asset = client
        .get(format!(
            "{}/relay/agents/studio/assets/app.js",
            relay.base_url
        ))
        .bearer_auth(&first.access_token)
        .send()
        .await
        .expect("proxy asset")
        .error_for_status()
        .expect("asset status")
        .text()
        .await
        .expect("asset body");
    assert!(asset.contains("freehandRelayBlackbox"));

    let invalid_html = client
        .get(format!(
            "{}/relay/agents/studio/invalid-html",
            relay.base_url
        ))
        .bearer_auth(&first.access_token)
        .send()
        .await
        .expect("invalid html proxy");
    assert_eq!(invalid_html.status(), StatusCode::OK);
    assert_eq!(
        invalid_html
            .bytes()
            .await
            .expect("invalid HTML bytes")
            .as_ref(),
        &[0xff, 0xfe]
    );

    let denied = client
        .get(format!("{}/relay/agents/studio/", relay.base_url))
        .bearer_auth(&second.access_token)
        .send()
        .await
        .expect("cross-account proxy");
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);

    let mut denied_connect_request = format!(
        "{}/relay/agents/studio/connect/echo",
        relay.base_url.replace("http://", "ws://")
    )
    .into_client_request()
    .expect("cross-account connect request");
    denied_connect_request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", second.access_token)
            .parse()
            .expect("cross-account connect authorization"),
    );
    let denied_connect = connect_async(denied_connect_request)
        .await
        .expect_err("cross-account connect must reject before upgrade");
    match denied_connect {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        error => panic!("expected cross-account HTTP rejection, got {error}"),
    }

    let mut invalid_connect_request = format!(
        "{}/relay/agents/studio/connect/http:%2F%2Fevil",
        relay.base_url.replace("http://", "ws://")
    )
    .into_client_request()
    .expect("invalid connect request");
    invalid_connect_request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", first.access_token)
            .parse()
            .expect("invalid connect authorization"),
    );
    let invalid_connect = connect_async(invalid_connect_request)
        .await
        .expect_err("invalid connect target must reject before upgrade");
    match invalid_connect {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        error => panic!("expected invalid-target HTTP rejection, got {error}"),
    }

    let mut ws_request = format!(
        "{}/relay/agents/studio/adp",
        relay.base_url.replace("http://", "ws://")
    )
    .into_client_request()
    .expect("websocket request");
    ws_request.headers_mut().insert(
        "cookie",
        format!(
            "freehand_relay_session={}; freehand_adp_auth=upstream-token",
            first.access_token
        )
        .parse()
        .expect("cookie header"),
    );
    let (mut socket, _) = connect_async(ws_request).await.expect("relay websocket");
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            "relay-adp-roundtrip".into(),
        ))
        .await
        .expect("send websocket");
    let echoed = socket
        .next()
        .await
        .expect("echo frame")
        .expect("echo result");
    assert_eq!(
        echoed.into_text().expect("echo text"),
        "relay-adp-roundtrip"
    );

    let mut generic_request = format!(
        "{}/relay/agents/studio/connect/echo?mode=opaque",
        relay.base_url.replace("http://", "ws://")
    )
    .into_client_request()
    .expect("generic websocket request");
    generic_request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", logged_in.access_token)
            .parse()
            .expect("generic websocket authorization"),
    );
    let (mut generic_socket, _) = connect_async(generic_request)
        .await
        .expect("generic Relay WebSocket");
    generic_socket
        .send(tokio_tungstenite::tungstenite::Message::Binary(
            b"opaque-peer-bytes".to_vec().into(),
        ))
        .await
        .expect("send generic bytes");
    let generic_echo = generic_socket
        .next()
        .await
        .expect("generic echo frame")
        .expect("generic echo result");
    assert_eq!(generic_echo.into_data(), b"opaque-peer-bytes".to_vec());
    generic_socket
        .close(None)
        .await
        .expect("close generic socket");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let directory_after_close: AgentDirectory = client
        .get(format!("{}/relay/api/agents", relay.base_url))
        .bearer_auth(&first.access_token)
        .send()
        .await
        .expect("directory after generic close")
        .error_for_status()
        .expect("directory after generic close status")
        .json()
        .await
        .expect("directory after generic close body");
    assert!(directory_after_close.agents[0].online);

    let mut abrupt_request = format!(
        "{}/relay/agents/studio/connect/echo?mode=opaque",
        relay.base_url.replace("http://", "ws://")
    )
    .into_client_request()
    .expect("abrupt generic websocket request");
    abrupt_request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", first.access_token)
            .parse()
            .expect("abrupt generic websocket authorization"),
    );
    let (abrupt_socket, _) = connect_async(abrupt_request)
        .await
        .expect("abrupt generic Relay WebSocket");
    drop(abrupt_socket);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let directory_after_abrupt_drop: AgentDirectory = client
        .get(format!("{}/relay/api/agents", relay.base_url))
        .bearer_auth(&first.access_token)
        .send()
        .await
        .expect("directory after abrupt drop")
        .error_for_status()
        .expect("directory after abrupt drop status")
        .json()
        .await
        .expect("directory after abrupt drop body");
    assert!(directory_after_abrupt_drop.agents[0].online);

    let mut second_generic_request = format!(
        "{}/relay/agents/studio/connect/echo?mode=opaque",
        relay.base_url.replace("http://", "ws://")
    )
    .into_client_request()
    .expect("second generic websocket request");
    second_generic_request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", first.access_token)
            .parse()
            .expect("second generic websocket authorization"),
    );
    let (mut second_generic_socket, _) = connect_async(second_generic_request)
        .await
        .expect("second generic Relay WebSocket");
    second_generic_socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            "second-exchange".into(),
        ))
        .await
        .expect("send second generic frame");
    let second_generic_echo = second_generic_socket
        .next()
        .await
        .expect("second generic echo frame")
        .expect("second generic echo result");
    assert_eq!(
        second_generic_echo.into_text().expect("second echo text"),
        "second-exchange"
    );
    agent_task.abort();

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let offline = directory_socket
                .next()
                .await
                .expect("offline directory frame")
                .expect("offline directory result")
                .into_text()
                .expect("offline directory text");
            let offline: RelayDirectoryOutFrame =
                serde_json::from_str(&offline).expect("offline directory json");
            if matches!(
                offline,
                RelayDirectoryOutFrame::Snapshot { directory }
                    if directory.agents.len() == 1 && !directory.agents[0].online
            ) {
                break;
            }
        }
    })
    .await
    .expect("offline directory timeout");
}

#[tokio::test]
async fn auth_errors_expiry_and_corrupt_restart_fail_explicitly() {
    let temp = TempDir::new().expect("tempdir");
    let store_path = temp.path().join("relay.json");
    let upstream = spawn_upstream().await;
    let relay = spawn_relay(&store_path, 1).await;
    let client = Client::new();

    let short_password = client
        .post(format!("{}/relay/api/auth/register", relay.base_url))
        .json(&serde_json::json!({"username":"short","password":"short"}))
        .send()
        .await
        .expect("short password");
    assert_eq!(short_password.status(), StatusCode::BAD_REQUEST);

    let auth = register(&client, &relay, "owner").await;
    let duplicate = client
        .post(format!("{}/relay/api/auth/register", relay.base_url))
        .json(&serde_json::json!({
            "username":"owner",
            "password":"relay-password-456"
        }))
        .send()
        .await
        .expect("duplicate");
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let wrong_password = client
        .post(format!("{}/relay/api/auth/login", relay.base_url))
        .json(&serde_json::json!({"username":"owner","password":"wrong-password"}))
        .send()
        .await
        .expect("wrong password");
    assert_eq!(wrong_password.status(), StatusCode::UNAUTHORIZED);

    let missing_token = client
        .get(format!("{}/relay/api/agents", relay.base_url))
        .send()
        .await
        .expect("missing token");
    assert_eq!(missing_token.status(), StatusCode::UNAUTHORIZED);

    let unauthenticated_directory_request = format!(
        "{}/relay/api/agents/subscribe",
        relay.base_url.replace("http://", "ws://")
    )
    .into_client_request()
    .expect("unauthenticated directory request");
    let unauthenticated_directory = connect_async(unauthenticated_directory_request)
        .await
        .expect_err("unauthenticated directory must reject before upgrade");
    match unauthenticated_directory {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
        error => panic!("expected unauthenticated HTTP rejection, got {error}"),
    }

    let missing_session_count = client
        .post(format!("{}/relay/api/agents/heartbeat", relay.base_url))
        .bearer_auth(&auth.access_token)
        .json(&serde_json::json!({
            "agentId":"incomplete",
            "displayName":"Incomplete",
            "nodeId":"node-incomplete",
            "role":"worker",
            "status":"idle"
        }))
        .send()
        .await
        .expect("missing session count");
    assert_eq!(
        missing_session_count.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let unreachable_heartbeat = client
        .post(format!("{}/relay/api/agents/heartbeat", relay.base_url))
        .bearer_auth(&auth.access_token)
        .json(&serde_json::json!({
            "agentId":"unreachable",
            "displayName":"Unreachable",
            "nodeId":"node-unreachable",
            "role":"worker",
            "status":"error",
            "activeSessionCount":0
        }))
        .send()
        .await
        .expect("unreachable heartbeat");
    assert_eq!(unreachable_heartbeat.status(), StatusCode::ACCEPTED);
    let mut unreachable_request = format!(
        "{}/relay/agents/unreachable/adp",
        relay.base_url.replace("http://", "ws://")
    )
    .into_client_request()
    .expect("unreachable websocket request");
    unreachable_request.headers_mut().insert(
        "authorization",
        format!("Bearer {}", auth.access_token)
            .parse()
            .expect("authorization header"),
    );
    let unreachable_error = connect_async(unreachable_request)
        .await
        .expect_err("offline Agent must reject before Relay upgrade");
    match unreachable_error {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        error => panic!("expected explicit HTTP rejection, got {error}"),
    }

    heartbeat(&client, &relay, &auth, &upstream).await;
    let agent_task = start_agent(&client, &relay, &auth, &upstream).await;
    agent_task.abort();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let directory: AgentDirectory = client
        .get(format!("{}/relay/api/agents", relay.base_url))
        .bearer_auth(&auth.access_token)
        .send()
        .await
        .expect("expired directory")
        .error_for_status()
        .expect("expired directory status")
        .json()
        .await
        .expect("expired directory body");
    assert!(!directory.agents[0].online);
    let expired_proxy = client
        .get(format!("{}/relay/agents/studio/", relay.base_url))
        .bearer_auth(&auth.access_token)
        .send()
        .await
        .expect("expired proxy");
    assert_eq!(expired_proxy.status(), StatusCode::NOT_FOUND);

    drop(relay);
    std::fs::write(&store_path, b"not-json").expect("corrupt store");
    assert!(RelayStore::load(&store_path).is_err());

    let absent_store = temp.path().join("absent.json");
    assert!(RelayStore::load(&absent_store).is_err());
    RelayStore::initialize(&absent_store).expect("explicit store initialization");
    assert!(RelayStore::initialize(&absent_store).is_err());
    std::fs::write(
        &absent_store,
        br#"{"schemaVersion":1,"accounts":{},"tokens":{}}"#,
    )
    .expect("incomplete store");
    assert!(RelayStore::load(&absent_store).is_err());
}
