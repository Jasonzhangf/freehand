//! Blocking HTTP client for the account-scoped config server.
//!
//! This module owns the client-side transport boundary: it knows how to reach
//! `/relay/api/config`, carries one Relay account bearer token, and maps
//! stale-revision 409 responses into an explicit conflict carrying the server
//! document. It never owns local config truth or UI projection.

use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest::header::IF_MATCH;
use serde::Deserialize;

use crate::{
    ACCOUNT_CONFIG_SCHEMA_VERSION, AccountConfigDocument, AccountConfigError,
    ConfigDocumentContent, validate_config_document,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const ACCOUNT_CONFIG_PATH: &str = "/relay/api/config";
const ACCOUNT_ME_PATH: &str = "/relay/api/auth/me";

#[derive(Debug, thiserror::Error)]
pub enum AccountConfigClientError {
    #[error("relay account config transport failed: {0}")]
    Transport(String),
    #[error("relay account config rejected the request: {status} {message}")]
    Rejected { status: u16, message: String },
    #[error("relay account config document was not found")]
    NotFound,
    #[error("account config revision conflict; server document is attached")]
    Conflict(Box<AccountConfigDocument>),
    #[error("relay returned an invalid account config document: {0}")]
    Invalid(String),
}

/// Account-scoped config transport. `relay_base_url` is the Relay root
/// (for example `http://relay.example:19091` or a deployment prefix ending in
/// `/relay/`); the client mounts `/relay/api/config` exactly once.
#[derive(Debug, Clone)]
pub struct AccountConfigClient {
    base_url: String,
    access_token: String,
    http: Client,
}

impl AccountConfigClient {
    pub fn new(
        relay_base_url: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Result<Self, AccountConfigClientError> {
        let base_url = normalize_relay_base_url(relay_base_url.into())?;
        let access_token = access_token.into();
        if access_token.trim().is_empty() {
            return Err(AccountConfigClientError::Invalid(
                "Relay access token must not be empty".to_owned(),
            ));
        }
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| {
                AccountConfigClientError::Transport(format!(
                    "build account config HTTP client: {error}"
                ))
            })?;
        Ok(Self {
            base_url,
            access_token,
            http,
        })
    }

    /// Resolve the authenticated Relay account id through `/relay/api/auth/me`.
    pub fn account_id(&self) -> Result<String, AccountConfigClientError> {
        let response = self
            .http
            .get(self.endpoint(ACCOUNT_ME_PATH))
            .bearer_auth(&self.access_token)
            .send()
            .map_err(|error| {
                AccountConfigClientError::Transport(format!("GET auth/me: {error}"))
            })?;
        if !response.status().is_success() {
            return Err(response_error(response.status(), response));
        }
        let body: AccountMeBody = response.json().map_err(|error| {
            AccountConfigClientError::Invalid(format!("decode auth/me response: {error}"))
        })?;
        if body.account_id.trim().is_empty() {
            return Err(AccountConfigClientError::Invalid(
                "auth/me returned an empty account id".to_owned(),
            ));
        }
        Ok(body.account_id)
    }

    /// Pull the latest account document. The response is revalidated against
    /// the strict non-secret schema before being returned.
    pub fn pull(&self) -> Result<AccountConfigDocument, AccountConfigClientError> {
        let response = self
            .http
            .get(self.endpoint(ACCOUNT_CONFIG_PATH))
            .bearer_auth(&self.access_token)
            .send()
            .map_err(|error| {
                AccountConfigClientError::Transport(format!("GET account config: {error}"))
            })?;
        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Err(AccountConfigClientError::NotFound);
        }
        if !status.is_success() {
            return Err(response_error(status, response));
        }
        let document: AccountConfigDocument = response.json().map_err(|error| {
            AccountConfigClientError::Invalid(format!("decode account config: {error}"))
        })?;
        validate_pulled_document(&document)?;
        Ok(document)
    }

    /// Push one validated non-secret document. `if_match` is optional only for
    /// a first write; stale matches return an explicit conflict with the
    /// server current document and never overwrite it.
    pub fn push(
        &self,
        if_match: Option<&str>,
        content: ConfigDocumentContent,
    ) -> Result<AccountConfigDocument, AccountConfigClientError> {
        validate_config_document(&content).map_err(|error| {
            AccountConfigClientError::Invalid(format!("candidate document: {error}"))
        })?;
        let body = AccountConfigDocument {
            schema_version: ACCOUNT_CONFIG_SCHEMA_VERSION,
            revision: None,
            etag: None,
            updated_at: None,
            document: content,
        };
        let mut request = self
            .http
            .put(self.endpoint(ACCOUNT_CONFIG_PATH))
            .bearer_auth(&self.access_token)
            .json(&body);
        if let Some(if_match) = if_match.map(str::trim).filter(|value| !value.is_empty()) {
            request = request.header(IF_MATCH, if_match);
        }
        let response = request.send().map_err(|error| {
            AccountConfigClientError::Transport(format!("PUT account config: {error}"))
        })?;
        let status = response.status();
        if status == StatusCode::CONFLICT {
            let conflict: ConflictBody = response.json().map_err(|error| {
                AccountConfigClientError::Invalid(format!("decode conflict response: {error}"))
            })?;
            validate_pulled_document(&conflict.server_document)?;
            return Err(AccountConfigClientError::Conflict(Box::new(
                conflict.server_document,
            )));
        }
        if !status.is_success() {
            return Err(response_error(status, response));
        }
        let document: AccountConfigDocument = response.json().map_err(|error| {
            AccountConfigClientError::Invalid(format!("decode pushed account config: {error}"))
        })?;
        validate_pulled_document(&document)?;
        Ok(document)
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

fn normalize_relay_base_url(raw: String) -> Result<String, AccountConfigClientError> {
    let trimmed = raw.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(AccountConfigClientError::Invalid(format!(
            "relay base url must be http(s): {trimmed}"
        )));
    }
    let without_slash = trimmed.trim_end_matches('/');
    let without_relay = without_slash
        .strip_suffix("/relay")
        .unwrap_or(without_slash);
    if without_relay.is_empty() {
        return Err(AccountConfigClientError::Invalid(
            "relay base url must include a host".to_owned(),
        ));
    }
    Ok(without_relay.to_owned())
}

fn validate_pulled_document(
    document: &AccountConfigDocument,
) -> Result<(), AccountConfigClientError> {
    if document.schema_version != ACCOUNT_CONFIG_SCHEMA_VERSION {
        return Err(AccountConfigClientError::Invalid(format!(
            "unsupported schemaVersion {}; expected {ACCOUNT_CONFIG_SCHEMA_VERSION}",
            document.schema_version
        )));
    }
    if document.revision.is_none() || document.etag.is_none() || document.updated_at.is_none() {
        return Err(AccountConfigClientError::Invalid(
            "server document is missing revision/etag/updatedAt".to_owned(),
        ));
    }
    validate_config_document(&document.document).map_err(|error: AccountConfigError| {
        AccountConfigClientError::Invalid(format!("server document: {error}"))
    })
}

fn response_error(
    status: StatusCode,
    response: reqwest::blocking::Response,
) -> AccountConfigClientError {
    let message = response
        .json::<ErrorBody>()
        .map(|body| body.message)
        .unwrap_or_else(|_| format!("relay returned HTTP {}", status.as_u16()));
    AccountConfigClientError::Rejected {
        status: status.as_u16(),
        message,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountMeBody {
    account_id: String,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ConflictBody {
    code: String,
    message: String,
    server_document: AccountConfigDocument,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn sample_content() -> ConfigDocumentContent {
        ConfigDocumentContent {
            provider_registry: vec![crate::SharedProviderDefinition {
                id: "primary".to_owned(),
                label: "Primary".to_owned(),
                provider_type: "openai-compatible".to_owned(),
                protocol: "chat".to_owned(),
                base_url: "https://api.example.com/v1".to_owned(),
                auth: crate::ProviderAuthReference {
                    auth_type: "env".to_owned(),
                    auth_source: "FREEHAND_PRIMARY_API_KEY".to_owned(),
                },
                model: "gpt-5.6".to_owned(),
            }],
            model_groups: Vec::new(),
            relay_endpoint_candidates: Vec::new(),
            remote_daemon_registry: Vec::new(),
        }
    }

    fn document_json() -> String {
        let content = sample_content();
        let doc = AccountConfigDocument {
            schema_version: ACCOUNT_CONFIG_SCHEMA_VERSION,
            revision: Some(7),
            etag: Some("\"etag-7\"".to_owned()),
            updated_at: Some("1786500000".to_owned()),
            document: content,
        };
        serde_json::to_string(&doc).expect("document json")
    }

    fn stub_server(response_body: &'static str, status: u16) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub");
        let addr = listener.local_addr().expect("stub addr");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept stub");
            let mut buffer = [0u8; 8192];
            let _ = stream.read(&mut buffer).expect("read request");
            let body = response_body;
            let reason = if status == 200 { "OK" } else { "Conflict" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        format!("http://{addr}")
    }

    #[test]
    fn config_endpoint_normalizes_relay_prefixes() {
        assert_eq!(
            normalize_relay_base_url("http://relay.test:19091".to_owned()).expect("root"),
            "http://relay.test:19091"
        );
        assert_eq!(
            normalize_relay_base_url("https://relay.test/relay/".to_owned()).expect("relay prefix"),
            "https://relay.test"
        );
        assert!(normalize_relay_base_url("not-a-url".to_owned()).is_err());
    }

    #[test]
    fn client_pull_returns_validated_document() {
        let server = stub_server(Box::leak(document_json().into_boxed_str()), 200);
        let client = AccountConfigClient::new(server, "token").expect("client");
        let document = client.pull().expect("pull");
        assert_eq!(document.revision, Some(7));
        assert_eq!(document.document.provider_registry.len(), 1);
    }

    #[test]
    fn client_push_conflict_carries_server_document() {
        let conflict = serde_json::json!({
            "code": "account_config_conflict",
            "message": "stale If-Match",
            "serverDocument": serde_json::from_str::<serde_json::Value>(&document_json()).unwrap(),
        })
        .to_string();
        let server = stub_server(Box::leak(conflict.into_boxed_str()), 409);
        let client = AccountConfigClient::new(server, "token").expect("client");
        let err = client
            .push(Some("\"stale\""), sample_content())
            .expect_err("stale push");
        match err {
            AccountConfigClientError::Conflict(document) => {
                assert_eq!(document.revision, Some(7));
            }
            other => panic!("expected conflict, got {other:?}"),
        }
    }

    #[test]
    fn client_requires_non_empty_token() {
        let err = AccountConfigClient::new("http://relay.test", "  ").expect_err("blank token");
        assert!(matches!(err, AccountConfigClientError::Invalid(_)));
    }
}
