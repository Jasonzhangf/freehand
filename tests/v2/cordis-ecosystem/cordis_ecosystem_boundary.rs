use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, CorrelationId, ImmutablePayload, PluginId};
use freehand_v2_cordis_ecosystem::{
    CompositionEvent, CordisContext, CordisError, PLUGIN_CONTRACT_VERSION, PluginRegistration,
    PluginRole,
};
use freehand_v2_plugin_capabilities::{CapabilityManifest, LocalCapabilityPlugin};

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

fn payload(value: &str) -> Arc<ImmutablePayload> {
    Arc::new(ImmutablePayload::new(value).unwrap())
}

#[test]
fn cordis_composition_routes_to_capability_and_terminates_correlation() {
    let mut context = CordisContext::new();
    let cap = capability_id("memory.write");
    context
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest(
                "memory",
                "memory.write",
                "ctx:write",
                "ctx:result",
            ))
            .unwrap(),
        ))
        .unwrap();
    let shared = payload("payload");
    let result = context
        .invoke(correlation("c1"), cap.clone(), Arc::clone(&shared))
        .unwrap();

    assert_eq!(result.invocation().capability_id(), &cap);
    assert!(result.invocation().success());
    assert!(Arc::ptr_eq(&shared, result.invocation().payload()));
    assert!(result.event_sequence().contains(&CompositionEvent::Invoked));
    assert!(
        result
            .event_sequence()
            .contains(&CompositionEvent::Completed)
    );
    assert!(context.events().is_terminal(&correlation("c1")));
}

#[test]
fn cordis_composition_unknown_capability_fails_closed() {
    let mut context = CordisContext::new();
    let err = context
        .invoke(correlation("c2"), capability_id("missing"), payload("x"))
        .unwrap_err();
    assert!(err.to_string().contains("unknown capability"));
    assert!(!context.is_in_flight(&correlation("c2")));
}

#[test]
fn capability_failure_is_recorded_and_does_not_leak_in_flight_state() {
    let mut context = CordisContext::new();
    let cap = capability_id("failing");
    context
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p", "failing", "in", "out"))
                .unwrap()
                .fail_next(),
        ))
        .unwrap();

    let err = context
        .invoke(correlation("c3"), cap, payload("x"))
        .unwrap_err();
    assert!(err.to_string().contains("invocation failed"));
    assert!(!context.is_in_flight(&correlation("c3")));
    assert!(!context.events().errors().is_empty());
}

#[test]
fn duplicate_registration_is_rejected_in_cordis_context() {
    let mut context = CordisContext::new();
    context
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p", "dup", "in", "out")).unwrap(),
        ))
        .unwrap();
    let err = context
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p2", "dup", "in", "out")).unwrap(),
        ))
        .unwrap_err();
    assert!(matches!(
        err,
        CordisError::Capability(
            freehand_v2_plugin_capabilities::CapabilityError::DuplicateCapability(_)
        )
    ));
}

#[test]
fn replace_allows_same_identity_new_provider_after_no_in_flight_work() {
    let mut context = CordisContext::new();
    let cap = capability_id("cap");
    context
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("old-provider", "cap", "old-in", "old-out"))
                .unwrap(),
        ))
        .unwrap();
    context
        .invoke(correlation("before"), cap.clone(), payload("one"))
        .unwrap();
    let replaced = context
        .replace_capability(Box::new(
            LocalCapabilityPlugin::new(manifest("new-provider", "cap", "new-in", "new-out"))
                .unwrap(),
        ))
        .unwrap();
    assert_eq!(replaced.plugin_id(), &plugin_id("new-provider"));

    let after = context
        .invoke(correlation("after"), cap, payload("two"))
        .unwrap();
    assert!(after.invocation().success());
    assert_eq!(after.invocation().payload().body(), "two");
}

#[test]
fn replace_missing_capability_fails() {
    let mut context = CordisContext::new();
    let err = context
        .replace_capability(Box::new(
            LocalCapabilityPlugin::new(manifest("p", "missing", "in", "out")).unwrap(),
        ))
        .unwrap_err();
    assert!(err.to_string().contains("unknown capability"));
}

#[test]
fn unload_prevents_future_composition_until_re_registered() {
    let mut context = CordisContext::new();
    let cap = capability_id("cap");
    context
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p", "cap", "in", "out")).unwrap(),
        ))
        .unwrap();
    context.unload(&cap).unwrap();
    let err = context
        .invoke(correlation("c4"), cap, payload("x"))
        .unwrap_err();
    assert!(err.to_string().contains("unknown capability"));
}

#[test]
fn event_ledger_records_plugin_invoked_completed_without_business_payload() {
    let mut context = CordisContext::new();
    context
        .register(Box::new(
            LocalCapabilityPlugin::new(manifest("p", "cap", "in", "out")).unwrap(),
        ))
        .unwrap();
    context
        .invoke(correlation("c5"), capability_id("cap"), payload("secret"))
        .unwrap();

    let records = context.events().events();
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|record| record.payload_ref().is_none()));
    assert!(context.events().is_terminal(&correlation("c5")));
}

#[test]
fn cordis_registers_typed_plugin_roles_in_deterministic_order() {
    let mut context = CordisContext::new();
    context
        .register_plugin(
            PluginRegistration::try_new(
                plugin_id("ui.run"),
                PluginRole::Ui,
                PLUGIN_CONTRACT_VERSION,
            )
            .expect("plugin registration"),
        )
        .expect("register UI plugin");
    context
        .register_plugin(
            PluginRegistration::try_new(
                plugin_id("design.orchestration"),
                PluginRole::Orchestration,
                PLUGIN_CONTRACT_VERSION,
            )
            .expect("plugin registration"),
        )
        .expect("register orchestration plugin");

    let registrations = context.plugin_registrations();
    assert_eq!(registrations.len(), 2);
    assert_eq!(
        registrations[0].plugin_id().as_str(),
        "design.orchestration"
    );
    assert_eq!(registrations[0].role(), PluginRole::Orchestration);
    assert_eq!(
        context
            .plugin_by_id(&plugin_id("ui.run"))
            .expect("registered plugin")
            .role(),
        PluginRole::Ui
    );
}

#[test]
fn duplicate_plugin_id_is_rejected_without_mutating_registry() {
    let mut context = CordisContext::new();
    let first = PluginRegistration::try_new(
        plugin_id("duplicate"),
        PluginRole::Memory,
        PLUGIN_CONTRACT_VERSION,
    )
    .expect("plugin registration");
    let second = PluginRegistration::try_new(
        plugin_id("duplicate"),
        PluginRole::Search,
        PLUGIN_CONTRACT_VERSION,
    )
    .expect("plugin registration");
    context.register_plugin(first).expect("first registration");

    let error = context
        .register_plugin(second)
        .expect_err("duplicate plugin");
    assert!(error.to_string().contains("plugin already registered"));
    assert_eq!(context.plugin_registrations().len(), 1);
    assert_eq!(
        context
            .plugin_by_id(&plugin_id("duplicate"))
            .expect("original plugin")
            .role(),
        PluginRole::Memory
    );
}

#[test]
fn invalid_plugin_registration_is_rejected_before_storage() {
    let context = CordisContext::new();
    let error =
        PluginRegistration::try_new(plugin_id("invalid-version"), PluginRole::NetworkReserved, 0)
            .expect_err("invalid contract version");
    assert!(
        error
            .to_string()
            .contains("unsupported plugin contract version")
    );
    assert!(context.plugin_registrations().is_empty());

    let unknown_role = serde_json::from_value::<PluginRegistration>(serde_json::json!({
        "plugin_id": "unknown-role",
        "role": "UnknownRole",
        "contract_version": 1
    }));
    assert!(unknown_role.is_err());
}
