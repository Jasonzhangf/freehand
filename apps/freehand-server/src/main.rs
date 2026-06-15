use freehand_contracts::{
    AgentId, FeatureId, ReasonResp01SemanticEvent, ReasonResp03TerminalEvent, SemanticEventKind,
    SessionId, TerminalStatus, TraceId, TurnId,
};
use freehand_ui_protocol::{
    TurnProjectionInput, UiClientKind, UiCommand, UiProtocolState, UiQueryResult,
    turn_projection_for_client, turn_projection_from_events,
};

fn main() {
    match run() {
        Ok(output) => println!("{output}"),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return Err("usage: freehand-server webui-smoke".to_owned());
    };
    if command != "webui-smoke" || args.next().is_some() {
        return Err("usage: freehand-server webui-smoke".to_owned());
    }
    Ok(render_webui_smoke())
}

fn render_webui_smoke() -> String {
    let mut state = UiProtocolState::default();
    let projection = sample_slave_turn_projection();
    state.apply_turn_projection(projection.clone());

    let queried = match state
        .query(&UiCommand::QueryLatestActiveTurn)
        .expect("query must be valid")
    {
        UiQueryResult::Turn(Some(turn)) => turn,
        _ => panic!("seeded latest active turn must exist"),
    };
    let webui_projection = turn_projection_for_client(queried.clone(), UiClientKind::WebUi);
    let cli_projection = turn_projection_for_client(queried, UiClientKind::Cli);

    format!(
        concat!(
            "<main data-freehand-webui=\"smoke\">",
            "<section data-query=\"latest-active-turn\" data-turn=\"{turn_id}\">",
            "<h1>Freehand WebUI Smoke</h1>",
            "<p data-terminal=\"true\">{terminal}</p>",
            "</section>",
            "<aside data-slave-card=\"{slave_card}\">{slave_text}</aside>",
            "<span data-cli-slave-card=\"{cli_slave_card}\"></span>",
            "</main>"
        ),
        turn_id = webui_projection.turn_id.as_str(),
        terminal = escape_html(webui_projection.terminal_text.as_deref().unwrap_or("")),
        slave_card = webui_projection.slave_substream_card,
        slave_text = escape_html(&webui_projection.text.join("")),
        cli_slave_card = cli_projection.slave_substream_card,
    )
}

fn sample_slave_turn_projection() -> freehand_ui_protocol::UiTurnProjection {
    turn_projection_from_events(TurnProjectionInput {
        source_agent_id: AgentId::new("slave-agent"),
        source_node_id: "slave-node".to_owned(),
        session_id: SessionId::new("session-webui-smoke"),
        turn_id: TurnId::new("turn-webui-smoke"),
        semantic_events: vec![
            ReasonResp01SemanticEvent {
                session_id: SessionId::new("session-webui-smoke"),
                turn_id: TurnId::new("turn-webui-smoke"),
                trace_id: TraceId::new("trace-webui-smoke"),
                feature_id: FeatureId::new("app.webui-smoke"),
                agent_id: AgentId::new("slave-agent"),
                kind: SemanticEventKind::Reasoning,
                content: "thinking".to_owned(),
            },
            ReasonResp01SemanticEvent {
                session_id: SessionId::new("session-webui-smoke"),
                turn_id: TurnId::new("turn-webui-smoke"),
                trace_id: TraceId::new("trace-webui-smoke"),
                feature_id: FeatureId::new("app.webui-smoke"),
                agent_id: AgentId::new("slave-agent"),
                kind: SemanticEventKind::Text,
                content: "slave answer".to_owned(),
            },
        ],
        tool_calls: Vec::new(),
        usage_events: Vec::new(),
        terminal_event: Some(ReasonResp03TerminalEvent {
            session_id: SessionId::new("session-webui-smoke"),
            turn_id: TurnId::new("turn-webui-smoke"),
            trace_id: TraceId::new("trace-webui-smoke"),
            feature_id: FeatureId::new("app.webui-smoke"),
            agent_id: AgentId::new("slave-agent"),
            status: TerminalStatus::Success,
            summary: "terminal final text".to_owned(),
        }),
        error_events: Vec::new(),
        slave_substream_card: true,
    })
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webui_smoke_renders_query_terminal_and_slave_card_from_protocol_truth() {
        let html = render_webui_smoke();
        assert!(html.contains("data-freehand-webui=\"smoke\""));
        assert!(html.contains("data-query=\"latest-active-turn\""));
        assert!(html.contains("terminal final text"));
        assert!(html.contains("data-slave-card=\"true\""));
        assert!(html.contains("slave answer"));
        assert!(html.contains("data-cli-slave-card=\"false\""));
    }
}
