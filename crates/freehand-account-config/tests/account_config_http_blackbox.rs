use std::sync::Arc;

use axum::http::HeaderMap;
use freehand_account_config::{
    AccountAuthenticator, AccountConfigDocument, AccountConfigError, AccountConfigStore,
    ConfigDocumentContent, ProviderAuthReference, RelayEndpointCandidate, SharedProviderDefinition,
    config_router,
};
use tempfile::TempDir;
use tower::ServiceExt;

fn sample_document() -> AccountConfigDocument {
    AccountConfigDocument {
        schema_version: 1,
        revision: None,
        etag: None,
        updated_at: None,
        document: ConfigDocumentContent {
            provider_registry: vec![SharedProviderDefinition {
                id: "primary".to_owned(),
                label: "Primary".to_owned(),
                provider_type: "openai-compatible".to_owned(),
                protocol: "chat".to_owned(),
                base_url: "https://api.example.com/v1".to_owned(),
                auth: ProviderAuthReference {
                    auth_type: "env".to_owned(),
                    auth_source: "FREEHAND_PRIMARY_API_KEY".to_owned(),
                },
                model: "gpt-5.6".to_owned(),
            }],
            model_groups: vec![],
            relay_endpoint_candidates: vec![RelayEndpointCandidate {
                id: "claw".to_owned(),
                url: "https://relay.example.com".to_owned(),
                token_env_name: "FREEHAND_RELAY_TOKEN".to_owned(),
            }],
            remote_daemon_registry: vec![],
        },
    }
}

fn authenticator() -> Arc<dyn AccountAuthenticator> {
    Arc::new(|headers: &HeaderMap| {
        headers
            .get("x-test-account-id")
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| AccountConfigError::Invalid("authentication required".to_owned()))
    })
}

#[tokio::test]
async fn account_config_http_isolation_conflict_and_secret_boundary() {
    let temp = TempDir::new().expect("tempdir");
    let store = Arc::new(AccountConfigStore::new(temp.path().to_path_buf()).expect("store"));
    let app = config_router(store, authenticator());

    let missing_auth = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/relay/api/config")
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(missing_auth.status(), axum::http::StatusCode::UNAUTHORIZED);

    let first = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/relay/api/config")
                .header("x-test-account-id", "account-a")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&sample_document()).expect("body"),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(first.status(), axum::http::StatusCode::OK);
    let first_etag = first
        .headers()
        .get("etag")
        .expect("etag")
        .to_str()
        .expect("etag text")
        .to_owned();

    let same_account = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/relay/api/config")
                .header("x-test-account-id", "account-a")
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(same_account.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(same_account.into_body(), usize::MAX)
        .await
        .expect("body");
    let text = String::from_utf8(body.to_vec()).expect("UTF-8");
    assert!(text.contains("FREEHAND_PRIMARY_API_KEY"));
    assert!(!text.contains("sk-"));
    assert!(!text.contains("\"password\""));
    assert!(!text.contains("\"tokenValue\""));

    let other_account = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/relay/api/config")
                .header("x-test-account-id", "account-b")
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(other_account.status(), axum::http::StatusCode::NOT_FOUND);

    let mut updated_document = sample_document();
    updated_document.document.provider_registry[0].label = "Primary Updated".to_owned();
    let update = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/relay/api/config")
                .header("x-test-account-id", "account-a")
                .header("if-match", &first_etag)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&updated_document).expect("body"),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(update.status(), axum::http::StatusCode::OK);

    let stale = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/relay/api/config")
                .header("x-test-account-id", "account-a")
                .header("if-match", &first_etag)
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    serde_json::to_vec(&sample_document()).expect("body"),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(stale.status(), axum::http::StatusCode::CONFLICT);
    let stale_body = axum::body::to_bytes(stale.into_body(), usize::MAX)
        .await
        .expect("body");
    let stale_json: serde_json::Value = serde_json::from_slice(&stale_body).expect("conflict JSON");
    assert_eq!(
        stale_json["serverDocument"]["revision"],
        serde_json::json!(2)
    );

    let secret_payload = serde_json::json!({
        "schemaVersion": 1,
        "document": {
            "providerRegistry": [{
                "id": "primary",
                "label": "Primary",
                "providerType": "openai-compatible",
                "protocol": "chat",
                "baseUrl": "https://api.example.com/v1",
                "auth": {"authType": "inline", "authSource": "sk-live-secret"},
                "model": "gpt-5.6"
            }],
            "modelGroups": [],
            "relayEndpointCandidates": [],
            "remoteDaemonRegistry": []
        }
    });
    let secret = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/relay/api/config")
                .header("x-test-account-id", "account-secret")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(secret_payload.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(secret.status(), axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn account_config_put_to_missing_account_returns_not_found() {
    let temp = TempDir::new().expect("tempdir");
    let store = Arc::new(AccountConfigStore::new(temp.path().to_path_buf()).expect("store"));
    let app = config_router(store, authenticator());

    let body = serde_json::to_vec(&sample_document()).expect("body");
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("PUT")
                .uri("/relay/api/config")
                .header("x-test-account-id", "account-never-written")
                .header("if-match", "\"stale\"")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let text = String::from_utf8(body_bytes.to_vec()).expect("utf-8");
    assert!(text.contains("account_config_not_found"));
}
