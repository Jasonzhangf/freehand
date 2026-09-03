use freehand_v2_contracts::{SessionId, TurnId};
use freehand_v2_session_canvas_plugin::{
    CanvasBand, CanvasEdge, CanvasError, CanvasNode, SessionCanvasPlugin,
};

fn session(id: &str) -> SessionId {
    SessionId::try_new(id).expect("session id")
}

fn turn(id: &str) -> TurnId {
    TurnId::try_new(id).expect("turn id")
}

fn node(id: &str, band: CanvasBand) -> CanvasNode {
    CanvasNode::new(session(id), turn("id-t"), band, None)
}

#[test]
fn derive_publish_and_focus_session() {
    let mut plugin = SessionCanvasPlugin::new();
    plugin
        .derive(
            vec![
                node("s1", CanvasBand::Active),
                node("s2", CanvasBand::History),
            ],
            vec![],
        )
        .expect("derive");
    plugin.focus(session("s1")).expect("focus");
    let projection = plugin.publish();
    assert_eq!(projection.focus().map(|s| s.as_str()), Some("s1"));
    assert_eq!(projection.nodes().len(), 2);
}

#[test]
fn orphan_edge_is_rejected() {
    let mut plugin = SessionCanvasPlugin::new();
    let err = plugin
        .derive(
            vec![node("s1", CanvasBand::Active)],
            vec![CanvasEdge::new(session("s1"), session("missing"))],
        )
        .expect_err("orphan edge");
    assert_eq!(err, CanvasError::OrphanEdge);
}

#[test]
fn filter_band_returns_only_matching_nodes() {
    let mut plugin = SessionCanvasPlugin::new();
    plugin
        .derive(
            vec![
                node("s1", CanvasBand::Active),
                node("s2", CanvasBand::Recent),
            ],
            vec![],
        )
        .expect("derive");
    assert_eq!(plugin.filter(CanvasBand::Recent).len(), 1);
}

#[test]
fn focus_unknown_session_is_rejected() {
    let mut plugin = SessionCanvasPlugin::new();
    plugin
        .derive(vec![node("s1", CanvasBand::Active)], vec![])
        .expect("derive");
    let err = plugin.focus(session("missing")).expect_err("focus");
    assert_eq!(err, CanvasError::UnknownFocus("missing".to_owned()));
}
