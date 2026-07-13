//! Config loading and validation for Freehand.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

pub const CONFIG_FILE_RELATIVE_PATH: &str = ".freehand/config.toml";

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
    pub paired_agent_name: String,
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
pub struct LoadedConfig {
    agents: BTreeMap<String, AgentConfig>,
    providers: BTreeMap<String, ProviderConfig>,
}

impl LoadedConfig {
    pub fn agents(&self) -> &BTreeMap<String, AgentConfig> {
        &self.agents
    }

    pub fn providers(&self) -> &BTreeMap<String, ProviderConfig> {
        &self.providers
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
        let paired = self.agents.get(&agent.paired_agent_name).ok_or_else(|| {
            ConfigError::PairedAgentNotFound {
                agent_name: agent.name.clone(),
                paired_agent_name: agent.paired_agent_name.clone(),
            }
        })?;

        Ok(SelectedAgentConfig {
            name: agent.name.clone(),
            mode: agent.mode,
            node_id: agent.node_id.clone(),
            paired_agent_name: agent.paired_agent_name.clone(),
            paired_agent_mode: paired.mode,
            paired_node_id: paired.node_id.clone(),
            paired_allowed_pair_ip: paired.allowed_pair_ip,
            paired_pair_token_env: paired.pair_token_env.clone(),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedAgentConfig {
    pub name: String,
    pub mode: AgentMode,
    pub node_id: String,
    pub paired_agent_name: String,
    pub paired_agent_mode: AgentMode,
    pub paired_node_id: String,
    pub paired_allowed_pair_ip: Option<IpAddr>,
    pub paired_pair_token_env: String,
    pub allowed_pair_ip: Option<IpAddr>,
    pub pair_token_env: String,
    pub pair_token: String,
    pub provider: SelectedProviderConfig,
    pub fallback_provider: Option<SelectedProviderConfig>,
    pub restart_required_on_change: bool,
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
    #[error("agent `{agent_name}` paired_agent must be a non-empty agent name")]
    EmptyPairedAgentBinding { agent_name: String },
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
    #[error(
        "agent `{agent_name}` expects reciprocal pairing from `{paired_agent_name}`, but that agent points to `{actual_paired_agent_name}`"
    )]
    PairedAgentReciprocalMismatch {
        agent_name: String,
        paired_agent_name: String,
        actual_paired_agent_name: String,
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
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAgentConfig {
    name: String,
    mode: AgentMode,
    node_id: String,
    paired_agent: String,
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
    validate_provider_id(&update.provider_id)?;
    validate_provider_base_url(&update.provider_id, &update.base_url)?;
    let provider_type = parse_provider_type(&update.provider_id, &update.provider_type)?;
    let protocol = parse_provider_protocol(&update.provider_id, &update.protocol)?;
    resolve_provider_protocol(&update.provider_id, provider_type, Some(protocol))?;
    if update.default_model.trim().is_empty() {
        return Err(ConfigError::EmptyProviderDefaultModel {
            provider_id: update.provider_id,
        });
    }
    if update.api_key_env.trim().is_empty() {
        return Err(ConfigError::EmptyProviderApiKeyEnv {
            provider_id: update.provider_id,
        });
    }

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
    apply_provider_config_update(path, &mut document, &update, provider_type, protocol)?;
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

fn parse_config(path: &Path, raw: &str) -> Result<LoadedConfig, ConfigError> {
    let parsed: RawConfig = toml::from_str(raw).map_err(|source| ConfigError::ParseConfig {
        path: path.to_path_buf(),
        source,
    })?;
    validate_config(parsed)
}

fn validate_config(parsed: RawConfig) -> Result<LoadedConfig, ConfigError> {
    if parsed.agents.is_empty() {
        return Err(ConfigError::NoAgentsDefined);
    }
    if parsed.providers.is_empty() {
        return Err(ConfigError::NoProvidersDefined);
    }

    let mut providers = BTreeMap::new();
    for (table_name, raw_provider) in parsed.providers {
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
    for (table_name, raw_agent) in parsed.agents {
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
        if raw_agent.paired_agent.trim().is_empty() {
            return Err(ConfigError::EmptyPairedAgentBinding {
                agent_name: raw_agent.name,
            });
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
            paired_agent_name: raw_agent.paired_agent,
            allowed_pair_ip: raw_agent.allowed_pair_ip,
            pair_token_env: raw_agent.pair_token,
            provider_id: raw_agent.provider,
            fallback_provider_id: raw_agent.fallback_provider,
        };
        agents.insert(raw_agent.name, agent);
    }

    for agent in agents.values() {
        if agent.paired_agent_name == agent.name {
            return Err(ConfigError::SelfPairedAgent {
                agent_name: agent.name.clone(),
            });
        }
        let paired = agents.get(&agent.paired_agent_name).ok_or_else(|| {
            ConfigError::PairedAgentNotFound {
                agent_name: agent.name.clone(),
                paired_agent_name: agent.paired_agent_name.clone(),
            }
        })?;
        if paired.mode == agent.mode {
            return Err(ConfigError::PairedAgentModeMismatch {
                agent_name: agent.name.clone(),
                agent_mode: agent.mode.as_str().to_owned(),
                paired_agent_name: paired.name.clone(),
                paired_agent_mode: paired.mode.as_str().to_owned(),
            });
        }
        if paired.paired_agent_name != agent.name {
            return Err(ConfigError::PairedAgentReciprocalMismatch {
                agent_name: agent.name.clone(),
                paired_agent_name: paired.name.clone(),
                actual_paired_agent_name: paired.paired_agent_name.clone(),
            });
        }
    }

    Ok(LoadedConfig { agents, providers })
}

fn apply_provider_config_update(
    path: &Path,
    document: &mut toml::Value,
    update: &ProviderConfigUpdate,
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
    provider.insert(
        "id".to_owned(),
        toml::Value::String(update.provider_id.clone()),
    );
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
    providers.insert(update.provider_id.clone(), toml::Value::Table(provider));

    let agents = root
        .get_mut("agents")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| ConfigError::MissingConfigTable {
            path: path.to_path_buf(),
            table: "agents".to_owned(),
        })?;
    let agent = agents
        .get_mut(&update.agent_name)
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| ConfigError::AgentNotFound {
            agent_name: update.agent_name.clone(),
        })?;
    agent.insert(
        "provider".to_owned(),
        toml::Value::String(update.provider_id.clone()),
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

fn validate_provider_base_url(provider_id: &str, base_url: &str) -> Result<(), ConfigError> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err(ConfigError::EmptyProviderBaseUrl {
            provider_id: provider_id.to_owned(),
        });
    }
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return Err(ConfigError::InvalidProviderBaseUrl {
            provider_id: provider_id.to_owned(),
        });
    };
    if !matches!(scheme, "http" | "https")
        || rest.trim().is_empty()
        || rest.contains(char::is_whitespace)
    {
        return Err(ConfigError::InvalidProviderBaseUrl {
            provider_id: provider_id.to_owned(),
        });
    }
    let authority = rest.split('/').next().unwrap_or_default();
    let host = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    if host.trim().is_empty() {
        return Err(ConfigError::InvalidProviderBaseUrl {
            provider_id: provider_id.to_owned(),
        });
    }
    Ok(())
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
paired_agent = "worker"
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
        assert_eq!(worker.paired_agent_name, "master");
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
paired_agent = "worker"
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
paired_agent = "worker"
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
paired_agent = "master"
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
paired_agent = "worker"
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
paired_agent = "worker"
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "master"
node_id = "worker-node"
paired_agent = "master"
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
paired_agent = "worker"
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "other-master"
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
                actual_paired_agent_name
            } if agent_name == "master"
                && paired_agent_name == "worker"
                && actual_paired_agent_name == "other-master"
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
paired_agent = "worker"
pair_token = "{token_name}"
provider = "mini27"
fallback_provider = "backup"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
        assert_eq!(selected.paired_agent_name, "worker");
        assert_eq!(selected.paired_agent_mode, AgentMode::Slave);
        assert_eq!(selected.paired_node_id, "worker-node");
        assert_eq!(selected.paired_pair_token_env, "WORKER_TOKEN");
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
paired_agent = "worker"
pair_token = "{pair_token_env}"
provider = "primary"
{fallback_line}

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
paired_agent = "master"
pair_token = "{pair_token_env}"
provider = "claude"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
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
        assert_eq!(selected.paired_agent_name, "master");
        assert_eq!(selected.paired_node_id, "master-node");
        assert_eq!(selected.paired_pair_token_env, "MASTER_TOKEN");

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
paired_agent = "worker"
pair_token = "{token_name}"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
paired_agent = "worker"
pair_token = "{token_name}"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
paired_agent = "worker"
pair_token = "FREEHAND_MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
paired_agent = "worker"
pair_token = "FREEHAND_MASTER_TOKEN"
provider = "minimonth"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
paired_agent = "worker"
pair_token = "{pair_token_env}"
provider = "old"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
paired_agent = "worker"
pair_token = "FREEHAND_MASTER_TOKEN"
provider = "old"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
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
}
