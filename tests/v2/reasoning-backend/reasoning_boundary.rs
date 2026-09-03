use std::sync::Arc;

use freehand_v2_contracts::{CorrelationId, EventId, ImmutablePayload, SessionId};
use freehand_v2_reasoning_backend::{
    BackendId, NativeBackend, OpenCodeBackend, ReasoningCursor, ReasoningError, ReasoningEventKind,
    ReasoningRequest, ReasoningService, RuntimeGroupId,
};

fn session(value: &str) -> SessionId {
    SessionId::try_new(value).expect("session id")
}

fn correlation(value: &str) -> CorrelationId {
    CorrelationId::try_new(value).expect("correlation id")
}

fn group(value: &str) -> RuntimeGroupId {
    RuntimeGroupId::try_new(value).expect("runtime group id")
}

fn payload(value: &str) -> Arc<ImmutablePayload> {
    Arc::new(ImmutablePayload::new(value).expect("payload"))
}

fn cursor_for(
    event: &freehand_v2_reasoning_backend::ReasoningEvent,
    last_seq: u64,
) -> ReasoningCursor {
    ReasoningCursor::try_new(
        event.session_id().clone(),
        event.event_id().clone(),
        event.backend_id().clone(),
        event.generation(),
        last_seq,
    )
    .expect("cursor")
}

#[test]
fn native_and_opencode_satisfy_same_service_contract() {
    let mut service = ReasoningService::new();
    let native_group = group("native");
    let opencode_group = group("opencode");

    let native_cap = service
        .bind(
            native_group.clone(),
            Box::new(NativeBackend::new().expect("native backend")),
        )
        .unwrap();
    let opencode_cap = service
        .bind(
            opencode_group.clone(),
            Box::new(OpenCodeBackend::new().expect("opencode backend")),
        )
        .unwrap();

    assert_eq!(native_cap.provider(), "freehand-native");
    assert_eq!(opencode_cap.provider(), "opencode-adaptor");
    assert!(native_cap.can_resume());
    assert!(opencode_cap.can_subscribe());

    let native_event = service
        .start(
            &native_group,
            ReasoningRequest::new(
                session("native-session"),
                correlation("native-corr"),
                payload("native-input"),
                None,
            ),
        )
        .unwrap();
    let opencode_event = service
        .start(
            &opencode_group,
            ReasoningRequest::new(
                session("opencode-session"),
                correlation("opencode-corr"),
                payload("opencode-input"),
                None,
            ),
        )
        .unwrap();

    assert_eq!(native_event.kind(), ReasoningEventKind::Started);
    assert_eq!(opencode_event.kind(), ReasoningEventKind::Started);
    assert_eq!(native_event.backend_id().as_str(), "freehand-native");
    assert_eq!(opencode_event.backend_id().as_str(), "opencode-adaptor");
    assert_eq!(opencode_event.correlation_id().as_str(), "opencode-corr");
}

#[test]
fn native_payload_is_shared_by_arc_across_adjacent_consumers() {
    let mut service = ReasoningService::new();
    let group_id = group("native");
    service
        .bind(group_id.clone(), Box::new(NativeBackend::new().unwrap()))
        .unwrap();

    let shared = payload("shared-payload");
    let event = service
        .start(
            &group_id,
            ReasoningRequest::new(session("s"), correlation("c"), Arc::clone(&shared), None),
        )
        .unwrap();

    assert_eq!(event.payload().body(), "shared-payload");
    assert!(Arc::ptr_eq(&shared, event.payload()));
}

#[test]
fn resume_and_subscribe_return_normalized_events() {
    let mut service = ReasoningService::new();
    let group_id = group("native");
    service
        .bind(group_id.clone(), Box::new(NativeBackend::new().unwrap()))
        .unwrap();
    let sid = session("s");
    let first = service
        .start(
            &group_id,
            ReasoningRequest::new(sid.clone(), correlation("c1"), payload("first"), None),
        )
        .unwrap();
    let cursor = cursor_for(&first, 1);

    let resume = service
        .resume(
            cursor,
            ReasoningRequest::new(sid.clone(), correlation("c2"), payload("second"), None),
        )
        .unwrap();
    let subscribed = service.subscribe(&sid, &correlation("c3")).unwrap();

    assert_eq!(resume.kind(), ReasoningEventKind::Delta);
    assert_eq!(subscribed.kind(), ReasoningEventKind::Response);
    assert_eq!(subscribed.backend_id().as_str(), "freehand-native");
}

#[test]
fn interrupt_removes_service_in_flight_state() {
    let mut service = ReasoningService::new();
    let group_id = group("native");
    service
        .bind(group_id.clone(), Box::new(NativeBackend::new().unwrap()))
        .unwrap();
    let sid = session("s");
    service
        .start(
            &group_id,
            ReasoningRequest::new(sid.clone(), correlation("c"), payload("x"), None),
        )
        .unwrap();

    assert_eq!(
        service.inspect(&sid).unwrap(),
        freehand_v2_reasoning_backend::ReasoningState::Running
    );
    let interrupted = service.interrupt(&sid).unwrap();
    assert_eq!(interrupted.kind(), ReasoningEventKind::Interrupted);
    assert_eq!(
        service.inspect(&sid).unwrap_err(),
        ReasoningError::UnknownSession("s".to_owned())
    );
}

#[test]
fn duplicate_bind_is_rejected() {
    let mut service = ReasoningService::new();
    let group_id = group("native");
    service
        .bind(group_id.clone(), Box::new(NativeBackend::new().unwrap()))
        .unwrap();
    let err = service
        .bind(group_id, Box::new(OpenCodeBackend::new().unwrap()))
        .unwrap_err();
    assert_eq!(
        err,
        ReasoningError::RuntimeGroupAlreadyBound("native".to_owned())
    );
}

#[test]
fn missing_active_backend_fails_closed() {
    let mut service = ReasoningService::new();
    let err = service
        .start(
            &group("missing"),
            ReasoningRequest::new(session("s"), correlation("c"), payload("x"), None),
        )
        .unwrap_err();
    assert_eq!(err, ReasoningError::NoActiveBackend("missing".to_owned()));
}

#[test]
fn replacement_does_not_migrate_an_in_flight_request() {
    let mut service = ReasoningService::new();
    let group_id = group("native");
    service
        .bind(group_id.clone(), Box::new(NativeBackend::new().unwrap()))
        .unwrap();
    service
        .start(
            &group_id,
            ReasoningRequest::new(session("s"), correlation("c"), payload("x"), None),
        )
        .unwrap();

    let err = service
        .replace_backend(&group_id, Box::new(OpenCodeBackend::new().unwrap()))
        .unwrap_err();
    assert_eq!(err, ReasoningError::BackendInFlight("native".to_owned()));
}

#[test]
fn replacement_at_safe_boundary_increments_backend_generation() {
    let mut service = ReasoningService::new();
    let group_id = group("native");
    service
        .bind(group_id.clone(), Box::new(NativeBackend::new().unwrap()))
        .unwrap();
    let sid = session("s");
    service
        .start(
            &group_id,
            ReasoningRequest::new(sid.clone(), correlation("c"), payload("x"), None),
        )
        .unwrap();
    service.interrupt(&sid).unwrap();

    let capability = service
        .replace_backend(&group_id, Box::new(OpenCodeBackend::new().unwrap()))
        .unwrap();
    assert_eq!(capability.provider(), "opencode-adaptor");
    let event = service
        .start(
            &group_id,
            ReasoningRequest::new(session("s2"), correlation("c2"), payload("y"), None),
        )
        .unwrap();
    assert_eq!(event.generation(), 2);
    assert_eq!(event.backend_id().as_str(), "opencode-adaptor");
}

#[test]
fn stale_generation_cursor_is_rejected_after_replacement() {
    let mut service = ReasoningService::new();
    let group_id = group("native");
    service
        .bind(group_id.clone(), Box::new(NativeBackend::new().unwrap()))
        .unwrap();
    let sid = session("s");
    let first = service
        .start(
            &group_id,
            ReasoningRequest::new(sid.clone(), correlation("c"), payload("x"), None),
        )
        .unwrap();
    service.interrupt(&sid).unwrap();
    service
        .replace_backend(&group_id, Box::new(NativeBackend::new().unwrap()))
        .unwrap();

    let stale = cursor_for(&first, 1);
    let err = service
        .resume(
            stale,
            ReasoningRequest::new(sid, correlation("c2"), payload("y"), None),
        )
        .unwrap_err();
    assert_eq!(
        err,
        ReasoningError::StaleGeneration {
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn opencode_state_that_cannot_be_reconciled_fails_explicitly() {
    let mut service = ReasoningService::new();
    let group_id = group("opencode");
    service
        .bind(group_id, Box::new(OpenCodeBackend::new().unwrap()))
        .unwrap();
    let backend_id = BackendId::try_new("opencode-adaptor").unwrap();
    let cursor = ReasoningCursor::try_new(
        session("unknown"),
        EventId::try_new("e").unwrap(),
        backend_id,
        1,
        1,
    )
    .unwrap();

    let err = service
        .resume(
            cursor,
            ReasoningRequest::new(session("unknown"), correlation("c"), payload("x"), None),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        ReasoningError::OpenCodeStateNotReconcilable(_)
    ));
}

#[test]
fn invalid_cursor_is_rejected() {
    let err = ReasoningCursor::try_new(
        session("s"),
        EventId::try_new("e").unwrap(),
        BackendId::try_new("native").unwrap(),
        1,
        0,
    )
    .unwrap_err();
    assert!(matches!(err, ReasoningError::InvalidCursor(_)));
}

#[test]
fn replace_requires_active_backend() {
    let mut service = ReasoningService::new();
    let err = service
        .replace_backend(&group("missing"), Box::new(NativeBackend::new().unwrap()))
        .unwrap_err();
    assert_eq!(err, ReasoningError::NoActiveBackend("missing".to_owned()));
}
