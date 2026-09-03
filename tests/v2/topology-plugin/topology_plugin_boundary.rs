use freehand_v2_contracts::{CapabilityId, NodeId};
use freehand_v2_topology_plugin::{TopologyError, TopologyNode, TopologyPlugin};

fn node(machine: &str, node: &str, agent: &str, channel: &str) -> TopologyNode {
    TopologyNode::new(
        machine,
        NodeId::try_new(node).expect("node id"),
        agent,
        channel,
        vec![CapabilityId::try_new("ui.render").expect("capability")],
    )
    .expect("node")
}

#[test]
fn load_publish_and_focus_agent() {
    let mut plugin = TopologyPlugin::new();
    plugin.load(vec![node("mac-studio", "node-1", "agent-a", "channel-1")]);
    plugin.focus("agent-a").expect("focus");
    let projection = plugin.publish();
    assert_eq!(projection.focus(), Some("agent-a"));
    assert_eq!(projection.nodes()[0].channel_id(), "channel-1");
}

#[test]
fn empty_machine_is_rejected() {
    let err = TopologyNode::new(
        "",
        NodeId::try_new("node").expect("node id"),
        "agent",
        "channel",
        vec![],
    )
    .expect_err("empty machine");
    assert_eq!(err, TopologyError::EmptyMachine);
}

#[test]
fn focus_unknown_agent_is_rejected() {
    let mut plugin = TopologyPlugin::new();
    plugin.load(vec![node("m", "n", "agent-a", "channel-1")]);
    let err = plugin.focus("agent-b").expect_err("focus");
    assert_eq!(err, TopologyError::UnknownFocus("agent-b".to_owned()));
}
