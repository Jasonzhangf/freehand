use freehand_v2_contracts::{ControlKind, CorrelationId, ErrorKind, EventId, PayloadRef, PluginId};
use freehand_v2_control_events::{EventLedger, EventLedgerError, LedgerCursor};

fn event_id(value: &str) -> EventId {
    EventId::try_new(value).expect("event id")
}

fn correlation_id(value: &str) -> CorrelationId {
    CorrelationId::try_new(value).expect("correlation id")
}

fn plugin_id(value: &str) -> PluginId {
    PluginId::try_new(value).expect("plugin id")
}

#[test]
fn accepted_events_route_to_declared_owner_in_order() {
    let mut ledger = EventLedger::new();
    let owner = plugin_id("plugin-a");
    let correlation = correlation_id("corr-001");

    ledger
        .emit(
            event_id("e-1"),
            correlation.clone(),
            ControlKind::PluginInvoked,
            owner.clone(),
            None,
        )
        .unwrap();
    ledger
        .emit(
            event_id("e-2"),
            correlation.clone(),
            ControlKind::PluginCompleted,
            owner.clone(),
            None,
        )
        .unwrap();

    let routed = ledger.owner_events(&owner);
    assert_eq!(routed.len(), 2);
    assert_eq!(routed[0].seq(), 0);
    assert_eq!(routed[1].seq(), 1);
    assert_eq!(ledger.next_seq(), 2);
}

#[test]
fn duplicate_event_id_is_rejected_before_mutation() {
    let mut ledger = EventLedger::new();
    ledger
        .emit(
            event_id("dup"),
            correlation_id("corr-001"),
            ControlKind::PluginInvoked,
            plugin_id("plugin-a"),
            None,
        )
        .unwrap();

    let err = ledger
        .emit(
            event_id("dup"),
            correlation_id("corr-002"),
            ControlKind::PluginCompleted,
            plugin_id("plugin-b"),
            None,
        )
        .unwrap_err();

    assert_eq!(err, EventLedgerError::DuplicateEventId("dup".to_owned()));
    assert_eq!(ledger.events().len(), 1);
    assert_eq!(ledger.errors().len(), 0);
}

#[test]
fn acknowledgement_moves_event_and_replay_returns_suffix() {
    let mut ledger = EventLedger::new();
    let correlation = correlation_id("corr-002");
    ledger
        .emit(
            event_id("e-1"),
            correlation.clone(),
            ControlKind::PluginInvoked,
            plugin_id("plugin-a"),
            None,
        )
        .unwrap();
    ledger
        .emit(
            event_id("e-2"),
            correlation,
            ControlKind::PluginCompleted,
            plugin_id("plugin-a"),
            None,
        )
        .unwrap();

    let cursor = ledger.acknowledge(&event_id("e-1")).unwrap();
    assert_eq!(cursor.last_applied_seq(), 1);

    let replay = ledger.replay_from(&LedgerCursor::new(1)).unwrap();
    assert_eq!(replay.len(), 1);
    assert_eq!(replay[0].event_id(), &event_id("e-2"));
}

#[test]
fn terminal_correlation_rejects_later_events() {
    let mut ledger = EventLedger::new();
    let correlation = correlation_id("corr-003");
    ledger
        .emit(
            event_id("e-1"),
            correlation.clone(),
            ControlKind::PluginInvoked,
            plugin_id("plugin-a"),
            None,
        )
        .unwrap();

    assert_eq!(ledger.complete(&correlation).unwrap(), 1);
    assert!(ledger.is_terminal(&correlation));
    assert_eq!(
        ledger
            .emit(
                event_id("e-2"),
                correlation.clone(),
                ControlKind::PluginCompleted,
                plugin_id("plugin-a"),
                None,
            )
            .unwrap_err(),
        EventLedgerError::AlreadyTerminal(correlation.as_str().to_owned())
    );
    assert_eq!(
        ledger.complete(&correlation).unwrap_err(),
        EventLedgerError::AlreadyTerminal(correlation.as_str().to_owned())
    );
}

#[test]
fn already_acknowledged_event_is_rejected() {
    let mut ledger = EventLedger::new();
    ledger
        .emit(
            event_id("e-ack"),
            correlation_id("corr-004"),
            ControlKind::PluginInvoked,
            plugin_id("plugin-a"),
            None,
        )
        .unwrap();
    ledger.acknowledge(&event_id("e-ack")).unwrap();
    assert_eq!(
        ledger.acknowledge(&event_id("e-ack")).unwrap_err(),
        EventLedgerError::AlreadyAcknowledged("e-ack".to_owned())
    );
}

#[test]
fn error_chain_records_rejection_without_event_mutation() {
    let mut ledger = EventLedger::new();
    ledger
        .emit(
            event_id("e-source"),
            correlation_id("corr-005"),
            ControlKind::PluginInvoked,
            plugin_id("plugin-a"),
            None,
        )
        .unwrap();

    let error = ledger
        .reject(
            correlation_id("corr-005"),
            ErrorKind::InvalidPayload,
            "bad payload ref",
            Some(event_id("e-source")),
        )
        .unwrap();

    assert_eq!(error.kind(), ErrorKind::InvalidPayload);
    assert_eq!(error.message(), "bad payload ref");
    assert_eq!(error.source_event_id(), Some(&event_id("e-source")));
    assert_eq!(ledger.events().len(), 1);
    assert_eq!(ledger.errors().len(), 1);
}

#[test]
fn payload_ref_is_reference_only_and_does_not_contain_body() {
    let mut ledger = EventLedger::new();
    let payload_ref = PayloadRef::new("payload-001").expect("payload ref");
    ledger
        .emit(
            event_id("e-payload"),
            correlation_id("corr-006"),
            ControlKind::PluginInvoked,
            plugin_id("plugin-a"),
            Some(payload_ref),
        )
        .unwrap();

    let event = &ledger.events()[0];
    assert!(event.payload_ref().is_some());
    let value = serde_json::to_value(event).expect("event value");
    assert!(!value.to_string().contains("body"));
    assert!(value.to_string().contains("payload-001"));
}

#[test]
fn replay_invalid_cursor_is_rejected() {
    let mut ledger = EventLedger::new();
    ledger
        .emit(
            event_id("e-1"),
            correlation_id("corr-007"),
            ControlKind::PluginInvoked,
            plugin_id("plugin-a"),
            None,
        )
        .unwrap();

    assert!(matches!(
        ledger.replay_from(&LedgerCursor::new(99)),
        Err(EventLedgerError::InvalidCursor(_))
    ));
}

#[test]
fn owner_route_is_isolated() {
    let mut ledger = EventLedger::new();
    ledger
        .emit(
            event_id("e-a1"),
            correlation_id("corr-a"),
            ControlKind::PluginInvoked,
            plugin_id("plugin-a"),
            None,
        )
        .unwrap();
    ledger
        .emit(
            event_id("e-b1"),
            correlation_id("corr-b"),
            ControlKind::PluginInvoked,
            plugin_id("plugin-b"),
            None,
        )
        .unwrap();

    assert_eq!(ledger.owner_events(&plugin_id("plugin-a")).len(), 1);
    assert_eq!(ledger.owner_events(&plugin_id("plugin-b")).len(), 1);
    assert_eq!(ledger.owner_events(&plugin_id("plugin-c")).len(), 0);
}
