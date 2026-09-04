use std::sync::Arc;

use freehand_v2_channel_registry::{BearerToken, ChannelRegistry};
use freehand_v2_contracts::{
    CapabilityId, CorrelationId, ImmutablePayload, NodeId, PayloadFrame, PluginId, SessionId,
    UiCommand, UiCommandWire, WireFrame,
};
use freehand_v2_memory_plugin::{MemoryPlugin, MemoryRecord};
use freehand_v2_public_vertical_slice::{PublicVerticalSlice, VerticalSliceError};
use freehand_v2_search_plugin::{SearchPlugin, SearchRecord};
use freehand_v2_ui_adaptor::{SlotId, UiCommandReceiptStatus};

fn session_id() -> SessionId {
    SessionId::try_new("m8-session").expect("session id")
}

fn correlation() -> CorrelationId {
    CorrelationId::try_new("m8-correlation").expect("correlation id")
}

fn capability() -> CapabilityId {
    CapabilityId::try_new("local.capability").expect("capability id")
}

fn command() -> UiCommand {
    let payload = ImmutablePayload::new("v2-blackbox-payload-001").expect("payload");
    UiCommand::new(correlation(), session_id(), capability(), payload)
}

#[test]
fn vertical_slice_success_preserves_arc_payload_and_isolates_control() {
    let mut slice = PublicVerticalSlice::new();
    let cmd = command();
    let payload_arc = Arc::clone(cmd.payload().arc());
    let outcome = slice.submit(cmd).expect("successful vertical slice");

    assert_eq!(outcome.receipt().status(), UiCommandReceiptStatus::Accepted);
    assert!(Arc::ptr_eq(&payload_arc, outcome.payload_arc().arc()));
    assert_eq!(outcome.payload_arc().body(), "v2-blackbox-payload-001");

    let events = slice.control_events();
    assert!(
        events
            .iter()
            .any(|event| event.kind() == freehand_v2_contracts::ControlKind::PluginInvoked)
    );
    assert!(
        events
            .iter()
            .any(|event| event.kind() == freehand_v2_contracts::ControlKind::PluginCompleted)
    );

    let session_events = slice.session_events(&session_id());
    let kinds: Vec<&str> = session_events
        .iter()
        .map(|event| match event.kind() {
            freehand_v2_sessionlog::EventKind::Input => "input",
            freehand_v2_sessionlog::EventKind::Surface => "surface",
            freehand_v2_sessionlog::EventKind::Result => "result",
            freehand_v2_sessionlog::EventKind::Recovery => "recovery",
        })
        .collect();
    assert!(kinds.contains(&"input"));
    assert!(kinds.contains(&"surface"));
    assert!(kinds.contains(&"result"));

    let stored = slice
        .query_projection(&SlotId::try_new("run").expect("slot"))
        .expect("stored projection");
    assert!(Arc::ptr_eq(
        stored.payload(),
        outcome.projection().payload()
    ));

    let wire = outcome.projection().to_wire();
    let json = freehand_v2_ui_adaptor::projection_wire_to_json(&wire);
    assert_eq!(json["payload_body"], "v2-blackbox-payload-001:result");
    assert!(json.get("control").is_none());
    assert!(json.get("event_id").is_none());
}

#[test]
fn vertical_slice_capability_failure_is_explicit_and_does_not_project_success() {
    let mut slice = PublicVerticalSlice::with_failing_capability();
    let err = slice.submit(command()).expect_err("failing capability");
    assert!(matches!(err, VerticalSliceError::Capability(_)));
    assert_eq!(
        slice.projection_count(&SlotId::try_new("run").expect("slot")),
        0
    );
}

#[test]
fn vertical_slice_waiting_then_already_terminal() {
    let mut slice = PublicVerticalSlice::new();
    let waiting = slice.begin(command()).expect("waiting turn");
    assert!(waiting.is_waiting());

    let outcome = slice.resume(&correlation()).expect("resume turn");
    assert_eq!(outcome.receipt().status(), UiCommandReceiptStatus::Accepted);

    let err = slice.submit(command()).expect_err("already terminal");
    assert!(matches!(err, VerticalSliceError::AlreadyTerminal(_)));
}

#[test]
fn vertical_slice_ui_plugin_replacement_preserves_selection() {
    use freehand_v2_ui_plugin_family::{
        InMemoryUiPlugin, UiPluginDefinition, UiPluginSlotRegistry, UiPluginState,
    };

    let slot = SlotId::try_new("run").expect("slot");
    let plugin_id = PluginId::try_new("ui.run").expect("plugin id");
    let definition = UiPluginDefinition::try_new(
        plugin_id.clone(),
        slot.clone(),
        "run-v1",
        1,
        vec![capability()],
    )
    .expect("definition");
    let mut registry = UiPluginSlotRegistry::new();
    registry
        .mount(Box::new(InMemoryUiPlugin::new(definition).expect("plugin")))
        .expect("mount");

    let mut slice = PublicVerticalSlice::new();
    let outcome = slice.submit(command()).expect("submit");
    registry
        .render(
            &slot,
            outcome.projection().clone(),
            Some("session-m8".to_owned()),
        )
        .expect("render");

    let replacement = UiPluginDefinition::try_new(
        PluginId::try_new("ui.run.next").expect("plugin id"),
        slot.clone(),
        "run-v2",
        1,
        vec![capability()],
    )
    .expect("definition");
    let replaced = registry
        .replace(Box::new(
            InMemoryUiPlugin::new(replacement).expect("plugin"),
        ))
        .expect("replace");
    assert_eq!(replaced.state(), UiPluginState::Ready);
    assert_eq!(replaced.selection(), Some("session-m8"));
}

#[test]
fn vertical_slice_channel_connection_replacement_retains_channel_session() {
    let mut registry = ChannelRegistry::new();
    let token = BearerToken::try_new("m8-token").expect("token");
    registry
        .register(
            "endpoint-a",
            NodeId::try_new("node-a").expect("node id"),
            token.clone(),
            1,
            vec![capability()],
        )
        .expect("endpoint");
    registry
        .open_session("m8-channel-session", "endpoint-a", &token)
        .expect("open");
    registry
        .attach_connection("m8-channel-session", "conn-1")
        .expect("attach");

    let session_before = registry.session("m8-channel-session").expect("session");
    assert_eq!(session_before.event_count(), 0);

    registry
        .send(
            "m8-channel-session",
            freehand_v2_channel_registry::FrameKind::Control,
            correlation(),
            Some("payload-ref".to_owned()),
            None,
        )
        .expect("send");
    let replacement = registry
        .replace_connection("m8-channel-session", "conn-2")
        .expect("replace connection");
    assert_eq!(replacement.connection_id(), "conn-2");

    let session_after = registry.session("m8-channel-session").expect("session");
    assert_eq!(session_after.event_count(), 1);
    assert_eq!(session_after.generation(), 2);
    assert_eq!(
        session_after.state(),
        freehand_v2_channel_registry::ChannelSessionState::Open
    );
}

#[test]
fn vertical_slice_search_and_memory_plugins_are_usable() {
    let mut search = SearchPlugin::new();
    search
        .index(
            SearchRecord::new(
                "m8-search-1",
                "session",
                "session-m8",
                vec!["agent".to_owned(), "memory".to_owned()],
                Some("payload-ref".to_owned()),
            )
            .expect("search record"),
        )
        .expect("index");
    let hits = search.query("memory").expect("query");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].record().record_id(), "m8-search-1");

    let mut memory = MemoryPlugin::new();
    memory.attach(session_id());
    memory
        .summarize(
            MemoryRecord::new(
                "m8-memory-1",
                session_id(),
                "m8 summary",
                "vertical-slice",
                Some("payload-ref".to_owned()),
            )
            .expect("memory record"),
        )
        .expect("summarize");
    assert_eq!(memory.load(&session_id()).len(), 1);
    assert_eq!(memory.export(&session_id()).expect("export").len(), 1);
}

#[test]
fn vertical_slice_rejects_control_fields_in_payload_wire() {
    let wire = serde_json::from_value::<UiCommandWire>(serde_json::json!({
        "correlation_id": "c1",
        "session_id": "s1",
        "capability_id": "cap1",
        "payload": {
            "body": "payload",
            "control": "should-reject"
        }
    }));
    assert!(wire.is_err());

    let frame = serde_json::from_value::<WireFrame>(serde_json::json!({
        "frame_class": "Payload",
        "frame": {
            "version": "V1",
            "command": {
                "correlation_id": "c1",
                "session_id": "s1",
                "capability_id": "cap1",
                "payload": {
                    "body": "payload"
                }
            }
        }
    }));
    assert!(frame.is_ok(), "wire frame decode failed: {frame:?}");

    let frame = frame.expect("frame");
    let encoded = serde_json::to_string(&frame).expect("encode frame");
    let decoded = WireFrame::decode(&encoded).expect("decode frame");
    assert!(matches!(decoded, WireFrame::Payload(PayloadFrame { .. })));
}
