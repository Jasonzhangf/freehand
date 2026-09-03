use freehand_v2_contracts::PluginId;
use freehand_v2_notification_plugin::{Importance, NotificationError, NotificationPlugin};

fn source(name: &str) -> PluginId {
    PluginId::try_new(name).expect("plugin id")
}

#[test]
fn ranking_is_importance_then_time_then_id() {
    let mut plugin = NotificationPlugin::new();
    plugin
        .admit("n1", source("a"), Importance::Medium, 10, None)
        .expect("admit");
    plugin
        .admit("n2", source("a"), Importance::High, 20, None)
        .expect("admit");
    plugin
        .admit("n3", source("a"), Importance::Critical, 5, None)
        .expect("admit");

    let projection = plugin.publish();
    assert_eq!(projection.items()[0].notification_id(), "n3");
    assert_eq!(projection.items()[1].notification_id(), "n2");
    assert_eq!(projection.items()[2].notification_id(), "n1");
}

#[test]
fn ack_updates_projection_without_mutating_source_identity() {
    let mut plugin = NotificationPlugin::new();
    plugin
        .admit("n1", source("task-owner"), Importance::High, 1, None)
        .expect("admit");
    plugin.acknowledge("n1").expect("ack");

    let item = plugin.get("n1").expect("item");
    assert_eq!(item.source().as_str(), "task-owner");
    assert_eq!(
        item.state(),
        freehand_v2_notification_plugin::NotificationState::Acknowledged
    );
}

#[test]
fn duplicate_admit_is_rejected() {
    let mut plugin = NotificationPlugin::new();
    plugin
        .admit("n1", source("a"), Importance::Low, 1, None)
        .expect("admit");
    let err = plugin
        .admit("n1", source("a"), Importance::Low, 1, None)
        .expect_err("duplicate");
    assert_eq!(err, NotificationError::Duplicate("n1".to_owned()));
}

#[test]
fn unknown_ack_is_rejected() {
    let mut plugin = NotificationPlugin::new();
    let err = plugin.acknowledge("missing").expect_err("ack");
    assert_eq!(err, NotificationError::Unknown("missing".to_owned()));
}

#[test]
fn empty_id_is_rejected() {
    let mut plugin = NotificationPlugin::new();
    let err = plugin
        .admit("", source("a"), Importance::Low, 1, None)
        .expect_err("empty");
    assert_eq!(err, NotificationError::EmptyId);
}
