use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, CorrelationId, ImmutablePayload, PluginId};
use freehand_v2_plugin_capabilities::{
    CapabilityError, CapabilityManifest, CapabilityRegistry, LocalCapabilityPlugin,
    MANIFEST_VERSION,
};

fn plugin_id(value: &str) -> PluginId {
    PluginId::try_new(value).unwrap()
}

fn capability_id(value: &str) -> CapabilityId {
    CapabilityId::try_new(value).unwrap()
}

fn correlation(value: &str) -> CorrelationId {
    CorrelationId::try_new(value).unwrap()
}

fn manifest(plugin: &str, capability: &str, input: &str, output: &str) -> CapabilityManifest {
    CapabilityManifest::try_new(
        plugin_id(plugin),
        capability_id(capability),
        input,
        output,
        vec!["capability.result".to_owned()],
        vec!["capability.invoke".to_owned()],
        None,
    )
    .unwrap()
}

#[test]
fn register_and_invoke_local_capability_succeeds() {
    let mut registry = CapabilityRegistry::new();
    let cap = capability_id("mem.write");
    let registered = registry
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("memory", "mem.write", "ctx:write", "ctx:result"))
                .unwrap(),
        ))
        .unwrap();
    assert_eq!(registered.capability_id(), &cap);

    let plugin = registry.get(&cap).unwrap();
    let payload = Arc::new(ImmutablePayload::new("note").unwrap());
    let result = plugin.invoke(&correlation("c1"), payload).unwrap();
    assert!(result.success());
    assert_eq!(result.capability_id(), &cap);
    assert_eq!(result.payload().body(), "note");
}

#[test]
fn invocation_preserves_arc_payload_identity() {
    let mut registry = CapabilityRegistry::new();
    let cap = capability_id("cap-one");
    registry
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p", "cap-one", "input", "output")).unwrap(),
        ))
        .unwrap();

    let shared = Arc::new(ImmutablePayload::new("shared").unwrap());
    let result = registry
        .get(&cap)
        .unwrap()
        .invoke(&correlation("c"), Arc::clone(&shared))
        .unwrap();
    assert!(Arc::ptr_eq(&shared, result.payload()));
}

#[test]
fn duplicate_registration_is_rejected() {
    let mut registry = CapabilityRegistry::new();
    registry
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p", "dup", "in", "out")).unwrap(),
        ))
        .unwrap();
    let err = registry
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p2", "dup", "in", "out")).unwrap(),
        ))
        .unwrap_err();
    assert_eq!(err, CapabilityError::DuplicateCapability("dup".to_owned()));
}

#[test]
fn failing_plugin_returns_explicit_failure_without_result() {
    let mut registry = CapabilityRegistry::new();
    let cap = capability_id("fail");
    let plugin = LocalCapabilityPlugin::new(manifest("p", "fail", "in", "out"))
        .unwrap()
        .fail_next();
    registry.register(Box::new(plugin)).unwrap();
    let err = registry
        .get(&cap)
        .unwrap()
        .invoke(
            &correlation("c"),
            Arc::new(ImmutablePayload::new("x").unwrap()),
        )
        .unwrap_err();
    assert_eq!(err, CapabilityError::InvocationFailed("fail".to_owned()));
}

#[test]
fn unload_removes_plugin_and_unknown_get_fails() {
    let mut registry = CapabilityRegistry::new();
    let cap = capability_id("cap");
    registry
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p", "cap", "in", "out")).unwrap(),
        ))
        .unwrap();
    registry.unload(&cap).unwrap();
    assert!(!registry.contains(&cap));
    assert!(registry.get(&cap).is_none());
}

#[test]
fn unload_unknown_capability_fails() {
    let mut registry = CapabilityRegistry::new();
    let err = registry.unload(&capability_id("missing")).unwrap_err();
    assert_eq!(
        err,
        CapabilityError::UnknownCapability("missing".to_owned())
    );
}

#[test]
fn unsupported_manifest_version_is_rejected() {
    assert_eq!(
        CapabilityManifest::validate_version(MANIFEST_VERSION + 1).unwrap_err(),
        CapabilityError::UnsupportedManifestVersion(MANIFEST_VERSION + 1)
    );
}

#[test]
fn empty_contracts_are_rejected() {
    let err = CapabilityManifest::try_new(
        plugin_id("p"),
        capability_id("c"),
        "",
        "out",
        vec![],
        vec![],
        None,
    )
    .unwrap_err();
    assert!(matches!(err, CapabilityError::InvalidManifest(_)));

    let err = CapabilityManifest::try_new(
        plugin_id("p"),
        capability_id("c"),
        "in",
        "",
        vec![],
        vec![],
        None,
    )
    .unwrap_err();
    assert!(matches!(err, CapabilityError::InvalidManifest(_)));
}

#[test]
fn registry_length_and_empty_state_are_deterministic() {
    let mut registry = CapabilityRegistry::new();
    assert!(registry.is_empty());
    registry
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p", "cap", "in", "out")).unwrap(),
        ))
        .unwrap();
    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
}

#[test]
fn replace_existing_capability_updates_contract_and_keeps_identity() {
    let mut registry = CapabilityRegistry::new();
    let cap = capability_id("cap");
    registry
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p1", "cap", "old-in", "old-out")).unwrap(),
        ))
        .unwrap();
    let replaced = registry
        .replace(Box::new(
            LocalCapabilityPlugin::new(manifest("p2", "cap", "new-in", "new-out")).unwrap(),
        ))
        .unwrap();
    assert_eq!(replaced.plugin_id(), &plugin_id("p2"));
    assert_eq!(replaced.input_contract(), "new-in");
    assert_eq!(
        registry.get(&cap).unwrap().manifest().plugin_id(),
        &plugin_id("p2")
    );
}

#[test]
fn replace_missing_capability_fails() {
    let mut registry = CapabilityRegistry::new();
    let err = registry
        .replace(Box::new(
            LocalCapabilityPlugin::new(manifest("p", "missing", "in", "out")).unwrap(),
        ))
        .unwrap_err();
    assert_eq!(
        err,
        CapabilityError::UnknownCapability("missing".to_owned())
    );
}
