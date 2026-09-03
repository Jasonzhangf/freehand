use std::collections::HashMap;

use freehand_v2_contracts::PluginId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NotificationError {
    #[error("notification id cannot be empty")]
    EmptyId,
    #[error("source plugin id cannot be empty")]
    EmptySource,
    #[error("duplicate notification id: {0}")]
    Duplicate(String),
    #[error("unknown notification id: {0}")]
    Unknown(String),
    #[error("notification already terminal: {0}")]
    AlreadyTerminal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Importance {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationState {
    Unread,
    Read,
    Acknowledged,
    Snoozed,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationItem {
    notification_id: String,
    source: PluginId,
    importance: Importance,
    occurred_at: u64,
    state: NotificationState,
    payload_ref: Option<String>,
}

impl NotificationItem {
    pub fn new(
        notification_id: impl Into<String>,
        source: PluginId,
        importance: Importance,
        occurred_at: u64,
        payload_ref: Option<String>,
    ) -> Result<Self, NotificationError> {
        let notification_id = notification_id.into();
        if notification_id.is_empty() {
            return Err(NotificationError::EmptyId);
        }
        Ok(Self {
            notification_id,
            source,
            importance,
            occurred_at,
            state: NotificationState::Unread,
            payload_ref,
        })
    }

    pub fn notification_id(&self) -> &str {
        &self.notification_id
    }

    pub fn source(&self) -> &PluginId {
        &self.source
    }

    pub fn importance(&self) -> Importance {
        self.importance
    }

    pub fn occurred_at(&self) -> u64 {
        self.occurred_at
    }

    pub fn state(&self) -> NotificationState {
        self.state
    }

    pub fn payload_ref(&self) -> Option<&str> {
        self.payload_ref.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationProjection {
    revision: u64,
    items: Vec<NotificationItem>,
}

impl NotificationProjection {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn items(&self) -> &[NotificationItem] {
        &self.items
    }
}

#[derive(Default)]
pub struct NotificationPlugin {
    items: HashMap<String, NotificationItem>,
    revision: u64,
}

impl NotificationPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn admit(
        &mut self,
        notification_id: impl Into<String>,
        source: PluginId,
        importance: Importance,
        occurred_at: u64,
        payload_ref: Option<String>,
    ) -> Result<NotificationItem, NotificationError> {
        let item = NotificationItem::new(
            notification_id,
            source,
            importance,
            occurred_at,
            payload_ref,
        )?;
        if self.items.contains_key(item.notification_id()) {
            return Err(NotificationError::Duplicate(
                item.notification_id().to_owned(),
            ));
        }
        let id = item.notification_id().to_owned();
        self.items.insert(id, item.clone());
        self.revision += 1;
        Ok(item)
    }

    pub fn rank(&self) -> Vec<NotificationItem> {
        let mut items: Vec<_> = self.items.values().cloned().collect();
        items.sort_by(|a, b| {
            b.importance()
                .cmp(&a.importance())
                .then_with(|| b.occurred_at().cmp(&a.occurred_at()))
                .then_with(|| a.notification_id().cmp(b.notification_id()))
        });
        items
    }

    pub fn publish(&self) -> NotificationProjection {
        NotificationProjection {
            revision: self.revision,
            items: self.rank(),
        }
    }

    pub fn acknowledge(&mut self, notification_id: &str) -> Result<(), NotificationError> {
        self.transition(notification_id, NotificationState::Acknowledged)
    }

    pub fn snooze(&mut self, notification_id: &str) -> Result<(), NotificationError> {
        self.transition(notification_id, NotificationState::Snoozed)
    }

    pub fn archive(&mut self, notification_id: &str) -> Result<(), NotificationError> {
        self.transition(notification_id, NotificationState::Archived)
    }

    pub fn get(&self, notification_id: &str) -> Option<&NotificationItem> {
        self.items.get(notification_id)
    }

    pub fn unread_count(&self) -> usize {
        self.items
            .values()
            .filter(|item| item.state() == NotificationState::Unread)
            .count()
    }

    fn transition(
        &mut self,
        notification_id: &str,
        next: NotificationState,
    ) -> Result<(), NotificationError> {
        let item = self
            .items
            .get_mut(notification_id)
            .ok_or_else(|| NotificationError::Unknown(notification_id.to_owned()))?;
        if item.state() == NotificationState::Archived
            || (next != NotificationState::Archived
                && item.state() == NotificationState::Acknowledged)
        {
            return Err(NotificationError::AlreadyTerminal(
                notification_id.to_owned(),
            ));
        }
        item.state = next;
        self.revision += 1;
        Ok(())
    }
}
