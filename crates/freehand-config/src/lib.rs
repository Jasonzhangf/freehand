//! Config loading and validation for Freehand.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CONFIG_FILE_RELATIVE_PATH: &str = ".freehand/config.toml";
pub const MIN_AGENT_RESOURCE_COUNT: usize = 1;
pub const MAX_AGENT_RESOURCE_COUNT: usize = 5;
pub const REMOTE_DAEMON_BOOTSTRAP_KIND: &str = "freehand.remote-daemon-bootstrap";
pub const REMOTE_DAEMON_BOOTSTRAP_SCHEMA_VERSION: u32 = 1;
const REMOTE_DAEMON_BOOTSTRAP_URL_PREFIX: &str = "freehand://daemon/import?payload=";
const REMOTE_DAEMON_BOOTSTRAP_WEB_URL_PREFIX: &str =
    "https://freehand.local/daemon/import?payload=";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    Master,
    Slave,
}

impl AgentMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Master => "master",
            Self::Slave => "slave",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    OpenAi,
    Anthropic,
}

impl ProviderType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocol {
    Responses,
    ChatCompletions,
    Messages,
}

impl ProviderProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat_completions",
            Self::Messages => "messages",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ProviderAuthType {
    #[serde(rename = "apikey")]
    ApiKey,
}

impl ProviderAuthType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ApiKey => "apikey",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfig {
    pub name: String,
    pub mode: AgentMode,
    pub node_id: String,
    pub paired_agent_names: Vec<String>,
    pub allowed_pair_ip: Option<IpAddr>,
    pub pair_token_env: String,
    pub provider_id: String,
    pub fallback_provider_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderAuthConfig {
    ApiKeyInline { api_key: String },
    ApiKeyEnv { env_var: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAuthSourceKind {
    Inline,
    Env,
}

impl ProviderAuthSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Env => "env",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    pub id: String,
    pub enabled: bool,
    pub provider_type: ProviderType,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub default_model: String,
    pub auth_type: ProviderAuthType,
    pub auth: ProviderAuthConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeProviderConfigProjection {
    pub id: String,
    pub enabled: bool,
    pub provider_type: ProviderType,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub base_url_host: String,
    pub default_model: String,
    pub auth_type: ProviderAuthType,
    pub auth_source: ProviderAuthSourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderConfig {
    pub id: String,
    pub provider_type: ProviderType,
    pub protocol: ProviderProtocol,
    pub base_url: String,
    pub default_model: String,
    pub auth_type: ProviderAuthType,
    pub auth_source: ProviderAuthSourceKind,
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfigUpdate {
    pub agent_name: String,
    pub provider_id: String,
    pub provider_type: String,
    pub protocol: String,
    pub base_url: String,
    pub default_model: String,
    pub api_key_env: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProviderSelectionConfigUpdate {
    pub agent_name: String,
    pub provider_id: String,
    pub fallback_provider_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResourceConfigUpdate {
    pub agent_name: String,
    pub resource_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteDaemonEndpointKind {
    Tailscale,
    Ipv4,
    Ipv6,
    Relay,
}

impl RemoteDaemonEndpointKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tailscale => "tailscale",
            Self::Ipv4 => "ipv4",
            Self::Ipv6 => "ipv6",
            Self::Relay => "relay",
        }
    }

    fn is_direct(self) -> bool {
        matches!(self, Self::Tailscale | Self::Ipv4 | Self::Ipv6)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDaemonAccountConfig {
    pub id: String,
    pub label: String,
    pub relay_url: Option<String>,
    pub auth_token_env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDaemonEndpointConfig {
    pub id: String,
    pub kind: RemoteDaemonEndpointKind,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub web_url: Option<String>,
    pub adp_url: Option<String>,
    pub relay_host_id: Option<String>,
    pub auth_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDaemonConfig {
    pub id: String,
    pub account_id: String,
    pub label: String,
    pub node_id: String,
    #[serde(rename = "activeEndpoint", alias = "activeEndpointId")]
    pub active_endpoint_id: String,
    pub endpoints: Vec<RemoteDaemonEndpointConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteDaemonRouteHealthStatus {
    Success,
    Failure,
    AuthFailure,
}

impl RemoteDaemonRouteHealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::AuthFailure => "auth_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDaemonRouteHealthRecord {
    pub endpoint_id: String,
    pub status: RemoteDaemonRouteHealthStatus,
    pub rtt_ms: Option<u32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDaemonRouteCandidate {
    pub endpoint_id: String,
    pub kind: RemoteDaemonEndpointKind,
    pub endpoint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDaemonRoutePlan {
    pub account: RemoteDaemonAccountConfig,
    pub daemon: RemoteDaemonConfig,
    pub candidates: Vec<RemoteDaemonRouteCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDaemonRouteSelectionDiagnostic {
    pub endpoint_id: String,
    pub kind: RemoteDaemonEndpointKind,
    pub endpoint: String,
    pub selectable: bool,
    pub score: i32,
    pub reasons: Vec<String>,
    pub health: Option<RemoteDaemonRouteHealthRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedRemoteDaemonRouteConfig {
    pub account: RemoteDaemonAccountConfig,
    pub daemon: RemoteDaemonConfig,
    pub selected_endpoint: RemoteDaemonEndpointConfig,
    pub diagnostics: Vec<RemoteDaemonRouteSelectionDiagnostic>,
    pub restart_required_on_change: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteDaemonRegistryConfig {
    accounts: BTreeMap<String, RemoteDaemonAccountConfig>,
    daemons: BTreeMap<String, RemoteDaemonConfig>,
}

impl RemoteDaemonRegistryConfig {
    pub fn accounts(&self) -> &BTreeMap<String, RemoteDaemonAccountConfig> {
        &self.accounts
    }

    pub fn daemons(&self) -> &BTreeMap<String, RemoteDaemonConfig> {
        &self.daemons
    }

    pub fn select_daemon(
        &self,
        daemon_id: &str,
    ) -> Result<SelectedRemoteDaemonConfig, ConfigError> {
        let (account, daemon) = self.account_and_daemon(daemon_id)?;
        let active_endpoint = daemon
            .endpoints
            .iter()
            .find(|endpoint| endpoint.id == daemon.active_endpoint_id)
            .ok_or_else(|| ConfigError::RemoteDaemonActiveEndpointNotFound {
                daemon_id: daemon.id.clone(),
                endpoint_id: daemon.active_endpoint_id.clone(),
            })?;
        Ok(SelectedRemoteDaemonConfig {
            account: account.clone(),
            daemon: daemon.clone(),
            active_endpoint: active_endpoint.clone(),
            restart_required_on_change: true,
        })
    }

    pub fn build_route_plan(&self, daemon_id: &str) -> Result<RemoteDaemonRoutePlan, ConfigError> {
        let (account, daemon) = self.account_and_daemon(daemon_id)?;
        let candidates = daemon
            .endpoints
            .iter()
            .map(|endpoint| RemoteDaemonRouteCandidate {
                endpoint_id: endpoint.id.clone(),
                kind: endpoint.kind,
                endpoint: remote_daemon_endpoint_display(endpoint),
            })
            .collect();
        Ok(RemoteDaemonRoutePlan {
            account: account.clone(),
            daemon: daemon.clone(),
            candidates,
        })
    }

    pub fn select_route(
        &self,
        daemon_id: &str,
        health_records: &[RemoteDaemonRouteHealthRecord],
    ) -> Result<SelectedRemoteDaemonRouteConfig, ConfigError> {
        let plan = self.build_route_plan(daemon_id)?;
        let mut endpoint_ids = BTreeSet::new();
        for endpoint in &plan.daemon.endpoints {
            endpoint_ids.insert(endpoint.id.as_str());
        }
        let mut health_by_endpoint: BTreeMap<&str, &RemoteDaemonRouteHealthRecord> =
            BTreeMap::new();
        for record in health_records {
            let endpoint_id = record.endpoint_id.trim();
            if endpoint_id.is_empty() || !endpoint_ids.contains(endpoint_id) {
                return Err(ConfigError::RemoteDaemonRouteHealthEndpointNotFound {
                    daemon_id: plan.daemon.id.clone(),
                    endpoint_id: record.endpoint_id.clone(),
                });
            }
            if health_by_endpoint.insert(endpoint_id, record).is_some() {
                return Err(ConfigError::DuplicateRemoteDaemonRouteHealth {
                    daemon_id: plan.daemon.id.clone(),
                    endpoint_id: endpoint_id.to_owned(),
                });
            }
        }

        let diagnostics = plan
            .daemon
            .endpoints
            .iter()
            .enumerate()
            .map(|(index, endpoint)| {
                let mut reasons = Vec::new();
                let health = health_by_endpoint.get(endpoint.id.as_str()).copied();
                let score = remote_daemon_endpoint_kind_cost(endpoint.kind)
                    + remote_daemon_endpoint_priority_cost(endpoint.kind)
                    + remote_daemon_health_score(health, &mut reasons);
                reasons.insert(
                    0,
                    format!(
                        "priority:{}",
                        remote_daemon_endpoint_priority(endpoint.kind)
                    ),
                );
                reasons.insert(
                    0,
                    format!(
                        "path-cost:{}",
                        remote_daemon_endpoint_kind_cost(endpoint.kind)
                    ),
                );
                reasons.push(format!("declared-order:{index}"));
                RemoteDaemonRouteSelectionDiagnostic {
                    endpoint_id: endpoint.id.clone(),
                    kind: endpoint.kind,
                    endpoint: remote_daemon_endpoint_display(endpoint),
                    selectable: health
                        .map(|record| record.status == RemoteDaemonRouteHealthStatus::Success)
                        .unwrap_or(true),
                    score,
                    reasons,
                    health: health.cloned(),
                }
            })
            .collect::<Vec<_>>();

        let selected_diagnostic = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.selectable)
            .min_by(|left, right| {
                left.score
                    .cmp(&right.score)
                    .then_with(|| left.endpoint.cmp(&right.endpoint))
                    .then_with(|| left.endpoint_id.cmp(&right.endpoint_id))
            })
            .ok_or_else(|| ConfigError::RemoteDaemonNoSelectableEndpoint {
                daemon_id: plan.daemon.id.clone(),
            })?;
        let selected_endpoint = plan
            .daemon
            .endpoints
            .iter()
            .find(|endpoint| endpoint.id == selected_diagnostic.endpoint_id)
            .cloned()
            .ok_or_else(|| ConfigError::RemoteDaemonActiveEndpointNotFound {
                daemon_id: plan.daemon.id.clone(),
                endpoint_id: selected_diagnostic.endpoint_id.clone(),
            })?;

        Ok(SelectedRemoteDaemonRouteConfig {
            account: plan.account,
            daemon: plan.daemon,
            selected_endpoint,
            diagnostics,
            restart_required_on_change: true,
        })
    }

    pub fn build_bootstrap_bundle(
        &self,
        daemon_id: &str,
        credential: RemoteDaemonBootstrapCredential,
        exported_at_unix: u64,
        expires_at_unix: u64,
        nonce: impl Into<String>,
    ) -> Result<RemoteDaemonBootstrapBundle, ConfigError> {
        let selected = self.select_daemon(daemon_id)?;
        self.build_bootstrap_bundle_from_parts(
            selected.account,
            selected.daemon,
            credential,
            exported_at_unix,
            expires_at_unix,
            nonce,
        )
    }

    pub fn build_bootstrap_bundle_for_selected_route(
        &self,
        daemon_id: &str,
        health_records: &[RemoteDaemonRouteHealthRecord],
        credential: RemoteDaemonBootstrapCredential,
        exported_at_unix: u64,
        expires_at_unix: u64,
        nonce: impl Into<String>,
    ) -> Result<RemoteDaemonBootstrapBundle, ConfigError> {
        let selected = self.select_route(daemon_id, health_records)?;
        let mut daemon = selected.daemon;
        daemon.active_endpoint_id = selected.selected_endpoint.id;
        self.build_bootstrap_bundle_from_parts(
            selected.account,
            daemon,
            credential,
            exported_at_unix,
            expires_at_unix,
            nonce,
        )
    }

    fn account_and_daemon(
        &self,
        daemon_id: &str,
    ) -> Result<(&RemoteDaemonAccountConfig, &RemoteDaemonConfig), ConfigError> {
        let daemon =
            self.daemons
                .get(daemon_id)
                .ok_or_else(|| ConfigError::RemoteDaemonNotFound {
                    daemon_id: daemon_id.to_owned(),
                })?;
        let account = self.accounts.get(&daemon.account_id).ok_or_else(|| {
            ConfigError::RemoteDaemonAccountNotFound {
                daemon_id: daemon.id.clone(),
                account_id: daemon.account_id.clone(),
            }
        })?;
        Ok((account, daemon))
    }

    fn build_bootstrap_bundle_from_parts(
        &self,
        account: RemoteDaemonAccountConfig,
        daemon: RemoteDaemonConfig,
        credential: RemoteDaemonBootstrapCredential,
        exported_at_unix: u64,
        expires_at_unix: u64,
        nonce: impl Into<String>,
    ) -> Result<RemoteDaemonBootstrapBundle, ConfigError> {
        if expires_at_unix <= exported_at_unix {
            return Err(ConfigError::InvalidRemoteDaemonBootstrap {
                reason: "expiresAtUnix must be after exportedAtUnix".to_owned(),
            });
        }
        let nonce = nonce.into();
        if nonce.trim().is_empty() {
            return Err(ConfigError::InvalidRemoteDaemonBootstrap {
                reason: "nonce is required".to_owned(),
            });
        }
        if credential.value.trim().is_empty() {
            return Err(ConfigError::InvalidRemoteDaemonBootstrap {
                reason: "credential value is required".to_owned(),
            });
        }
        Ok(RemoteDaemonBootstrapBundle {
            kind: REMOTE_DAEMON_BOOTSTRAP_KIND.to_owned(),
            schema_version: REMOTE_DAEMON_BOOTSTRAP_SCHEMA_VERSION,
            exported_at_unix,
            expires_at_unix,
            nonce,
            account,
            daemon,
            credential,
        })
    }
}

fn remote_daemon_endpoint_display(endpoint: &RemoteDaemonEndpointConfig) -> String {
    match endpoint.kind {
        RemoteDaemonEndpointKind::Tailscale
        | RemoteDaemonEndpointKind::Ipv4
        | RemoteDaemonEndpointKind::Ipv6 => {
            let host = endpoint.host.as_deref().unwrap_or_default();
            let port = endpoint
                .port
                .map(|port| port.to_string())
                .unwrap_or_else(|| "?".to_owned());
            format!("{host}:{port}")
        }
        RemoteDaemonEndpointKind::Relay => endpoint
            .web_url
            .clone()
            .or_else(|| endpoint.relay_host_id.clone())
            .unwrap_or_else(|| "relay".to_owned()),
    }
}

fn remote_daemon_endpoint_kind_cost(kind: RemoteDaemonEndpointKind) -> i32 {
    match kind {
        RemoteDaemonEndpointKind::Tailscale => 10,
        RemoteDaemonEndpointKind::Ipv6 => 20,
        RemoteDaemonEndpointKind::Ipv4 => 30,
        RemoteDaemonEndpointKind::Relay => 80,
    }
}

fn remote_daemon_endpoint_priority(kind: RemoteDaemonEndpointKind) -> usize {
    match kind {
        RemoteDaemonEndpointKind::Tailscale => 0,
        RemoteDaemonEndpointKind::Ipv6 => 1,
        RemoteDaemonEndpointKind::Ipv4 => 2,
        RemoteDaemonEndpointKind::Relay => 3,
    }
}

fn remote_daemon_endpoint_priority_cost(kind: RemoteDaemonEndpointKind) -> i32 {
    (remote_daemon_endpoint_priority(kind) as i32) * 5
}

fn remote_daemon_health_score(
    health: Option<&RemoteDaemonRouteHealthRecord>,
    reasons: &mut Vec<String>,
) -> i32 {
    let Some(health) = health else {
        reasons.push("health:unknown".to_owned());
        return 20;
    };
    match health.status {
        RemoteDaemonRouteHealthStatus::AuthFailure => {
            reasons.push(format!(
                "health:auth-failure:{}",
                health.error.as_deref().unwrap_or("auth failed")
            ));
            900
        }
        RemoteDaemonRouteHealthStatus::Failure => {
            reasons.push(format!(
                "health:failure:{}",
                health.error.as_deref().unwrap_or("failed")
            ));
            500
        }
        RemoteDaemonRouteHealthStatus::Success => {
            reasons.push("health:recent-success".to_owned());
            if let Some(rtt_ms) = health.rtt_ms {
                reasons.push(format!("rtt:{rtt_ms}"));
                ((rtt_ms as i32) / 10).clamp(0, 100) - 25
            } else {
                -10
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedRemoteDaemonConfig {
    pub account: RemoteDaemonAccountConfig,
    pub daemon: RemoteDaemonConfig,
    pub active_endpoint: RemoteDaemonEndpointConfig,
    pub restart_required_on_change: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteDaemonBootstrapCredentialKind {
    OneTimeToken,
}

impl RemoteDaemonBootstrapCredentialKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OneTimeToken => "one_time_token",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDaemonBootstrapCredential {
    pub kind: RemoteDaemonBootstrapCredentialKind,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDaemonBootstrapBundle {
    pub kind: String,
    pub schema_version: u32,
    pub exported_at_unix: u64,
    pub expires_at_unix: u64,
    pub nonce: String,
    pub account: RemoteDaemonAccountConfig,
    pub daemon: RemoteDaemonConfig,
    pub credential: RemoteDaemonBootstrapCredential,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDaemonBootstrapSafeSummary {
    pub account_id: String,
    pub daemon_id: String,
    pub active_endpoint_id: String,
    pub endpoint_count: usize,
    pub credential_kind: String,
    pub expires_at_unix: u64,
}

impl RemoteDaemonBootstrapBundle {
    pub fn safe_summary(&self) -> RemoteDaemonBootstrapSafeSummary {
        RemoteDaemonBootstrapSafeSummary {
            account_id: self.account.id.clone(),
            daemon_id: self.daemon.id.clone(),
            active_endpoint_id: self.daemon.active_endpoint_id.clone(),
            endpoint_count: self.daemon.endpoints.len(),
            credential_kind: self.credential.kind.as_str().to_owned(),
            expires_at_unix: self.expires_at_unix,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    agents: BTreeMap<String, AgentConfig>,
    providers: BTreeMap<String, ProviderConfig>,
    remote_daemon_registry: RemoteDaemonRegistryConfig,
}

impl LoadedConfig {
    pub fn agents(&self) -> &BTreeMap<String, AgentConfig> {
        &self.agents
    }

    pub fn providers(&self) -> &BTreeMap<String, ProviderConfig> {
        &self.providers
    }

    pub fn safe_provider_registry(&self) -> Vec<SafeProviderConfigProjection> {
        self.providers
            .values()
            .map(ProviderConfig::safe_projection)
            .collect()
    }

    pub fn remote_daemon_registry(&self) -> &RemoteDaemonRegistryConfig {
        &self.remote_daemon_registry
    }

    pub fn select_agent(&self, agent_name: &str) -> Result<SelectedAgentConfig, ConfigError> {
        let agent = self
            .agents
            .get(agent_name)
            .ok_or_else(|| ConfigError::AgentNotFound {
                agent_name: agent_name.to_owned(),
            })?;
        let pair_token =
            env::var(&agent.pair_token_env).map_err(|_| ConfigError::MissingEnvVar {
                env_var: agent.pair_token_env.clone(),
                owner: ConfigEnvOwner::Agent {
                    agent_name: agent.name.clone(),
                },
            })?;
        if pair_token.trim().is_empty() {
            return Err(ConfigError::EmptyEnvVar {
                env_var: agent.pair_token_env.clone(),
                owner: ConfigEnvOwner::Agent {
                    agent_name: agent.name.clone(),
                },
            });
        }

        let provider = select_provider_for_agent(
            &self.providers,
            &agent.name,
            &agent.provider_id,
            ProviderRouteRole::Primary,
        )?;
        let fallback_provider = match agent.fallback_provider_id.as_deref() {
            Some(fallback_provider_id) => {
                if fallback_provider_id == agent.provider_id {
                    return Err(ConfigError::FallbackProviderMatchesPrimary {
                        agent_name: agent.name.clone(),
                        provider_id: agent.provider_id.clone(),
                    });
                }
                Some(select_provider_for_agent(
                    &self.providers,
                    &agent.name,
                    fallback_provider_id,
                    ProviderRouteRole::Fallback,
                )?)
            }
            None => None,
        };
        let mut paired_agents = Vec::new();
        for paired_agent_name in &agent.paired_agent_names {
            let paired = self.agents.get(paired_agent_name).ok_or_else(|| {
                ConfigError::PairedAgentNotFound {
                    agent_name: agent.name.clone(),
                    paired_agent_name: paired_agent_name.clone(),
                }
            })?;
            paired_agents.push(SelectedPeerAgentConfig {
                name: paired.name.clone(),
                mode: paired.mode,
                node_id: paired.node_id.clone(),
                allowed_pair_ip: paired.allowed_pair_ip,
                pair_token_env: paired.pair_token_env.clone(),
                provider_id: paired.provider_id.clone(),
                fallback_provider_id: paired.fallback_provider_id.clone(),
            });
        }

        Ok(SelectedAgentConfig {
            name: agent.name.clone(),
            mode: agent.mode,
            node_id: agent.node_id.clone(),
            paired_agents,
            allowed_pair_ip: agent.allowed_pair_ip,
            pair_token_env: agent.pair_token_env.clone(),
            pair_token,
            provider,
            fallback_provider,
            restart_required_on_change: true,
        })
    }
}

impl ProviderAuthConfig {
    pub fn source_kind(&self) -> ProviderAuthSourceKind {
        match self {
            Self::ApiKeyInline { .. } => ProviderAuthSourceKind::Inline,
            Self::ApiKeyEnv { .. } => ProviderAuthSourceKind::Env,
        }
    }
}

impl ProviderConfig {
    pub fn safe_projection(&self) -> SafeProviderConfigProjection {
        SafeProviderConfigProjection {
            id: self.id.clone(),
            enabled: self.enabled,
            provider_type: self.provider_type,
            protocol: self.protocol,
            base_url: safe_provider_base_url_for_projection(&self.base_url),
            base_url_host: provider_base_url_host_for_projection(&self.base_url),
            default_model: self.default_model.clone(),
            auth_type: self.auth_type,
            auth_source: self.auth.source_kind(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedAgentConfig {
    pub name: String,
    pub mode: AgentMode,
    pub node_id: String,
    pub paired_agents: Vec<SelectedPeerAgentConfig>,
    pub allowed_pair_ip: Option<IpAddr>,
    pub pair_token_env: String,
    pub pair_token: String,
    pub provider: SelectedProviderConfig,
    pub fallback_provider: Option<SelectedProviderConfig>,
    pub restart_required_on_change: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedPeerAgentConfig {
    pub name: String,
    pub mode: AgentMode,
    pub node_id: String,
    pub allowed_pair_ip: Option<IpAddr>,
    pub pair_token_env: String,
    pub provider_id: String,
    pub fallback_provider_id: Option<String>,
}

impl SelectedAgentConfig {
    pub fn master_peer(&self) -> Option<&SelectedPeerAgentConfig> {
        self.paired_agents
            .iter()
            .find(|peer| peer.mode == AgentMode::Master)
    }

    pub fn worker_peers(&self) -> impl Iterator<Item = &SelectedPeerAgentConfig> {
        self.paired_agents
            .iter()
            .filter(|peer| peer.mode == AgentMode::Slave)
    }

    pub fn worker_peer_names(&self) -> Vec<String> {
        self.worker_peers().map(|peer| peer.name.clone()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderRouteRole {
    Primary,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigEnvOwner {
    Agent { agent_name: String },
    Provider { provider_id: String },
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing HOME environment variable for default config path")]
    MissingHomeEnv,
    #[error("failed to read config file `{path}`: {source}")]
    ReadConfig {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config file `{path}`: {source}")]
    ParseConfig {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("config must define at least one `[agents.<name>]` entry")]
    NoAgentsDefined,
    #[error("config must define at least one `[providers.<id>]` entry")]
    NoProvidersDefined,
    #[error("agent table `{table_name}` has mismatched name field `{field_name}`")]
    AgentNameMismatch {
        table_name: String,
        field_name: String,
    },
    #[error("provider table `{table_name}` has mismatched id field `{field_name}`")]
    ProviderIdMismatch {
        table_name: String,
        field_name: String,
    },
    #[error("agent `{agent_name}` pair_token must be a non-empty environment variable name")]
    EmptyPairTokenEnv { agent_name: String },
    #[error("agent `{agent_name}` node_id must be non-empty")]
    EmptyAgentNodeId { agent_name: String },
    #[error("agent `{agent_name}` paired_agents contains an empty agent name")]
    EmptyPairedAgentName { agent_name: String },
    #[error("agent `{agent_name}` paired_agents contains duplicate `{paired_agent_name}`")]
    DuplicatePairedAgentBinding {
        agent_name: String,
        paired_agent_name: String,
    },
    #[error("agent `{agent_name}` provider must be a non-empty provider id")]
    EmptyProviderBinding { agent_name: String },
    #[error("agent `{agent_name}` cannot pair with itself")]
    SelfPairedAgent { agent_name: String },
    #[error("agent `{agent_name}` references missing paired agent `{paired_agent_name}`")]
    PairedAgentNotFound {
        agent_name: String,
        paired_agent_name: String,
    },
    #[error(
        "agent `{agent_name}` mode `{agent_mode}` is invalid for paired agent `{paired_agent_name}` mode `{paired_agent_mode}`"
    )]
    PairedAgentModeMismatch {
        agent_name: String,
        agent_mode: String,
        paired_agent_name: String,
        paired_agent_mode: String,
    },
    #[error("master agent `{agent_name}` must pair with at least one slave worker")]
    MasterRequiresWorkerPeer { agent_name: String },
    #[error("slave agent `{agent_name}` must pair with exactly one master, found {peer_count}")]
    SlaveRequiresSingleMasterPeer {
        agent_name: String,
        peer_count: usize,
    },
    #[error(
        "agent `{agent_name}` expects reciprocal pairing from `{paired_agent_name}`, but that agent points to `{actual_paired_agent_names}`"
    )]
    PairedAgentReciprocalMismatch {
        agent_name: String,
        paired_agent_name: String,
        actual_paired_agent_names: String,
    },
    #[error("provider `{provider_id}` base_url must be non-empty")]
    EmptyProviderBaseUrl { provider_id: String },
    #[error("provider `{provider_id}` id must use only letters, numbers, `_`, `-`, or `.`")]
    InvalidProviderId { provider_id: String },
    #[error("provider `{provider_id}` base_url must be an http(s) URL with a host")]
    InvalidProviderBaseUrl { provider_id: String },
    #[error("provider `{provider_id}` type `{provider_type}` is not supported")]
    UnsupportedProviderType {
        provider_id: String,
        provider_type: String,
    },
    #[error("provider `{provider_id}` protocol `{protocol}` is not supported")]
    UnsupportedProviderProtocol {
        provider_id: String,
        protocol: String,
    },
    #[error("provider `{provider_id}` default_model must be non-empty")]
    EmptyProviderDefaultModel { provider_id: String },
    #[error("provider `{provider_id}` must declare an explicit protocol")]
    MissingProviderProtocol { provider_id: String },
    #[error("provider `{provider_id}` auth must define exactly one of `api_key` or `api_key_env`")]
    InvalidProviderAuthSource { provider_id: String },
    #[error("provider `{provider_id}` api_key must be non-empty")]
    EmptyProviderApiKey { provider_id: String },
    #[error("provider `{provider_id}` api_key_env must be a non-empty environment variable name")]
    EmptyProviderApiKeyEnv { provider_id: String },
    #[error(
        "provider `{provider_id}` protocol `{protocol}` is invalid for provider type `{provider_type}`"
    )]
    InvalidProviderProtocol {
        provider_id: String,
        provider_type: String,
        protocol: String,
    },
    #[error("agent `{agent_name}` not found in config")]
    AgentNotFound { agent_name: String },
    #[error("agent resource count must be between {min} and {max}, received {resource_count}")]
    AgentResourceCountOutOfRange {
        resource_count: usize,
        min: usize,
        max: usize,
    },
    #[error("agent resource count can be updated only for a master agent, got `{agent_name}`")]
    AgentResourceUpdateRequiresMaster { agent_name: String },
    #[error("remote daemon account table `{table_name}` has mismatched id field `{field_id}`")]
    RemoteDaemonAccountIdMismatch {
        table_name: String,
        field_id: String,
    },
    #[error("remote daemon account `{account_id}` id must be non-empty")]
    EmptyRemoteDaemonAccountId { account_id: String },
    #[error("remote daemon account `{account_id}` relay_url must be an http(s) URL with a host")]
    InvalidRemoteDaemonRelayUrl { account_id: String },
    #[error("remote daemon account `{account_id}` auth_token_env must be non-empty when declared")]
    EmptyRemoteDaemonAccountAuthTokenEnv { account_id: String },
    #[error("remote daemon table `{table_name}` has mismatched id field `{field_id}`")]
    RemoteDaemonIdMismatch {
        table_name: String,
        field_id: String,
    },
    #[error("remote daemon `{daemon_id}` id must be non-empty")]
    EmptyRemoteDaemonId { daemon_id: String },
    #[error("remote daemon `{daemon_id}` references missing account `{account_id}`")]
    RemoteDaemonAccountNotFound {
        daemon_id: String,
        account_id: String,
    },
    #[error("remote daemon `{daemon_id}` not found in registry")]
    RemoteDaemonNotFound { daemon_id: String },
    #[error("remote daemon `{daemon_id}` node_id must be non-empty")]
    EmptyRemoteDaemonNodeId { daemon_id: String },
    #[error("remote daemon `{daemon_id}` active_endpoint must be non-empty")]
    EmptyRemoteDaemonActiveEndpoint { daemon_id: String },
    #[error("remote daemon `{daemon_id}` must declare at least one endpoint")]
    RemoteDaemonMissingEndpoints { daemon_id: String },
    #[error("remote daemon `{daemon_id}` endpoint id must be non-empty")]
    EmptyRemoteDaemonEndpointId { daemon_id: String },
    #[error("remote daemon `{daemon_id}` contains duplicate endpoint `{endpoint_id}`")]
    DuplicateRemoteDaemonEndpointId {
        daemon_id: String,
        endpoint_id: String,
    },
    #[error("remote daemon `{daemon_id}` active endpoint `{endpoint_id}` is not declared")]
    RemoteDaemonActiveEndpointNotFound {
        daemon_id: String,
        endpoint_id: String,
    },
    #[error("remote daemon `{daemon_id}` direct endpoint `{endpoint_id}` must declare host")]
    RemoteDaemonEndpointMissingHost {
        daemon_id: String,
        endpoint_id: String,
    },
    #[error(
        "remote daemon `{daemon_id}` direct endpoint `{endpoint_id}` must declare a valid port"
    )]
    RemoteDaemonEndpointInvalidPort {
        daemon_id: String,
        endpoint_id: String,
    },
    #[error("remote daemon `{daemon_id}` relay endpoint `{endpoint_id}` must declare web_url")]
    RemoteDaemonRelayEndpointMissingWebUrl {
        daemon_id: String,
        endpoint_id: String,
    },
    #[error(
        "remote daemon `{daemon_id}` relay endpoint `{endpoint_id}` requires account `{account_id}` relay_url"
    )]
    RemoteDaemonRelayEndpointMissingAccountRelay {
        daemon_id: String,
        endpoint_id: String,
        account_id: String,
    },
    #[error(
        "remote daemon `{daemon_id}` relay endpoint `{endpoint_id}` web_url must be an http(s) URL with a host"
    )]
    InvalidRemoteDaemonEndpointWebUrl {
        daemon_id: String,
        endpoint_id: String,
    },
    #[error("remote daemon `{daemon_id}` route health references unknown endpoint `{endpoint_id}`")]
    RemoteDaemonRouteHealthEndpointNotFound {
        daemon_id: String,
        endpoint_id: String,
    },
    #[error("remote daemon `{daemon_id}` route health contains duplicate endpoint `{endpoint_id}`")]
    DuplicateRemoteDaemonRouteHealth {
        daemon_id: String,
        endpoint_id: String,
    },
    #[error("remote daemon `{daemon_id}` has no selectable endpoint")]
    RemoteDaemonNoSelectableEndpoint { daemon_id: String },
    #[error("remote daemon bootstrap is invalid: {reason}")]
    InvalidRemoteDaemonBootstrap { reason: String },
    #[error("remote daemon bootstrap expired at {expires_at_unix}, now {now_unix}")]
    RemoteDaemonBootstrapExpired { expires_at_unix: u64, now_unix: u64 },
    #[error("agent `{agent_name}` references missing provider `{provider_id}`")]
    AgentProviderNotFound {
        agent_name: String,
        provider_id: String,
    },
    #[error("agent `{agent_name}` references missing fallback provider `{provider_id}`")]
    AgentFallbackProviderNotFound {
        agent_name: String,
        provider_id: String,
    },
    #[error("agent `{agent_name}` selected disabled provider `{provider_id}`")]
    ProviderDisabled {
        provider_id: String,
        agent_name: String,
    },
    #[error("agent `{agent_name}` selected disabled fallback provider `{provider_id}`")]
    FallbackProviderDisabled {
        provider_id: String,
        agent_name: String,
    },
    #[error(
        "agent `{agent_name}` fallback provider `{provider_id}` must differ from the primary provider"
    )]
    FallbackProviderMatchesPrimary {
        agent_name: String,
        provider_id: String,
    },
    #[error("{owner} environment variable `{env_var}` is not set")]
    MissingEnvVar {
        env_var: String,
        owner: ConfigEnvOwner,
    },
    #[error("{owner} environment variable `{env_var}` is empty")]
    EmptyEnvVar {
        env_var: String,
        owner: ConfigEnvOwner,
    },
    #[error("config file `{path}` must be a TOML table")]
    InvalidConfigRoot { path: PathBuf },
    #[error("config file `{path}` is missing table `{table}`")]
    MissingConfigTable { path: PathBuf, table: String },
    #[error("failed to serialize config file `{path}`: {source}")]
    SerializeConfig {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
    #[error("failed to write config file `{path}`: {source}")]
    WriteConfig {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to replace config file `{path}`: {source}")]
    ReplaceConfig {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl std::fmt::Display for ConfigEnvOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent { agent_name } => write!(f, "agent `{agent_name}`"),
            Self::Provider { provider_id } => write!(f, "provider `{provider_id}`"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    #[serde(default)]
    agents: BTreeMap<String, RawAgentConfig>,
    #[serde(default)]
    providers: BTreeMap<String, RawProviderConfig>,
    #[serde(default)]
    remote_daemon_accounts: BTreeMap<String, RawRemoteDaemonAccountConfig>,
    #[serde(default)]
    remote_daemons: BTreeMap<String, RawRemoteDaemonConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgentConfig {
    name: String,
    mode: AgentMode,
    node_id: String,
    paired_agents: Vec<String>,
    allowed_pair_ip: Option<IpAddr>,
    pair_token: String,
    provider: String,
    #[serde(default)]
    fallback_provider: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProviderConfig {
    id: String,
    enabled: bool,
    #[serde(rename = "type")]
    provider_type: ProviderType,
    #[serde(default)]
    protocol: Option<ProviderProtocol>,
    #[serde(alias = "baseURL")]
    base_url: String,
    #[serde(alias = "defaultModel")]
    default_model: String,
    auth: RawProviderAuthConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProviderAuthConfig {
    #[serde(rename = "type")]
    auth_type: ProviderAuthType,
    #[serde(default, alias = "apiKey")]
    api_key: Option<String>,
    #[serde(default, alias = "apiKeyEnv")]
    api_key_env: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRemoteDaemonAccountConfig {
    id: String,
    label: Option<String>,
    #[serde(default)]
    relay_url: Option<String>,
    #[serde(default)]
    auth_token_env: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRemoteDaemonConfig {
    id: String,
    account: String,
    #[serde(default)]
    label: Option<String>,
    node_id: String,
    active_endpoint: String,
    endpoints: Vec<RawRemoteDaemonEndpointConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRemoteDaemonEndpointConfig {
    id: String,
    kind: RemoteDaemonEndpointKind,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    web_url: Option<String>,
    #[serde(default)]
    adp_url: Option<String>,
    #[serde(default)]
    relay_host_id: Option<String>,
    #[serde(default)]
    auth_required: Option<bool>,
}

pub fn build_remote_daemon_bootstrap_link(
    bundle: &RemoteDaemonBootstrapBundle,
) -> Result<String, ConfigError> {
    validate_remote_daemon_bootstrap_bundle(bundle, None)?;
    let json = serde_json::to_string(bundle).map_err(|source| {
        ConfigError::InvalidRemoteDaemonBootstrap {
            reason: format!("serialize failed: {source}"),
        }
    })?;
    Ok(format!(
        "{REMOTE_DAEMON_BOOTSTRAP_URL_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    ))
}

pub fn build_remote_daemon_bootstrap_web_link(
    bundle: &RemoteDaemonBootstrapBundle,
) -> Result<String, ConfigError> {
    validate_remote_daemon_bootstrap_bundle(bundle, None)?;
    let json = serde_json::to_string(bundle).map_err(|source| {
        ConfigError::InvalidRemoteDaemonBootstrap {
            reason: format!("serialize failed: {source}"),
        }
    })?;
    Ok(format!(
        "{REMOTE_DAEMON_BOOTSTRAP_WEB_URL_PREFIX}{}",
        URL_SAFE_NO_PAD.encode(json.as_bytes())
    ))
}

pub fn parse_remote_daemon_bootstrap_link(
    input: &str,
    now_unix: u64,
) -> Result<RemoteDaemonBootstrapBundle, ConfigError> {
    let encoded = extract_remote_daemon_bootstrap_payload(input)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|source| ConfigError::InvalidRemoteDaemonBootstrap {
            reason: format!("payload is not base64url: {source}"),
        })?;
    let bundle: RemoteDaemonBootstrapBundle = serde_json::from_slice(&bytes).map_err(|source| {
        ConfigError::InvalidRemoteDaemonBootstrap {
            reason: format!("payload is not valid JSON: {source}"),
        }
    })?;
    validate_remote_daemon_bootstrap_bundle(&bundle, Some(now_unix))?;
    Ok(bundle)
}

fn extract_remote_daemon_bootstrap_payload(input: &str) -> Result<String, ConfigError> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(ConfigError::InvalidRemoteDaemonBootstrap {
            reason: "payload is empty".to_owned(),
        });
    }
    if !raw.contains("://") && !raw.contains('?') {
        return Ok(raw.to_owned());
    }
    for prefix in [
        REMOTE_DAEMON_BOOTSTRAP_URL_PREFIX,
        REMOTE_DAEMON_BOOTSTRAP_WEB_URL_PREFIX,
    ] {
        if let Some(payload) = raw.strip_prefix(prefix) {
            let value = payload
                .split('&')
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            if value.is_empty() {
                return Err(ConfigError::InvalidRemoteDaemonBootstrap {
                    reason: "payload query parameter is empty".to_owned(),
                });
            }
            return Ok(value);
        }
    }
    Err(ConfigError::InvalidRemoteDaemonBootstrap {
        reason: "unsupported remote daemon bootstrap URL".to_owned(),
    })
}

fn validate_remote_daemon_bootstrap_bundle(
    bundle: &RemoteDaemonBootstrapBundle,
    now_unix: Option<u64>,
) -> Result<(), ConfigError> {
    if bundle.kind != REMOTE_DAEMON_BOOTSTRAP_KIND {
        return Err(ConfigError::InvalidRemoteDaemonBootstrap {
            reason: "unsupported kind".to_owned(),
        });
    }
    if bundle.schema_version != REMOTE_DAEMON_BOOTSTRAP_SCHEMA_VERSION {
        return Err(ConfigError::InvalidRemoteDaemonBootstrap {
            reason: "unsupported schemaVersion".to_owned(),
        });
    }
    if bundle.expires_at_unix <= bundle.exported_at_unix {
        return Err(ConfigError::InvalidRemoteDaemonBootstrap {
            reason: "expiresAtUnix must be after exportedAtUnix".to_owned(),
        });
    }
    if let Some(now_unix) = now_unix {
        if bundle.expires_at_unix <= now_unix {
            return Err(ConfigError::RemoteDaemonBootstrapExpired {
                expires_at_unix: bundle.expires_at_unix,
                now_unix,
            });
        }
    }
    if bundle.nonce.trim().is_empty() {
        return Err(ConfigError::InvalidRemoteDaemonBootstrap {
            reason: "nonce is required".to_owned(),
        });
    }
    if bundle.credential.value.trim().is_empty() {
        return Err(ConfigError::InvalidRemoteDaemonBootstrap {
            reason: "credential value is required".to_owned(),
        });
    }
    validate_remote_daemon_registry(
        BTreeMap::from([(
            bundle.account.id.clone(),
            RawRemoteDaemonAccountConfig {
                id: bundle.account.id.clone(),
                label: Some(bundle.account.label.clone()),
                relay_url: bundle.account.relay_url.clone(),
                auth_token_env: bundle.account.auth_token_env.clone(),
            },
        )]),
        BTreeMap::from([(
            bundle.daemon.id.clone(),
            RawRemoteDaemonConfig {
                id: bundle.daemon.id.clone(),
                account: bundle.daemon.account_id.clone(),
                label: Some(bundle.daemon.label.clone()),
                node_id: bundle.daemon.node_id.clone(),
                active_endpoint: bundle.daemon.active_endpoint_id.clone(),
                endpoints: bundle
                    .daemon
                    .endpoints
                    .iter()
                    .map(|endpoint| RawRemoteDaemonEndpointConfig {
                        id: endpoint.id.clone(),
                        kind: endpoint.kind,
                        host: endpoint.host.clone(),
                        port: endpoint.port,
                        web_url: endpoint.web_url.clone(),
                        adp_url: endpoint.adp_url.clone(),
                        relay_host_id: endpoint.relay_host_id.clone(),
                        auth_required: Some(endpoint.auth_required),
                    })
                    .collect(),
            },
        )]),
    )?;
    Ok(())
}

pub fn default_config_path() -> Result<PathBuf, ConfigError> {
    let home = env::var_os("HOME").ok_or(ConfigError::MissingHomeEnv)?;
    Ok(PathBuf::from(home).join(CONFIG_FILE_RELATIVE_PATH))
}

pub fn load_default_config() -> Result<LoadedConfig, ConfigError> {
    let path = default_config_path()?;
    load_config_from_path(&path)
}

pub fn load_config_from_path(path: impl AsRef<Path>) -> Result<LoadedConfig, ConfigError> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| ConfigError::ReadConfig {
        path: path.to_path_buf(),
        source,
    })?;
    parse_config(path, &raw)
}

pub fn update_provider_config_in_path(
    path: impl AsRef<Path>,
    update: ProviderConfigUpdate,
) -> Result<SelectedAgentConfig, ConfigError> {
    let (provider_id, provider_type, protocol) = validate_provider_config_update(&update)?;

    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| ConfigError::ReadConfig {
        path: path.to_path_buf(),
        source,
    })?;
    let mut document: toml::Value =
        toml::from_str(&raw).map_err(|source| ConfigError::ParseConfig {
            path: path.to_path_buf(),
            source,
        })?;
    apply_provider_config_update(
        path,
        &mut document,
        &update,
        &provider_id,
        provider_type,
        protocol,
    )?;
    let updated =
        toml::to_string_pretty(&document).map_err(|source| ConfigError::SerializeConfig {
            path: path.to_path_buf(),
            source,
        })?;
    let loaded = parse_config(path, &updated)?;
    let selected = loaded.select_agent(&update.agent_name)?;
    persist_config_atomically(path, &updated)?;
    Ok(selected)
}

pub fn upsert_provider_config_in_path(
    path: impl AsRef<Path>,
    update: ProviderConfigUpdate,
) -> Result<SelectedAgentConfig, ConfigError> {
    let (provider_id, provider_type, protocol) = validate_provider_config_update(&update)?;

    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| ConfigError::ReadConfig {
        path: path.to_path_buf(),
        source,
    })?;
    let loaded = parse_config(path, &raw)?;
    if !loaded.agents().contains_key(&update.agent_name) {
        return Err(ConfigError::AgentNotFound {
            agent_name: update.agent_name,
        });
    }
    let mut document: toml::Value =
        toml::from_str(&raw).map_err(|source| ConfigError::ParseConfig {
            path: path.to_path_buf(),
            source,
        })?;
    apply_provider_definition_config_update(
        path,
        &mut document,
        &update,
        &provider_id,
        provider_type,
        protocol,
    )?;
    let updated =
        toml::to_string_pretty(&document).map_err(|source| ConfigError::SerializeConfig {
            path: path.to_path_buf(),
            source,
        })?;
    let updated_loaded = parse_config(path, &updated)?;
    let selected = updated_loaded.select_agent(&update.agent_name)?;
    persist_config_atomically(path, &updated)?;
    Ok(selected)
}

pub fn switch_agent_provider_in_path(
    path: impl AsRef<Path>,
    update: AgentProviderSelectionConfigUpdate,
) -> Result<SelectedAgentConfig, ConfigError> {
    let agent_name = update.agent_name.trim().to_owned();
    let provider_id = update.provider_id.trim().to_owned();
    validate_provider_id(&provider_id)?;
    let fallback_provider_id = update
        .fallback_provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    if let Some(fallback_provider_id) = &fallback_provider_id {
        validate_provider_id(fallback_provider_id)?;
    }

    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| ConfigError::ReadConfig {
        path: path.to_path_buf(),
        source,
    })?;
    let loaded = parse_config(path, &raw)?;
    if !loaded.agents().contains_key(&agent_name) {
        return Err(ConfigError::AgentNotFound { agent_name });
    }
    let mut document: toml::Value =
        toml::from_str(&raw).map_err(|source| ConfigError::ParseConfig {
            path: path.to_path_buf(),
            source,
        })?;
    apply_agent_provider_selection_update(
        path,
        &mut document,
        &agent_name,
        &provider_id,
        fallback_provider_id.as_deref(),
    )?;
    let updated =
        toml::to_string_pretty(&document).map_err(|source| ConfigError::SerializeConfig {
            path: path.to_path_buf(),
            source,
        })?;
    let updated_loaded = parse_config(path, &updated)?;
    let selected = updated_loaded.select_agent(&agent_name)?;
    persist_config_atomically(path, &updated)?;
    Ok(selected)
}

pub fn update_agent_resource_config_in_path(
    path: impl AsRef<Path>,
    update: AgentResourceConfigUpdate,
) -> Result<SelectedAgentConfig, ConfigError> {
    if !(MIN_AGENT_RESOURCE_COUNT..=MAX_AGENT_RESOURCE_COUNT).contains(&update.resource_count) {
        return Err(ConfigError::AgentResourceCountOutOfRange {
            resource_count: update.resource_count,
            min: MIN_AGENT_RESOURCE_COUNT,
            max: MAX_AGENT_RESOURCE_COUNT,
        });
    }

    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| ConfigError::ReadConfig {
        path: path.to_path_buf(),
        source,
    })?;
    let loaded = parse_config(path, &raw)?;
    let selected_agent =
        loaded
            .agents()
            .get(&update.agent_name)
            .ok_or_else(|| ConfigError::AgentNotFound {
                agent_name: update.agent_name.clone(),
            })?;
    if selected_agent.mode != AgentMode::Master {
        return Err(ConfigError::AgentResourceUpdateRequiresMaster {
            agent_name: update.agent_name,
        });
    }

    let mut document: toml::Value =
        toml::from_str(&raw).map_err(|source| ConfigError::ParseConfig {
            path: path.to_path_buf(),
            source,
        })?;
    apply_agent_resource_config_update(path, &mut document, &update, &loaded)?;
    let updated =
        toml::to_string_pretty(&document).map_err(|source| ConfigError::SerializeConfig {
            path: path.to_path_buf(),
            source,
        })?;
    let updated_loaded = parse_config(path, &updated)?;
    let selected = updated_loaded.select_agent(&update.agent_name)?;
    persist_config_atomically(path, &updated)?;
    Ok(selected)
}

fn parse_config(path: &Path, raw: &str) -> Result<LoadedConfig, ConfigError> {
    let parsed: RawConfig = toml::from_str(raw).map_err(|source| ConfigError::ParseConfig {
        path: path.to_path_buf(),
        source,
    })?;
    validate_config(parsed)
}

fn validate_config(parsed: RawConfig) -> Result<LoadedConfig, ConfigError> {
    let RawConfig {
        agents: raw_agents,
        providers: raw_providers,
        remote_daemon_accounts: raw_remote_daemon_accounts,
        remote_daemons: raw_remote_daemons,
    } = parsed;

    if raw_agents.is_empty() {
        return Err(ConfigError::NoAgentsDefined);
    }
    if raw_providers.is_empty() {
        return Err(ConfigError::NoProvidersDefined);
    }

    let mut providers = BTreeMap::new();
    for (table_name, raw_provider) in raw_providers {
        if raw_provider.id != table_name {
            return Err(ConfigError::ProviderIdMismatch {
                table_name,
                field_name: raw_provider.id,
            });
        }
        validate_provider_base_url(&raw_provider.id, &raw_provider.base_url)?;
        if raw_provider.default_model.trim().is_empty() {
            return Err(ConfigError::EmptyProviderDefaultModel {
                provider_id: raw_provider.id,
            });
        }
        let protocol = resolve_provider_protocol(
            &raw_provider.id,
            raw_provider.provider_type,
            raw_provider.protocol,
        )?;
        let auth_type = raw_provider.auth.auth_type;
        let auth = validate_provider_auth(&raw_provider.id, raw_provider.auth)?;

        let provider = ProviderConfig {
            id: raw_provider.id.clone(),
            enabled: raw_provider.enabled,
            provider_type: raw_provider.provider_type,
            protocol,
            base_url: raw_provider.base_url,
            default_model: raw_provider.default_model,
            auth_type,
            auth,
        };
        providers.insert(raw_provider.id, provider);
    }

    let mut agents = BTreeMap::new();
    for (table_name, raw_agent) in raw_agents {
        if raw_agent.name != table_name {
            return Err(ConfigError::AgentNameMismatch {
                table_name,
                field_name: raw_agent.name,
            });
        }
        if raw_agent.pair_token.trim().is_empty() {
            return Err(ConfigError::EmptyPairTokenEnv {
                agent_name: raw_agent.name,
            });
        }
        if raw_agent.node_id.trim().is_empty() {
            return Err(ConfigError::EmptyAgentNodeId {
                agent_name: raw_agent.name,
            });
        }
        if raw_agent.paired_agents.is_empty() {
            return Err(match raw_agent.mode {
                AgentMode::Master => ConfigError::MasterRequiresWorkerPeer {
                    agent_name: raw_agent.name,
                },
                AgentMode::Slave => ConfigError::SlaveRequiresSingleMasterPeer {
                    agent_name: raw_agent.name,
                    peer_count: 0,
                },
            });
        }
        let mut paired_agent_names = Vec::new();
        let mut seen_paired_agents = BTreeSet::new();
        for paired_agent_name in raw_agent.paired_agents {
            let trimmed = paired_agent_name.trim();
            if trimmed.is_empty() {
                return Err(ConfigError::EmptyPairedAgentName {
                    agent_name: raw_agent.name,
                });
            }
            if !seen_paired_agents.insert(trimmed.to_owned()) {
                return Err(ConfigError::DuplicatePairedAgentBinding {
                    agent_name: raw_agent.name,
                    paired_agent_name: trimmed.to_owned(),
                });
            }
            paired_agent_names.push(trimmed.to_owned());
        }
        if raw_agent.provider.trim().is_empty() {
            return Err(ConfigError::EmptyProviderBinding {
                agent_name: raw_agent.name,
            });
        }

        let agent = AgentConfig {
            name: raw_agent.name.clone(),
            mode: raw_agent.mode,
            node_id: raw_agent.node_id,
            paired_agent_names,
            allowed_pair_ip: raw_agent.allowed_pair_ip,
            pair_token_env: raw_agent.pair_token,
            provider_id: raw_agent.provider,
            fallback_provider_id: raw_agent.fallback_provider,
        };
        agents.insert(raw_agent.name, agent);
    }

    for agent in agents.values() {
        if agent.mode == AgentMode::Master && agent.paired_agent_names.is_empty() {
            return Err(ConfigError::MasterRequiresWorkerPeer {
                agent_name: agent.name.clone(),
            });
        }
        if agent.mode == AgentMode::Slave && agent.paired_agent_names.len() != 1 {
            return Err(ConfigError::SlaveRequiresSingleMasterPeer {
                agent_name: agent.name.clone(),
                peer_count: agent.paired_agent_names.len(),
            });
        }
        for paired_agent_name in &agent.paired_agent_names {
            if paired_agent_name == &agent.name {
                return Err(ConfigError::SelfPairedAgent {
                    agent_name: agent.name.clone(),
                });
            }
            let paired =
                agents
                    .get(paired_agent_name)
                    .ok_or_else(|| ConfigError::PairedAgentNotFound {
                        agent_name: agent.name.clone(),
                        paired_agent_name: paired_agent_name.clone(),
                    })?;
            if paired.mode == agent.mode {
                return Err(ConfigError::PairedAgentModeMismatch {
                    agent_name: agent.name.clone(),
                    agent_mode: agent.mode.as_str().to_owned(),
                    paired_agent_name: paired.name.clone(),
                    paired_agent_mode: paired.mode.as_str().to_owned(),
                });
            }
            if !paired
                .paired_agent_names
                .iter()
                .any(|candidate| candidate == &agent.name)
            {
                return Err(ConfigError::PairedAgentReciprocalMismatch {
                    agent_name: agent.name.clone(),
                    paired_agent_name: paired.name.clone(),
                    actual_paired_agent_names: paired.paired_agent_names.join(","),
                });
            }
        }
    }

    let remote_daemon_registry =
        validate_remote_daemon_registry(raw_remote_daemon_accounts, raw_remote_daemons)?;

    Ok(LoadedConfig {
        agents,
        providers,
        remote_daemon_registry,
    })
}

fn validate_remote_daemon_registry(
    raw_accounts: BTreeMap<String, RawRemoteDaemonAccountConfig>,
    raw_daemons: BTreeMap<String, RawRemoteDaemonConfig>,
) -> Result<RemoteDaemonRegistryConfig, ConfigError> {
    let mut accounts = BTreeMap::new();
    for (table_name, raw_account) in raw_accounts {
        let account_id = raw_account.id.trim().to_owned();
        if account_id.is_empty() {
            return Err(ConfigError::EmptyRemoteDaemonAccountId {
                account_id: table_name,
            });
        }
        if account_id != table_name {
            return Err(ConfigError::RemoteDaemonAccountIdMismatch {
                table_name,
                field_id: account_id,
            });
        }
        let relay_url = match raw_account.relay_url {
            Some(value) => {
                let trimmed = value.trim().to_owned();
                if trimmed.is_empty() || !is_http_url_with_host(&trimmed) {
                    return Err(ConfigError::InvalidRemoteDaemonRelayUrl {
                        account_id: account_id.clone(),
                    });
                }
                Some(trimmed)
            }
            None => None,
        };
        let auth_token_env = match raw_account.auth_token_env {
            Some(value) => {
                let trimmed = value.trim().to_owned();
                if trimmed.is_empty() {
                    return Err(ConfigError::EmptyRemoteDaemonAccountAuthTokenEnv {
                        account_id: account_id.clone(),
                    });
                }
                Some(trimmed)
            }
            None => None,
        };
        accounts.insert(
            account_id.clone(),
            RemoteDaemonAccountConfig {
                id: account_id.clone(),
                label: raw_account.label.unwrap_or_else(|| account_id.clone()),
                relay_url,
                auth_token_env,
            },
        );
    }

    let mut daemons = BTreeMap::new();
    for (table_name, raw_daemon) in raw_daemons {
        let daemon_id = raw_daemon.id.trim().to_owned();
        if daemon_id.is_empty() {
            return Err(ConfigError::EmptyRemoteDaemonId {
                daemon_id: table_name,
            });
        }
        if daemon_id != table_name {
            return Err(ConfigError::RemoteDaemonIdMismatch {
                table_name,
                field_id: daemon_id,
            });
        }
        let account_id = raw_daemon.account.trim().to_owned();
        let account =
            accounts
                .get(&account_id)
                .ok_or_else(|| ConfigError::RemoteDaemonAccountNotFound {
                    daemon_id: daemon_id.clone(),
                    account_id: account_id.clone(),
                })?;
        let node_id = raw_daemon.node_id.trim().to_owned();
        if node_id.is_empty() {
            return Err(ConfigError::EmptyRemoteDaemonNodeId {
                daemon_id: daemon_id.clone(),
            });
        }
        let active_endpoint_id = raw_daemon.active_endpoint.trim().to_owned();
        if active_endpoint_id.is_empty() {
            return Err(ConfigError::EmptyRemoteDaemonActiveEndpoint {
                daemon_id: daemon_id.clone(),
            });
        }
        if raw_daemon.endpoints.is_empty() {
            return Err(ConfigError::RemoteDaemonMissingEndpoints {
                daemon_id: daemon_id.clone(),
            });
        }
        let mut endpoints = Vec::new();
        let mut seen_endpoint_ids = BTreeSet::new();
        for raw_endpoint in raw_daemon.endpoints {
            let endpoint_id = raw_endpoint.id.trim().to_owned();
            if endpoint_id.is_empty() {
                return Err(ConfigError::EmptyRemoteDaemonEndpointId {
                    daemon_id: daemon_id.clone(),
                });
            }
            if !seen_endpoint_ids.insert(endpoint_id.clone()) {
                return Err(ConfigError::DuplicateRemoteDaemonEndpointId {
                    daemon_id: daemon_id.clone(),
                    endpoint_id,
                });
            }
            if raw_endpoint.kind.is_direct() {
                let host = raw_endpoint.host.unwrap_or_default().trim().to_owned();
                if host.is_empty() {
                    return Err(ConfigError::RemoteDaemonEndpointMissingHost {
                        daemon_id: daemon_id.clone(),
                        endpoint_id,
                    });
                }
                let Some(port) = raw_endpoint.port else {
                    return Err(ConfigError::RemoteDaemonEndpointInvalidPort {
                        daemon_id: daemon_id.clone(),
                        endpoint_id,
                    });
                };
                endpoints.push(RemoteDaemonEndpointConfig {
                    id: endpoint_id,
                    kind: raw_endpoint.kind,
                    host: Some(host),
                    port: Some(port),
                    web_url: raw_endpoint
                        .web_url
                        .map(|value| value.trim().to_owned())
                        .filter(|value| !value.is_empty()),
                    adp_url: raw_endpoint
                        .adp_url
                        .map(|value| value.trim().to_owned())
                        .filter(|value| !value.is_empty()),
                    relay_host_id: raw_endpoint
                        .relay_host_id
                        .map(|value| value.trim().to_owned())
                        .filter(|value| !value.is_empty()),
                    auth_required: raw_endpoint.auth_required.unwrap_or(true),
                });
                continue;
            }
            if account.relay_url.is_none() {
                return Err(ConfigError::RemoteDaemonRelayEndpointMissingAccountRelay {
                    daemon_id: daemon_id.clone(),
                    endpoint_id,
                    account_id: account_id.clone(),
                });
            }
            let web_url = raw_endpoint.web_url.unwrap_or_default().trim().to_owned();
            if web_url.is_empty() {
                return Err(ConfigError::RemoteDaemonRelayEndpointMissingWebUrl {
                    daemon_id: daemon_id.clone(),
                    endpoint_id,
                });
            }
            if !is_http_url_with_host(&web_url) {
                return Err(ConfigError::InvalidRemoteDaemonEndpointWebUrl {
                    daemon_id: daemon_id.clone(),
                    endpoint_id,
                });
            }
            endpoints.push(RemoteDaemonEndpointConfig {
                id: endpoint_id,
                kind: raw_endpoint.kind,
                host: raw_endpoint
                    .host
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty()),
                port: raw_endpoint.port,
                web_url: Some(web_url),
                adp_url: raw_endpoint
                    .adp_url
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty()),
                relay_host_id: raw_endpoint
                    .relay_host_id
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty()),
                auth_required: raw_endpoint.auth_required.unwrap_or(true),
            });
        }
        if !seen_endpoint_ids.contains(&active_endpoint_id) {
            return Err(ConfigError::RemoteDaemonActiveEndpointNotFound {
                daemon_id: daemon_id.clone(),
                endpoint_id: active_endpoint_id,
            });
        }
        daemons.insert(
            daemon_id.clone(),
            RemoteDaemonConfig {
                id: daemon_id.clone(),
                account_id,
                label: raw_daemon.label.unwrap_or_else(|| daemon_id.clone()),
                node_id,
                active_endpoint_id,
                endpoints,
            },
        );
    }

    Ok(RemoteDaemonRegistryConfig { accounts, daemons })
}

fn apply_provider_config_update(
    path: &Path,
    document: &mut toml::Value,
    update: &ProviderConfigUpdate,
    provider_id: &str,
    provider_type: ProviderType,
    protocol: ProviderProtocol,
) -> Result<(), ConfigError> {
    apply_provider_definition_config_update(
        path,
        document,
        update,
        provider_id,
        provider_type,
        protocol,
    )?;
    apply_agent_primary_provider_update(path, document, &update.agent_name, provider_id)
}

fn apply_provider_definition_config_update(
    path: &Path,
    document: &mut toml::Value,
    update: &ProviderConfigUpdate,
    provider_id: &str,
    provider_type: ProviderType,
    protocol: ProviderProtocol,
) -> Result<(), ConfigError> {
    let root = document
        .as_table_mut()
        .ok_or_else(|| ConfigError::InvalidConfigRoot {
            path: path.to_path_buf(),
        })?;
    let providers = root
        .get_mut("providers")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| ConfigError::MissingConfigTable {
            path: path.to_path_buf(),
            table: "providers".to_owned(),
        })?;
    let mut provider = toml::map::Map::new();
    provider.insert("id".to_owned(), toml::Value::String(provider_id.to_owned()));
    provider.insert("enabled".to_owned(), toml::Value::Boolean(true));
    provider.insert(
        "type".to_owned(),
        toml::Value::String(provider_type.as_str().to_owned()),
    );
    provider.insert(
        "protocol".to_owned(),
        toml::Value::String(protocol.as_str().to_owned()),
    );
    provider.insert(
        "base_url".to_owned(),
        toml::Value::String(update.base_url.trim().to_owned()),
    );
    provider.insert(
        "default_model".to_owned(),
        toml::Value::String(update.default_model.trim().to_owned()),
    );
    let mut auth = toml::map::Map::new();
    auth.insert("type".to_owned(), toml::Value::String("apikey".to_owned()));
    auth.insert(
        "api_key_env".to_owned(),
        toml::Value::String(update.api_key_env.trim().to_owned()),
    );
    provider.insert("auth".to_owned(), toml::Value::Table(auth));
    providers.insert(provider_id.to_owned(), toml::Value::Table(provider));
    Ok(())
}

fn apply_agent_primary_provider_update(
    path: &Path,
    document: &mut toml::Value,
    agent_name: &str,
    provider_id: &str,
) -> Result<(), ConfigError> {
    let root = document
        .as_table_mut()
        .ok_or_else(|| ConfigError::InvalidConfigRoot {
            path: path.to_path_buf(),
        })?;
    let agents = root
        .get_mut("agents")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| ConfigError::MissingConfigTable {
            path: path.to_path_buf(),
            table: "agents".to_owned(),
        })?;
    let agent = agents
        .get_mut(agent_name)
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| ConfigError::AgentNotFound {
            agent_name: agent_name.to_owned(),
        })?;
    agent.insert(
        "provider".to_owned(),
        toml::Value::String(provider_id.to_owned()),
    );
    Ok(())
}

fn apply_agent_provider_selection_update(
    path: &Path,
    document: &mut toml::Value,
    agent_name: &str,
    provider_id: &str,
    fallback_provider_id: Option<&str>,
) -> Result<(), ConfigError> {
    let root = document
        .as_table_mut()
        .ok_or_else(|| ConfigError::InvalidConfigRoot {
            path: path.to_path_buf(),
        })?;
    let agents = root
        .get_mut("agents")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| ConfigError::MissingConfigTable {
            path: path.to_path_buf(),
            table: "agents".to_owned(),
        })?;
    let agent = agents
        .get_mut(agent_name)
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| ConfigError::AgentNotFound {
            agent_name: agent_name.to_owned(),
        })?;
    agent.insert(
        "provider".to_owned(),
        toml::Value::String(provider_id.to_owned()),
    );
    if let Some(fallback_provider_id) = fallback_provider_id {
        agent.insert(
            "fallback_provider".to_owned(),
            toml::Value::String(fallback_provider_id.to_owned()),
        );
    } else {
        agent.remove("fallback_provider");
    }
    Ok(())
}

fn apply_agent_resource_config_update(
    path: &Path,
    document: &mut toml::Value,
    update: &AgentResourceConfigUpdate,
    loaded: &LoadedConfig,
) -> Result<(), ConfigError> {
    let master_config =
        loaded
            .agents()
            .get(&update.agent_name)
            .ok_or_else(|| ConfigError::AgentNotFound {
                agent_name: update.agent_name.clone(),
            })?;
    let current_peers = master_config.paired_agent_names.clone();
    let first_peer = current_peers
        .first()
        .expect("validated Master config always has one Worker")
        .clone();
    let worker_template_config = loaded
        .agents()
        .get(&first_peer)
        .expect("validated Master peer exists");

    let root = document
        .as_table_mut()
        .ok_or_else(|| ConfigError::InvalidConfigRoot {
            path: path.to_path_buf(),
        })?;
    let agents = root
        .get_mut("agents")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| ConfigError::MissingConfigTable {
            path: path.to_path_buf(),
            table: "agents".to_owned(),
        })?;
    let worker_template = agents
        .get(&first_peer)
        .and_then(toml::Value::as_table)
        .cloned()
        .ok_or_else(|| ConfigError::AgentNotFound {
            agent_name: first_peer.clone(),
        })?;

    let mut desired_peers = current_peers
        .iter()
        .take(update.resource_count)
        .cloned()
        .collect::<Vec<_>>();
    let mut next_suffix = desired_peers.len() + 1;
    while desired_peers.len() < update.resource_count {
        let candidate = format!("{first_peer}-{next_suffix}");
        next_suffix += 1;
        if candidate == update.agent_name
            || agents.contains_key(&candidate)
            || desired_peers.contains(&candidate)
        {
            continue;
        }
        desired_peers.push(candidate);
    }

    for removed_peer in current_peers.iter().skip(update.resource_count) {
        agents.remove(removed_peer);
    }

    for peer_name in &desired_peers {
        if !agents.contains_key(peer_name) {
            let mut worker = worker_template.clone();
            worker.insert("name".to_owned(), toml::Value::String(peer_name.clone()));
            worker.insert(
                "node_id".to_owned(),
                toml::Value::String(format!("{peer_name}-node")),
            );
            agents.insert(peer_name.clone(), toml::Value::Table(worker));
        }
        let worker = agents
            .get_mut(peer_name)
            .and_then(toml::Value::as_table_mut)
            .expect("validated or cloned Worker table");
        worker.insert(
            "paired_agents".to_owned(),
            toml::Value::Array(vec![toml::Value::String(update.agent_name.clone())]),
        );
        worker.insert(
            "provider".to_owned(),
            toml::Value::String(worker_template_config.provider_id.clone()),
        );
        match &worker_template_config.fallback_provider_id {
            Some(provider_id) => {
                worker.insert(
                    "fallback_provider".to_owned(),
                    toml::Value::String(provider_id.clone()),
                );
            }
            None => {
                worker.remove("fallback_provider");
            }
        }
    }

    let master = agents
        .get_mut(&update.agent_name)
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| ConfigError::AgentNotFound {
            agent_name: update.agent_name.clone(),
        })?;
    master.insert(
        "paired_agents".to_owned(),
        toml::Value::Array(desired_peers.into_iter().map(toml::Value::String).collect()),
    );
    Ok(())
}

fn persist_config_atomically(path: &Path, contents: &str) -> Result<(), ConfigError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("config.toml");
    let tmp_path = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let mut tmp = fs::File::create(&tmp_path).map_err(|source| ConfigError::WriteConfig {
        path: tmp_path.clone(),
        source,
    })?;
    tmp.write_all(contents.as_bytes())
        .and_then(|_| tmp.sync_all())
        .map_err(|source| ConfigError::WriteConfig {
            path: tmp_path.clone(),
            source,
        })?;
    fs::rename(&tmp_path, path).map_err(|source| ConfigError::ReplaceConfig {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn validate_provider_id(provider_id: &str) -> Result<(), ConfigError> {
    let valid = !provider_id.trim().is_empty()
        && provider_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'));
    if valid {
        Ok(())
    } else {
        Err(ConfigError::InvalidProviderId {
            provider_id: provider_id.to_owned(),
        })
    }
}

pub fn safe_provider_base_url_for_projection(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    let Some((scheme, rest)) = without_query.split_once("://") else {
        return String::new();
    };
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority)
        .trim();
    if scheme.trim().is_empty() || host.is_empty() {
        return String::new();
    }
    if path.is_empty() {
        format!("{}://{}", scheme.trim(), host)
    } else {
        format!(
            "{}://{}/{}",
            scheme.trim(),
            host,
            path.trim_end_matches('/')
        )
    }
}

pub fn provider_base_url_host_for_projection(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
    let without_userinfo = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let host = if without_userinfo.starts_with('[') {
        without_userinfo
            .split_once(']')
            .map(|(host, _)| format!("{host}]"))
            .unwrap_or_else(|| without_userinfo.to_owned())
    } else {
        without_userinfo
            .split(':')
            .next()
            .unwrap_or(without_userinfo)
            .to_owned()
    };
    host.trim().to_owned()
}

fn validate_provider_base_url(provider_id: &str, base_url: &str) -> Result<(), ConfigError> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::EmptyProviderBaseUrl {
            provider_id: provider_id.to_owned(),
        });
    }
    if !is_http_url_with_host(trimmed) {
        return Err(ConfigError::InvalidProviderBaseUrl {
            provider_id: provider_id.to_owned(),
        });
    }
    Ok(())
}

fn is_http_url_with_host(value: &str) -> bool {
    let trimmed = value.trim();
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return false;
    };
    if !matches!(scheme, "http" | "https")
        || rest.trim().is_empty()
        || rest.contains(char::is_whitespace)
    {
        return false;
    }
    let authority = rest.split('/').next().unwrap_or_default();
    let host = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    !host.trim().is_empty()
}

fn parse_provider_type(
    provider_id: &str,
    provider_type: &str,
) -> Result<ProviderType, ConfigError> {
    match provider_type.trim() {
        "openai" => Ok(ProviderType::OpenAi),
        "anthropic" => Ok(ProviderType::Anthropic),
        value => Err(ConfigError::UnsupportedProviderType {
            provider_id: provider_id.to_owned(),
            provider_type: value.to_owned(),
        }),
    }
}

fn parse_provider_protocol(
    provider_id: &str,
    protocol: &str,
) -> Result<ProviderProtocol, ConfigError> {
    match protocol.trim() {
        "responses" => Ok(ProviderProtocol::Responses),
        "chat_completions" => Ok(ProviderProtocol::ChatCompletions),
        "messages" => Ok(ProviderProtocol::Messages),
        value => Err(ConfigError::UnsupportedProviderProtocol {
            provider_id: provider_id.to_owned(),
            protocol: value.to_owned(),
        }),
    }
}

fn validate_provider_auth(
    provider_id: &str,
    auth: RawProviderAuthConfig,
) -> Result<ProviderAuthConfig, ConfigError> {
    match auth.auth_type {
        ProviderAuthType::ApiKey => match (auth.api_key, auth.api_key_env) {
            (Some(api_key), None) => {
                if api_key.trim().is_empty() {
                    return Err(ConfigError::EmptyProviderApiKey {
                        provider_id: provider_id.to_owned(),
                    });
                }
                Ok(ProviderAuthConfig::ApiKeyInline { api_key })
            }
            (None, Some(env_var)) => {
                if env_var.trim().is_empty() {
                    return Err(ConfigError::EmptyProviderApiKeyEnv {
                        provider_id: provider_id.to_owned(),
                    });
                }
                Ok(ProviderAuthConfig::ApiKeyEnv { env_var })
            }
            _ => Err(ConfigError::InvalidProviderAuthSource {
                provider_id: provider_id.to_owned(),
            }),
        },
    }
}

fn resolve_provider_protocol(
    provider_id: &str,
    provider_type: ProviderType,
    protocol: Option<ProviderProtocol>,
) -> Result<ProviderProtocol, ConfigError> {
    let resolved = match (provider_type, protocol) {
        (ProviderType::OpenAi, Some(ProviderProtocol::Responses)) => ProviderProtocol::Responses,
        (ProviderType::OpenAi, Some(ProviderProtocol::ChatCompletions)) => {
            ProviderProtocol::ChatCompletions
        }
        (ProviderType::Anthropic, Some(ProviderProtocol::Messages)) => ProviderProtocol::Messages,
        (_, None) => {
            return Err(ConfigError::MissingProviderProtocol {
                provider_id: provider_id.to_owned(),
            });
        }
        (_, Some(protocol)) => {
            return Err(ConfigError::InvalidProviderProtocol {
                provider_id: provider_id.to_owned(),
                provider_type: provider_type.as_str().to_owned(),
                protocol: protocol.as_str().to_owned(),
            });
        }
    };
    Ok(resolved)
}

fn validate_provider_config_update(
    update: &ProviderConfigUpdate,
) -> Result<(String, ProviderType, ProviderProtocol), ConfigError> {
    let provider_id = update.provider_id.trim().to_owned();
    validate_provider_id(&provider_id)?;
    validate_provider_base_url(&provider_id, &update.base_url)?;
    let provider_type = parse_provider_type(&provider_id, &update.provider_type)?;
    let protocol = parse_provider_protocol(&provider_id, &update.protocol)?;
    resolve_provider_protocol(&provider_id, provider_type, Some(protocol))?;
    if update.default_model.trim().is_empty() {
        return Err(ConfigError::EmptyProviderDefaultModel { provider_id });
    }
    if update.api_key_env.trim().is_empty() {
        return Err(ConfigError::EmptyProviderApiKeyEnv { provider_id });
    }
    Ok((provider_id, provider_type, protocol))
}

fn resolve_provider_api_key(provider: &ProviderConfig) -> Result<String, ConfigError> {
    match &provider.auth {
        ProviderAuthConfig::ApiKeyInline { api_key } => Ok(api_key.clone()),
        ProviderAuthConfig::ApiKeyEnv { env_var } => {
            let api_key = env::var(env_var).map_err(|_| ConfigError::MissingEnvVar {
                env_var: env_var.clone(),
                owner: ConfigEnvOwner::Provider {
                    provider_id: provider.id.clone(),
                },
            })?;
            if api_key.trim().is_empty() {
                return Err(ConfigError::EmptyEnvVar {
                    env_var: env_var.clone(),
                    owner: ConfigEnvOwner::Provider {
                        provider_id: provider.id.clone(),
                    },
                });
            }
            Ok(api_key)
        }
    }
}

fn select_provider_for_agent(
    providers: &BTreeMap<String, ProviderConfig>,
    agent_name: &str,
    provider_id: &str,
    role: ProviderRouteRole,
) -> Result<SelectedProviderConfig, ConfigError> {
    let provider = providers.get(provider_id).ok_or_else(|| match role {
        ProviderRouteRole::Primary => ConfigError::AgentProviderNotFound {
            agent_name: agent_name.to_owned(),
            provider_id: provider_id.to_owned(),
        },
        ProviderRouteRole::Fallback => ConfigError::AgentFallbackProviderNotFound {
            agent_name: agent_name.to_owned(),
            provider_id: provider_id.to_owned(),
        },
    })?;
    if !provider.enabled {
        return Err(match role {
            ProviderRouteRole::Primary => ConfigError::ProviderDisabled {
                provider_id: provider.id.clone(),
                agent_name: agent_name.to_owned(),
            },
            ProviderRouteRole::Fallback => ConfigError::FallbackProviderDisabled {
                provider_id: provider.id.clone(),
                agent_name: agent_name.to_owned(),
            },
        });
    }
    Ok(SelectedProviderConfig {
        id: provider.id.clone(),
        provider_type: provider.provider_type,
        protocol: provider.protocol,
        base_url: provider.base_url.clone(),
        default_model: provider.default_model.clone(),
        auth_type: provider.auth_type,
        auth_source: provider.auth.source_kind(),
        api_key: resolve_provider_api_key(provider)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn write_temp_config(contents: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time drift")
            .as_nanos();
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("freehand-config-test-{nanos}-{counter}.toml"));
        fs::write(&path, contents).expect("write temp config");
        path
    }

    fn unique_env_name(prefix: &str) -> String {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{prefix}_{counter}")
    }

    #[test]
    fn loads_named_agents_and_providers_from_config_file() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[providers.claude]
id = "claude"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.anthropic.com"
default_model = "claude-sonnet-4-20250514"

[providers.claude.auth]
type = "apikey"
api_key_env = "ANTHROPIC_API_KEY"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
allowed_pair_ip = "127.0.0.1"
pair_token = "SLAVE_TOKEN"
provider = "claude"
"#,
        );

        let config = load_config_from_path(&path).expect("load config");
        let worker = config.agents().get("worker").expect("worker exists");
        let mini27 = config.providers().get("mini27").expect("provider exists");

        assert_eq!(config.agents().len(), 2);
        assert_eq!(config.providers().len(), 2);
        assert_eq!(worker.name, "worker");
        assert_eq!(worker.mode, AgentMode::Slave);
        assert_eq!(worker.node_id, "worker-node");
        assert_eq!(worker.paired_agent_names, vec!["master".to_owned()]);
        assert_eq!(worker.provider_id, "claude");
        assert_eq!(
            worker.allowed_pair_ip,
            Some("127.0.0.1".parse().expect("ip"))
        );
        assert_eq!(worker.pair_token_env, "SLAVE_TOKEN");
        assert_eq!(mini27.protocol, ProviderProtocol::Responses);
        assert_eq!(mini27.auth.source_kind(), ProviderAuthSourceKind::Inline);

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn loads_master_with_three_ordered_worker_peers() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker-alpha", "worker-beta", "worker-gamma"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker-alpha]
name = "worker-alpha"
mode = "slave"
node_id = "worker-alpha-node"
paired_agents = ["master"]
pair_token = "WORKER_ALPHA_TOKEN"
provider = "mini27"

[agents.worker-beta]
name = "worker-beta"
mode = "slave"
node_id = "worker-beta-node"
paired_agents = ["master"]
pair_token = "WORKER_BETA_TOKEN"
provider = "mini27"

[agents.worker-gamma]
name = "worker-gamma"
mode = "slave"
node_id = "worker-gamma-node"
paired_agents = ["master"]
pair_token = "WORKER_GAMMA_TOKEN"
provider = "mini27"
"#,
        );
        // SAFETY: test process controls this environment variable in a scoped test.
        unsafe { env::set_var("MASTER_TOKEN", "pair-token") };

        let config = load_config_from_path(&path).expect("load config");
        let selected = config.select_agent("master").expect("select master");

        assert_eq!(
            config
                .agents()
                .get("master")
                .expect("master")
                .paired_agent_names,
            vec![
                "worker-alpha".to_owned(),
                "worker-beta".to_owned(),
                "worker-gamma".to_owned()
            ]
        );
        assert_eq!(
            selected.worker_peer_names(),
            vec![
                "worker-alpha".to_owned(),
                "worker-beta".to_owned(),
                "worker-gamma".to_owned()
            ]
        );
        assert_eq!(selected.paired_agents[1].node_id, "worker-beta-node");
        assert_eq!(
            selected.paired_agents[2].pair_token_env,
            "WORKER_GAMMA_TOKEN"
        );

        // SAFETY: undo the test environment mutation before exit.
        unsafe { env::remove_var("MASTER_TOKEN") };
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn loads_remote_daemon_registry_with_account_and_endpoint_candidates() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"

[remote_daemon_accounts.jason]
id = "jason"
label = "Jason"
relay_url = "https://relay.freehand.local/relay/"
auth_token_env = "FREEHAND_RELAY_TOKEN"

[remote_daemons.studio]
id = "studio"
account = "jason"
label = "Mac Studio"
node_id = "studio-node"
active_endpoint = "tailscale-main"

[[remote_daemons.studio.endpoints]]
id = "tailscale-main"
kind = "tailscale"
host = "100.66.1.82"
port = 4042

[[remote_daemons.studio.endpoints]]
id = "relay-web"
kind = "relay"
web_url = "https://relay.freehand.local/daemon/studio/web"
adp_url = "wss://relay.freehand.local/daemon/studio/adp"
relay_host_id = "studio-host"
auth_required = true

[remote_daemons.air]
id = "air"
account = "jason"
label = "MacBook Air"
node_id = "air-node"
active_endpoint = "tailscale-main"

[[remote_daemons.air.endpoints]]
id = "tailscale-main"
kind = "tailscale"
host = "100.91.0.21"
port = 4042
"#,
        );

        let config = load_config_from_path(&path).expect("load config");
        let registry = config.remote_daemon_registry();
        assert_eq!(registry.accounts().len(), 1);
        assert_eq!(registry.daemons().len(), 2);

        let selected = registry.select_daemon("studio").expect("select daemon");
        assert_eq!(selected.account.id, "jason");
        assert_eq!(selected.daemon.node_id, "studio-node");
        assert_eq!(
            selected.active_endpoint.kind,
            RemoteDaemonEndpointKind::Tailscale
        );
        assert_eq!(
            selected.active_endpoint.host.as_deref(),
            Some("100.66.1.82")
        );
        assert_eq!(selected.active_endpoint.port, Some(4042));
        assert_eq!(selected.daemon.endpoints.len(), 2);

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn remote_daemon_route_selection_prefers_direct_and_explains_diagnostics() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"

[remote_daemon_accounts.jason]
id = "jason"
relay_url = "https://relay.freehand.local/relay/"

[remote_daemons.studio]
id = "studio"
account = "jason"
node_id = "studio-node"
active_endpoint = "relay-web"

[[remote_daemons.studio.endpoints]]
id = "tailscale-main"
kind = "tailscale"
host = "100.66.1.82"
port = 4042

[[remote_daemons.studio.endpoints]]
id = "relay-web"
kind = "relay"
web_url = "https://relay.freehand.local/daemon/studio/web"
relay_host_id = "studio-host"
"#,
        );
        let config = load_config_from_path(&path).expect("load config");
        let registry = config.remote_daemon_registry();

        let plan = registry.build_route_plan("studio").expect("route plan");
        assert_eq!(
            plan.candidates
                .iter()
                .map(|candidate| candidate.endpoint_id.as_str())
                .collect::<Vec<_>>(),
            vec!["tailscale-main", "relay-web"]
        );
        let selected = registry.select_route("studio", &[]).expect("select route");

        assert_eq!(selected.selected_endpoint.id, "tailscale-main");
        assert_eq!(
            selected
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.endpoint_id == "tailscale-main")
                .expect("tailscale diagnostic")
                .reasons[0],
            "path-cost:10"
        );
        assert!(selected.diagnostics.iter().any(|diagnostic| {
            diagnostic.endpoint_id == "relay-web"
                && diagnostic
                    .reasons
                    .iter()
                    .any(|reason| reason == "health:unknown")
        }));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn remote_daemon_route_selection_uses_relay_when_direct_health_failed() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"

[remote_daemon_accounts.jason]
id = "jason"
relay_url = "https://relay.freehand.local/relay/"

[remote_daemons.studio]
id = "studio"
account = "jason"
node_id = "studio-node"
active_endpoint = "tailscale-main"

[[remote_daemons.studio.endpoints]]
id = "tailscale-main"
kind = "tailscale"
host = "100.66.1.82"
port = 4042

[[remote_daemons.studio.endpoints]]
id = "relay-web"
kind = "relay"
web_url = "https://relay.freehand.local/daemon/studio/web"
relay_host_id = "studio-host"
"#,
        );
        let config = load_config_from_path(&path).expect("load config");
        let registry = config.remote_daemon_registry();
        let health = vec![RemoteDaemonRouteHealthRecord {
            endpoint_id: "tailscale-main".to_owned(),
            status: RemoteDaemonRouteHealthStatus::Failure,
            rtt_ms: None,
            error: Some("connect timeout".to_owned()),
        }];

        let selected = registry
            .select_route("studio", &health)
            .expect("select relay route");
        assert_eq!(selected.selected_endpoint.id, "relay-web");
        assert!(selected.diagnostics.iter().any(|diagnostic| {
            diagnostic.endpoint_id == "tailscale-main"
                && !diagnostic.selectable
                && diagnostic
                    .reasons
                    .iter()
                    .any(|reason| reason == "health:failure:connect timeout")
        }));

        let bundle = registry
            .build_bootstrap_bundle_for_selected_route(
                "studio",
                &health,
                RemoteDaemonBootstrapCredential {
                    kind: RemoteDaemonBootstrapCredentialKind::OneTimeToken,
                    value: "secret-once".to_owned(),
                },
                100,
                200,
                "nonce-1",
            )
            .expect("route-selected bundle");
        assert_eq!(bundle.daemon.active_endpoint_id, "relay-web");

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn remote_daemon_route_selection_fails_when_no_endpoint_is_selectable() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"

[remote_daemon_accounts.jason]
id = "jason"
relay_url = "https://relay.freehand.local/relay/"

[remote_daemons.studio]
id = "studio"
account = "jason"
node_id = "studio-node"
active_endpoint = "tailscale-main"

[[remote_daemons.studio.endpoints]]
id = "tailscale-main"
kind = "tailscale"
host = "100.66.1.82"
port = 4042

[[remote_daemons.studio.endpoints]]
id = "relay-web"
kind = "relay"
web_url = "https://relay.freehand.local/daemon/studio/web"
"#,
        );
        let config = load_config_from_path(&path).expect("load config");
        let registry = config.remote_daemon_registry();
        let error = registry
            .select_route(
                "studio",
                &[
                    RemoteDaemonRouteHealthRecord {
                        endpoint_id: "tailscale-main".to_owned(),
                        status: RemoteDaemonRouteHealthStatus::AuthFailure,
                        rtt_ms: None,
                        error: Some("token rejected".to_owned()),
                    },
                    RemoteDaemonRouteHealthRecord {
                        endpoint_id: "relay-web".to_owned(),
                        status: RemoteDaemonRouteHealthStatus::AuthFailure,
                        rtt_ms: None,
                        error: Some("token rejected".to_owned()),
                    },
                ],
            )
            .expect_err("no selectable endpoint");
        assert!(matches!(
            error,
            ConfigError::RemoteDaemonNoSelectableEndpoint { .. }
        ));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn remote_daemon_bootstrap_link_round_trips_and_redacts_secret_summary() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"

[remote_daemon_accounts.jason]
id = "jason"
label = "Jason"
relay_url = "https://relay.freehand.local/relay/"

[remote_daemons.studio]
id = "studio"
account = "jason"
node_id = "studio-node"
active_endpoint = "relay-web"

[[remote_daemons.studio.endpoints]]
id = "relay-web"
kind = "relay"
web_url = "https://relay.freehand.local/daemon/studio/web"
relay_host_id = "studio-host"
"#,
        );
        let config = load_config_from_path(&path).expect("load config");
        let bundle = config
            .remote_daemon_registry()
            .build_bootstrap_bundle(
                "studio",
                RemoteDaemonBootstrapCredential {
                    kind: RemoteDaemonBootstrapCredentialKind::OneTimeToken,
                    value: "secret-once".to_owned(),
                },
                100,
                200,
                "nonce-1",
            )
            .expect("bundle");

        let link = build_remote_daemon_bootstrap_link(&bundle).expect("link");
        let encoded_payload = link
            .strip_prefix(REMOTE_DAEMON_BOOTSTRAP_URL_PREFIX)
            .expect("app bootstrap prefix");
        let json_payload = String::from_utf8(
            URL_SAFE_NO_PAD
                .decode(encoded_payload.as_bytes())
                .expect("decode bootstrap payload"),
        )
        .expect("utf8 bootstrap payload");
        let value: serde_json::Value =
            serde_json::from_str(&json_payload).expect("bootstrap payload json");
        assert_eq!(value["daemon"]["activeEndpoint"], "relay-web");
        assert!(
            value["daemon"].get("activeEndpointId").is_none(),
            "Android import schema requires activeEndpoint, not activeEndpointId"
        );

        let parsed = parse_remote_daemon_bootstrap_link(&link, 150).expect("parse");
        assert_eq!(parsed.daemon.id, "studio");
        assert_eq!(parsed.credential.value, "secret-once");
        assert_eq!(
            parsed.daemon.endpoints[0].kind,
            RemoteDaemonEndpointKind::Relay
        );
        let summary = format!("{:?}", parsed.safe_summary());
        assert!(summary.contains("studio"));
        assert!(!summary.contains("secret-once"));

        let expired = parse_remote_daemon_bootstrap_link(&link, 200).expect_err("expired");
        assert!(matches!(
            expired,
            ConfigError::RemoteDaemonBootstrapExpired { .. }
        ));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_remote_daemon_active_endpoint_not_declared() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"

[remote_daemon_accounts.jason]
id = "jason"
label = "Jason"

[remote_daemons.studio]
id = "studio"
account = "jason"
node_id = "studio-node"
active_endpoint = "missing"

[[remote_daemons.studio.endpoints]]
id = "tailscale-main"
kind = "tailscale"
host = "100.66.1.82"
port = 4042
"#,
        );

        let error = load_config_from_path(&path).expect_err("invalid config");
        assert!(matches!(
            error,
            ConfigError::RemoteDaemonActiveEndpointNotFound { .. }
        ));
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_relay_endpoint_when_account_has_no_relay_url() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"

[remote_daemon_accounts.jason]
id = "jason"
label = "Jason"

[remote_daemons.studio]
id = "studio"
account = "jason"
node_id = "studio-node"
active_endpoint = "relay-web"

[[remote_daemons.studio.endpoints]]
id = "relay-web"
kind = "relay"
web_url = "https://relay.freehand.local/daemon/studio/web"
"#,
        );

        let error = load_config_from_path(&path).expect_err("invalid config");
        assert!(matches!(
            error,
            ConfigError::RemoteDaemonRelayEndpointMissingAccountRelay { .. }
        ));
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_legacy_singular_paired_agent_field() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "MASTER_TOKEN"
provider = "mini27"
"#,
        );

        let err = load_config_from_path(&path).expect_err("legacy field must fail");
        assert!(
            err.to_string().contains("unknown field `paired_agent`"),
            "unexpected error: {err}"
        );

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_duplicate_and_multi_master_worker_peers() {
        let duplicate_path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker", "worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"
"#,
        );
        let duplicate_err =
            load_config_from_path(&duplicate_path).expect_err("duplicate peer must fail");
        assert!(matches!(
            duplicate_err,
            ConfigError::DuplicatePairedAgentBinding {
                agent_name,
                paired_agent_name
            } if agent_name == "master" && paired_agent_name == "worker"
        ));
        fs::remove_file(duplicate_path).expect("cleanup");

        let multi_master_path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master-a]
name = "master-a"
mode = "master"
node_id = "master-a-node"
paired_agents = ["worker"]
pair_token = "MASTER_A_TOKEN"
provider = "mini27"

[agents.master-b]
name = "master-b"
mode = "master"
node_id = "master-b-node"
paired_agents = ["worker"]
pair_token = "MASTER_B_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master-a", "master-b"]
pair_token = "WORKER_TOKEN"
provider = "mini27"
"#,
        );
        let multi_master_err =
            load_config_from_path(&multi_master_path).expect_err("worker multi-master must fail");
        assert!(matches!(
            multi_master_err,
            ConfigError::SlaveRequiresSingleMasterPeer {
                agent_name,
                peer_count
            } if agent_name == "worker" && peer_count == 2
        ));
        fs::remove_file(multi_master_path).expect("cleanup");
    }

    #[test]
    fn supports_user_style_provider_field_aliases() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "chat_completions"
baseURL = "http://guizhouyun.site:2080"
defaultModel = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
apiKey = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"
"#,
        );

        let config = load_config_from_path(&path).expect("load config");
        let provider = config.providers().get("mini27").expect("provider exists");

        assert_eq!(provider.base_url, "http://guizhouyun.site:2080");
        assert_eq!(provider.default_model, "MiniMax-M2.7");
        assert_eq!(provider.protocol, ProviderProtocol::ChatCompletions);
        assert_eq!(provider.auth.source_kind(), ProviderAuthSourceKind::Inline);

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_mismatched_table_name_and_name_field() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "other"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"
"#,
        );

        let err = load_config_from_path(&path).expect_err("should fail");
        assert!(matches!(
            err,
            ConfigError::AgentNameMismatch {
                table_name,
                field_name
            } if table_name == "master" && field_name == "other"
        ));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_agent_paired_with_itself() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["master"]
pair_token = "MASTER_TOKEN"
provider = "mini27"
"#,
        );

        let err = load_config_from_path(&path).expect_err("should fail");
        assert!(matches!(
            err,
            ConfigError::SelfPairedAgent { agent_name } if agent_name == "master"
        ));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_missing_paired_agent_reference() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"
"#,
        );

        let err = load_config_from_path(&path).expect_err("should fail");
        assert!(matches!(
            err,
            ConfigError::PairedAgentNotFound {
                agent_name,
                paired_agent_name
            } if agent_name == "master" && paired_agent_name == "worker"
        ));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_paired_agents_with_same_mode() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "master"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"
"#,
        );

        let err = load_config_from_path(&path).expect_err("should fail");
        assert!(matches!(
            err,
            ConfigError::PairedAgentModeMismatch {
                agent_name,
                agent_mode,
                paired_agent_name,
                paired_agent_mode
            } if agent_name == "master"
                && agent_mode == "master"
                && paired_agent_name == "worker"
                && paired_agent_mode == "master"
        ));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_non_reciprocal_paired_agents() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["other-master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"
"#,
        );

        let err = load_config_from_path(&path).expect_err("should fail");
        assert!(matches!(
            err,
            ConfigError::PairedAgentReciprocalMismatch {
                agent_name,
                paired_agent_name,
                actual_paired_agent_names
            } if agent_name == "master"
                && paired_agent_name == "worker"
                && actual_paired_agent_names == "other-master"
        ));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn resolves_selected_agent_with_inline_provider_api_key() {
        let token_name = unique_env_name("FREEHAND_MASTER_TOKEN");
        let path = write_temp_config(&format!(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "chat_completions"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[providers.backup]
id = "backup"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://backup.example.com"
default_model = "backup-model"

[providers.backup.auth]
type = "apikey"
api_key = "sk-backup"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "{token_name}"
provider = "mini27"
fallback_provider = "backup"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"
"#,
        ));
        // SAFETY: test process controls this environment variable in a scoped test.
        unsafe { env::set_var(&token_name, "token-value") };

        let config = load_config_from_path(&path).expect("load config");
        let selected = config.select_agent("master").expect("select agent");

        assert_eq!(selected.name, "master");
        assert_eq!(selected.mode, AgentMode::Master);
        assert_eq!(selected.node_id, "master-node");
        assert_eq!(selected.paired_agents.len(), 1);
        let worker_peer = &selected.paired_agents[0];
        assert_eq!(worker_peer.name, "worker");
        assert_eq!(worker_peer.mode, AgentMode::Slave);
        assert_eq!(worker_peer.node_id, "worker-node");
        assert_eq!(worker_peer.pair_token_env, "WORKER_TOKEN");
        assert_eq!(selected.pair_token_env, token_name);
        assert_eq!(selected.pair_token, "token-value");
        assert_eq!(selected.provider.id, "mini27");
        assert_eq!(
            selected.provider.protocol,
            ProviderProtocol::ChatCompletions
        );
        assert_eq!(
            selected.provider.auth_source,
            ProviderAuthSourceKind::Inline
        );
        assert_eq!(selected.provider.api_key, "sk-inline");
        let fallback = selected
            .fallback_provider
            .as_ref()
            .expect("fallback provider");
        assert_eq!(fallback.id, "backup");
        assert_eq!(fallback.provider_type, ProviderType::Anthropic);
        assert_eq!(fallback.protocol, ProviderProtocol::Messages);
        assert_eq!(fallback.default_model, "backup-model");
        assert_eq!(fallback.api_key, "sk-backup");
        assert!(selected.restart_required_on_change);

        // SAFETY: undo the test environment mutation before exit.
        unsafe { env::remove_var(&token_name) };
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_invalid_fallback_provider_bindings() {
        for (fallback_line, expected) in [
            (
                r#"fallback_provider = "missing""#,
                "references missing fallback provider",
            ),
            (
                r#"fallback_provider = "primary""#,
                "must differ from the primary provider",
            ),
            (
                r#"fallback_provider = "disabled""#,
                "selected disabled fallback provider",
            ),
        ] {
            let pair_token_env = unique_env_name("FALLBACK_PAIR_TOKEN");
            let path = write_temp_config(&format!(
                r#"
[providers.primary]
id = "primary"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://primary.example.com"
default_model = "primary-model"

[providers.primary.auth]
type = "apikey"
api_key = "sk-primary"

[providers.disabled]
id = "disabled"
enabled = false
type = "anthropic"
protocol = "messages"
base_url = "https://disabled.example.com"
default_model = "disabled-model"

[providers.disabled.auth]
type = "apikey"
api_key = "sk-disabled"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "{pair_token_env}"
provider = "primary"
{fallback_line}

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "primary"
"#,
            ));
            // SAFETY: test process controls this unique environment variable.
            unsafe { env::set_var(&pair_token_env, "token-value") };

            let config = load_config_from_path(&path).expect("load config");
            let err = config
                .select_agent("master")
                .expect_err("fallback must fail");
            assert!(
                err.to_string().contains(expected),
                "expected `{expected}`, got `{err}`"
            );

            // SAFETY: undo the test environment mutation before exit.
            unsafe { env::remove_var(&pair_token_env) };
            fs::remove_file(path).expect("cleanup");
        }
    }

    #[test]
    fn resolves_selected_agent_with_provider_api_key_env() {
        let pair_token_env = unique_env_name("SLAVE_TOKEN");
        let api_key_env = unique_env_name("ANTHROPIC_API_KEY");
        let path = write_temp_config(&format!(
            r#"
[providers.claude]
id = "claude"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.anthropic.com"
default_model = "claude-sonnet-4-20250514"

[providers.claude.auth]
type = "apikey"
api_key_env = "{api_key_env}"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "{pair_token_env}"
provider = "claude"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "claude"
"#,
        ));
        // SAFETY: test process controls these environment variables in a scoped test.
        unsafe {
            env::set_var(&pair_token_env, "pair-secret");
            env::set_var(&api_key_env, "claude-secret");
        }

        let config = load_config_from_path(&path).expect("load config");
        let selected = config.select_agent("worker").expect("select agent");

        assert_eq!(selected.provider.id, "claude");
        assert_eq!(selected.provider.protocol, ProviderProtocol::Messages);
        assert_eq!(selected.provider.auth_source, ProviderAuthSourceKind::Env);
        assert_eq!(selected.provider.api_key, "claude-secret");
        assert_eq!(selected.paired_agents.len(), 1);
        let master_peer = &selected.paired_agents[0];
        assert_eq!(master_peer.name, "master");
        assert_eq!(master_peer.node_id, "master-node");
        assert_eq!(master_peer.pair_token_env, "MASTER_TOKEN");

        // SAFETY: undo the test environment mutation before exit.
        unsafe {
            env::remove_var(&pair_token_env);
            env::remove_var(&api_key_env);
        }
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_missing_pair_token_env_at_selection_time() {
        let token_name = unique_env_name("FREEHAND_MISSING_TOKEN");
        let path = write_temp_config(&format!(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "{token_name}"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"
"#,
        ));

        let config = load_config_from_path(&path).expect("load config");
        let err = config.select_agent("master").expect_err("should fail");

        assert!(matches!(
            err,
            ConfigError::MissingEnvVar {
                env_var,
                owner: ConfigEnvOwner::Agent { agent_name }
            } if env_var == token_name && agent_name == "master"
        ));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_disabled_provider_selection() {
        let token_name = unique_env_name("FREEHAND_MASTER_TOKEN");
        let path = write_temp_config(&format!(
            r#"
[providers.mini27]
id = "mini27"
enabled = false
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "{token_name}"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"
"#,
        ));
        // SAFETY: test process controls this environment variable in a scoped test.
        unsafe { env::set_var(&token_name, "token-value") };

        let config = load_config_from_path(&path).expect("load config");
        let err = config.select_agent("master").expect_err("should fail");

        assert!(matches!(
            err,
            ConfigError::ProviderDisabled {
                provider_id,
                agent_name
            } if provider_id == "mini27" && agent_name == "master"
        ));

        // SAFETY: undo the test environment mutation before exit.
        unsafe { env::remove_var(&token_name) };
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn default_config_path_points_under_home_dot_freehand() {
        let home = env::temp_dir().join("freehand-config-home");
        fs::create_dir_all(&home).expect("create home");
        // SAFETY: test process controls HOME for this check.
        unsafe { env::set_var("HOME", &home) };
        let path = default_config_path().expect("default path");
        assert_eq!(path, home.join(CONFIG_FILE_RELATIVE_PATH));
        // SAFETY: undo the test environment mutation before exit.
        unsafe { env::remove_var("HOME") };
        fs::remove_dir_all(home).expect("cleanup");
    }

    #[test]
    fn rejects_provider_without_explicit_protocol() {
        let path = write_temp_config(
            r#"
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "FREEHAND_MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "mini27"
"#,
        );

        let err = load_config_from_path(&path).expect_err("should fail");
        assert!(matches!(
            err,
            ConfigError::MissingProviderProtocol { provider_id } if provider_id == "mini27"
        ));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_unknown_provider_field() {
        let path = write_temp_config(
            r#"
[providers.minimonth]
id = "minimonth"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.53hk.cn"
default_model = "MiniMax-M2.7"
transportBackend = "vercel-ai-sdk"

[providers.minimonth.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "FREEHAND_MASTER_TOKEN"
provider = "minimonth"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "minimonth"
"#,
        );

        let err = load_config_from_path(&path).expect_err("should fail");
        assert!(matches!(err, ConfigError::ParseConfig { .. }));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn update_provider_config_persists_env_based_provider_without_secret_projection() {
        let pair_token_env = unique_env_name("FREEHAND_UPDATE_PAIR_TOKEN");
        let provider_key_env = unique_env_name("FREEHAND_UPDATE_PROVIDER_KEY");
        let path = write_temp_config(&format!(
            r#"
[providers.old]
id = "old"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://old.example.test/v1"
default_model = "old-model"

[providers.old.auth]
type = "apikey"
api_key = "sk-inline-should-disappear"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "{pair_token_env}"
provider = "old"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "old"
"#
        ));
        // SAFETY: test process controls these environment variables in a scoped test.
        unsafe {
            env::set_var(&pair_token_env, "pair-token");
            env::set_var(&provider_key_env, "provider-secret");
        }

        let selected = update_provider_config_in_path(
            &path,
            ProviderConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "minimax".to_owned(),
                provider_type: "openai".to_owned(),
                protocol: "responses".to_owned(),
                base_url: "https://api.minimaxi.com/v1".to_owned(),
                default_model: "MiniMax-M3".to_owned(),
                api_key_env: provider_key_env.clone(),
            },
        )
        .expect("update provider config");

        assert_eq!(selected.provider.id, "minimax");
        assert_eq!(selected.provider.default_model, "MiniMax-M3");
        assert_eq!(selected.provider.auth_source, ProviderAuthSourceKind::Env);
        assert_eq!(selected.provider.api_key, "provider-secret");
        assert!(selected.restart_required_on_change);

        let raw = fs::read_to_string(&path).expect("read updated config");
        assert!(raw.contains("[providers.minimax]"));
        assert!(raw.contains(&format!("api_key_env = \"{provider_key_env}\"")));
        assert!(!raw.contains("provider-secret"));

        // SAFETY: undo the test environment mutation before exit.
        unsafe {
            env::remove_var(&pair_token_env);
            env::remove_var(&provider_key_env);
        }
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn safe_provider_registry_projects_all_providers_without_secrets() {
        let path = write_temp_config(
            r#"
[providers.cc]
id = "cc"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://user:password@api.anyint.ai/openai/v1?token=secret"
default_model = "gpt-5.5"

[providers.cc.auth]
type = "apikey"
api_key_env = "FREEHAND_CC_API_KEY"

[providers.minimax]
id = "minimax"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.minimaxi.com/anthropic"
default_model = "MiniMax-M3"

[providers.minimax.auth]
type = "apikey"
api_key = "sk-minimax-secret"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "FREEHAND_MASTER_TOKEN"
provider = "cc"
fallback_provider = "minimax"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "cc"
fallback_provider = "minimax"
"#,
        );

        let config = load_config_from_path(&path).expect("load config");
        let registry = config.safe_provider_registry();

        assert_eq!(registry.len(), 2);
        let cc = registry
            .iter()
            .find(|provider| provider.id == "cc")
            .expect("cc provider");
        assert_eq!(cc.provider_type, ProviderType::OpenAi);
        assert_eq!(cc.protocol, ProviderProtocol::Responses);
        assert_eq!(cc.base_url, "https://api.anyint.ai/openai/v1");
        assert_eq!(cc.base_url_host, "api.anyint.ai");
        assert_eq!(cc.auth_source, ProviderAuthSourceKind::Env);
        let minimax = registry
            .iter()
            .find(|provider| provider.id == "minimax")
            .expect("minimax provider");
        assert_eq!(minimax.provider_type, ProviderType::Anthropic);
        assert_eq!(minimax.protocol, ProviderProtocol::Messages);
        assert_eq!(minimax.base_url, "https://api.minimaxi.com/anthropic");
        assert_eq!(minimax.auth_source, ProviderAuthSourceKind::Inline);

        let debug = format!("{registry:?}");
        assert!(!debug.contains("sk-minimax-secret"));
        assert!(!debug.contains("password"));
        assert!(!debug.contains("token=secret"));
        assert!(!debug.contains("api_key"));

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn upsert_provider_config_adds_provider_without_switching_agent_selection() {
        let pair_token_env = unique_env_name("FREEHAND_UPSERT_PAIR_TOKEN");
        let path = write_temp_config(&format!(
            r#"
[providers.cc]
id = "cc"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://api.anyint.ai/openai/v1"
default_model = "gpt-5.5"

[providers.cc.auth]
type = "apikey"
api_key = "sk-cc-inline"

[providers.minimax]
id = "minimax"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.minimaxi.com/anthropic"
default_model = "MiniMax-M3"

[providers.minimax.auth]
type = "apikey"
api_key = "sk-minimax-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "{pair_token_env}"
provider = "cc"
fallback_provider = "minimax"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "cc"
fallback_provider = "minimax"
"#
        ));
        // SAFETY: test process controls this unique environment variable.
        unsafe { env::set_var(&pair_token_env, "pair-token") };

        let selected = upsert_provider_config_in_path(
            &path,
            ProviderConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "new.openai".to_owned(),
                provider_type: "openai".to_owned(),
                protocol: "responses".to_owned(),
                base_url: "https://new.example.test/openai/v1".to_owned(),
                default_model: "gpt-next".to_owned(),
                api_key_env: "FREEHAND_NEW_OPENAI_KEY".to_owned(),
            },
        )
        .expect("upsert provider");

        assert_eq!(selected.provider.id, "cc");
        assert_eq!(
            selected
                .fallback_provider
                .as_ref()
                .map(|provider| provider.id.as_str()),
            Some("minimax")
        );
        let raw = fs::read_to_string(&path).expect("read config");
        assert!(raw.contains("[providers.cc]"));
        assert!(raw.contains("[providers.minimax]"));
        assert!(raw.contains("[providers.\"new.openai\"]"));
        assert!(raw.contains("provider = \"cc\""));
        assert!(raw.contains("fallback_provider = \"minimax\""));
        assert!(raw.contains("api_key_env = \"FREEHAND_NEW_OPENAI_KEY\""));

        // SAFETY: undo the test environment mutation before exit.
        unsafe { env::remove_var(&pair_token_env) };
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn switch_agent_provider_persists_selection_without_rewriting_registry() {
        let pair_token_env = unique_env_name("FREEHAND_SWITCH_PAIR_TOKEN");
        let cc_key_env = unique_env_name("FREEHAND_SWITCH_CC_KEY");
        let path = write_temp_config(&format!(
            r#"
[providers.cc]
id = "cc"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://api.anyint.ai/openai/v1"
default_model = "gpt-5.5"

[providers.cc.auth]
type = "apikey"
api_key_env = "{cc_key_env}"

[providers.minimax]
id = "minimax"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.minimaxi.com/anthropic"
default_model = "MiniMax-M3"

[providers.minimax.auth]
type = "apikey"
api_key = "sk-minimax-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "{pair_token_env}"
provider = "cc"
fallback_provider = "minimax"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "cc"
fallback_provider = "minimax"
"#
        ));
        // SAFETY: test process controls these unique environment variables.
        unsafe {
            env::set_var(&pair_token_env, "pair-token");
            env::set_var(&cc_key_env, "cc-secret");
        }

        let selected = switch_agent_provider_in_path(
            &path,
            AgentProviderSelectionConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "minimax".to_owned(),
                fallback_provider_id: None,
            },
        )
        .expect("switch provider");

        assert_eq!(selected.provider.id, "minimax");
        assert_eq!(selected.provider.default_model, "MiniMax-M3");
        assert!(selected.fallback_provider.is_none());
        let raw = fs::read_to_string(&path).expect("read config");
        assert!(raw.contains("[providers.cc]"));
        assert!(raw.contains("[providers.minimax]"));
        assert!(raw.contains("api_key_env = \""));
        assert!(!raw.contains("cc-secret"));
        let saved: toml::Value = toml::from_str(&raw).expect("saved toml");
        let agents = saved
            .get("agents")
            .and_then(toml::Value::as_table)
            .expect("agents table");
        let master = agents
            .get("master")
            .and_then(toml::Value::as_table)
            .expect("master table");
        assert_eq!(
            master.get("provider").and_then(toml::Value::as_str),
            Some("minimax")
        );
        assert!(master.get("fallback_provider").is_none());
        let worker = agents
            .get("worker")
            .and_then(toml::Value::as_table)
            .expect("worker table");
        assert_eq!(
            worker
                .get("fallback_provider")
                .and_then(toml::Value::as_str),
            Some("minimax")
        );

        // SAFETY: undo the test environment mutation before exit.
        unsafe {
            env::remove_var(&pair_token_env);
            env::remove_var(&cc_key_env);
        }
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn switch_agent_provider_rejects_invalid_selection_without_overwrite() {
        let pair_token_env = unique_env_name("FREEHAND_BAD_SWITCH_PAIR_TOKEN");
        let path = write_temp_config(&format!(
            r#"
[providers.cc]
id = "cc"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://api.anyint.ai/openai/v1"
default_model = "gpt-5.5"

[providers.cc.auth]
type = "apikey"
api_key = "sk-cc-inline"

[providers.minimax]
id = "minimax"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.minimaxi.com/anthropic"
default_model = "MiniMax-M3"

[providers.minimax.auth]
type = "apikey"
api_key = "sk-minimax-inline"

[providers.disabled]
id = "disabled"
enabled = false
type = "anthropic"
protocol = "messages"
base_url = "https://disabled.example.test"
default_model = "disabled-model"

[providers.disabled.auth]
type = "apikey"
api_key = "sk-disabled"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "{pair_token_env}"
provider = "cc"
fallback_provider = "minimax"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "cc"
fallback_provider = "minimax"
"#
        ));
        // SAFETY: test process controls this unique environment variable.
        unsafe { env::set_var(&pair_token_env, "pair-token") };

        let before = fs::read_to_string(&path).expect("read before");
        let same_fallback_err = switch_agent_provider_in_path(
            &path,
            AgentProviderSelectionConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "minimax".to_owned(),
                fallback_provider_id: Some("minimax".to_owned()),
            },
        )
        .expect_err("same fallback rejected");
        assert!(matches!(
            same_fallback_err,
            ConfigError::FallbackProviderMatchesPrimary { .. }
        ));
        assert_eq!(fs::read_to_string(&path).expect("read after"), before);

        let disabled_err = switch_agent_provider_in_path(
            &path,
            AgentProviderSelectionConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "disabled".to_owned(),
                fallback_provider_id: None,
            },
        )
        .expect_err("disabled provider rejected");
        assert!(matches!(disabled_err, ConfigError::ProviderDisabled { .. }));
        assert_eq!(fs::read_to_string(&path).expect("read after"), before);

        // SAFETY: undo the test environment mutation before exit.
        unsafe { env::remove_var(&pair_token_env) };
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn update_provider_config_rejects_invalid_url_without_overwriting_config() {
        let path = write_temp_config(
            r#"
[providers.old]
id = "old"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://old.example.test/v1"
default_model = "old-model"

[providers.old.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "FREEHAND_MASTER_TOKEN"
provider = "old"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "old"
"#,
        );
        let before = fs::read_to_string(&path).expect("read original config");
        let err = update_provider_config_in_path(
            &path,
            ProviderConfigUpdate {
                agent_name: "master".to_owned(),
                provider_id: "bad".to_owned(),
                provider_type: "openai".to_owned(),
                protocol: "responses".to_owned(),
                base_url: "not-a-url".to_owned(),
                default_model: "model".to_owned(),
                api_key_env: "FREEHAND_API_KEY".to_owned(),
            },
        )
        .expect_err("invalid URL must fail");
        assert!(matches!(
            err,
            ConfigError::InvalidProviderBaseUrl { provider_id } if provider_id == "bad"
        ));
        let after = fs::read_to_string(&path).expect("read config after failed update");
        assert_eq!(after, before);

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn update_agent_resource_config_grows_and_shrinks_shared_provider_workers() {
        let pair_token_env = unique_env_name("FREEHAND_AGENT_RESOURCE_PAIR_TOKEN");
        let path = write_temp_config(&format!(
            r#"
[providers.primary]
id = "primary"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://primary.example.test/v1"
default_model = "primary-model"

[providers.primary.auth]
type = "apikey"
api_key = "primary-secret"

[providers.backup]
id = "backup"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://backup.example.test"
default_model = "backup-model"

[providers.backup.auth]
type = "apikey"
api_key = "backup-secret"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "{pair_token_env}"
provider = "primary"
fallback_provider = "backup"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "primary"
fallback_provider = "backup"
"#,
        ));
        // SAFETY: test process owns this unique environment variable.
        unsafe { env::set_var(&pair_token_env, "pair-token") };

        let grown = update_agent_resource_config_in_path(
            &path,
            AgentResourceConfigUpdate {
                agent_name: "master".to_owned(),
                resource_count: 5,
            },
        )
        .expect("grow Agent resources");
        assert_eq!(
            grown.worker_peer_names(),
            vec!["worker", "worker-2", "worker-3", "worker-4", "worker-5"]
        );
        let grown_config = load_config_from_path(&path).expect("load grown config");
        assert_eq!(grown_config.agents().len(), 6);
        for worker_name in grown.worker_peer_names() {
            let worker = grown_config.agents().get(&worker_name).expect("worker");
            assert_eq!(worker.paired_agent_names, vec!["master"]);
            assert_eq!(worker.provider_id, "primary");
            assert_eq!(worker.fallback_provider_id.as_deref(), Some("backup"));
        }

        let shrunk = update_agent_resource_config_in_path(
            &path,
            AgentResourceConfigUpdate {
                agent_name: "master".to_owned(),
                resource_count: 2,
            },
        )
        .expect("shrink Agent resources");
        assert_eq!(shrunk.worker_peer_names(), vec!["worker", "worker-2"]);
        let shrunk_config = load_config_from_path(&path).expect("load shrunk config");
        assert_eq!(shrunk_config.agents().len(), 3);
        assert!(!shrunk_config.agents().contains_key("worker-3"));

        // SAFETY: undo the test environment mutation before exit.
        unsafe { env::remove_var(&pair_token_env) };
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn update_agent_resource_config_rejects_invalid_intent_without_overwrite() {
        let path = write_temp_config(
            r#"
[providers.primary]
id = "primary"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://primary.example.test/v1"
default_model = "primary-model"

[providers.primary.auth]
type = "apikey"
api_key = "primary-secret"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "MASTER_TOKEN"
provider = "primary"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "WORKER_TOKEN"
provider = "primary"
"#,
        );
        let before = fs::read_to_string(&path).expect("read original config");

        for resource_count in [0, 6] {
            let err = update_agent_resource_config_in_path(
                &path,
                AgentResourceConfigUpdate {
                    agent_name: "master".to_owned(),
                    resource_count,
                },
            )
            .expect_err("out-of-range resource count must fail");
            assert!(matches!(
                err,
                ConfigError::AgentResourceCountOutOfRange { .. }
            ));
            assert_eq!(
                fs::read_to_string(&path).expect("read unchanged config"),
                before
            );
        }

        let err = update_agent_resource_config_in_path(
            &path,
            AgentResourceConfigUpdate {
                agent_name: "worker".to_owned(),
                resource_count: 2,
            },
        )
        .expect_err("Worker resource update must fail");
        assert!(matches!(
            err,
            ConfigError::AgentResourceUpdateRequiresMaster { agent_name }
                if agent_name == "worker"
        ));
        assert_eq!(
            fs::read_to_string(&path).expect("read unchanged config"),
            before
        );

        let err = update_agent_resource_config_in_path(
            &path,
            AgentResourceConfigUpdate {
                agent_name: "missing".to_owned(),
                resource_count: 2,
            },
        )
        .expect_err("unknown agent update must fail");
        assert!(
            matches!(err, ConfigError::AgentNotFound { agent_name } if agent_name == "missing")
        );
        assert_eq!(
            fs::read_to_string(&path).expect("read unchanged config"),
            before
        );

        fs::remove_file(path).expect("cleanup");
    }
}
