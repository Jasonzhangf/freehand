use freehand_v2_contracts::{EventId, SessionId};
use freehand_v2_sessionlog::{
    CURRENT_FORMAT_VERSION, EventKind, SessionLog, SessionLogError, SurfaceOp,
};

fn event_id(value: &str) -> EventId {
    EventId::try_new(value).expect("event id")
}

fn session_id(value: &str) -> SessionId {
    SessionId::try_new(value).expect("session id")
}

#[test]
fn append_input_surface_and_result_are_contiguous_and_readable() {
    let mut log = SessionLog::new();
    let session = session_id("s-001");
    let cursor = log
        .create_session(session.clone(), 1, Some("test".to_owned()))
        .unwrap();
    assert_eq!(cursor.last_applied_seq(), 0);

    let cursor = log
        .append_event(
            &session,
            event_id("e-1"),
            2,
            EventKind::Input,
            "input",
            None,
            vec![],
            false,
        )
        .unwrap();
    assert_eq!(cursor.last_applied_seq(), 1);

    log.append_event(
        &session,
        event_id("e-2"),
        3,
        EventKind::Surface,
        "surface",
        Some(SurfaceOp::Replace),
        vec![1],
        false,
    )
    .unwrap();
    log.append_event(
        &session,
        event_id("e-3"),
        4,
        EventKind::Result,
        "result",
        None,
        vec![1, 2],
        false,
    )
    .unwrap();

    let events = log.read_session(&session).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(
        events.iter().map(|event| event.seq()).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    let surface = log.derive_surface(&session).unwrap();
    assert_eq!(surface.generation, 3);
    assert!(surface.nodes.iter().any(|node| node.content == "input"));
    assert!(surface.nodes.iter().any(|node| node.content == "surface"));
    assert!(surface.nodes.iter().any(|node| node.content == "result"));
}

#[test]
fn replay_from_cursor_returns_only_ordered_suffix() {
    let mut log = SessionLog::new();
    let session = session_id("s-002");
    log.create_session(session.clone(), 1, None).unwrap();
    for (id, kind) in [
        ("e-1", EventKind::Input),
        ("e-2", EventKind::Surface),
        ("e-3", EventKind::Result),
    ] {
        log.append_event(&session, event_id(id), 2, kind, id, None, vec![], false)
            .unwrap();
    }

    let events = log.read_session(&session).unwrap();
    let second = events[1].clone();
    let cursor = freehand_v2_sessionlog::SessionCursor::try_new(session.clone(), 1).unwrap();
    let suffix = log.replay_from(&cursor).unwrap();
    assert_eq!(suffix.len(), 2);
    assert_eq!(suffix[0].event_id(), second.event_id());
}

#[test]
fn replace_undo_and_recovery_are_append_only() {
    let mut log = SessionLog::new();
    let session = session_id("s-003");
    log.create_session(session.clone(), 1, None).unwrap();
    log.append_event(
        &session,
        event_id("e-1"),
        2,
        EventKind::Surface,
        "old",
        Some(SurfaceOp::Replace),
        vec![],
        false,
    )
    .unwrap();
    log.append_event(
        &session,
        event_id("e-2"),
        3,
        EventKind::Surface,
        "new",
        Some(SurfaceOp::Replace),
        vec![],
        false,
    )
    .unwrap();
    log.append_event(
        &session,
        event_id("e-3"),
        4,
        EventKind::Recovery,
        "interrupted",
        Some(SurfaceOp::Undo),
        vec![0, 1],
        false,
    )
    .unwrap();

    let surface = log.derive_surface(&session).unwrap();
    assert!(!surface.nodes.iter().any(|node| node.content == "old"));
    assert!(!surface.nodes.iter().any(|node| node.content == "new"));
    assert_eq!(log.read_session(&session).unwrap().len(), 3);
}

#[test]
fn fork_from_closed_child_parent_records_lineage() {
    let mut log = SessionLog::new();
    let parent = session_id("parent");
    log.create_session(parent.clone(), 1, None).unwrap();
    log.append_event(
        &parent,
        event_id("in-1"),
        2,
        EventKind::Input,
        "prompt",
        None,
        vec![],
        false,
    )
    .unwrap();
    log.append_event(
        &parent,
        event_id("out-1"),
        3,
        EventKind::Result,
        "closed",
        None,
        vec![0],
        false,
    )
    .unwrap();

    let child = session_id("child");
    let child_cursor = log
        .create_child_session(child.clone(), &parent, 2, 4, None)
        .unwrap();
    assert_eq!(child_cursor.last_applied_seq(), 2);
    let child_header_events = log.read_session(&child).unwrap();
    assert_eq!(child_header_events.len(), 0);
}

#[test]
fn duplicate_event_id_is_rejected() {
    let mut log = SessionLog::new();
    let session = session_id("s-004");
    log.create_session(session.clone(), 1, None).unwrap();
    log.append_event(
        &session,
        event_id("dup"),
        2,
        EventKind::Input,
        "one",
        None,
        vec![],
        false,
    )
    .unwrap();
    let err = log
        .append_event(
            &session,
            event_id("dup"),
            3,
            EventKind::Input,
            "two",
            None,
            vec![],
            false,
        )
        .unwrap_err();
    assert_eq!(err, SessionLogError::DuplicateEventId("dup".to_owned()));
    assert_eq!(log.read_session(&session).unwrap().len(), 1);
}

#[test]
fn sequence_gap_is_rejected_and_log_unchanged() {
    let mut log = SessionLog::new();
    let session = session_id("s-005");
    log.create_session(session.clone(), 1, None).unwrap();
    let err = log
        .append_event_with_seq(
            &session,
            3,
            event_id("gap"),
            2,
            EventKind::Input,
            "bad",
            None,
            vec![],
            false,
        )
        .unwrap_err();
    assert_eq!(
        err,
        SessionLogError::SequenceGap {
            expected: 0,
            actual: 3
        }
    );
    assert_eq!(log.read_session(&session).unwrap().len(), 0);
}

#[test]
fn invalid_cursor_beyond_boundary_is_rejected() {
    let mut log = SessionLog::new();
    let session = session_id("s-006");
    log.create_session(session.clone(), 1, None).unwrap();
    let cursor = freehand_v2_sessionlog::SessionCursor::try_new(session.clone(), 99).unwrap();
    assert!(matches!(
        log.replay_from(&cursor),
        Err(SessionLogError::InvalidCursor { .. })
    ));
}

#[test]
fn fork_inside_open_turn_is_rejected() {
    let mut log = SessionLog::new();
    let parent = session_id("open-parent");
    log.create_session(parent.clone(), 1, None).unwrap();
    log.append_event(
        &parent,
        event_id("open-in"),
        2,
        EventKind::Input,
        "prompt",
        None,
        vec![],
        false,
    )
    .unwrap();

    assert_eq!(
        log.create_child_session(session_id("child"), &parent, 1, 3, None),
        Err(SessionLogError::ForkInsideOpenTurn)
    );
}

#[test]
fn unsupported_format_version_is_rejected() {
    assert_eq!(
        SessionLog::ensure_format_version(999),
        Err(SessionLogError::UnsupportedFormatVersion(999))
    );
    assert_eq!(CURRENT_FORMAT_VERSION, 1);
}
