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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfig {
    pub name: String,
    pub mode: AgentMode,
    pub allowed_pair_ip: Option<IpAddr>,
    pub pair_token_env: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    agents: BTreeMap<String, AgentConfig>,
}

impl LoadedConfig {
    pub fn agents(&self) -> &BTreeMap<String, AgentConfig> {
        &self.agents
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
                agent_name: agent.name.clone(),
            })?;
        if pair_token.trim().is_empty() {
            return Err(ConfigError::EmptyEnvVar {
                env_var: agent.pair_token_env.clone(),
                agent_name: agent.name.clone(),
            });
        }
        Ok(SelectedAgentConfig {
            name: agent.name.clone(),
            mode: agent.mode,
            allowed_pair_ip: agent.allowed_pair_ip,
            pair_token_env: agent.pair_token_env.clone(),
            pair_token,
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
    pub restart_required_on_change: bool,
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
    #[error("agent table `{table_name}` has mismatched name field `{field_name}`")]
    AgentNameMismatch {
        table_name: String,
        field_name: String,
    },
    #[error("agent `{agent_name}` pair_token must be a non-empty environment variable name")]
    EmptyPairTokenEnv { agent_name: String },
    #[error("agent `{agent_name}` not found in config")]
    AgentNotFound { agent_name: String },
    #[error("environment variable `{env_var}` for agent `{agent_name}` is not set")]
    MissingEnvVar { env_var: String, agent_name: String },
    #[error("environment variable `{env_var}` for agent `{agent_name}` is empty")]
    EmptyEnvVar { env_var: String, agent_name: String },
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    #[serde(default)]
    agents: BTreeMap<String, RawAgentConfig>,
}

#[derive(Debug, Deserialize)]
struct RawAgentConfig {
    name: String,
    mode: AgentMode,
    allowed_pair_ip: Option<IpAddr>,
    pair_token: String,
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

        let agent = AgentConfig {
            name: raw_agent.name.clone(),
            mode: raw_agent.mode,
            allowed_pair_ip: raw_agent.allowed_pair_ip,
            pair_token_env: raw_agent.pair_token,
        };
        agents.insert(raw_agent.name, agent);
    }

    Ok(LoadedConfig { agents })
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

    #[test]
    fn loads_named_agents_from_config_file() {
        let path = write_temp_config(
            r#"
[agents.master]
name = "master"
mode = "master"
pair_token = "MASTER_TOKEN"

[agents.worker]
name = "worker"
mode = "slave"
allowed_pair_ip = "127.0.0.1"
pair_token = "SLAVE_TOKEN"
"#,
        );

        let config = load_config_from_path(&path).expect("load config");
        let worker = config.agents().get("worker").expect("worker exists");

        assert_eq!(config.agents().len(), 2);
        assert_eq!(worker.name, "worker");
        assert_eq!(worker.mode, AgentMode::Slave);
        assert_eq!(
            worker.allowed_pair_ip,
            Some("127.0.0.1".parse().expect("ip"))
        );
        assert_eq!(worker.pair_token_env, "SLAVE_TOKEN");

        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_mismatched_table_name_and_name_field() {
        let path = write_temp_config(
            r#"
[agents.master]
name = "other"
mode = "master"
pair_token = "MASTER_TOKEN"
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
    fn resolves_selected_agent_and_pair_token_env() {
        let path = write_temp_config(
            r#"
[agents.master]
name = "master"
mode = "master"
pair_token = "FREEHAND_MASTER_TOKEN"
"#,
        );
        let token_name = "FREEHAND_MASTER_TOKEN";
        // SAFETY: test process controls this environment variable in a scoped test.
        unsafe { env::set_var(token_name, "token-value") };

        let config = load_config_from_path(&path).expect("load config");
        let selected = config.select_agent("master").expect("select agent");

        assert_eq!(selected.name, "master");
        assert_eq!(selected.mode, AgentMode::Master);
        assert_eq!(selected.pair_token_env, token_name);
        assert_eq!(selected.pair_token, "token-value");
        assert!(selected.restart_required_on_change);

        // SAFETY: undo the test environment mutation before exit.
        unsafe { env::remove_var(token_name) };
        fs::remove_file(path).expect("cleanup");
    }

    #[test]
    fn rejects_missing_pair_token_env_at_selection_time() {
        let path = write_temp_config(
            r#"
[agents.master]
name = "master"
mode = "master"
pair_token = "FREEHAND_MISSING_TOKEN"
"#,
        );

        let config = load_config_from_path(&path).expect("load config");
        let err = config.select_agent("master").expect_err("should fail");

        assert!(matches!(
            err,
            ConfigError::MissingEnvVar {
                env_var,
                agent_name
            } if env_var == "FREEHAND_MISSING_TOKEN" && agent_name == "master"
        ));

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
}
