use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, ImmutablePayload, PluginId};
use freehand_v2_ui_adaptor::{ProjectionKind, SlotId, UiProjection};
use freehand_v2_ui_plugin_family::{
    InMemoryUiPlugin, UiPluginDefinition, UiPluginError, UiPluginSlotRegistry,
};

fn slot(value: &str) -> SlotId {
    SlotId::try_new(value).expect("slot id")
}

fn capability(value: &str) -> CapabilityId {
    CapabilityId::try_new(value).expect("capability id")
}

fn definition(plugin_id: &str, slot_id: &str, instance_id: &str) -> UiPluginDefinition {
    UiPluginDefinition::try_new(
        PluginId::try_new(plugin_id).expect("plugin id"),
        slot(slot_id),
        instance_id,
        1,
        vec![capability("ui.render")],
    )
    .expect("definition")
}

fn projection(payload: &str) -> (Arc<ImmutablePayload>, UiProjection) {
    let payload = Arc::new(ImmutablePayload::new(payload).expect("payload"));
    let projection = UiProjection::new(
        "projection-1",
        slot("ui.run"),
        ProjectionKind::Run,
        "session-log-owner",
        payload.clone(),
    )
    .expect("projection");
    (payload, projection)
}

#[test]
fn mount_and_render_preserve_arc_payload() {
    let mut registry = UiPluginSlotRegistry::new();
    registry
        .mount(Box::new(
            InMemoryUiPlugin::new(definition("ui-run-a", "ui.run", "instance-a")).expect("plugin"),
        ))
        .expect("mount");
    let (payload, projection) = projection("run-body");
    let view = registry
        .render(&slot("ui.run"), projection, Some("session-1".to_owned()))
        .expect("render");

    assert_eq!(view.definition().plugin_id().as_str(), "ui-run-a");
    assert_eq!(view.selection(), Some("session-1"));
    assert!(Arc::ptr_eq(
        &payload,
        view.projection().expect("projection").payload()
    ));
}

#[test]
fn replace_same_slot_preserves_selection_and_rebuilds_projection() {
    let mut registry = UiPluginSlotRegistry::new();
    registry
        .mount(Box::new(
            InMemoryUiPlugin::new(definition("ui-run-a", "ui.run", "instance-a")).expect("plugin"),
        ))
        .expect("mount");
    let (_, projection) = projection("body");
    registry
        .render(&slot("ui.run"), projection, Some("session-1".to_owned()))
        .expect("render");

    let view = registry
        .replace(Box::new(
            InMemoryUiPlugin::new(definition("ui-run-b", "ui.run", "instance-b")).expect("plugin"),
        ))
        .expect("replace");

    assert_eq!(view.definition().plugin_id().as_str(), "ui-run-b");
    assert_eq!(view.selection(), Some("session-1"));
    assert_eq!(
        view.projection().expect("projection").payload().body(),
        "body"
    );
}

#[test]
fn duplicate_slot_mount_is_rejected() {
    let mut registry = UiPluginSlotRegistry::new();
    registry
        .mount(Box::new(
            InMemoryUiPlugin::new(definition("ui-run-a", "ui.run", "instance-a")).expect("plugin"),
        ))
        .expect("mount");
    let err = registry
        .mount(Box::new(
            InMemoryUiPlugin::new(definition("ui-run-b", "ui.run", "instance-b")).expect("plugin"),
        ))
        .expect_err("duplicate mount should fail");
    assert_eq!(err, UiPluginError::DuplicateSlot("ui.run".to_owned()));
}

#[test]
fn replace_with_different_slot_is_rejected() {
    let mut registry = UiPluginSlotRegistry::new();
    registry
        .mount(Box::new(
            InMemoryUiPlugin::new(definition("ui-run-a", "ui.run", "instance-a")).expect("plugin"),
        ))
        .expect("mount");
    let err = registry
        .replace(Box::new(
            InMemoryUiPlugin::new(definition("ui-sessions-b", "ui.sessions", "instance-b"))
                .expect("plugin"),
        ))
        .expect_err("replace should fail");
    assert_eq!(err, UiPluginError::UnknownSlot("ui.sessions".to_owned()));
}

#[test]
fn render_unknown_slot_fails() {
    let mut registry = UiPluginSlotRegistry::new();
    let (_, projection) = projection("body");
    let err = registry
        .render(&slot("ui.missing"), projection, None)
        .expect_err("render should fail");
    assert_eq!(err, UiPluginError::UnknownSlot("ui.missing".to_owned()));
}

#[test]
fn unmount_unknown_slot_fails() {
    let mut registry = UiPluginSlotRegistry::new();
    let err = registry
        .unmount(&slot("ui.missing"))
        .expect_err("unmount should fail");
    assert_eq!(err, UiPluginError::UnknownSlot("ui.missing".to_owned()));
}

#[test]
fn definition_rejects_empty_capabilities() {
    let err = UiPluginDefinition::try_new(
        PluginId::try_new("p").expect("plugin id"),
        slot("ui.run"),
        "instance",
        1,
        vec![],
    )
    .expect_err("definition should fail");
    assert!(err.to_string().contains("capabilities cannot be empty"));
}
