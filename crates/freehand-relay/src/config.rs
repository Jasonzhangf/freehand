use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use crate::store::RelayStoreError;

pub const RELAY_BIND_ENV: &str = "FREEHAND_RELAY_BIND";
pub const RELAY_STORE_ENV: &str = "FREEHAND_RELAY_STORE";
pub const RELAY_PRESENCE_LEASE_SECONDS_ENV: &str = "FREEHAND_RELAY_PRESENCE_LEASE_SECONDS";
pub const RELAY_SECURE_COOKIE_ENV: &str = "FREEHAND_RELAY_SECURE_COOKIE";
pub const RELAY_UPDATES_DIR_ENV: &str = "FREEHAND_RELAY_UPDATES_DIR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayRuntimeConfig {
    pub store_path: PathBuf,
    pub presence_lease_seconds: u64,
    pub updates_dir: Option<PathBuf>,
}

impl RelayRuntimeConfig {
    pub fn from_env() -> Result<Self, RelayStoreError> {
        let store_path = required_env(RELAY_STORE_ENV).map(PathBuf::from)?;
        let presence_lease_seconds = required_env(RELAY_PRESENCE_LEASE_SECONDS_ENV)?
            .parse::<u64>()
            .map_err(|error| {
                RelayStoreError::Invalid(format!(
                    "{RELAY_PRESENCE_LEASE_SECONDS_ENV} is invalid: {error}"
                ))
            })?;
        if presence_lease_seconds == 0 {
            return Err(RelayStoreError::Invalid(format!(
                "{RELAY_PRESENCE_LEASE_SECONDS_ENV} must be positive"
            )));
        }
        let updates_dir = optional_env(RELAY_UPDATES_DIR_ENV)?.map(PathBuf::from);
        if let Some(path) = updates_dir.as_ref()
            && path.as_os_str().is_empty()
        {
            return Err(RelayStoreError::Invalid(format!(
                "{RELAY_UPDATES_DIR_ENV} must not be empty when set"
            )));
        }
        Ok(Self {
            store_path,
            presence_lease_seconds,
            updates_dir,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayServerConfig {
    pub bind: SocketAddr,
    pub runtime: RelayRuntimeConfig,
    pub secure_cookie: bool,
}

impl RelayServerConfig {
    pub fn from_env() -> Result<Self, RelayStoreError> {
        let bind = required_env(RELAY_BIND_ENV)?
            .parse::<SocketAddr>()
            .map_err(|error| {
                RelayStoreError::Invalid(format!("{RELAY_BIND_ENV} is invalid: {error}"))
            })?;
        Ok(Self {
            bind,
            runtime: RelayRuntimeConfig::from_env()?,
            secure_cookie: parse_required_bool(
                RELAY_SECURE_COOKIE_ENV,
                required_env(RELAY_SECURE_COOKIE_ENV)?,
            )?,
        })
    }
}

fn parse_required_bool(name: &str, value: String) -> Result<bool, RelayStoreError> {
    match value.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(RelayStoreError::Invalid(format!(
            "{name} must be `true` or `false`"
        ))),
    }
}

fn required_env(name: &str) -> Result<String, RelayStoreError> {
    env::var(name).map_err(|error| match error {
        env::VarError::NotPresent => RelayStoreError::Invalid(format!("{name} is required")),
        env::VarError::NotUnicode(_) => {
            RelayStoreError::Invalid(format!("{name} is not valid Unicode"))
        }
    })
}

fn optional_env(name: &str) -> Result<Option<String>, RelayStoreError> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(RelayStoreError::Invalid(format!(
            "{name} is not valid Unicode"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_cookie_accepts_only_explicit_boolean_values() {
        assert!(parse_required_bool(RELAY_SECURE_COOKIE_ENV, "true".to_owned()).expect("true"));
        assert!(!parse_required_bool(RELAY_SECURE_COOKIE_ENV, "false".to_owned()).expect("false"));
        assert!(
            parse_required_bool(RELAY_SECURE_COOKIE_ENV, "1".to_owned())
                .expect_err("ambiguous value must fail")
                .to_string()
                .contains("must be `true` or `false`")
        );
    }
}
