use std::sync::Arc;

use freehand_v2_contracts::{
    CapabilityId, ControlEvent, ControlKind, CorrelationId, ErrorEvent, ErrorKind, EventId,
    ImmutablePayload, NodeId, PayloadFrame, PayloadRef, PluginId, ProtocolVersion, SessionId,
    TurnId, UiCommand, WireFrame,
};

#[test]
fn typed_ids_reject_empty_values_and_round_trip() {
    assert!(NodeId::try_new("").is_err());
    let session = SessionId::try_new("session-001").expect("valid session id");
    let encoded = serde_json::to_string(&session).expect("serialize id");
    let decoded: SessionId = serde_json::from_str(&encoded).expect("deserialize id");
    assert_eq!(decoded, session);
    assert!(serde_json::from_str::<SessionId>("\"\"").is_err());
    assert_eq!(
        PluginId::try_new("plugin-001")
            .expect("valid plugin id")
            .as_str(),
        "plugin-001"
    );
    assert_eq!(
        TurnId::try_new("turn-001").expect("valid turn id").as_str(),
        "turn-001"
    );
}

#[test]
fn adjacent_consumers_share_one_immutable_payload_allocation() {
    let payload = ImmutablePayload::new("v2-blackbox-payload-001").expect("valid payload");
    let plugin_input = payload.clone();
    let reasoning_input = plugin_input.clone();

    assert!(Arc::ptr_eq(payload.arc(), plugin_input.arc()));
    assert!(Arc::ptr_eq(plugin_input.arc(), reasoning_input.arc()));
    assert_eq!(reasoning_input.body(), "v2-blackbox-payload-001");
}

#[test]
fn explicit_wire_copy_rebuilds_immutable_value_without_changing_content() {
    let payload = ImmutablePayload::new("wire-boundary-payload").expect("valid payload");
    let wire = payload.to_wire();
    let rebuilt = ImmutablePayload::from_wire(wire).expect("rebuild payload");

    assert_eq!(rebuilt.body(), payload.body());
    assert!(!Arc::ptr_eq(payload.arc(), rebuilt.arc()));
    assert!(serde_json::from_str::<freehand_v2_contracts::PayloadWire>(r#"{"body":""}"#).is_err());
}

#[test]
fn control_and_error_frames_cannot_embed_business_payload_bytes() {
    let payload = ImmutablePayload::new("secret-business-content").expect("valid payload");
    let payload_ref = PayloadRef::new("payload-001").expect("valid payload reference");
    let control = ControlEvent::new(
        EventId::try_new("event-001").expect("event id"),
        CorrelationId::try_new("corr-001").expect("correlation id"),
        ControlKind::PluginInvoked,
        Some(payload_ref),
    );
    let error = ErrorEvent::new(
        CorrelationId::try_new("corr-001").expect("correlation id"),
        ErrorKind::Rejected,
        "payload rejected",
    );

    let control_json = serde_json::to_string(&control).expect("serialize control");
    let error_json = serde_json::to_string(&error).expect("serialize error");
    assert!(!control_json.contains(payload.body()));
    assert!(!error_json.contains(payload.body()));
    assert!(control_json.contains("payload_ref"));
    assert!(!control_json.contains("secret-business-content"));
    assert!(PayloadRef::new("").is_err());
    assert!(serde_json::from_str::<PayloadRef>(r#"{"payload_id":""}"#).is_err());
}

#[test]
fn frame_class_and_protocol_version_are_explicit() {
    let frame = WireFrame::Payload(PayloadFrame::new(
        ProtocolVersion::V1,
        UiCommand::new(
            CorrelationId::try_new("corr-002").expect("correlation id"),
            SessionId::try_new("session-002").expect("session id"),
            CapabilityId::try_new("capability-001").expect("capability id"),
            ImmutablePayload::new("frame-payload").expect("valid payload"),
        ),
    ));
    let encoded = serde_json::to_string(&frame).expect("serialize frame");
    let decoded = WireFrame::decode(&encoded).expect("decode frame");
    assert_eq!(decoded, frame);

    let mut unknown_json: serde_json::Value =
        serde_json::from_str(&encoded).expect("parse encoded frame");
    unknown_json["frame"]["unknown"] = serde_json::json!(true);
    let unknown_field = serde_json::to_string(&unknown_json).expect("serialize unknown frame");
    assert!(WireFrame::decode(&unknown_field).is_err());

    let invalid_version = encoded.replace("\"V1\"", "\"V2\"");
    assert!(WireFrame::decode(&invalid_version).is_err());
}

#[test]
fn ui_command_rejects_empty_payload_during_wire_decode() {
    let raw = r#"{
        "correlation_id":"corr-003",
        "session_id":"session-003",
        "capability_id":"capability-003",
        "payload":{"body":""}
    }"#;
    assert!(serde_json::from_str::<UiCommand>(raw).is_err());
}

#[test]
fn immutable_payload_constructor_rejects_empty_values() {
    assert!(ImmutablePayload::new("").is_err());
}
