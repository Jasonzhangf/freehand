use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand::RngCore;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::{AgentDirectory, AgentHeartbeat, AgentPresence, AuthResponse};

const TOKEN_BYTES: usize = 32;
const STORE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum RelayStoreError {
    #[error("invalid relay request: {0}")]
    Invalid(String),
    #[error("relay account already exists")]
    Conflict,
    #[error("relay store already exists")]
    StoreAlreadyExists,
    #[error("invalid relay username or password")]
    Unauthorized,
    #[error("relay agent was not found")]
    AgentNotFound,
    #[error("relay store I/O failed: {0}")]
    Io(String),
    #[error("relay store data is corrupt: {0}")]
    Corrupt(String),
    #[error("relay upstream failed: {0}")]
    Upstream(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountRecord {
    account_id: String,
    username: String,
    password_hash: String,
    created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenRecord {
    token_hash: String,
    account_id: String,
    created_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentRecord {
    account_id: String,
    heartbeat: AgentHeartbeat,
    last_seen_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoreData {
    schema_version: u32,
    accounts: BTreeMap<String, AccountRecord>,
    tokens: BTreeMap<String, TokenRecord>,
    agents: BTreeMap<String, AgentRecord>,
}

impl StoreData {
    fn empty() -> Self {
        Self {
            schema_version: STORE_SCHEMA_VERSION,
            accounts: BTreeMap::new(),
            tokens: BTreeMap::new(),
            agents: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct RelayStore {
    path: PathBuf,
    data: StoreData,
}

impl RelayStore {
    pub fn initialize(path: impl Into<PathBuf>) -> Result<Self, RelayStoreError> {
        let path = path.into();
        let data = StoreData::empty();
        initialize_data(&path, &data)?;
        Ok(Self { path, data })
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self, RelayStoreError> {
        let path = path.into();
        if !path.is_file() {
            return Err(RelayStoreError::Io(format!(
                "store file does not exist: {}",
                path.display()
            )));
        }
        let raw = fs::read(&path).map_err(|error| RelayStoreError::Io(error.to_string()))?;
        let data: StoreData = serde_json::from_slice(&raw)
            .map_err(|error| RelayStoreError::Corrupt(error.to_string()))?;
        if data.schema_version != STORE_SCHEMA_VERSION {
            return Err(RelayStoreError::Corrupt(format!(
                "unsupported schemaVersion {}; expected {STORE_SCHEMA_VERSION}",
                data.schema_version
            )));
        }
        Ok(Self { path, data })
    }

    pub fn register(
        &mut self,
        username: String,
        password: String,
        now_unix: u64,
    ) -> Result<AuthResponse, RelayStoreError> {
        let username = normalize_username(username)?;
        validate_password(&password)?;
        if self
            .data
            .accounts
            .values()
            .any(|account| account.username == username)
        {
            return Err(RelayStoreError::Conflict);
        }
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|error| RelayStoreError::Io(error.to_string()))?
            .to_string();
        let account_id = format!("account-{}", random_hex(16));
        let mut candidate = self.data.clone();
        candidate.accounts.insert(
            account_id.clone(),
            AccountRecord {
                account_id: account_id.clone(),
                username: username.clone(),
                password_hash,
                created_at_unix: now_unix,
            },
        );
        let response = issue_token(&mut candidate, account_id, username, now_unix);
        self.commit(candidate)?;
        Ok(response)
    }

    pub fn login(
        &mut self,
        username: String,
        password: String,
        now_unix: u64,
    ) -> Result<AuthResponse, RelayStoreError> {
        let username = normalize_username(username)?;
        let account = self
            .data
            .accounts
            .values()
            .find(|account| account.username == username)
            .cloned()
            .ok_or(RelayStoreError::Unauthorized)?;
        let parsed = PasswordHash::new(&account.password_hash)
            .map_err(|error| RelayStoreError::Corrupt(error.to_string()))?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| RelayStoreError::Unauthorized)?;
        let mut candidate = self.data.clone();
        let response = issue_token(
            &mut candidate,
            account.account_id,
            account.username,
            now_unix,
        );
        self.commit(candidate)?;
        Ok(response)
    }

    pub fn authenticate(&self, token: &str) -> Result<String, RelayStoreError> {
        let token_hash = hash_token(token);
        self.data
            .tokens
            .get(&token_hash)
            .map(|record| record.account_id.clone())
            .ok_or(RelayStoreError::Unauthorized)
    }

    pub fn heartbeat(
        &mut self,
        account_id: String,
        heartbeat: AgentHeartbeat,
        now_unix: u64,
    ) -> Result<AgentPresence, RelayStoreError> {
        validate_heartbeat(&heartbeat)?;
        let key = agent_key(&account_id, &heartbeat.agent_id);
        let mut candidate = self.data.clone();
        candidate.agents.insert(
            key,
            AgentRecord {
                account_id,
                heartbeat: heartbeat.clone(),
                last_seen_unix: now_unix,
            },
        );
        self.commit(candidate)?;
        Ok(project_presence(&heartbeat, now_unix, now_unix, u64::MAX))
    }

    pub fn directory(&self, account_id: &str, now_unix: u64, lease_seconds: u64) -> AgentDirectory {
        let mut agents = self
            .data
            .agents
            .values()
            .filter(|record| record.account_id == account_id)
            .map(|record| {
                project_presence(
                    &record.heartbeat,
                    record.last_seen_unix,
                    now_unix,
                    lease_seconds,
                )
            })
            .collect::<Vec<_>>();
        agents.sort_by(|left, right| {
            right
                .online
                .cmp(&left.online)
                .then_with(|| left.display_name.cmp(&right.display_name))
                .then_with(|| left.agent_id.cmp(&right.agent_id))
        });
        AgentDirectory {
            account_id: account_id.to_owned(),
            generated_at_unix: now_unix,
            agents,
        }
    }

    pub fn disconnect(&mut self, account_id: &str, agent_id: &str) -> Result<(), RelayStoreError> {
        let mut candidate = self.data.clone();
        if let Some(record) = candidate.agents.get_mut(&agent_key(account_id, agent_id)) {
            record.last_seen_unix = 0;
        }
        self.commit(candidate)
    }

    fn commit(&mut self, candidate: StoreData) -> Result<(), RelayStoreError> {
        persist_data(&self.path, &candidate)?;
        self.data = candidate;
        Ok(())
    }
}

fn issue_token(
    data: &mut StoreData,
    account_id: String,
    username: String,
    now_unix: u64,
) -> AuthResponse {
    let access_token = random_hex(TOKEN_BYTES);
    let token_hash = hash_token(&access_token);
    data.tokens.insert(
        token_hash.clone(),
        TokenRecord {
            token_hash,
            account_id: account_id.clone(),
            created_at_unix: now_unix,
        },
    );
    AuthResponse {
        access_token,
        account_id,
        username,
    }
}

fn persist_data(path: &Path, data: &StoreData) -> Result<(), RelayStoreError> {
    let parent = path
        .parent()
        .ok_or_else(|| RelayStoreError::Io("store path has no parent".to_owned()))?;
    fs::create_dir_all(parent).map_err(|error| RelayStoreError::Io(error.to_string()))?;
    let bytes =
        serde_json::to_vec_pretty(data).map_err(|error| RelayStoreError::Io(error.to_string()))?;
    let temp_path = temporary_path(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|error| RelayStoreError::Io(error.to_string()))?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| RelayStoreError::Io(error.to_string()))?;
    drop(file);
    if let Err(rename_error) = fs::rename(&temp_path, path) {
        return match fs::remove_file(&temp_path) {
            Ok(()) => Err(RelayStoreError::Io(rename_error.to_string())),
            Err(cleanup_error) => Err(RelayStoreError::Io(format!(
                "{rename_error}; failed to remove temp store {}: {cleanup_error}",
                temp_path.display()
            ))),
        };
    }
    sync_parent(parent)
}

fn initialize_data(path: &Path, data: &StoreData) -> Result<(), RelayStoreError> {
    let parent = path
        .parent()
        .ok_or_else(|| RelayStoreError::Io("store path has no parent".to_owned()))?;
    fs::create_dir_all(parent).map_err(|error| RelayStoreError::Io(error.to_string()))?;
    let bytes =
        serde_json::to_vec_pretty(data).map_err(|error| RelayStoreError::Io(error.to_string()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                RelayStoreError::StoreAlreadyExists
            } else {
                RelayStoreError::Io(error.to_string())
            }
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| RelayStoreError::Io(error.to_string()))?;
    sync_parent(parent)
}

fn sync_parent(parent: &Path) -> Result<(), RelayStoreError> {
    let directory =
        fs::File::open(parent).map_err(|error| RelayStoreError::Io(error.to_string()))?;
    directory
        .sync_all()
        .map_err(|error| RelayStoreError::Io(error.to_string()))
}

fn normalize_username(value: String) -> Result<String, RelayStoreError> {
    let value = value.trim().to_lowercase();
    if value.is_empty() || value.len() > 128 {
        return Err(RelayStoreError::Invalid(
            "username must contain 1 to 128 characters".to_owned(),
        ));
    }
    Ok(value)
}

fn validate_password(value: &str) -> Result<(), RelayStoreError> {
    if !(12..=1024).contains(&value.len()) {
        return Err(RelayStoreError::Invalid(
            "password must contain 12 to 1024 characters".to_owned(),
        ));
    }
    Ok(())
}

fn validate_heartbeat(heartbeat: &AgentHeartbeat) -> Result<(), RelayStoreError> {
    for (field, value) in [
        ("agentId", heartbeat.agent_id.trim()),
        ("displayName", heartbeat.display_name.trim()),
        ("nodeId", heartbeat.node_id.trim()),
    ] {
        if value.is_empty() || value.len() > 128 {
            return Err(RelayStoreError::Invalid(format!(
                "{field} must contain 1 to 128 characters"
            )));
        }
    }
    Ok(())
}

fn project_presence(
    heartbeat: &AgentHeartbeat,
    last_seen_unix: u64,
    now_unix: u64,
    lease_seconds: u64,
) -> AgentPresence {
    AgentPresence {
        agent_id: heartbeat.agent_id.clone(),
        display_name: heartbeat.display_name.clone(),
        node_id: heartbeat.node_id.clone(),
        role: heartbeat.role,
        status: heartbeat.status,
        active_session_count: heartbeat.active_session_count,
        last_seen_unix,
        online: now_unix.saturating_sub(last_seen_unix) <= lease_seconds,
    }
}

fn agent_key(account_id: &str, agent_id: &str) -> String {
    format!("{account_id}\u{1f}{agent_id}")
}

fn random_hex(bytes: usize) -> String {
    let mut value = vec![0_u8; bytes];
    OsRng.fill_bytes(&mut value);
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!("tmp-{}", random_hex(8)))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::model::{AgentRole, AgentWorkStatus};

    fn heartbeat(agent_id: &str) -> AgentHeartbeat {
        AgentHeartbeat {
            agent_id: agent_id.to_owned(),
            display_name: "Studio Master".to_owned(),
            node_id: "node-studio".to_owned(),
            role: AgentRole::Master,
            status: AgentWorkStatus::Running,
            active_session_count: 2,
        }
    }

    #[test]
    fn account_token_and_presence_survive_restart() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("relay.json");
        let mut store = RelayStore::initialize(&path).expect("initialize");
        let auth = store
            .register("Jason".to_owned(), "relay-password-123".to_owned(), 10)
            .expect("register");
        let account_id = store
            .authenticate(&auth.access_token)
            .expect("authenticate");
        store
            .heartbeat(account_id.clone(), heartbeat("studio"), 20)
            .expect("heartbeat");
        drop(store);

        let store = RelayStore::load(&path).expect("reload");
        assert_eq!(
            store.authenticate(&auth.access_token).expect("token"),
            account_id
        );
        let directory = store.directory(&account_id, 21, 45);
        assert_eq!(directory.agents.len(), 1);
        assert!(directory.agents[0].online);
        assert_eq!(directory.agents[0].active_session_count, 2);
        assert!(
            !fs::read_to_string(path)
                .expect("store text")
                .contains(&auth.access_token)
        );
    }

    #[test]
    fn wrong_password_cross_account_and_expired_presence_are_rejected() {
        let dir = tempdir().expect("tempdir");
        let mut store = RelayStore::initialize(dir.path().join("relay.json")).expect("initialize");
        let first = store
            .register("first".to_owned(), "relay-password-123".to_owned(), 10)
            .expect("first");
        let second = store
            .register("second".to_owned(), "relay-password-456".to_owned(), 11)
            .expect("second");
        assert!(matches!(
            store.login("first".to_owned(), "wrong-password".to_owned(), 12),
            Err(RelayStoreError::Unauthorized)
        ));
        store
            .heartbeat(first.account_id.clone(), heartbeat("studio"), 20)
            .expect("heartbeat");
        assert!(
            store
                .directory(&second.account_id, 21, 45)
                .agents
                .is_empty()
        );
        assert!(!store.directory(&first.account_id, 66, 45).agents[0].online);
        store
            .disconnect(&first.account_id, "studio")
            .expect("disconnect");
        assert!(!store.directory(&first.account_id, 66, 45).agents[0].online);
    }

    #[test]
    fn failed_persistence_does_not_publish_candidate_truth() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("relay.json");
        let mut store = RelayStore::initialize(&path).expect("initialize");
        store.path = dir.path().to_path_buf();

        assert!(
            store
                .register("owner".to_owned(), "relay-password-123".to_owned(), 10)
                .is_err()
        );
        assert!(store.data.accounts.is_empty());
        assert!(store.data.tokens.is_empty());
        assert!(store.data.agents.is_empty());

        store.path = path;
        store
            .register("owner".to_owned(), "relay-password-123".to_owned(), 11)
            .expect("retry after restoring persistence path");
    }
}
