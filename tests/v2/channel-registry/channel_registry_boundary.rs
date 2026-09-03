use freehand_v2_channel_registry::{
    BearerToken, ChannelError, ChannelRegistry, ChannelSessionState, FrameKind,
};
use freehand_v2_contracts::{CapabilityId, CorrelationId, NodeId};

fn token(value: &str) -> BearerToken {
    BearerToken::try_new(value).expect("token")
}

fn node(value: &str) -> NodeId {
    NodeId::try_new(value).expect("node id")
}

fn capability(value: &str) -> CapabilityId {
    CapabilityId::try_new(value).expect("capability")
}

fn register(registry: &mut ChannelRegistry, endpoint: &str) {
    registry
        .register(
            endpoint,
            node("node-1"),
            token("secret"),
            1,
            vec![capability("ui.render")],
        )
        .expect("register");
}

#[test]
fn register_and_discover_endpoint() {
    let mut registry = ChannelRegistry::new();
    register(&mut registry, "endpoint-1");
    let manifest = registry.discover("endpoint-1").expect("discover");
    assert_eq!(manifest.generation(), 1);
    assert_eq!(manifest.node_id().as_str(), "node-1");
}

#[test]
fn open_session_requires_valid_token_and_attaches_connection() {
    let mut registry = ChannelRegistry::new();
    register(&mut registry, "endpoint-1");
    let err = registry
        .open_session("session-1", "endpoint-1", &token("wrong"))
        .expect_err("invalid token");
    assert_eq!(err, ChannelError::InvalidToken("endpoint-1".to_owned()));

    let session = registry
        .open_session("session-1", "endpoint-1", &token("secret"))
        .expect("open");
    assert_eq!(session.state(), ChannelSessionState::Open);
    let connection = registry
        .attach_connection("session-1", "conn-1")
        .expect("attach");
    assert_eq!(connection.connection_id(), "conn-1");
}

#[test]
fn replace_connection_preserves_session_state_and_replay() {
    let mut registry = ChannelRegistry::new();
    register(&mut registry, "endpoint-1");
    registry
        .open_session("session-1", "endpoint-1", &token("secret"))
        .expect("open");
    registry
        .attach_connection("session-1", "conn-1")
        .expect("attach");
    registry
        .send(
            "session-1",
            FrameKind::Payload,
            CorrelationId::try_new("c1").expect("correlation"),
            Some("payload-ref".to_owned()),
            None,
        )
        .expect("send");
    let connection = registry
        .replace_connection("session-1", "conn-2")
        .expect("replace");
    assert_eq!(connection.generation(), 2);
    assert_eq!(
        registry
            .session("session-1")
            .expect("session")
            .event_count(),
        1
    );
    assert_eq!(registry.replay("session-1", 0).expect("replay").len(), 1);
}

#[test]
fn suspend_and_reattach_retain_session_frames() {
    let mut registry = ChannelRegistry::new();
    register(&mut registry, "endpoint-1");
    registry
        .open_session("session-1", "endpoint-1", &token("secret"))
        .expect("open");
    registry
        .attach_connection("session-1", "conn-1")
        .expect("attach");
    registry
        .send(
            "session-1",
            FrameKind::Control,
            CorrelationId::try_new("c1").expect("correlation"),
            None,
            Some("control".to_owned()),
        )
        .expect("send");
    registry.suspend("session-1").expect("suspend");
    assert_eq!(
        registry.session("session-1").expect("session").state(),
        ChannelSessionState::Suspended
    );
    let connection = registry.reattach("session-1", "conn-3").expect("reattach");
    assert_eq!(connection.generation(), 2);
    assert_eq!(registry.replay("session-1", 0).expect("replay").len(), 1);
}

#[test]
fn invalid_token_duplicate_endpoint_and_unknown_session_are_rejected() {
    let mut registry = ChannelRegistry::new();
    register(&mut registry, "endpoint-1");
    let err = registry
        .register("endpoint-1", node("node-2"), token("secret"), 1, vec![])
        .expect_err("duplicate");
    assert_eq!(
        err,
        ChannelError::DuplicateEndpoint("endpoint-1".to_owned())
    );
    let err = registry
        .open_session("session-1", "missing", &token("secret"))
        .expect_err("unknown endpoint");
    assert_eq!(err, ChannelError::UnknownEndpoint("missing".to_owned()));
    let err = registry
        .attach_connection("missing", "conn-1")
        .expect_err("unknown session");
    assert_eq!(err, ChannelError::UnknownSession("missing".to_owned()));
}

#[test]
fn unsupported_protocol_version_is_rejected() {
    let mut registry = ChannelRegistry::new();
    let err = registry
        .register("endpoint-2", node("node-1"), token("secret"), 2, vec![])
        .expect_err("unsupported version");
    assert_eq!(err, ChannelError::UnsupportedVersion("2".to_owned()));
}

#[test]
fn send_and_replay_fail_after_close_or_with_invalid_cursor() {
    let mut registry = ChannelRegistry::new();
    register(&mut registry, "endpoint-1");
    registry
        .open_session("session-1", "endpoint-1", &token("secret"))
        .expect("open");
    registry.close("session-1").expect("close");
    let err = registry
        .send(
            "session-1",
            FrameKind::Payload,
            CorrelationId::try_new("c1").expect("correlation"),
            None,
            None,
        )
        .expect_err("closed send");
    assert_eq!(err, ChannelError::SessionClosed("session-1".to_owned()));
    let err = registry.replay("session-1", 5).expect_err("invalid cursor");
    assert_eq!(err, ChannelError::InvalidCursor);
}
