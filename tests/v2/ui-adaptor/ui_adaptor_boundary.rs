use std::sync::Arc;

use freehand_v2_contracts::{CapabilityId, CorrelationId, ImmutablePayload, SessionId, UiCommand};
use freehand_v2_ui_adaptor::{
    ProjectionKind, SlotId, UiAdaptor, UiCommandReceiptStatus, UiControlEventKind, UiError,
    UiQuery, UiSubscribe,
};

fn slot(value: &str) -> SlotId {
    SlotId::try_new(value).expect("slot id")
}

fn correlation(value: &str) -> CorrelationId {
    CorrelationId::try_new(value).expect("correlation id")
}

fn session(value: &str) -> SessionId {
    SessionId::try_new(value).expect("session id")
}

fn capability(value: &str) -> CapabilityId {
    CapabilityId::try_new(value).expect("capability id")
}

fn command(correlation_id: &str, payload: &str) -> UiCommand {
    UiCommand::new(
        correlation(correlation_id),
        session("session-1"),
        capability("ui.run.submit"),
        ImmutablePayload::new(payload).expect("payload"),
    )
}

#[test]
fn publish_projection_shares_arc_payload_with_adaptor_projection() {
    let mut adaptor = UiAdaptor::new();
    let payload = Arc::new(ImmutablePayload::new("run-body").expect("payload"));
    let projection = adaptor
        .publish_projection(
            slot("ui.run"),
            ProjectionKind::Run,
            "session-log-owner",
            payload.clone(),
        )
        .expect("publish projection");

    assert!(Arc::ptr_eq(&payload, projection.payload()));
    assert_eq!(projection.slot_id(), &slot("ui.run"));
    assert_eq!(projection.kind(), ProjectionKind::Run);
}

#[test]
fn accept_command_records_receipt_and_control_event_without_payload() {
    let mut adaptor = UiAdaptor::new();
    let cmd = command("cmd-1", "secret-payload");
    let receipt = adaptor
        .accept_command(slot("ui.run"), &cmd)
        .expect("accept command");

    assert_eq!(receipt.status(), UiCommandReceiptStatus::Accepted);
    assert!(
        adaptor
            .events()
            .iter()
            .any(|event| event.kind() == UiControlEventKind::CommandAccepted)
    );
    let serialized = serde_json::to_string(adaptor.events()).expect("events json");
    assert!(!serialized.contains("secret-payload"));
}

#[test]
fn duplicate_command_is_rejected() {
    let mut adaptor = UiAdaptor::new();
    let cmd = command("cmd-1", "body");
    adaptor
        .accept_command(slot("ui.run"), &cmd)
        .expect("first accept");
    let err = adaptor
        .accept_command(slot("ui.run"), &cmd)
        .expect_err("duplicate should fail");
    assert_eq!(err, UiError::DuplicateCommand("cmd-1".to_owned()));
}

#[test]
fn query_returns_latest_revision_for_slot() {
    let mut adaptor = UiAdaptor::new();
    adaptor
        .publish_projection(
            slot("ui.run"),
            ProjectionKind::Run,
            "session-log-owner",
            Arc::new(ImmutablePayload::new("first").expect("payload")),
        )
        .expect("first projection");
    adaptor
        .publish_projection(
            slot("ui.run"),
            ProjectionKind::Run,
            "session-log-owner",
            Arc::new(ImmutablePayload::new("second").expect("payload")),
        )
        .expect("second projection");

    let latest = adaptor.query(&slot("ui.run")).expect("query");
    assert_eq!(latest.revision(), 1);
    assert_eq!(latest.payload().body(), "second");
}

#[test]
fn query_unknown_slot_fails() {
    let adaptor = UiAdaptor::new();
    let err = adaptor
        .query(&slot("ui.missing"))
        .expect_err("query should fail");
    assert_eq!(err, UiError::UnknownSlot("ui.missing".to_owned()));
}

#[test]
fn subscribe_requires_existing_slot() {
    let mut adaptor = UiAdaptor::new();
    let subscription = UiSubscribe::new("sub-1", slot("ui.missing"), 0).expect("subscription");
    let err = adaptor
        .subscribe(subscription)
        .expect_err("subscribe should fail");
    assert_eq!(err, UiError::UnknownSlot("ui.missing".to_owned()));
}

#[test]
fn query_validates_non_empty_id() {
    let err = UiQuery::new("", slot("ui.run"), "session", "").expect_err("query id");
    assert_eq!(err, UiError::EmptyQueryId);
}
