//! Config loading and validation for Freehand.

use std::collections::BTreeMap;
use std::env;
use std::fs;
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
    pub allowed_pair_ip: Option<IpAddr>,
    pub pair_token_env: String,
    pub provider_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderAuthConfig {
    ApiKeyInline { api_key: String },
    ApiKeyEnv { env_var: String },
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
    pub api_key: String,
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

        let provider = self.providers.get(&agent.provider_id).ok_or_else(|| {
            ConfigError::AgentProviderNotFound {
                agent_name: agent.name.clone(),
                provider_id: agent.provider_id.clone(),
            }
        })?;
        if !provider.enabled {
            return Err(ConfigError::ProviderDisabled {
                provider_id: provider.id.clone(),
                agent_name: agent.name.clone(),
            });
        }

        let api_key = resolve_provider_api_key(provider)?;

        Ok(SelectedAgentConfig {
            name: agent.name.clone(),
            mode: agent.mode,
            allowed_pair_ip: agent.allowed_pair_ip,
            pair_token_env: agent.pair_token_env.clone(),
            pair_token,
            provider: SelectedProviderConfig {
                id: provider.id.clone(),
                provider_type: provider.provider_type,
                protocol: provider.protocol,
                base_url: provider.base_url.clone(),
                default_model: provider.default_model.clone(),
                auth_type: provider.auth_type,
                api_key,
            },
            restart_required_on_change: true,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedAgentConfig {
    pub name: String,
    pub mode: AgentMode,
    pub allowed_pair_ip: Option<IpAddr>,
    pub pair_token_env: String,
    pub pair_token: String,
    pub provider: SelectedProviderConfig,
    pub restart_required_on_change: bool,
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
    #[error("agent `{agent_name}` provider must be a non-empty provider id")]
    EmptyProviderBinding { agent_name: String },
    #[error("provider `{provider_id}` base_url must be non-empty")]
    EmptyProviderBaseUrl { provider_id: String },
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
    #[error("agent `{agent_name}` selected disabled provider `{provider_id}`")]
    ProviderDisabled {
        provider_id: String,
        agent_name: String,
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
    allowed_pair_ip: Option<IpAddr>,
    pair_token: String,
    provider: String,
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
        if raw_provider.base_url.trim().is_empty() {
            return Err(ConfigError::EmptyProviderBaseUrl {
                provider_id: raw_provider.id,
            });
        }
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
        if raw_agent.provider.trim().is_empty() {
            return Err(ConfigError::EmptyProviderBinding {
                agent_name: raw_agent.name,
            });
        }

        let agent = AgentConfig {
            name: raw_agent.name.clone(),
            mode: raw_agent.mode,
            allowed_pair_ip: raw_agent.allowed_pair_ip,
            pair_token_env: raw_agent.pair_token,
            provider_id: raw_agent.provider,
        };
        agents.insert(raw_agent.name, agent);
    }

    Ok(LoadedConfig { agents, providers })
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
pair_token = "MASTER_TOKEN"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
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
        assert_eq!(worker.provider_id, "claude");
        assert_eq!(
            worker.allowed_pair_ip,
            Some("127.0.0.1".parse().expect("ip"))
        );
        assert_eq!(worker.pair_token_env, "SLAVE_TOKEN");
        assert_eq!(mini27.protocol, ProviderProtocol::Responses);

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
pair_token = "MASTER_TOKEN"
provider = "mini27"
"#,
        );

        let config = load_config_from_path(&path).expect("load config");
        let provider = config.providers().get("mini27").expect("provider exists");

        assert_eq!(provider.base_url, "http://guizhouyun.site:2080");
        assert_eq!(provider.default_model, "MiniMax-M2.7");
        assert_eq!(provider.protocol, ProviderProtocol::ChatCompletions);

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
pair_token = "MASTER_TOKEN"
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

[agents.master]
name = "master"
mode = "master"
pair_token = "{token_name}"
provider = "mini27"
"#,
        ));
        // SAFETY: test process controls this environment variable in a scoped test.
        unsafe { env::set_var(&token_name, "token-value") };

        let config = load_config_from_path(&path).expect("load config");
        let selected = config.select_agent("master").expect("select agent");

        assert_eq!(selected.name, "master");
        assert_eq!(selected.mode, AgentMode::Master);
        assert_eq!(selected.pair_token_env, token_name);
        assert_eq!(selected.pair_token, "token-value");
        assert_eq!(selected.provider.id, "mini27");
        assert_eq!(
            selected.provider.protocol,
            ProviderProtocol::ChatCompletions
        );
        assert_eq!(selected.provider.api_key, "sk-inline");
        assert!(selected.restart_required_on_change);

        // SAFETY: undo the test environment mutation before exit.
        unsafe { env::remove_var(&token_name) };
        fs::remove_file(path).expect("cleanup");
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
pair_token = "{pair_token_env}"
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
        assert_eq!(selected.provider.api_key, "claude-secret");

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
pair_token = "{token_name}"
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
pair_token = "{token_name}"
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
pair_token = "FREEHAND_MASTER_TOKEN"
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
pair_token = "FREEHAND_MASTER_TOKEN"
provider = "minimonth"
"#,
        );

        let err = load_config_from_path(&path).expect_err("should fail");
        assert!(matches!(err, ConfigError::ParseConfig { .. }));

        fs::remove_file(path).expect("cleanup");
    }
}
