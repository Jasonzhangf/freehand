//! Device-local mirror of the last accepted account config document.
//!
//! The mirror is not a config truth owner: it persists only the non-secret
//! schema surface plus server revision/etag and an explicit sync status so a
//! client can show synced / not-configured / conflict / failed without using
//! WebUI local state as truth.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    ACCOUNT_CONFIG_SCHEMA_VERSION, AccountConfigDocument, ConfigDocumentContent,
    validate_config_document,
};

const MIRROR_FILE_NAME: &str = "account-config.mirror.json";

#[derive(Debug, thiserror::Error)]
pub enum AccountConfigMirrorError {
    #[error("account config mirror I/O failed: {0}")]
    Io(String),
    #[error("account config mirror is invalid: {0}")]
    Invalid(String),
    #[error("account config mirror is corrupt: {0}")]
    Corrupt(String),
}

/// Explicit device-side sync status. Missing server documents and transport
/// failures never fall back to a fake "synced" state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountConfigSyncStatus {
    NotConfigured,
    Synced,
    Conflict,
    Failed { message: String },
}

impl AccountConfigSyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::Synced => "synced",
            Self::Conflict => "conflict",
            Self::Failed { .. } => "failed",
        }
    }
}

/// Durable device-side projection of the last accepted server document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountConfigMirror {
    pub schema_version: u32,
    pub account_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    pub status: AccountConfigSyncStatus,
    #[serde(default)]
    pub document: ConfigDocumentContent,
}

impl AccountConfigMirror {
    pub fn synced(
        account_id: impl Into<String>,
        document: AccountConfigDocument,
    ) -> Result<Self, AccountConfigMirrorError> {
        validate_config_document(&document.document)
            .map_err(|error| AccountConfigMirrorError::Invalid(format!("document: {error}")))?;
        Ok(Self {
            schema_version: ACCOUNT_CONFIG_SCHEMA_VERSION,
            account_id: account_id.into(),
            revision: document.revision,
            etag: document.etag,
            updated_at: document.updated_at,
            status: AccountConfigSyncStatus::Synced,
            document: document.document,
        })
    }

    pub fn conflict(
        account_id: impl Into<String>,
        document: AccountConfigDocument,
    ) -> Result<Self, AccountConfigMirrorError> {
        validate_config_document(&document.document).map_err(|error| {
            AccountConfigMirrorError::Invalid(format!("server document: {error}"))
        })?;
        Ok(Self {
            schema_version: ACCOUNT_CONFIG_SCHEMA_VERSION,
            account_id: account_id.into(),
            revision: document.revision,
            etag: document.etag,
            updated_at: document.updated_at,
            status: AccountConfigSyncStatus::Conflict,
            document: document.document,
        })
    }

    pub fn not_configured(account_id: impl Into<String>) -> Self {
        Self {
            schema_version: ACCOUNT_CONFIG_SCHEMA_VERSION,
            account_id: account_id.into(),
            revision: None,
            etag: None,
            updated_at: None,
            status: AccountConfigSyncStatus::NotConfigured,
            document: ConfigDocumentContent::default(),
        }
    }

    pub fn failed(account_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            schema_version: ACCOUNT_CONFIG_SCHEMA_VERSION,
            account_id: account_id.into(),
            revision: None,
            etag: None,
            updated_at: None,
            status: AccountConfigSyncStatus::Failed {
                message: message.into(),
            },
            document: ConfigDocumentContent::default(),
        }
    }

    pub fn load_from_runtime_home(
        runtime_home: &Path,
    ) -> Result<Option<Self>, AccountConfigMirrorError> {
        let path = mirror_path(runtime_home);
        if !path.is_file() {
            return Ok(None);
        }
        let raw = fs::read(&path).map_err(|error| {
            AccountConfigMirrorError::Io(format!("read {}: {error}", path.display()))
        })?;
        let mirror: AccountConfigMirror = serde_json::from_slice(&raw).map_err(|error| {
            AccountConfigMirrorError::Corrupt(format!("{}: {error}", path.display()))
        })?;
        if mirror.schema_version != ACCOUNT_CONFIG_SCHEMA_VERSION {
            return Err(AccountConfigMirrorError::Corrupt(format!(
                "{} unsupported schemaVersion {}",
                path.display(),
                mirror.schema_version
            )));
        }
        if mirror.account_id.trim().is_empty() {
            return Err(AccountConfigMirrorError::Corrupt(format!(
                "{} missing account_id",
                path.display()
            )));
        }
        if mirror.revision.is_some() || mirror.etag.is_some() {
            validate_config_document(&mirror.document).map_err(|error| {
                AccountConfigMirrorError::Corrupt(format!("{}: {error}", path.display()))
            })?;
        }
        Ok(Some(mirror))
    }

    pub fn save_to_runtime_home(
        &self,
        runtime_home: &Path,
    ) -> Result<(), AccountConfigMirrorError> {
        validate_config_document(&self.document)
            .map_err(|error| AccountConfigMirrorError::Invalid(format!("document: {error}")))?;
        let raw = serde_json::to_vec_pretty(self)
            .map_err(|error| AccountConfigMirrorError::Io(error.to_string()))?;
        let path = mirror_path(runtime_home);
        let temp = temporary_path(&path);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| {
                AccountConfigMirrorError::Io(format!("create {}: {error}", temp.display()))
            })?;
        if let Err(error) = file
            .write_all(&raw)
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::rename(&temp, &path))
            .and_then(|()| fs::File::open(runtime_home).and_then(|directory| directory.sync_all()))
        {
            let _ = fs::remove_file(&temp);
            return Err(AccountConfigMirrorError::Io(format!(
                "persist {}: {error}",
                path.display()
            )));
        }
        Ok(())
    }
}

fn mirror_path(runtime_home: &Path) -> PathBuf {
    runtime_home.join(MIRROR_FILE_NAME)
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| MIRROR_FILE_NAME.to_owned());
    name.push_str(&format!(".tmp{}", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigDocumentContent;
    use tempfile::tempdir;

    fn sample_document() -> AccountConfigDocument {
        AccountConfigDocument {
            schema_version: ACCOUNT_CONFIG_SCHEMA_VERSION,
            revision: Some(3),
            etag: Some("\"etag-3\"".to_owned()),
            updated_at: Some("1786500000".to_owned()),
            document: ConfigDocumentContent::default(),
        }
    }

    #[test]
    fn mirror_round_trips_through_runtime_home() {
        let dir = tempdir().expect("temp dir");
        let mirror = AccountConfigMirror::synced("jason", sample_document()).expect("mirror");
        mirror.save_to_runtime_home(dir.path()).expect("save");
        let loaded = AccountConfigMirror::load_from_runtime_home(dir.path())
            .expect("load")
            .expect("mirror exists");
        assert_eq!(loaded.account_id, "jason");
        assert_eq!(loaded.revision, Some(3));
        assert_eq!(loaded.status, AccountConfigSyncStatus::Synced);
    }

    #[test]
    fn missing_mirror_is_explicit_none() {
        let dir = tempdir().expect("temp dir");
        assert!(
            AccountConfigMirror::load_from_runtime_home(dir.path())
                .expect("load")
                .is_none()
        );
    }

    #[test]
    fn corrupt_mirror_fails_explicitly() {
        let dir = tempdir().expect("temp dir");
        fs::write(dir.path().join(MIRROR_FILE_NAME), b"{not json").expect("write");
        assert!(matches!(
            AccountConfigMirror::load_from_runtime_home(dir.path()),
            Err(AccountConfigMirrorError::Corrupt(_))
        ));
    }
}
