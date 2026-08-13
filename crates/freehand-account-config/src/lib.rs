//! Account-scoped, versioned, non-secret configuration sharing.
//!
//! This crate owns the account config document schema, revision/etag
//! lifecycle, account-isolated atomic persistence, secret-boundary
//! validation, and the HTTP projection served beneath `/relay/api/config`.
//! It must never depend on `freehand-relay`, `freehand-config`, or any
//! runtime/UI/provider truth owner; the Relay host composes this crate with
//! the authenticated account boundary.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CONFIG_SCHEMA_VERSION: u32 = 1;
const ETAG_PREFIX: &str = "\"";
const DOCUMENT_FILE_SUFFIX: &str = ".json";

pub const ACCOUNT_CONFIG_SCHEMA_VERSION: u32 = CONFIG_SCHEMA_VERSION;

pub mod client;
pub mod mirror;

pub use client::{AccountConfigClient, AccountConfigClientError};
pub use mirror::{AccountConfigMirror, AccountConfigMirrorError, AccountConfigSyncStatus};

#[derive(Debug, thiserror::Error)]
pub enum AccountConfigError {
    #[error("invalid account config document: {0}")]
    Invalid(String),
    #[error("account config store I/O failed: {0}")]
    Io(String),
    #[error("account config document is corrupt: {0}")]
    Corrupt(String),
    #[error("account config revision conflict; server document is attached")]
    Conflict,
    #[error("account config document was not found")]
    NotFound,
}

/// Relay-host supplied boundary: resolve one authenticated account id from
/// request headers. The account-config crate never inspects Relay secrets.
pub trait AccountAuthenticator: Send + Sync {
    fn authenticate(&self, headers: &HeaderMap) -> Result<String, AccountConfigError>;
}

impl<F> AccountAuthenticator for F
where
    F: Fn(&HeaderMap) -> Result<String, AccountConfigError> + Send + Sync,
{
    fn authenticate(&self, headers: &HeaderMap) -> Result<String, AccountConfigError> {
        self(headers)
    }
}

/// Versioned account config document. `revision` and `etag` are server-owned
/// and recomputed on every successful PUT; client payloads may omit them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountConfigDocument {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub document: ConfigDocumentContent,
}

/// Schema-allowed shared fields only. Field names and values are the
/// non-secret projection boundary; secret values never enter this content.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigDocumentContent {
    #[serde(default)]
    pub provider_registry: Vec<SharedProviderDefinition>,
    #[serde(default)]
    pub model_groups: Vec<SharedModelGroup>,
    #[serde(default)]
    pub relay_endpoint_candidates: Vec<RelayEndpointCandidate>,
    #[serde(default)]
    pub remote_daemon_registry: Vec<RemoteDaemonEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedProviderDefinition {
    pub id: String,
    pub label: String,
    pub provider_type: String,
    pub protocol: String,
    pub base_url: String,
    pub auth: ProviderAuthReference,
    pub model: String,
}

/// Credential-free provider authentication reference. `env` carries only the
/// environment variable name; `inline` is rejected because values would be
/// secrets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderAuthReference {
    pub auth_type: String,
    pub auth_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SharedModelGroup {
    pub id: String,
    pub label: String,
    pub primary: ModelRoute,
    #[serde(default)]
    pub fallback: Option<ModelRoute>,
    #[serde(default)]
    pub load_balance: Vec<ModelRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelRoute {
    pub provider_id: String,
    pub model: String,
    #[serde(default)]
    pub weight: Option<u32>,
}

/// Explicit endpoint candidate: URL plus token environment-variable name only.
/// The token value is resolved on the consuming device and never uploaded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelayEndpointCandidate {
    pub id: String,
    pub url: String,
    pub token_env_name: String,
}

/// Remote daemon directory entry without one-time credentials or host paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoteDaemonEntry {
    pub daemon_id: String,
    pub display_name: String,
    pub relay_endpoint_id: String,
}

/// Validated document plus server-computed revision metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAccountConfig {
    pub revision: u64,
    pub etag: String,
    pub updated_at_unix: u64,
    pub content: ConfigDocumentContent,
}

/// Account-scoped store. Each account owns one file beneath the configured
/// directory; an account id is never used as a filesystem path segment.
pub struct AccountConfigStore {
    root: PathBuf,
    documents: Mutex<BTreeMap<String, StoredAccountConfig>>,
}

impl AccountConfigStore {
    pub fn from_env() -> Result<Self, AccountConfigError> {
        let value = std::env::var("FREEHAND_RELAY_ACCOUNT_CONFIG_DIR").map_err(|error| {
            AccountConfigError::Invalid(format!(
                "FREEHAND_RELAY_ACCOUNT_CONFIG_DIR is required: {error}"
            ))
        })?;
        if value.trim().is_empty() {
            return Err(AccountConfigError::Invalid(
                "FREEHAND_RELAY_ACCOUNT_CONFIG_DIR must not be empty".to_owned(),
            ));
        }
        Self::new(PathBuf::from(value))
    }

    pub fn new(root: PathBuf) -> Result<Self, AccountConfigError> {
        fs::create_dir_all(&root).map_err(|error| {
            AccountConfigError::Io(format!("create account config directory: {error}"))
        })?;
        let canonical = root.canonicalize().map_err(|error| {
            AccountConfigError::Io(format!("resolve account config directory: {error}"))
        })?;
        Ok(Self {
            root: canonical,
            documents: Mutex::new(BTreeMap::new()),
        })
    }

    /// Latest document for one account. Missing documents are an explicit
    /// not-found error; no synthetic default document is published.
    pub fn get(&self, account_id: &str) -> Result<StoredAccountConfig, AccountConfigError> {
        validate_account_id(account_id)?;
        let mut documents = self
            .documents
            .lock()
            .map_err(|error| AccountConfigError::Io(format!("store lock poisoned: {error}")))?;
        let path = self.account_path(account_id);
        if !path.is_file() {
            return Err(AccountConfigError::NotFound);
        }
        let raw = fs::read(&path)
            .map_err(|error| AccountConfigError::Io(format!("read account config: {error}")))?;
        let parsed: AccountConfigDocument = serde_json::from_slice(&raw)
            .map_err(|error| AccountConfigError::Corrupt(error.to_string()))?;
        if parsed.schema_version != CONFIG_SCHEMA_VERSION {
            return Err(AccountConfigError::Corrupt(format!(
                "unsupported schemaVersion {}; expected {CONFIG_SCHEMA_VERSION}",
                parsed.schema_version
            )));
        }
        validate_config_document(&parsed.document)?;
        let revision = parsed.revision.ok_or_else(|| {
            AccountConfigError::Corrupt("stored document is missing revision".to_owned())
        })?;
        let updated_at_unix = parse_unix_timestamp(parsed.updated_at.as_deref())?;
        let etag = compute_etag(&parsed.document)?;
        if parsed.etag.as_deref() != Some(etag.as_str()) {
            return Err(AccountConfigError::Corrupt(
                "stored document etag does not match canonical content".to_owned(),
            ));
        }
        let stored = StoredAccountConfig {
            revision,
            etag,
            updated_at_unix,
            content: parsed.document,
        };
        documents.insert(account_id.to_owned(), stored.clone());
        Ok(stored)
    }

    /// Atomically persist one validated document guarded by `If-Match`.
    /// `None` accepts a first-write; a stale matcher returns Conflict with the
    /// server current document. Publication happens only after durable write.
    pub fn put(
        &self,
        account_id: &str,
        content: ConfigDocumentContent,
        if_match: Option<&str>,
        now_unix: u64,
    ) -> Result<StoredAccountConfig, AccountConfigError> {
        validate_account_id(account_id)?;
        validate_config_document(&content)?;
        let mut documents = self
            .documents
            .lock()
            .map_err(|error| AccountConfigError::Io(format!("store lock poisoned: {error}")))?;
        let current = match documents.get(account_id) {
            Some(stored) => Some(stored.clone()),
            None => self.load_from_disk_unlocked(account_id)?.inspect(|stored| {
                documents.insert(account_id.to_owned(), stored.clone());
            }),
        };
        let expected_etag = if_match.map(str::trim).filter(|value| !value.is_empty());
        match (&current, expected_etag) {
            (Some(current), Some(expected)) => {
                if current.etag != expected {
                    return Err(AccountConfigError::Conflict);
                }
            }
            (None, Some(_)) => return Err(AccountConfigError::NotFound),
            (Some(_), None) => {
                return Err(AccountConfigError::Invalid(
                    "existing document requires If-Match".to_owned(),
                ));
            }
            (None, None) => {}
        }
        let next_revision = current
            .as_ref()
            .map(|stored| stored.revision + 1)
            .unwrap_or(1);
        let etag = compute_etag(&content)?;
        let candidate = StoredAccountConfig {
            revision: next_revision,
            etag: etag.clone(),
            updated_at_unix: now_unix,
            content,
        };
        self.persist_atomically(account_id, &candidate)?;
        documents.insert(account_id.to_owned(), candidate.clone());
        Ok(candidate)
    }

    fn load_from_disk_unlocked(
        &self,
        account_id: &str,
    ) -> Result<Option<StoredAccountConfig>, AccountConfigError> {
        let path = self.account_path(account_id);
        if !path.is_file() {
            return Ok(None);
        }
        let raw = fs::read(&path)
            .map_err(|error| AccountConfigError::Io(format!("read account config: {error}")))?;
        let parsed: AccountConfigDocument = serde_json::from_slice(&raw)
            .map_err(|error| AccountConfigError::Corrupt(error.to_string()))?;
        if parsed.schema_version != CONFIG_SCHEMA_VERSION {
            return Err(AccountConfigError::Corrupt(format!(
                "unsupported schemaVersion {}; expected {CONFIG_SCHEMA_VERSION}",
                parsed.schema_version
            )));
        }
        validate_config_document(&parsed.document)?;
        let revision = parsed.revision.ok_or_else(|| {
            AccountConfigError::Corrupt("stored document is missing revision".to_owned())
        })?;
        let updated_at_unix = parse_unix_timestamp(parsed.updated_at.as_deref())?;
        let etag = compute_etag(&parsed.document)?;
        if parsed.etag.as_deref() != Some(etag.as_str()) {
            return Err(AccountConfigError::Corrupt(
                "stored document etag does not match canonical content".to_owned(),
            ));
        }
        Ok(Some(StoredAccountConfig {
            revision,
            etag,
            updated_at_unix,
            content: parsed.document,
        }))
    }

    fn persist_atomically(
        &self,
        account_id: &str,
        candidate: &StoredAccountConfig,
    ) -> Result<(), AccountConfigError> {
        let document = AccountConfigDocument {
            schema_version: CONFIG_SCHEMA_VERSION,
            revision: Some(candidate.revision),
            etag: Some(candidate.etag.clone()),
            updated_at: Some(candidate.updated_at_unix.to_string()),
            document: candidate.content.clone(),
        };
        let raw = serde_json::to_vec_pretty(&document)
            .map_err(|error| AccountConfigError::Io(error.to_string()))?;
        let path = self.account_path(account_id);
        let temp_path = temporary_path(&path);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| {
                AccountConfigError::Io(format!("create account config temp file: {error}"))
            })?;
        if let Err(error) = file
            .write_all(&raw)
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::rename(&temp_path, &path))
            .and_then(|()| fs::File::open(&self.root).and_then(|directory| directory.sync_all()))
        {
            let _ = fs::remove_file(&temp_path);
            return Err(AccountConfigError::Io(format!(
                "persist account config: {error}"
            )));
        }
        Ok(())
    }

    fn account_path(&self, account_id: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(account_id.as_bytes());
        let digest = format!("{:x}", hasher.finalize());
        self.root.join(format!("{digest}{DOCUMENT_FILE_SUFFIX}"))
    }
}

/// Validate schema shape and the non-secret field boundary. This is the single
/// admission gate for account config content; every GET/PUT passes through it.
pub fn validate_config_document(content: &ConfigDocumentContent) -> Result<(), AccountConfigError> {
    let mut provider_ids = BTreeMap::new();
    for provider in &content.provider_registry {
        require_identifier(&provider.id, "provider id")?;
        require_identifier(&provider.label, "provider label")?;
        if provider_ids.insert(provider.id.clone(), ()).is_some() {
            return Err(AccountConfigError::Invalid(format!(
                "duplicate provider id `{}`",
                provider.id
            )));
        }
        match provider.auth.auth_type.as_str() {
            "env" => {
                if !is_environment_variable_name(&provider.auth.auth_source) {
                    return Err(AccountConfigError::Invalid(format!(
                        "provider `{}` env auth source must be an environment variable name",
                        provider.id
                    )));
                }
            }
            "inline" => {
                return Err(AccountConfigError::Invalid(format!(
                    "provider `{}` must not carry an inline auth value",
                    provider.id
                )));
            }
            other => {
                return Err(AccountConfigError::Invalid(format!(
                    "provider `{}` has unsupported auth_type `{other}`",
                    provider.id
                )));
            }
        }
        if contains_secret_value(&provider.base_url)
            || contains_secret_value(&provider.model)
            || contains_secret_value(&provider.provider_type)
            || contains_secret_value(&provider.protocol)
        {
            return Err(AccountConfigError::Invalid(format!(
                "provider `{}` contains a secret-shaped value",
                provider.id
            )));
        }
    }
    let mut group_ids = BTreeMap::new();
    for group in &content.model_groups {
        require_identifier(&group.id, "model group id")?;
        if group_ids.insert(group.id.clone(), ()).is_some() {
            return Err(AccountConfigError::Invalid(format!(
                "duplicate model group id `{}`",
                group.id
            )));
        }
        validate_route(&group.primary, "model group primary")?;
        if !provider_ids.contains_key(&group.primary.provider_id) {
            return Err(AccountConfigError::Invalid(format!(
                "model group `{}` primary references unknown provider `{}`",
                group.id, group.primary.provider_id
            )));
        }
        if let Some(fallback) = &group.fallback {
            validate_route(fallback, "model group fallback")?;
            if !provider_ids.contains_key(&fallback.provider_id) {
                return Err(AccountConfigError::Invalid(format!(
                    "model group `{}` fallback references unknown provider `{}`",
                    group.id, fallback.provider_id
                )));
            }
        }
        for route in &group.load_balance {
            validate_route(route, "model group load balance")?;
            if !provider_ids.contains_key(&route.provider_id) {
                return Err(AccountConfigError::Invalid(format!(
                    "model group `{}` load-balance route references unknown provider `{}`",
                    group.id, route.provider_id
                )));
            }
        }
    }
    let mut endpoint_ids = BTreeMap::new();
    for endpoint in &content.relay_endpoint_candidates {
        require_identifier(&endpoint.id, "relay endpoint id")?;
        if endpoint_ids.insert(endpoint.id.clone(), ()).is_some() {
            return Err(AccountConfigError::Invalid(format!(
                "duplicate relay endpoint candidate id `{}`",
                endpoint.id
            )));
        }
        if !endpoint.url.starts_with("http://") && !endpoint.url.starts_with("https://") {
            return Err(AccountConfigError::Invalid(format!(
                "relay endpoint `{}` url must be http(s)",
                endpoint.id
            )));
        }
        if !is_environment_variable_name(&endpoint.token_env_name) {
            return Err(AccountConfigError::Invalid(format!(
                "relay endpoint `{}` token_env_name must be an environment variable name",
                endpoint.id
            )));
        }
        if contains_secret_value(&endpoint.url) || contains_secret_value(&endpoint.token_env_name) {
            return Err(AccountConfigError::Invalid(format!(
                "relay endpoint `{}` contains a secret-shaped value",
                endpoint.id
            )));
        }
    }
    let mut daemon_ids = BTreeMap::new();
    for daemon in &content.remote_daemon_registry {
        require_identifier(&daemon.daemon_id, "remote daemon id")?;
        if daemon_ids.insert(daemon.daemon_id.clone(), ()).is_some() {
            return Err(AccountConfigError::Invalid(format!(
                "duplicate remote daemon id `{}`",
                daemon.daemon_id
            )));
        }
        if !endpoint_ids.contains_key(&daemon.relay_endpoint_id) {
            return Err(AccountConfigError::Invalid(format!(
                "remote daemon `{}` references unknown relay endpoint `{}`",
                daemon.daemon_id, daemon.relay_endpoint_id
            )));
        }
        if contains_secret_value(&daemon.display_name) {
            return Err(AccountConfigError::Invalid(format!(
                "remote daemon `{}` contains a secret-shaped value",
                daemon.daemon_id
            )));
        }
    }
    Ok(())
}

/// Explicit safe projection: returns exactly the same schema-allowed content
/// after validation, so no server-owned revision or etag leaks into
/// projection-only consumers and no hidden field can pass through.
pub fn project_safe_document(
    content: &ConfigDocumentContent,
) -> Result<ConfigDocumentContent, AccountConfigError> {
    validate_config_document(content)?;
    Ok(content.clone())
}

fn validate_route(route: &ModelRoute, context: &str) -> Result<(), AccountConfigError> {
    require_identifier(&route.provider_id, "model route provider id")?;
    if contains_secret_value(&route.model) {
        return Err(AccountConfigError::Invalid(format!(
            "{context} contains a secret-shaped model value"
        )));
    }
    Ok(())
}

fn require_identifier(value: &str, context: &str) -> Result<(), AccountConfigError> {
    if value.is_empty() {
        return Err(AccountConfigError::Invalid(format!(
            "{context} must not be empty"
        )));
    }
    if value.len() > 256 {
        return Err(AccountConfigError::Invalid(format!(
            "{context} is too long"
        )));
    }
    Ok(())
}

fn validate_account_id(account_id: &str) -> Result<(), AccountConfigError> {
    require_identifier(account_id, "account id")
}

fn is_environment_variable_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && !value.starts_with(|character: char| character.is_ascii_digit())
}

/// Reject values that look like credentials or host paths. Field names such as
/// `token_env_name` are never scanned; only values are.
fn contains_secret_value(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    [
        "sk-",
        "api_key",
        "api-key",
        "apikey",
        "password",
        "secret",
        "bearer ",
        "token=",
        "pair_token",
        "pair-token",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
        || value.starts_with('/')
        || value.starts_with("~/")
        || value.contains("\\")
        || value.len() > 512
}

fn compute_etag(content: &ConfigDocumentContent) -> Result<String, AccountConfigError> {
    let canonical = serde_json::to_vec(content)
        .map_err(|error| AccountConfigError::Io(format!("canonicalize document: {error}")))?;
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    Ok(format!("{ETAG_PREFIX}{:x}{ETAG_PREFIX}", hasher.finalize()))
}

fn parse_unix_timestamp(value: Option<&str>) -> Result<u64, AccountConfigError> {
    value
        .map(|value| {
            value.parse::<u64>().map_err(|error| {
                AccountConfigError::Corrupt(format!("invalid updatedAt timestamp: {error}"))
            })
        })
        .transpose()?
        .ok_or_else(|| AccountConfigError::Corrupt("missing updatedAt timestamp".to_owned()))
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "account-config.json".to_owned());
    name.push_str(&format!(".tmp{}", std::process::id()));
    path.with_file_name(name)
}

fn error_response(status: StatusCode, error: AccountConfigError) -> Response {
    (
        status,
        Json(serde_json::json!({
            "code": error_code(&error),
            "message": error.to_string(),
        })),
    )
        .into_response()
}

fn error_code(error: &AccountConfigError) -> &'static str {
    match error {
        AccountConfigError::Invalid(_) => "invalid_account_config",
        AccountConfigError::Io(_) => "account_config_io_error",
        AccountConfigError::Corrupt(_) => "account_config_corrupt",
        AccountConfigError::Conflict => "account_config_conflict",
        AccountConfigError::NotFound => "account_config_not_found",
    }
}

#[derive(Clone)]
struct ConfigState {
    store: Arc<AccountConfigStore>,
    authenticator: Arc<dyn AccountAuthenticator>,
}

async fn get_config(State(state): State<ConfigState>, headers: HeaderMap) -> Response {
    let account_id = match state.authenticator.authenticate(&headers) {
        Ok(account_id) => account_id,
        Err(error) => return error_response(StatusCode::UNAUTHORIZED, error),
    };
    match state.store.get(&account_id) {
        Ok(stored) => {
            let body = AccountConfigDocument {
                schema_version: CONFIG_SCHEMA_VERSION,
                revision: Some(stored.revision),
                etag: Some(stored.etag.clone()),
                updated_at: Some(stored.updated_at_unix.to_string()),
                document: stored.content,
            };
            response_with_etag(Json(body).into_response(), &stored.etag)
        }
        Err(AccountConfigError::NotFound) => {
            error_response(StatusCode::NOT_FOUND, AccountConfigError::NotFound)
        }
        Err(AccountConfigError::Invalid(message)) => error_response(
            StatusCode::BAD_REQUEST,
            AccountConfigError::Invalid(message),
        ),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

async fn put_config(
    State(state): State<ConfigState>,
    headers: HeaderMap,
    Json(request): Json<AccountConfigDocument>,
) -> Response {
    let account_id = match state.authenticator.authenticate(&headers) {
        Ok(account_id) => account_id,
        Err(error) => return error_response(StatusCode::UNAUTHORIZED, error),
    };
    if request.schema_version != CONFIG_SCHEMA_VERSION {
        return error_response(
            StatusCode::BAD_REQUEST,
            AccountConfigError::Invalid(format!(
                "unsupported schemaVersion {}; expected {CONFIG_SCHEMA_VERSION}",
                request.schema_version
            )),
        );
    }
    if request.revision.is_some() || request.etag.is_some() || request.updated_at.is_some() {
        return error_response(
            StatusCode::BAD_REQUEST,
            AccountConfigError::Invalid(
                "revision, etag, and updatedAt are server-owned fields".to_owned(),
            ),
        );
    }
    let if_match = headers
        .get(header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let now = match now_unix() {
        Ok(now) => now,
        Err(error) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
    };
    match state
        .store
        .put(&account_id, request.document, if_match.as_deref(), now)
    {
        Ok(stored) => {
            let body = AccountConfigDocument {
                schema_version: CONFIG_SCHEMA_VERSION,
                revision: Some(stored.revision),
                etag: Some(stored.etag.clone()),
                updated_at: Some(stored.updated_at_unix.to_string()),
                document: stored.content,
            };
            response_with_etag(Json(body).into_response(), &stored.etag)
        }
        Err(AccountConfigError::Conflict) => {
            let stored = match state.store.get(&account_id) {
                Ok(stored) => stored,
                Err(error) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
            };
            let server_current = AccountConfigDocument {
                schema_version: CONFIG_SCHEMA_VERSION,
                revision: Some(stored.revision),
                etag: Some(stored.etag.clone()),
                updated_at: Some(stored.updated_at_unix.to_string()),
                document: stored.content,
            };
            (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "code": "account_config_conflict",
                    "message": "stale If-Match; server document is attached",
                    "serverDocument": server_current,
                })),
            )
                .into_response()
        }
        Err(AccountConfigError::NotFound) => {
            error_response(StatusCode::NOT_FOUND, AccountConfigError::NotFound)
        }
        Err(AccountConfigError::Invalid(message)) => error_response(
            StatusCode::BAD_REQUEST,
            AccountConfigError::Invalid(message),
        ),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

fn response_with_etag(mut response: Response, etag: &str) -> Response {
    match header::HeaderValue::from_str(etag) {
        Ok(value) => {
            response.headers_mut().insert(header::ETAG, value);
            response
        }
        Err(error) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            AccountConfigError::Io(format!("invalid computed etag header: {error}")),
        ),
    }
}

fn now_unix() -> Result<u64, AccountConfigError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| {
            AccountConfigError::Io(format!("system clock is before Unix epoch: {error}"))
        })
}

/// Build the `/relay/api/config` router. The Relay host supplies the account
/// authenticator; this crate never owns Relay account truth.
pub fn config_router(
    store: Arc<AccountConfigStore>,
    authenticator: Arc<dyn AccountAuthenticator>,
) -> Router {
    Router::new()
        .route("/relay/api/config", get(get_config).put(put_config))
        .with_state(ConfigState {
            store,
            authenticator,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_content() -> ConfigDocumentContent {
        ConfigDocumentContent {
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
        }
    }

    #[test]
    fn valid_document_projects_safely() {
        let content = sample_content();
        validate_config_document(&content).expect("valid");
        assert_eq!(project_safe_document(&content).expect("project"), content);
    }

    #[test]
    fn inline_auth_value_is_rejected() {
        let mut content = sample_content();
        content.provider_registry[0].auth.auth_type = "inline".to_owned();
        content.provider_registry[0].auth.auth_source = "sk-live-secret".to_owned();
        let error = validate_config_document(&content).expect_err("inline auth must fail");
        assert!(error.to_string().contains("inline auth value"));
    }

    #[test]
    fn secret_shaped_value_is_rejected() {
        let mut content = sample_content();
        content.provider_registry[0].auth.auth_source =
            "FREEHAND_PRIMARY_API_KEY=sk-live-secret".to_owned();
        let error = validate_config_document(&content).expect_err("secret value must fail");
        assert!(error.to_string().contains("environment variable name"));
    }

    #[test]
    fn unknown_group_provider_is_rejected() {
        let mut content = sample_content();
        content.model_groups.push(SharedModelGroup {
            id: "group".to_owned(),
            label: "Group".to_owned(),
            primary: ModelRoute {
                provider_id: "missing".to_owned(),
                model: "model".to_owned(),
                weight: None,
            },
            fallback: None,
            load_balance: vec![],
        });
        let error = validate_config_document(&content).expect_err("unknown provider must fail");
        assert!(error.to_string().contains("unknown provider"));
    }

    #[test]
    fn revision_etag_and_account_isolation_round_trip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = AccountConfigStore::new(temp.path().to_path_buf()).expect("store");
        let first = store
            .put("account-a", sample_content(), None, 1000)
            .expect("first put");
        assert_eq!(first.revision, 1);
        let mut updated_content = sample_content();
        updated_content.provider_registry[0].label = "Primary Updated".to_owned();
        let second = store
            .put("account-a", updated_content, Some(&first.etag), 1001)
            .expect("second put");
        assert_eq!(second.revision, 2);
        assert_ne!(second.etag, first.etag);
        assert_eq!(store.get("account-a").expect("account-a get"), second);
        assert!(matches!(
            store
                .get("account-b")
                .expect_err("cross account must not exist"),
            AccountConfigError::NotFound
        ));
        let stale = store
            .put("account-a", sample_content(), Some(&first.etag), 1002)
            .expect_err("stale If-Match must conflict");
        assert!(matches!(stale, AccountConfigError::Conflict));
        let missing_guard = store
            .put("account-a", sample_content(), None, 1003)
            .expect_err("existing document requires If-Match");
        assert!(missing_guard.to_string().contains("requires If-Match"));
    }

    #[test]
    fn store_restart_restores_revision_and_etag() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = AccountConfigStore::new(temp.path().to_path_buf()).expect("store");
        let stored = store
            .put("account-a", sample_content(), None, 1000)
            .expect("put");
        drop(store);
        let reloaded = AccountConfigStore::new(temp.path().to_path_buf()).expect("reload");
        let restored = reloaded.get("account-a").expect("restored");
        assert_eq!(restored.revision, stored.revision);
        assert_eq!(restored.etag, stored.etag);
        assert_eq!(restored.content, stored.content);
    }

    #[test]
    fn corrupt_document_is_rejected_on_load() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = AccountConfigStore::new(temp.path().to_path_buf()).expect("store");
        store
            .put("account-a", sample_content(), None, 1000)
            .expect("put");
        let path = store.account_path("account-a");
        let mut raw = fs::read(&path).expect("read");
        raw.extend_from_slice(b"{\"schemaVersion\":");
        fs::write(&path, raw).expect("corrupt write");
        drop(store);
        let reloaded = AccountConfigStore::new(temp.path().to_path_buf()).expect("reload");
        assert!(matches!(
            reloaded.get("account-a").expect_err("corrupt must fail"),
            AccountConfigError::Corrupt(_)
        ));
    }
}
