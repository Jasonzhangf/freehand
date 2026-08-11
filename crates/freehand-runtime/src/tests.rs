use super::*;
use freehand_contracts::{
    FeatureId, SemanticEventKind, TerminalStatus, ToolPreviewChangeKind, ToolPreviewContract,
    ToolPreviewFileChange,
};
use freehand_contracts::{ToolCallContract, ToolCallId};
use freehand_metadata::MetadataEnvelope;
use freehand_provider_core::{ProviderInputAttachment, ProviderInputAttachmentKind};
use freehand_reason::ProviderRawLedgerRow;
use freehand_ui_protocol::{
    UiConversationItemKind, UiModelRequestKind, UiModelRequestWaiting, UiModelTransportActivity,
    UiModelTransportKind, UiQueryResult, UiToolActivityStatus, build_command_dispatch_envelope,
};
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn runtime() -> RuntimeCommandDispatcher {
    RuntimeCommandDispatcher::new(RuntimeCommandDispatcherConfig {
        session_id: SessionId::new("runtime-session"),
        reason_agent_id: AgentId::new("reason-agent"),
        master_agent_id: AgentId::new("master-agent"),
        master_node_id: "master-node".to_owned(),
        slave_agent_id: AgentId::new("slave-agent"),
        slave_node_id: "slave-node".to_owned(),
        pair_token: "pair-token".to_owned(),
        allowed_pair_ip: None,
        model: "runtime-model".to_owned(),
        live: None,
    })
    .expect("runtime")
}

#[test]
fn live_bridge_projection_keeps_each_round_as_its_own_card() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            continue_single_response("first round continue"),
            tool_use_single_response(),
            complete_single_response("final round done"),
        ],
    );

    let request = live_request(false);
    fs::create_dir_all(&request.runtime_home).expect("create runtime home");
    fs::write(request.runtime_home.join("Cargo.toml"), "[workspace]\n")
        .expect("write master workspace fixture");
    let mut request = request;
    request.cwd = Some(request.runtime_home.clone());

    let outcome = run_worker_live_reason_turn(
        &live_selected_worker_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("worker live bridge");
    let first_request = rx.recv().expect("first request");
    let second_request = rx.recv().expect("second request");
    let third_request = rx.recv().expect("third request");
    handle.join().expect("join");

    assert!(first_request.contains("reply exactly pong"));
    assert!(second_request.contains("first round continue"));
    assert!(second_request.contains("\"tools\""));
    assert!(third_request.contains("\"type\":\"tool_result\""));
    assert_eq!(outcome.rounds, 3);
    assert_eq!(outcome.tool_executions, 1);

    let first_round_projection = project_runtime_turn_history(
        &AgentId::new("agent-live"),
        "agent-live-node",
        std::slice::from_ref(&outcome.turns[1]),
        None,
    );
    let first_round_public = freehand_ui_protocol::public_turn_projection(first_round_projection);
    assert_eq!(
        first_round_public.public_conversation[0].body,
        "reply exactly pong"
    );
    assert!(
        first_round_public
            .public_conversation
            .iter()
            .any(
                |item| item.kind == freehand_ui_protocol::UiConversationItemKind::ToolSummary
                    && item.status == "completed"
            )
    );
    assert!(
        first_round_public
            .public_conversation
            .iter()
            .all(|item| !item.body.contains("final round done"))
    );

    let final_projection = project_runtime_turn_history(
        &AgentId::new("agent-live"),
        "agent-live-node",
        std::slice::from_ref(&outcome.turn),
        None,
    );
    let public = freehand_ui_protocol::public_turn_projection(final_projection);
    assert_eq!(public.public_conversation[0].body, "reply exactly pong");
    assert!(
        public
            .public_conversation
            .iter()
            .all(|item| { item.kind != freehand_ui_protocol::UiConversationItemKind::ToolSummary })
    );
    assert!(
        public
            .public_conversation
            .iter()
            .all(|item| !item.body.contains("first round continue"))
    );
    assert!(
        public
            .public_conversation
            .iter()
            .any(|item| item.body.contains("final round done"))
    );
}

#[test]
fn master_lifecycle_closes_in_same_round_as_target_task_mutation() {
    let runtime_home = temp_runtime_home();
    let runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    create_lifecycle_test_worker(&runtime);
    let task = create_lifecycle_test_task(&runtime, "lifecycle-target");
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![task_tool_use_response(
            "toolu_lifecycle_assign",
            json!({
                "op": "assign",
                "task_id": task.task_id.as_str(),
                "agent_id": "worker"
            }),
        )],
    );

    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    set_single_worker_peer(&mut selected, "worker");
    let outcome = run_master_lifecycle_reason_turn(
        &selected,
        lifecycle_live_request(&runtime_home, "lifecycle-target-event"),
        LiveReasonTaskDecisionBoundary {
            task_id: task.task_id.clone(),
            initial_event_seq: task.last_event_seq,
            mode: LiveReasonTaskDecisionMode::TargetMutation,
            max_rounds: 8,
        },
    )
    .expect("lifecycle decision");

    assert_eq!(outcome.rounds, 1);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
    assert_eq!(
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
            .expect("reload task runtime")
            .query_task(&task.task_id)
            .expect("assigned task")
            .assignee
            .expect("configured worker assignee")
            .agent_id,
        AgentId::new("worker")
    );
    let history = TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
        .expect("reload task history runtime")
        .task_history(&task.task_id)
        .expect("task history");
    assert_eq!(
        history
            .iter()
            .filter(|event| event.event_type == "TaskAssigned")
            .count(),
        1
    );
    let _ = rx.recv().expect("single provider request");
    assert!(
        rx.try_recv().is_err(),
        "decision must not request another round"
    );
    handle.join().expect("join provider");
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn master_assignment_gate_pairs_failure_then_accepts_configured_worker() {
    let runtime_home = temp_runtime_home();
    let runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    create_lifecycle_test_worker(&runtime);
    runtime
        .create_agent(AgentCreateRequest {
            agent_id: AgentId::new("historical-worker"),
            capabilities: vec!["workspace".to_owned()],
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("create-historical-worker"),
        })
        .expect("create historical worker");
    let task = create_lifecycle_test_task(&runtime, "lifecycle-assignment-gate");
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            task_tool_use_response(
                "toolu_assign_historical",
                json!({
                    "op": "assign",
                    "task_id": task.task_id.as_str(),
                    "agent_id": "historical-worker"
                }),
            ),
            task_tool_use_response(
                "toolu_assign_configured",
                json!({
                    "op": "assign",
                    "task_id": task.task_id.as_str(),
                    "agent_id": "worker"
                }),
            ),
        ],
    );

    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    set_single_worker_peer(&mut selected, "worker");
    let outcome = run_master_lifecycle_reason_turn(
        &selected,
        lifecycle_live_request(&runtime_home, "lifecycle-assignment-gate-event"),
        LiveReasonTaskDecisionBoundary {
            task_id: task.task_id.clone(),
            initial_event_seq: task.last_event_seq,
            mode: LiveReasonTaskDecisionMode::TargetMutation,
            max_rounds: 8,
        },
    )
    .expect("corrected lifecycle assignment");

    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.tool_executions, 2);
    let requests = collect_provider_requests(&rx, 2);
    assert!(requests[1].contains(
        "Configured topology boundary: task assignment must target one configured Worker: `worker`."
    ));
    let reloaded =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("reload runtime");
    let assigned = reloaded.query_task(&task.task_id).expect("assigned task");
    assert_eq!(assigned.status, TaskStatus::Assigned);
    assert_eq!(
        assigned.assignee.expect("configured assignee").agent_id,
        AgentId::new("worker")
    );
    let history = reloaded.task_history(&task.task_id).expect("task history");
    let assigned_events = history
        .iter()
        .filter(|event| event.event_type == "TaskAssigned")
        .collect::<Vec<_>>();
    assert_eq!(assigned_events.len(), 1);
    assert_eq!(
        assigned_events[0]
            .payload
            .get("agent_id")
            .and_then(Value::as_str),
        Some("worker")
    );

    handle.join().expect("join provider");
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn master_assignment_gate_accepts_any_configured_worker_in_pool() {
    let runtime_home = temp_runtime_home();
    let runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    for worker_id in ["worker-alpha", "worker-beta", "worker-gamma"] {
        runtime
            .create_agent(AgentCreateRequest {
                agent_id: AgentId::new(worker_id),
                capabilities: vec!["workspace".to_owned()],
                actor: lifecycle_test_actor(),
                watermark: lifecycle_test_watermark(worker_id),
            })
            .expect("create configured worker");
    }
    runtime
        .create_agent(AgentCreateRequest {
            agent_id: AgentId::new("historical-worker"),
            capabilities: vec!["workspace".to_owned()],
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("historical"),
        })
        .expect("create historical worker");
    let task = create_lifecycle_test_task(&runtime, "lifecycle-worker-pool-gate");
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            task_tool_use_response(
                "toolu_assign_historical_pool",
                json!({
                    "op": "assign",
                    "task_id": task.task_id.as_str(),
                    "agent_id": "historical-worker"
                }),
            ),
            task_tool_use_response(
                "toolu_assign_beta_pool",
                json!({
                    "op": "assign",
                    "task_id": task.task_id.as_str(),
                    "agent_id": "worker-beta"
                }),
            ),
        ],
    );

    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    set_worker_peers(
        &mut selected,
        &["worker-alpha", "worker-beta", "worker-gamma"],
    );
    let outcome = run_master_lifecycle_reason_turn(
        &selected,
        lifecycle_live_request(&runtime_home, "lifecycle-worker-pool-gate-event"),
        LiveReasonTaskDecisionBoundary {
            task_id: task.task_id.clone(),
            initial_event_seq: task.last_event_seq,
            mode: LiveReasonTaskDecisionMode::TargetMutation,
            max_rounds: 8,
        },
    )
    .expect("corrected lifecycle assignment");

    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.tool_executions, 2);
    let requests = collect_provider_requests(&rx, 2);
    assert!(requests[1].contains("`worker-alpha`, `worker-beta`, `worker-gamma`"));
    let reloaded =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("reload runtime");
    let assigned = reloaded.query_task(&task.task_id).expect("assigned task");
    assert_eq!(assigned.status, TaskStatus::Assigned);
    assert_eq!(
        assigned.assignee.expect("configured assignee").agent_id,
        AgentId::new("worker-beta")
    );
    let history = reloaded.task_history(&task.task_id).expect("task history");
    assert_eq!(
        history
            .iter()
            .filter(|event| event.event_type == "TaskAssigned")
            .count(),
        1
    );

    handle.join().expect("join provider");
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn master_create_gate_rejects_implicit_dispatch_without_task_mutation() {
    let runtime_home = temp_runtime_home();
    let runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    create_lifecycle_test_worker(&runtime);
    let task_id = "lifecycle-create-gate";
    let create_payload = json!({
        "op": "create",
        "task_id": task_id,
        "title": "Lifecycle create gate",
        "content": "create one task without historical-agent dispatch",
        "goal": "prove configured Worker creation boundary",
        "deliverables": ["task truth"],
        "acceptance": ["only configured Worker is assigned"],
        "priority": 90,
        "target_cwd": std::env::temp_dir()
    });
    let mut corrected_create_payload = create_payload.clone();
    corrected_create_payload
        .as_object_mut()
        .expect("create payload object")
        .insert("dispatch".to_owned(), json!({"mode": "none"}));
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            task_tool_use_response("toolu_create_implicit_dispatch", create_payload),
            task_tool_use_response("toolu_create_explicit_none", corrected_create_payload),
            task_tool_use_response(
                "toolu_assign_configured_after_create",
                json!({
                    "op": "assign",
                    "task_id": task_id,
                    "agent_id": "worker"
                }),
            ),
            waiting_single_response("await configured Worker execution for the assigned task"),
        ],
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    set_single_worker_peer(&mut selected, "worker");

    let outcome = run_live_reason_turn(&selected, request).expect("corrected task creation flow");
    let requests = collect_provider_requests(&rx, 4);
    assert!(
        requests[1].contains("task creation must set dispatch.mode to `none` for later assignment")
    );
    assert!(requests[2].contains("Task created"));
    assert!(requests[3].contains("Task assigned"));
    assert_eq!(outcome.rounds, 4);
    assert_eq!(outcome.tool_executions, 3);

    let reloaded =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("reload runtime");
    let task = reloaded
        .query_task(&TaskId::new(task_id))
        .expect("created task");
    assert_eq!(task.status, TaskStatus::Assigned);
    assert_eq!(
        task.assignee.expect("configured assignee").agent_id,
        AgentId::new("worker")
    );
    let event_types = reloaded
        .task_history(&TaskId::new(task_id))
        .expect("task history")
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    assert_eq!(
        event_types,
        vec!["TaskCreated", "TaskWaitingAgent", "TaskAssigned"]
    );

    handle.join().expect("join provider");
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn master_lifecycle_ignores_unrelated_task_mutation() {
    let runtime_home = temp_runtime_home();
    let runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    create_lifecycle_test_worker(&runtime);
    let target = create_lifecycle_test_task(&runtime, "lifecycle-target");
    let unrelated = create_lifecycle_test_task(&runtime, "lifecycle-unrelated");
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            task_tool_use_response(
                "toolu_lifecycle_assign_unrelated",
                json!({
                    "op": "assign",
                    "task_id": unrelated.task_id.as_str(),
                    "agent_id": "worker"
                }),
            ),
            complete_single_response("unrelated mutation observed"),
        ],
    );

    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    set_single_worker_peer(&mut selected, "worker");
    let outcome = run_master_lifecycle_reason_turn(
        &selected,
        lifecycle_live_request(&runtime_home, "lifecycle-unrelated-event"),
        LiveReasonTaskDecisionBoundary {
            task_id: target.task_id.clone(),
            initial_event_seq: target.last_event_seq,
            mode: LiveReasonTaskDecisionMode::TargetMutation,
            max_rounds: 8,
        },
    )
    .expect("lifecycle decision");

    assert_eq!(outcome.rounds, 2);
    assert_eq!(
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
            .expect("reload target runtime")
            .query_task(&target.task_id)
            .expect("target task")
            .status,
        TaskStatus::WaitingAgent
    );
    assert_eq!(
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
            .expect("reload unrelated runtime")
            .query_task(&unrelated.task_id)
            .expect("unrelated task")
            .status,
        TaskStatus::Assigned
    );
    let _ = collect_provider_requests(&rx, 2);
    handle.join().expect("join provider");
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn master_lifecycle_round_budget_closes_blocked_without_mutation() {
    let runtime_home = temp_runtime_home();
    let runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    let target = create_lifecycle_test_task(&runtime, "lifecycle-budget");
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            continue_single_response("keep waiting"),
            continue_single_response("still waiting"),
        ],
    );

    let outcome = run_master_lifecycle_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        lifecycle_live_request(&runtime_home, "lifecycle-budget-event"),
        LiveReasonTaskDecisionBoundary {
            task_id: target.task_id.clone(),
            initial_event_seq: target.last_event_seq,
            mode: LiveReasonTaskDecisionMode::TargetMutation,
            max_rounds: 2,
        },
    )
    .expect("budget closeout");

    assert_eq!(outcome.rounds, 2);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Blocked)
    );
    assert!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .expect("terminal")
            .summary
            .contains("exceeded the 2-round budget")
    );
    assert_eq!(
        runtime
            .query_task(&target.task_id)
            .expect("unchanged target")
            .status,
        TaskStatus::WaitingAgent
    );
    let _ = collect_provider_requests(&rx, 2);
    handle.join().expect("join provider");
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bootstrap_restores_all_persisted_sessions_into_ui_state() {
    let runtime_home = temp_runtime_home();
    let (base_url_a, rx_a, handle_a) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response("answer a")],
    );
    run_live_reason_turn(
        &live_selected_agent(base_url_a, freehand_config::ProviderType::Anthropic),
        live_request_for(&runtime_home, "runtime-session-agent-live", 1),
    )
    .expect("persist session a");
    let _ = rx_a.recv().expect("provider request a");
    handle_a.join().expect("join a");

    let (base_url_b, rx_b, handle_b) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response("answer b")],
    );
    run_live_reason_turn(
        &live_selected_agent(base_url_b, freehand_config::ProviderType::Anthropic),
        live_request_for(&runtime_home, "runtime-session-other", 2),
    )
    .expect("persist session b");
    let _ = rx_b.recv().expect("provider request b");
    handle_b.join().expect("join b");

    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let session_list = runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionList)
        .expect("session list query");
    match session_list {
        UiQueryResult::SessionList(list) => {
            assert!(
                list.sessions.is_empty(),
                "turn-only persisted sessions stay out of the metadata-owned session list"
            );
        }
        other => panic!("unexpected session list query: {other:?}"),
    }

    let transcript = runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: SessionId::new("runtime-session-other"),
        })
        .expect("runtime session turns query")
        .expect("runtime query result");
    match transcript {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 1);
            assert_eq!(
                transcript.turns[0].user_text.as_deref(),
                Some("prompt for runtime-session-other")
            );
            assert!(
                transcript.turns[0]
                    .terminal_text
                    .as_deref()
                    .is_some_and(|text| text.contains("answer b"))
            );
        }
        other => panic!("unexpected session turns query: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_session_list_refreshes_current_agent_persistence_without_cross_agent_leakage() {
    let runtime_home = temp_runtime_home();
    let selected = live_selected_agent(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::Anthropic,
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let local_session = SessionId::new("background-local-session");
    ReasonPersistence::new(runtime_home.clone(), AgentId::new(selected.name.clone()))
        .create_session_metadata(
            local_session.clone(),
            Some("Background local session".to_owned()),
            None,
        )
        .expect("persist local background session");
    ReasonPersistence::new(runtime_home.clone(), AgentId::new("agent-live-worker"))
        .create_session_metadata(
            SessionId::new("foreign-worker-session"),
            Some("Foreign worker session".to_owned()),
            None,
        )
        .expect("persist foreign background session");

    let result = runtime
        .query_runtime(&UiCommand::QuerySessionList)
        .expect("runtime query")
        .expect("runtime-owned session list");
    match result {
        UiQueryResult::SessionList(list) => {
            assert!(
                list.sessions
                    .iter()
                    .any(|row| row.session_id == local_session)
            );
            assert!(
                list.sessions
                    .iter()
                    .all(|row| row.session_id.as_str() != "foreign-worker-session")
            );
        }
        other => panic!("unexpected session list result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn live_bridge_restores_same_session_history_into_follow_up_provider_request() {
    let runtime_home = temp_runtime_home();
    let session_id = SessionId::new("runtime-session-history");
    let first_request = LiveReasonTurnRequest {
        runtime_home: runtime_home.clone(),
        session_id: session_id.clone(),
        turn_id: TurnId::new("runtime-turn-1"),
        trace_id: TraceId::new("runtime-trace-1"),
        prompt: "first history prompt".to_owned(),
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: None,
        execution_profile: LiveReasonExecutionProfile::Workspace,
        stream: false,
        cancel_token: None,
    };
    let (base_url_first, rx_first, handle_first) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response("first history answer")],
    );
    let first_outcome = run_live_reason_turn(
        &live_selected_agent(base_url_first, freehand_config::ProviderType::Anthropic),
        first_request,
    )
    .expect("first request");
    let raw_first = rx_first.recv().expect("first provider request");
    handle_first.join().expect("join first provider");
    assert!(raw_first.contains("first history prompt"));
    assert_eq!(
        first_outcome.restore_status,
        LiveReasonRestoreStatus::CreatedNew
    );

    let second_request = LiveReasonTurnRequest {
        runtime_home: runtime_home.clone(),
        session_id: session_id.clone(),
        turn_id: TurnId::new("runtime-turn-2"),
        trace_id: TraceId::new("runtime-trace-2"),
        prompt: "second history prompt".to_owned(),
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: None,
        execution_profile: LiveReasonExecutionProfile::Workspace,
        stream: false,
        cancel_token: None,
    };
    let (base_url_second, rx_second, handle_second) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response("second history answer")],
    );
    let second_outcome = run_live_reason_turn(
        &live_selected_agent(base_url_second, freehand_config::ProviderType::Anthropic),
        second_request,
    )
    .expect("second request");
    let raw_second = rx_second.recv().expect("second provider request");
    handle_second.join().expect("join second provider");

    assert_eq!(
        second_outcome.restore_status,
        LiveReasonRestoreStatus::RestoredExisting
    );
    assert_eq!(second_outcome.restored_closed_turns, 1);
    assert!(raw_second.contains("Historical turn 1 (round 1):"));
    assert!(raw_second.contains("User: first history prompt"));
    assert!(raw_second.contains("Assistant: first history answer"));
    assert!(raw_second.contains("second history prompt"));

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn effective_context_uses_last_repaired_round_without_raw_failed_attempt() {
    let session_id = SessionId::new("runtime-session-repair-context");
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("history");
    let failed_round = closed_turn_for_context(
        &mut history,
        &session_id,
        "runtime-turn-7",
        "trace-7",
        "repair this task",
        TerminalStatus::Failed,
        "failed attempt details that should stay out of future prompt",
    );
    let repaired_round = closed_turn_for_context(
        &mut history,
        &session_id,
        "runtime-turn-7-r2",
        "trace-7-r2",
        "repair this task",
        TerminalStatus::Success,
        "repaired success summary",
    );
    let unrelated_turn = closed_turn_for_context(
        &mut history,
        &session_id,
        "runtime-turn-8",
        "trace-8",
        "next independent task",
        TerminalStatus::Success,
        "next task summary",
    );

    let segments = effective_turn_context_segments(&[failed_round, repaired_round, unrelated_turn]);
    let rendered = freehand_blocks::render_context_segments_as_text(&segments);

    assert_eq!(segments.len(), 2);
    assert!(rendered.contains("Historical turn 7 (round 2):"));
    assert!(rendered.contains("Assistant: repaired success summary"));
    assert!(rendered.contains("Historical turn 8 (round 1):"));
    assert!(
        !rendered.contains("failed attempt details that should stay out of future prompt"),
        "superseded failed repair attempt leaked into future context: {rendered}"
    );
}

#[test]
fn effective_context_hides_internal_parent_evaluation_prompt() {
    let session_id = SessionId::new("runtime-session-parent-eval-context");
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("history");
    let turn = closed_turn_for_context(
        &mut history,
        &session_id,
        "runtime-turn-23",
        "master-parent-evaluate-trace-23",
        "<freehand_parent_evaluation id=\"internal-parent-eval\">\ninternal framework prompt",
        TerminalStatus::Success,
        "parent evaluation final answer",
    );

    let segments = effective_turn_context_segments(&[turn]);
    let rendered = freehand_blocks::render_context_segments_as_text(&segments);

    assert_eq!(segments.len(), 1);
    assert!(
        !rendered.contains("<freehand_parent_evaluation"),
        "internal parent-evaluation prompt leaked into future provider context: {rendered}"
    );
    assert!(
        !rendered.contains("internal framework prompt"),
        "internal parent-evaluation prompt body leaked into future provider context: {rendered}"
    );
    assert!(
        rendered.contains("Assistant: parent evaluation final answer"),
        "parent evaluation terminal summary should remain usable context: {rendered}"
    );
}

#[test]
fn runtime_query_session_turns_restores_background_parent_evaluation() {
    let runtime_home = temp_runtime_home();
    let selected = live_selected_agent(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::Anthropic,
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let session_id = SessionId::new("parent-session-background-evaluation");
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("session history");
    let engine = ReasonTurnEngine::new();
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: TurnId::new("runtime-turn-1"),
                trace_id: TraceId::new("master-parent-evaluate-trace-event-1-attempt-0"),
                feature_id: FeatureId::new("runtime.master-worker-loop"),
                agent_id: AgentId::new("master"),
                user_text: "<freehand_parent_evaluation id=\"background-test\">\ninternal prompt"
                    .to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "master-model".to_owned(),
            },
        )
        .expect("start evaluation turn");
    let persistence =
        ReasonPersistence::new(runtime_home.clone(), AgentId::new(selected.name.clone()));
    persistence
        .record_turn_started(&history, &turn, 0)
        .expect("persist evaluation start");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        trace_id: turn.request.trace_id.clone(),
        feature_id: FeatureId::new("runtime.master-worker-loop"),
        agent_id: AgentId::new("master"),
        status: TerminalStatus::Success,
        summary: "overall goal evaluated; next task or final answer decided".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .expect("persist evaluation close");
    let timer_internal_prompt = "You are the production Master starting a new follow-up turn injected by a due timer.\n\
This is a new turn in the source session, not a resume or reopening of the source turn.\n\
Use current framework truth and the injected timer prompt; do not assume task state from memory.\n\
\n\
Injected timer prompt:\ninspect current Task Center truth";
    let mut timer_turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: session_id.clone(),
                    turn_id: TurnId::new("runtime-turn-2"),
                    trace_id: TraceId::new("master-timer-trace-event-2"),
                    feature_id: FeatureId::new("runtime.master-worker-loop"),
                    agent_id: AgentId::new("master"),
                    user_text:
                        "The tool result has been returned. Use it to continue the task, then provide the required Freehand completion schema when ready."
                            .to_owned(),
                    planned_context_segments: vec![original_task_segment(timer_internal_prompt)],
                    tool_schema_fingerprint: None,
                    model: "master-model".to_owned(),
                },
            )
            .expect("start timer turn");
    persistence
        .record_turn_started(&history, &timer_turn, 0)
        .expect("persist timer start");
    timer_turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: timer_turn.request.turn_id.clone(),
        trace_id: timer_turn.request.trace_id.clone(),
        feature_id: FeatureId::new("runtime.master-worker-loop"),
        agent_id: AgentId::new("master"),
        status: TerminalStatus::ToolPending,
        summary: "timer follow-up scheduled next inspection".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &timer_turn, 0)
        .expect("persist timer close");

    match runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("runtime query")
        .expect("runtime-owned session query")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.session_id, session_id);
            assert_eq!(transcript.turns.len(), 2);
            assert_eq!(transcript.turns[0].user_text, None);
            assert!(
                transcript.turns[0]
                    .terminal_text
                    .as_deref()
                    .is_some_and(|text| text.contains("overall goal evaluated"))
            );
            assert_eq!(transcript.turns[1].user_text, None);
            assert!(
                transcript.turns[1]
                    .terminal_text
                    .as_deref()
                    .is_some_and(|text| text.contains("timer follow-up scheduled"))
            );
        }
        other => panic!("unexpected session query result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn runtime_query_session_search_returns_worker_hits_under_parent_session() {
    let runtime_home = temp_runtime_home();
    let selected = live_selected_agent(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::Anthropic,
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let master_agent_id = AgentId::new(selected.name.clone());
    let worker_agent_id = AgentId::new("agent-live-worker");
    let parent_session_id = SessionId::new("webui-search-parent-session");
    let task_id = TaskId::new("task-search-child");
    let worker_session_id = worker_session_id_for_task(&task_id);
    let metadata_only_session_id = SessionId::new("webui-search-metadata-only-session");

    let master_persistence = ReasonPersistence::new(runtime_home.clone(), master_agent_id.clone());
    master_persistence
        .create_session_metadata(
            parent_session_id.clone(),
            Some("Roadmap Search Parent".to_owned()),
            None,
        )
        .expect("persist parent metadata");
    master_persistence
        .create_session_metadata(
            metadata_only_session_id.clone(),
            Some("Metadata Only needle-metadata".to_owned()),
            None,
        )
        .expect("persist metadata-only session");
    persist_search_fixture_turn(
        &master_persistence,
        &master_agent_id,
        &parent_session_id,
        "runtime-turn-parent",
        "parent turn summary with roadmap keyword",
    );

    let task_runtime =
        TaskRuntime::boot(&runtime_home, master_agent_id.clone()).expect("task runtime");
    task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(task_id.clone()),
            title: "Worker child search task".to_owned(),
            content: "worker child content".to_owned(),
            goal: "prove child search result parenting".to_owned(),
            deliverables: vec!["child result".to_owned()],
            acceptance: vec!["child result is nested".to_owned()],
            priority: 10,
            target_cwd: None,
            execution_profile: TaskExecutionProfile::Workspace,
            dispatch: TaskDispatchRequest::None,
            parent: TaskParentRef {
                session_id: Some(parent_session_id.clone()),
                turn_id: Some(TurnId::new("runtime-turn-parent")),
                trace_id: None,
            },
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("session-search-child-task"),
        })
        .expect("create parented task");

    let worker_persistence = ReasonPersistence::new(runtime_home.clone(), worker_agent_id);
    persist_search_fixture_turn(
        &worker_persistence,
        &AgentId::new("agent-live-worker"),
        &worker_session_id,
        "worker-turn-child",
        "worker child transcript contains rare needle-token",
    );

    match runtime
        .query_runtime(&UiCommand::QuerySessionSearch {
            query: "needle-token".to_owned(),
            limit: Some(10),
        })
        .expect("runtime query")
        .expect("runtime-owned session search")
    {
        UiQueryResult::SessionSearch(search) => {
            assert_eq!(search.query, "needle-token");
            assert_eq!(search.results.len(), 1);
            let result = &search.results[0];
            assert_eq!(result.session_id, parent_session_id);
            assert!(
                !result.session_id.as_str().starts_with("worker-task-"),
                "worker sessions must not be top-level search results"
            );
            assert_eq!(result.child_matches.len(), 1);
            assert_eq!(result.child_matches[0].session_id, worker_session_id);
            assert_eq!(
                result.child_matches[0].task_id.as_deref(),
                Some(task_id.as_str())
            );
            assert!(result.child_matches[0].snippet.contains("needle-token"));
        }
        other => panic!("unexpected session search result: {other:?}"),
    }

    match runtime
        .query_runtime(&UiCommand::QuerySessionSearch {
            query: "Roadmap".to_owned(),
            limit: Some(10),
        })
        .expect("runtime query")
        .expect("runtime-owned session search")
    {
        UiQueryResult::SessionSearch(search) => {
            assert_eq!(search.results.len(), 1);
            assert_eq!(search.results[0].session_id, parent_session_id);
            assert!(
                search.results[0]
                    .matched_fields
                    .contains(&"title".to_owned())
            );
        }
        other => panic!("unexpected session search result: {other:?}"),
    }

    match runtime
        .query_runtime(&UiCommand::QuerySessionSearch {
            query: "needle-metadata".to_owned(),
            limit: Some(10),
        })
        .expect("runtime query")
        .expect("runtime-owned session search")
    {
        UiQueryResult::SessionSearch(search) => {
            assert_eq!(search.results.len(), 1);
            assert_eq!(search.results[0].session_id, metadata_only_session_id);
            assert_eq!(search.results[0].latest_status, "session");
            assert!(
                search.results[0]
                    .matched_fields
                    .contains(&"title".to_owned())
            );
        }
        other => panic!("unexpected session search result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

fn persist_search_fixture_turn(
    persistence: &ReasonPersistence,
    agent_id: &AgentId,
    session_id: &SessionId,
    turn_id: &str,
    summary: &str,
) {
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("session history");
    let engine = ReasonTurnEngine::new();
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: TurnId::new(turn_id),
                trace_id: TraceId::new(format!("{turn_id}-trace")),
                feature_id: FeatureId::new("reason.persistence"),
                agent_id: agent_id.clone(),
                user_text: format!("search fixture prompt for {turn_id}"),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "search-fixture-model".to_owned(),
            },
        )
        .expect("start search fixture turn");
    persistence
        .record_turn_started(&history, &turn, 0)
        .expect("persist fixture start");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        trace_id: turn.request.trace_id.clone(),
        feature_id: FeatureId::new("reason.persistence"),
        agent_id: agent_id.clone(),
        status: TerminalStatus::Success,
        summary: summary.to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .expect("persist fixture close");
}

#[test]
fn runtime_query_session_turns_preserves_live_provider_retry_activity() {
    let runtime_home = temp_runtime_home();
    let selected = live_selected_agent(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::Anthropic,
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let session_id = SessionId::new("active-live-activity-session");
    let turn_id = TurnId::new("runtime-turn-1");
    let trace_id = TraceId::new("runtime-trace-1");
    let agent_id = AgentId::new(selected.name.clone());
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("session history");
    let engine = ReasonTurnEngine::new();
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: trace_id.clone(),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: agent_id.clone(),
                user_text: "visible active prompt".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "active-model".to_owned(),
            },
        )
        .expect("start active turn");
    ReasonPersistence::new(runtime_home.clone(), agent_id.clone())
        .record_turn_started(&history, &turn, 0)
        .expect("persist active turn");

    {
        let ui_state = runtime.ui_state();
        let mut ui = ui_state.lock().expect("lock ui");
        ui.apply_model_request_waiting_kind(UiModelRequestWaiting {
            source_agent_id: agent_id.clone(),
            source_node_id: selected.node_id.clone(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            kind: UiModelRequestKind::Thinking,
            detail: Some("Waiting for model response.".to_owned()),
            transport: Some(UiModelTransportActivity {
                kind: UiModelTransportKind::ProviderRetry,
                detail: Some("provider retry 6/10".to_owned()),
            }),
            slave_substream_card: false,
        });
    }

    match runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("runtime query")
        .expect("runtime-owned session query")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.session_id, session_id);
            assert_eq!(transcript.turns.len(), 1);
            let turn = &transcript.turns[0];
            let model_request = turn
                .model_request
                .as_ref()
                .expect("live provider retry must survive runtime query refresh");
            assert_eq!(model_request.kind, UiModelRequestKind::Thinking);
            let transport = model_request
                .transport
                .as_ref()
                .expect("provider retry transport activity");
            assert_eq!(transport.kind, UiModelTransportKind::ProviderRetry);
            assert_eq!(transport.detail.as_deref(), Some("provider retry 6/10"));
            assert!(turn.tool_activities.is_empty());
        }
        other => panic!("unexpected session query result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn runtime_query_session_turns_projects_background_provider_retry_from_error_center() {
    let runtime_home = temp_runtime_home();
    let selected = live_selected_agent(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::Anthropic,
    );
    let session_id = SessionId::new("background-provider-retry-session");
    let turn_id = TurnId::new("runtime-turn-77-r6");
    let trace_id = TraceId::new("master-parent-evaluate-trace-background-retry");
    let agent_id = AgentId::new(selected.name.clone());
    let persistence = ReasonPersistence::new(runtime_home.clone(), agent_id.clone());
    persistence
        .create_session_metadata(
            session_id.clone(),
            Some("Background retry session".to_owned()),
            None,
        )
        .expect("persist session metadata");
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("session history");
    let engine = ReasonTurnEngine::new();
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: trace_id.clone(),
                feature_id: FeatureId::new("runtime.master-worker-loop"),
                agent_id: agent_id.clone(),
                user_text: "<freehand_parent_evaluation id=\"retry-test\">\ninternal prompt"
                    .to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "master-model".to_owned(),
            },
        )
        .expect("start background turn");
    persistence
        .record_turn_started(&history, &turn, 0)
        .expect("persist active background turn");
    write_error_center_retry_metadata(
        &runtime_home,
        &agent_id,
        &session_id,
        &turn_id,
        &trace_id,
        ErrorCenterRetryFixture {
            recovery_action: "retry_same_step",
            retry_index: 6,
            retry_cap: 10,
        },
    );

    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    match runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("runtime query")
        .expect("runtime-owned session query")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 1);
            let turn = &transcript.turns[0];
            assert_eq!(turn.user_text, None);
            let model_request = turn
                .model_request
                .as_ref()
                .expect("background provider retry must project as model request activity");
            assert_eq!(model_request.kind, UiModelRequestKind::Thinking);
            let transport = model_request
                .transport
                .as_ref()
                .expect("background provider retry transport activity");
            assert_eq!(transport.kind, UiModelTransportKind::ProviderRetry);
            assert_eq!(
                transport.detail.as_deref(),
                Some(
                    "provider retry 6/10: anthropic_http_status_500; error: internal_error: provider fixture 500; raw_hash=hash-only"
                )
            );
        }
        other => panic!("unexpected session query result: {other:?}"),
    }
    let session_list = runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionList)
        .expect("session list query");
    match session_list {
        UiQueryResult::SessionList(list) => {
            let summary = list
                .sessions
                .iter()
                .find(|summary| summary.session_id == session_id)
                .expect("session summary");
            assert_eq!(summary.active_turn_id.as_ref(), Some(&turn_id));
            assert_eq!(summary.latest_status, "waiting_model");
        }
        other => panic!("unexpected session list result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn runtime_query_session_turns_does_not_reactivate_terminal_error_center_retry() {
    let runtime_home = temp_runtime_home();
    let selected = live_selected_agent(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::Anthropic,
    );
    let session_id = SessionId::new("terminal-provider-retry-session");
    let turn_id = TurnId::new("runtime-turn-78");
    let trace_id = TraceId::new("master-parent-evaluate-trace-terminal-retry");
    let agent_id = AgentId::new(selected.name.clone());
    let persistence = ReasonPersistence::new(runtime_home.clone(), agent_id.clone());
    persistence
        .create_session_metadata(
            session_id.clone(),
            Some("Terminal retry session".to_owned()),
            None,
        )
        .expect("persist session metadata");
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("session history");
    let engine = ReasonTurnEngine::new();
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: trace_id.clone(),
                feature_id: FeatureId::new("runtime.master-worker-loop"),
                agent_id: agent_id.clone(),
                user_text: "<freehand_parent_evaluation id=\"terminal-test\">\ninternal prompt"
                    .to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "master-model".to_owned(),
            },
        )
        .expect("start terminal turn");
    persistence
        .record_turn_started(&history, &turn, 0)
        .expect("persist active turn");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        trace_id: trace_id.clone(),
        feature_id: FeatureId::new("runtime.master-worker-loop"),
        agent_id: agent_id.clone(),
        status: TerminalStatus::Success,
        summary: "terminal evaluation already closed".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .expect("persist terminal turn");
    write_error_center_retry_metadata(
        &runtime_home,
        &agent_id,
        &session_id,
        &turn_id,
        &trace_id,
        ErrorCenterRetryFixture {
            recovery_action: "retry_same_step",
            retry_index: 6,
            retry_cap: 10,
        },
    );

    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    match runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("runtime query")
        .expect("runtime-owned session query")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 1);
            let turn = &transcript.turns[0];
            assert_eq!(turn.terminal_status, Some(TerminalStatus::Success));
            assert_eq!(turn.model_request, None);
        }
        other => panic!("unexpected session query result: {other:?}"),
    }
    let session_list = runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionList)
        .expect("session list query");
    match session_list {
        UiQueryResult::SessionList(list) => {
            let summary = list
                .sessions
                .iter()
                .find(|summary| summary.session_id == session_id)
                .expect("session summary");
            assert_eq!(summary.active_turn_id, None);
            assert_eq!(summary.latest_status, "success");
        }
        other => panic!("unexpected session list result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn runtime_query_session_turns_does_not_reactivate_historical_retry_before_later_terminal_round() {
    let runtime_home = temp_runtime_home();
    let selected = live_selected_agent(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::Anthropic,
    );
    let session_id = SessionId::new("historical-provider-retry-session");
    let retry_turn_id = TurnId::new("runtime-turn-79");
    let terminal_turn_id = TurnId::new("runtime-turn-79-r2");
    let retry_trace_id = TraceId::new("master-parent-evaluate-trace-historical-retry");
    let terminal_trace_id = TraceId::new("master-parent-evaluate-trace-historical-terminal");
    let agent_id = AgentId::new(selected.name.clone());
    let persistence = ReasonPersistence::new(runtime_home.clone(), agent_id.clone());
    persistence
        .create_session_metadata(
            session_id.clone(),
            Some("Historical retry session".to_owned()),
            None,
        )
        .expect("persist session metadata");
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("session history");
    let engine = ReasonTurnEngine::new();
    let retry_turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: retry_turn_id.clone(),
                trace_id: retry_trace_id.clone(),
                feature_id: FeatureId::new("runtime.master-worker-loop"),
                agent_id: agent_id.clone(),
                user_text: "<freehand_parent_evaluation id=\"historical-retry\">\ninternal prompt"
                    .to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "master-model".to_owned(),
            },
        )
        .expect("start historical retry turn");
    persistence
        .record_turn_started(&history, &retry_turn, 0)
        .expect("persist historical retry turn");
    let mut terminal_turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: terminal_turn_id.clone(),
                trace_id: terminal_trace_id.clone(),
                feature_id: FeatureId::new("runtime.master-worker-loop"),
                agent_id: agent_id.clone(),
                user_text:
                    "<freehand_parent_evaluation id=\"historical-terminal\">\ninternal prompt"
                        .to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "master-model".to_owned(),
            },
        )
        .expect("start later terminal turn");
    persistence
        .record_turn_started(&history, &terminal_turn, 0)
        .expect("persist later terminal turn");
    terminal_turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: terminal_turn.request.turn_id.clone(),
        trace_id: terminal_trace_id.clone(),
        feature_id: FeatureId::new("runtime.master-worker-loop"),
        agent_id: agent_id.clone(),
        status: TerminalStatus::Success,
        summary: "later evaluation already closed".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &terminal_turn, 0)
        .expect("persist later terminal turn");
    write_error_center_retry_metadata(
        &runtime_home,
        &agent_id,
        &session_id,
        &retry_turn_id,
        &retry_trace_id,
        ErrorCenterRetryFixture {
            recovery_action: "retry_same_step",
            retry_index: 6,
            retry_cap: 10,
        },
    );

    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    match runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("runtime query")
        .expect("runtime-owned session query")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 2);
            let retry_round = transcript
                .turns
                .iter()
                .find(|turn| turn.turn_id == retry_turn_id)
                .expect("retry round");
            assert_eq!(retry_round.model_request, None);
            let terminal_round = transcript
                .turns
                .iter()
                .find(|turn| turn.turn_id == terminal_turn_id)
                .expect("terminal round");
            assert_eq!(
                terminal_round.terminal_status,
                Some(TerminalStatus::Success)
            );
            assert_eq!(terminal_round.model_request, None);
        }
        other => panic!("unexpected session query result: {other:?}"),
    }
    let session_list = runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionList)
        .expect("session list query");
    match session_list {
        UiQueryResult::SessionList(list) => {
            let summary = list
                .sessions
                .iter()
                .find(|summary| summary.session_id == session_id)
                .expect("session summary");
            assert_eq!(summary.latest_turn_id.as_ref(), Some(&terminal_turn_id));
            assert_eq!(summary.active_turn_id, None);
            assert_eq!(summary.latest_status, "success");
        }
        other => panic!("unexpected session list result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

struct ErrorCenterRetryFixture<'a> {
    recovery_action: &'a str,
    retry_index: u64,
    retry_cap: u64,
}

fn write_error_center_retry_metadata(
    runtime_home: &Path,
    agent_id: &AgentId,
    session_id: &SessionId,
    turn_id: &TurnId,
    trace_id: &TraceId,
    fixture: ErrorCenterRetryFixture<'_>,
) {
    let ledger_path = metadata_ledger_path(runtime_home, agent_id, session_id);
    let mut center = MetadataCenter::with_ledger_path(&ledger_path).expect("metadata center");
    center
        .write(
            MetadataEnvelope::new(
                MetadataId::new(format!(
                    "error.center:{}:{}:{}",
                    trace_id.as_str(),
                    turn_id.as_str(),
                    fixture.recovery_action
                )),
                MetadataKind::RuntimeState,
                MetadataWriteOwner {
                    feature_id: FeatureId::new("error.center"),
                    crate_name: "freehand-control".to_owned(),
                    module_path: "freehand_control".to_owned(),
                    symbol_path: "classify_error_center_failure".to_owned(),
                },
                MetadataWriteNode {
                    pipeline_node: "RuntimeLive05ProviderError".to_owned(),
                    runtime_node_id: None,
                },
                MetadataSubject {
                    agent_id: Some(agent_id.clone()),
                    session_id: Some(session_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    trace_id: trace_id.clone(),
                },
                vec![
                    MetadataEntry {
                        key: "error.domain".to_owned(),
                        value: json!("provider"),
                    },
                    MetadataEntry {
                        key: "error.class".to_owned(),
                        value: json!("system"),
                    },
                    MetadataEntry {
                        key: "error.code".to_owned(),
                        value: json!("anthropic_http_status_500"),
                    },
                    MetadataEntry {
                        key: "error.source_owner".to_owned(),
                        value: json!("provider.reason-live-bridge"),
                    },
                    MetadataEntry {
                        key: "error.source_pipeline_node".to_owned(),
                        value: json!("RuntimeLive05ProviderError"),
                    },
                    MetadataEntry {
                        key: "error.recovery_action".to_owned(),
                        value: json!(fixture.recovery_action),
                    },
                    MetadataEntry {
                        key: "error.retry_index".to_owned(),
                        value: json!(fixture.retry_index),
                    },
                    MetadataEntry {
                        key: "error.retry_cap".to_owned(),
                        value: json!(fixture.retry_cap),
                    },
                    MetadataEntry {
                        key: "error.public_visibility".to_owned(),
                        value: json!("internal"),
                    },
                    MetadataEntry {
                        key: "error.owner_target".to_owned(),
                        value: json!("provider.reason-live-bridge"),
                    },
                    MetadataEntry {
                        key: "error.repair_fields".to_owned(),
                        value: json!([]),
                    },
                    MetadataEntry {
                        key: "error.raw_hash".to_owned(),
                        value: json!("hash-only"),
                    },
                    MetadataEntry {
                        key: "error.public_message".to_owned(),
                        value: json!("internal_error: provider fixture 500"),
                    },
                ],
            )
            .expect("error-center envelope"),
        )
        .expect("write error-center metadata");
}

#[test]
fn runtime_query_session_turns_preserves_live_tool_activity() {
    let runtime_home = temp_runtime_home();
    let selected = live_selected_agent(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::Anthropic,
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let session_id = SessionId::new("active-live-tool-session");
    let turn_id = TurnId::new("runtime-turn-1");
    let trace_id = TraceId::new("runtime-trace-1");
    let agent_id = AgentId::new(selected.name.clone());
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("session history");
    let engine = ReasonTurnEngine::new();
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: trace_id.clone(),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: agent_id.clone(),
                user_text: "visible active prompt".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "active-model".to_owned(),
            },
        )
        .expect("start active turn");
    ReasonPersistence::new(runtime_home.clone(), agent_id.clone())
        .record_turn_started(&history, &turn, 0)
        .expect("persist active turn");

    {
        let ui_state = runtime.ui_state();
        let mut ui = ui_state.lock().expect("lock ui");
        ui.apply_tool_call(
            agent_id.clone(),
            selected.node_id.clone(),
            &ReasonReq04ToolCall {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: trace_id.clone(),
                feature_id: FeatureId::new("tool.registry"),
                agent_id: agent_id.clone(),
                tool_call: ToolCallContract {
                    tool_call_id: ToolCallId::new("tool-live-refresh"),
                    tool_name: "task".to_owned(),
                    arguments: Vec::new(),
                    arguments_complete: true,
                },
            },
            false,
        );
    }

    match runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("runtime query")
        .expect("runtime-owned session query")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.session_id, session_id);
            assert_eq!(transcript.turns.len(), 1);
            let turn = &transcript.turns[0];
            assert!(turn.model_request.is_none());
            assert_eq!(turn.tool_activities.len(), 1);
            assert_eq!(
                turn.tool_activities[0].status,
                UiToolActivityStatus::Waiting
            );
            let tool_cards = freehand_ui_protocol::public_conversation_items(turn)
                .into_iter()
                .filter(|item| item.kind == UiConversationItemKind::ToolSummary)
                .collect::<Vec<_>>();
            assert_eq!(tool_cards.len(), 1);
            assert_eq!(tool_cards[0].status, "waiting");
        }
        other => panic!("unexpected session query result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn runtime_query_session_turns_restores_worker_task_namespace() {
    let runtime_home = temp_runtime_home();
    let selected = live_selected_agent(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::Anthropic,
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let worker_agent_id = AgentId::new(selected.worker_peer_names()[0].clone());
    let session_id = SessionId::new("worker-task-task-ui-proof");
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("history");
    let engine = ReasonTurnEngine::new();
    let mut turn = engine
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: session_id.clone(),
                    turn_id: TurnId::new("worker-turn-ui-proof"),
                    trace_id: TraceId::new("worker-trace-ui-proof"),
                    feature_id: FeatureId::new("runtime.master-worker-loop"),
                    agent_id: worker_agent_id.clone(),
                    user_text:
                        "The tool result has been returned. Use it to continue the task."
                            .to_owned(),
                    planned_context_segments: vec![original_task_segment(
                        "Execute the assigned Task Center task.\nTask ID: task-ui-proof\nTitle: internal task prompt must not render as a User message",
                    )],
                    tool_schema_fingerprint: None,
                    model: "worker-model".to_owned(),
                },
            )
            .expect("start worker turn");
    let worker_persistence = ReasonPersistence::new(runtime_home.clone(), worker_agent_id.clone());
    worker_persistence
        .record_turn_started(&history, &turn, 0)
        .expect("persist worker start");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        trace_id: turn.request.trace_id.clone(),
        feature_id: FeatureId::new("runtime.master-worker-loop"),
        agent_id: worker_agent_id.clone(),
        status: TerminalStatus::Success,
        summary: "worker task transcript restored".to_owned(),
    });
    worker_persistence
        .record_turn_closed(&history, &turn, 0)
        .expect("persist worker close");

    match runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("runtime query")
        .expect("runtime-owned session query")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.session_id, session_id);
            assert_eq!(transcript.turns.len(), 1);
            assert_eq!(transcript.turns[0].user_text, None);
            assert_eq!(
                transcript.turns[0].source.source_agent_id.as_str(),
                worker_agent_id.as_str()
            );
            assert!(
                transcript.turns[0]
                    .terminal_text
                    .as_deref()
                    .is_some_and(|text| text.contains("worker task transcript restored"))
            );
        }
        other => panic!("unexpected session query result: {other:?}"),
    }

    assert_eq!(
        runtime
            .query_runtime(&UiCommand::QuerySessionTurns {
                session_id: SessionId::new("worker-task-missing"),
            })
            .expect_err("missing Worker session must fail explicitly"),
        UiCommandDispatchPortError::TargetNotFound(
            "Worker session `worker-task-missing` has no persisted transcript".to_owned()
        )
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn live_worker_task_projection_hides_internal_user_text() {
    let ui_state = Arc::new(Mutex::new(UiProtocolState::new()));
    let session_id = SessionId::new("worker-task-task-live-proof");
    publish_live_pending_user_projection(
        &ui_state,
        &AgentId::new("worker"),
        "worker-node",
        &session_id,
        Path::new("/tmp"),
        &TurnId::new("worker-turn-live-proof"),
        "Execute the assigned Task Center task.\nTask ID: task-live-proof",
    );

    match ui_state
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("query worker transcript")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 1);
            assert_eq!(transcript.turns[0].session_id, session_id);
            assert_eq!(transcript.turns[0].user_text, None);
            assert!(
                transcript.turns[0]
                    .source
                    .source_agent_id
                    .as_str()
                    .contains("worker")
            );
        }
        other => panic!("unexpected query result: {other:?}"),
    }
}

#[test]
fn live_regular_session_projection_keeps_user_text() {
    let ui_state = Arc::new(Mutex::new(UiProtocolState::new()));
    let session_id = SessionId::new("regular-live-session-proof");
    publish_live_pending_user_projection(
        &ui_state,
        &AgentId::new("master"),
        "master-node",
        &session_id,
        Path::new("/tmp"),
        &TurnId::new("runtime-turn-live-proof"),
        "visible user prompt",
    );

    match ui_state
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("query regular transcript")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 1);
            assert_eq!(
                transcript.turns[0].user_text.as_deref(),
                Some("visible user prompt")
            );
        }
        other => panic!("unexpected query result: {other:?}"),
    }
}

fn closed_turn_for_context(
    history: &mut SessionHistory,
    session_id: &SessionId,
    turn_id: &str,
    trace_id: &str,
    prompt: &str,
    status: TerminalStatus,
    summary: &str,
) -> TurnRecord {
    let turn_id = TurnId::new(turn_id);
    let trace_id = TraceId::new(trace_id);
    let mut turn = ReasonTurnEngine::new()
        .start_turn(
            history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: trace_id.clone(),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-live"),
                user_text: prompt.to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model-a".to_owned(),
            },
        )
        .expect("turn");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id,
        trace_id,
        feature_id: FeatureId::new("provider.reason-live-bridge"),
        agent_id: AgentId::new("agent-live"),
        status,
        summary: summary.to_owned(),
    });
    turn
}

#[test]
fn runtime_dispatches_session_crud_into_shared_ui_projection() {
    let runtime_home = temp_runtime_home();
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let session_id = SessionId::new("session-crud-runtime");

    let create = build_command_dispatch_envelope(&UiCommand::CreateSession {
        session_id: session_id.clone(),
        title: Some("Initial".to_owned()),
        cwd: Some("/tmp".to_owned()),
    })
    .expect("create envelope");
    let receipt = runtime.dispatch(create).expect("create dispatch");
    assert_eq!(receipt.target_feature_id, "reason.persistence");
    assert_eq!(receipt.dispatch_status, "session_metadata_updated");

    let rename = build_command_dispatch_envelope(&UiCommand::RenameSession {
        session_id: session_id.clone(),
        title: "Renamed".to_owned(),
    })
    .expect("rename envelope");
    runtime.dispatch(rename).expect("rename dispatch");

    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionList)
        .expect("session list")
    {
        UiQueryResult::SessionList(list) => {
            assert_eq!(list.sessions.len(), 1);
            assert_eq!(list.sessions[0].session_id, session_id);
            assert_eq!(list.sessions[0].title.as_deref(), Some("Renamed"));
            assert!(!list.sessions[0].archived);
        }
        other => panic!("unexpected session list: {other:?}"),
    }

    let archive = build_command_dispatch_envelope(&UiCommand::ArchiveSession {
        session_id: session_id.clone(),
    })
    .expect("archive envelope");
    runtime.dispatch(archive).expect("archive dispatch");
    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QueryArchivedSessionList)
        .expect("archived list")
    {
        UiQueryResult::SessionList(list) => {
            assert_eq!(list.sessions.len(), 1);
            assert_eq!(list.sessions[0].session_id, session_id);
            assert!(list.sessions[0].archived);
        }
        other => panic!("unexpected archived list: {other:?}"),
    }

    let restore = build_command_dispatch_envelope(&UiCommand::RestoreSession {
        session_id: session_id.clone(),
    })
    .expect("restore envelope");
    runtime.dispatch(restore).expect("restore dispatch");
    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionList)
        .expect("active list")
    {
        UiQueryResult::SessionList(list) => {
            assert_eq!(list.sessions.len(), 1);
            assert_eq!(list.sessions[0].session_id, session_id);
            assert!(!list.sessions[0].archived);
        }
        other => panic!("unexpected active list: {other:?}"),
    }

    let missing = build_command_dispatch_envelope(&UiCommand::ArchiveSession {
        session_id: SessionId::new("missing-session"),
    })
    .expect("missing envelope");
    let err = runtime.dispatch(missing).expect_err("missing must fail");
    assert_eq!(
        err,
        UiCommandDispatchPortError::TargetNotFound("missing-session".to_owned())
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_timer_ui_commands_persist_and_project_owner_truth() {
    let runtime_home = temp_runtime_home();
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    match runtime
        .query_runtime(&UiCommand::QueryTimerList {
            include_terminal: true,
        })
        .expect("timer query")
        .expect("runtime timer projection")
    {
        UiQueryResult::TimerList(list) => {
            assert!(list.timers.is_empty());
            assert!(list.events.is_empty());
            assert_eq!(list.source_agent_id, AgentId::new("agent-live"));
        }
        other => panic!("unexpected timer query result: {other:?}"),
    }

    let schedule = build_command_dispatch_envelope(&UiCommand::ScheduleTimer {
        timer: UiTimerScheduleCommand {
            timer_id: Some("timer-ui-dispatch-proof".to_owned()),
            mode: "relative".to_owned(),
            delay_seconds: Some(300),
            run_at_unix_seconds: None,
            repeat: None,
            max_runs: Some(1),
            reason: "recheck delegated work".to_owned(),
            prompt: "Inspect TaskBoard and decide whether waiting work closed.".to_owned(),
            source_session_id: Some(SessionId::new("session-ui-dispatch")),
        },
    })
    .expect("schedule envelope");
    let receipt = runtime.dispatch(schedule).expect("schedule timer");
    assert_eq!(receipt.target_feature_id, "runtime.master-worker-loop");
    assert!(
        receipt
            .dispatch_status
            .starts_with("timer_scheduled:timer_id=timer-ui-dispatch-proof ")
    );

    match runtime
        .query_runtime(&UiCommand::QueryTimerList {
            include_terminal: false,
        })
        .expect("timer query")
        .expect("runtime timer projection")
    {
        UiQueryResult::TimerList(list) => {
            assert_eq!(list.timers.len(), 1);
            assert_eq!(list.timers[0].timer_id, "timer-ui-dispatch-proof");
            assert_eq!(list.timers[0].status, "active");
            assert_eq!(list.timers[0].reason, "recheck delegated work");
            assert_eq!(
                list.timers[0].source_session_id,
                Some(SessionId::new("session-ui-dispatch"))
            );
            assert_eq!(list.events.len(), 1);
            assert_eq!(list.events[0].event_type, "TimerScheduled");
        }
        other => panic!("unexpected timer query result: {other:?}"),
    }

    let cancel = build_command_dispatch_envelope(&UiCommand::CancelTimer {
        timer_id: "timer-ui-dispatch-proof".to_owned(),
    })
    .expect("cancel envelope");
    let receipt = runtime.dispatch(cancel).expect("cancel timer");
    assert_eq!(
        receipt.dispatch_status,
        "timer_cancelled:timer_id=timer-ui-dispatch-proof status=cancelled"
    );

    match runtime
        .query_runtime(&UiCommand::QueryTimerList {
            include_terminal: true,
        })
        .expect("timer query")
        .expect("runtime timer projection")
    {
        UiQueryResult::TimerList(list) => {
            assert_eq!(list.timers.len(), 1);
            assert_eq!(list.timers[0].status, "cancelled");
            assert!(
                list.events
                    .iter()
                    .any(|event| event.event_type == "TimerCancelled")
            );
        }
        other => panic!("unexpected timer query result: {other:?}"),
    }

    let missing = build_command_dispatch_envelope(&UiCommand::CancelTimer {
        timer_id: "missing-timer".to_owned(),
    })
    .expect("missing cancel envelope");
    let err = runtime.dispatch(missing).expect_err("missing timer fails");
    assert_eq!(
        err,
        UiCommandDispatchPortError::TargetNotFound("missing-timer".to_owned())
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_timer_ui_commands_reject_non_live_dispatcher() {
    let runtime = runtime();
    let schedule = build_command_dispatch_envelope(&UiCommand::ScheduleTimer {
        timer: UiTimerScheduleCommand {
            timer_id: Some("timer-non-live".to_owned()),
            mode: "relative".to_owned(),
            delay_seconds: Some(60),
            run_at_unix_seconds: None,
            repeat: None,
            max_runs: Some(1),
            reason: "non live proof".to_owned(),
            prompt: "This must not pretend to schedule without runtime home.".to_owned(),
            source_session_id: None,
        },
    })
    .expect("schedule envelope");
    let err = runtime
        .dispatch(schedule)
        .expect_err("non-live schedule fails");
    assert_eq!(
        err,
        UiCommandDispatchPortError::Unsupported(
            "timer scheduling requires a live runtime home".to_owned()
        )
    );
    assert!(
        runtime
            .query_runtime(&UiCommand::QueryTimerList {
                include_terminal: true,
            })
            .expect("non-live timer query")
            .is_none()
    );
}

#[test]
fn runtime_query_projects_tool_registry_owner_truth() {
    let runtime_home = temp_runtime_home();
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let result = runtime
        .query_runtime(&UiCommand::QueryToolRegistry)
        .expect("tool registry query")
        .expect("runtime-owned tool registry projection");

    match result {
        UiQueryResult::ToolRegistry(projection) => {
            assert_eq!(projection.source_agent_id, AgentId::new("agent-live"));
            assert_eq!(projection.registry_version, "reasonix-aligned-v1");
            assert!(
                projection
                    .guidance
                    .iter()
                    .any(|line| line.contains("exact JSON schema"))
            );
            let names = projection
                .tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>();
            assert!(names.contains(&"task"));
            assert!(names.contains(&"timer"));
            assert!(names.contains(&"web_fetch"));
            assert!(names.contains(&"read_file"));
            assert!(names.contains(&"glob"));
            assert!(names.contains(&"ls"));
            assert!(!names.contains(&"web_search"));

            let find = |name: &str| {
                projection
                    .tools
                    .iter()
                    .find(|tool| tool.name == name)
                    .unwrap_or_else(|| panic!("missing projected tool {name}: {names:?}"))
            };
            let task = find("task");
            assert!(task.exposed_to_master);
            assert!(!task.exposed_to_worker);
            assert_eq!(task.execution_scope, "framework");

            let timer = find("timer");
            assert!(timer.exposed_to_master);
            assert!(!timer.exposed_to_worker);
            assert_eq!(timer.execution_scope, "framework");

            let web_fetch = find("web_fetch");
            assert!(web_fetch.exposed_to_master);
            assert!(web_fetch.exposed_to_worker);
            assert_eq!(web_fetch.execution_scope, "network");

            let bash = find("bash");
            assert!(bash.implemented);
            assert!(!bash.exposed_to_master);
            assert!(!bash.exposed_to_worker);
            assert_eq!(bash.execution_scope, "shell");

            for worker_only in ["todo_write", "complete_step"] {
                let tool = find(worker_only);
                assert!(tool.exposed_to_worker);
                assert!(!tool.exposed_to_master);
            }

            let glob = find("glob");
            let glob_text = format!(
                "{}\n{}\n{}",
                glob.description,
                glob.examples.join("\n"),
                glob.guidance.join("\n")
            );
            assert!(glob_text.contains("locked workspace"));
            assert!(glob_text.contains("absolute"));
            assert!(glob_text.contains("symlink"));
            assert!(glob_text.contains("Leading-~") || glob_text.contains("leading `~`"));
        }
        other => panic!("unexpected tool registry query result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_query_projects_diagnostics_without_raw_secrets_or_absolute_home() {
    let runtime_home = temp_runtime_home();
    let logs_dir = runtime_home.join("logs");
    fs::create_dir_all(&logs_dir).expect("create logs dir");
    fs::write(
        logs_dir.join("daemonS.stdout.log"),
        "booting\nservice ready\nAuthorization: Bearer secret-token\nprovider request payload\nopened /Volumes/extension/code/freehand\n",
    )
    .expect("write diagnostics log");
    fs::write(logs_dir.join("ignore.txt"), "not a log").expect("write ignored file");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let result = runtime
        .query_runtime(&UiCommand::QueryDiagnostics)
        .expect("diagnostics query")
        .expect("runtime-owned diagnostics projection");

    match result {
        UiQueryResult::Diagnostics(projection) => {
            assert_eq!(projection.source_agent_id, AgentId::new("agent-live"));
            assert_eq!(projection.runtime_home, "~/.freehand");
            assert_eq!(projection.logs_dir, "logs");
            assert_eq!(projection.files.len(), 1);
            let file = &projection.files[0];
            assert_eq!(file.name, "daemonS.stdout.log");
            assert_eq!(file.relative_path, "logs/daemonS.stdout.log");
            assert!(file.size_bytes > 0);
            assert!(file.modified_at.is_some());
            let tail = file.tail_lines.join("\n");
            assert!(tail.contains("service ready"));
            assert!(tail.contains("[redacted diagnostic line: sensitive marker]"));
            assert!(!tail.contains("secret-token"));
            assert!(!tail.contains("provider request payload"));
            assert!(!tail.contains("/Volumes/extension"));
            assert!(!tail.contains(runtime_home.to_string_lossy().as_ref()));
        }
        other => panic!("unexpected diagnostics query result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_query_projects_config_status_without_secrets() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let config_path = runtime_home.join("config.toml");
    fs::write(
        &config_path,
        r#"
[providers.provider-live]
id = "provider-live"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://user:password@example.invalid:8443/v1?token=secret"
default_model = "MiniMax-M2.7"

[providers.provider-live.auth]
type = "apikey"
api_key_env = "FREEHAND_RUNTIME_CONFIG_QUERY_PROVIDER"

[agents.agent-live]
name = "agent-live"
mode = "master"
node_id = "agent-live-node"
paired_agents = ["agent-live-worker"]
pair_token = "FREEHAND_RUNTIME_CONFIG_QUERY_MASTER_TOKEN"
provider = "provider-live"
relay_url = "https://relay.example"
relay_token_env = "FREEHAND_RUNTIME_CONFIG_QUERY_RELAY_TOKEN"
local_web_url = "http://127.0.0.1:4142"

[agents.agent-live-worker]
name = "agent-live-worker"
mode = "slave"
node_id = "agent-live-worker-node"
paired_agents = ["agent-live"]
pair_token = "FREEHAND_RUNTIME_CONFIG_QUERY_WORKER_TOKEN"
provider = "provider-live"
relay_url = "https://relay.example/relay/"
relay_token_env = "FREEHAND_RUNTIME_CONFIG_QUERY_RELAY_TOKEN"
local_web_url = "http://127.0.0.1:4143"
"#,
    )
    .expect("write config");
    // SAFETY: this test owns these unique variable names and removes them before exit.
    unsafe {
        std::env::set_var("FREEHAND_RUNTIME_CONFIG_QUERY_PROVIDER", "test-api-key");
        std::env::set_var("FREEHAND_RUNTIME_CONFIG_QUERY_MASTER_TOKEN", "pair-token");
        std::env::set_var("FREEHAND_RUNTIME_CONFIG_QUERY_WORKER_TOKEN", "pair-token");
        std::env::set_var("FREEHAND_RUNTIME_CONFIG_QUERY_RELAY_TOKEN", "relay-token");
    }
    let selected = freehand_config::load_config_from_path(&config_path)
        .expect("load config")
        .select_agent("agent-live")
        .expect("select agent");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let result = runtime
        .query_runtime(&UiCommand::QueryConfigStatus)
        .expect("config query")
        .expect("runtime-owned result");

    match result {
        UiQueryResult::ConfigStatus(status) => {
            assert_eq!(status.agent_name, "agent-live");
            assert_eq!(status.agent_mode, "master");
            assert_eq!(status.node_id, "agent-live-node");
            assert_eq!(status.paired_agents.len(), 1);
            assert_eq!(status.paired_agents[0].agent_name, "agent-live-worker");
            assert_eq!(
                status.paired_agents[0].local_web_url.as_deref(),
                Some("http://127.0.0.1:4143")
            );
            assert_eq!(status.local_agent_directory.len(), 2);
            assert_eq!(status.local_agent_directory[0].agent_name, "agent-live");
            assert!(status.local_agent_directory[0].is_local);
            assert_eq!(
                status.local_agent_directory[0].relay_web_url.as_deref(),
                Some("https://relay.example/relay/agents/agent%2Dlive/")
            );
            assert_eq!(
                status.local_agent_directory[1].agent_name,
                "agent-live-worker"
            );
            assert!(status.local_agent_directory[1].is_local);
            assert_eq!(
                status.local_agent_directory[1].relay_web_url.as_deref(),
                Some("https://relay.example/relay/agents/agent%2Dlive%2Dworker/")
            );
            assert_eq!(status.provider_id, "provider-live");
            assert_eq!(status.provider_type, "anthropic");
            assert_eq!(status.provider_protocol, "messages");
            assert_eq!(status.provider_base_url, "https://example.invalid:8443/v1");
            assert_eq!(status.provider_base_url_host, "example.invalid");
            assert_eq!(status.default_model, "MiniMax-M2.7");
            assert_eq!(status.provider_web_search, "auto");
            assert_eq!(status.provider_web_search_effective, "hosted_declared");
            assert!(
                status
                    .provider_web_search_reason
                    .contains("anthropic/messages")
            );
            assert_eq!(status.provider_auth_type, "apikey");
            assert_eq!(status.provider_auth_source, "env");
            assert_eq!(status.provider_registry.len(), 1);
            assert_eq!(status.provider_registry[0].provider_id, "provider-live");
            assert_eq!(
                status.provider_registry[0].provider_web_search_effective,
                "hosted_declared"
            );
            assert_eq!(
                status.provider_registry[0].provider_base_url,
                "https://example.invalid:8443/v1"
            );
            assert!(status.restart_required_on_change);
            let encoded = serde_json::to_string(&status).expect("status json");
            assert!(!encoded.contains("test-api-key"));
            assert!(!encoded.contains("password"));
            assert!(!encoded.contains("token=secret"));
            assert!(!encoded.contains("api_key"));
            assert!(!encoded.contains("pair-token"));
        }
        other => panic!("unexpected query result: {other:?}"),
    }

    let remote = runtime
        .query_runtime_with_scope(&UiCommand::QueryConfigStatus, UiQueryAccessScope::Remote)
        .expect("remote config query")
        .expect("runtime-owned remote result");
    match remote {
        UiQueryResult::ConfigStatus(status) => {
            assert!(
                status
                    .paired_agents
                    .iter()
                    .all(|peer| peer.local_web_url.is_none())
            );
            assert!(
                status
                    .local_agent_directory
                    .iter()
                    .all(|agent| agent.web_url.is_none() && !agent.is_local)
            );
            assert!(
                status
                    .local_agent_directory
                    .iter()
                    .all(|agent| agent.relay_web_url.is_some())
            );
            let encoded = serde_json::to_string(&status).expect("remote status json");
            assert!(!encoded.contains("127.0.0.1"));
            assert!(!encoded.contains("relay-token"));
        }
        other => panic!("unexpected remote query result: {other:?}"),
    }

    // SAFETY: undo the test environment mutation before exit.
    unsafe {
        std::env::remove_var("FREEHAND_RUNTIME_CONFIG_QUERY_PROVIDER");
        std::env::remove_var("FREEHAND_RUNTIME_CONFIG_QUERY_MASTER_TOKEN");
        std::env::remove_var("FREEHAND_RUNTIME_CONFIG_QUERY_WORKER_TOKEN");
        std::env::remove_var("FREEHAND_RUNTIME_CONFIG_QUERY_RELAY_TOKEN");
    }
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatch_updates_provider_config_without_hot_reloading_active_model() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let config_path = runtime_home.join("config.toml");
    fs::write(
        &config_path,
        r#"
[providers.old]
id = "old"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://old.example.test/v1"
default_model = "old-model"

[providers.old.auth]
type = "apikey"
api_key_env = "FREEHAND_RUNTIME_PROVIDER_OLD"

[agents.agent-live]
name = "agent-live"
mode = "master"
node_id = "agent-live-node"
paired_agents = ["agent-live-worker"]
pair_token = "FREEHAND_RUNTIME_MASTER_TOKEN"
provider = "old"

[agents.agent-live-worker]
name = "agent-live-worker"
mode = "slave"
node_id = "agent-live-worker-node"
paired_agents = ["agent-live"]
pair_token = "FREEHAND_RUNTIME_WORKER_TOKEN"
provider = "old"
"#,
    )
    .expect("write config");
    // SAFETY: this test owns these unique variable names and removes them before exit.
    unsafe {
        std::env::set_var("FREEHAND_RUNTIME_PROVIDER_OLD", "old-secret");
        std::env::set_var("FREEHAND_RUNTIME_PROVIDER_NEW", "new-secret");
        std::env::set_var("FREEHAND_RUNTIME_MASTER_TOKEN", "pair-token");
        std::env::set_var("FREEHAND_RUNTIME_WORKER_TOKEN", "pair-token");
    }
    let selected = freehand_config::load_config_from_path(&config_path)
        .expect("load config")
        .select_agent("agent-live")
        .expect("select agent");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::UpdateProviderConfig {
                update: UiProviderConfigUpdate {
                    agent_name: "agent-live".to_owned(),
                    provider_id: "new-provider".to_owned(),
                    provider_type: "anthropic".to_owned(),
                    provider_protocol: "messages".to_owned(),
                    base_url: "https://new.example.test/v1".to_owned(),
                    default_model: "new-model".to_owned(),
                    web_search: "auto".to_owned(),
                    api_key_env: "FREEHAND_RUNTIME_PROVIDER_NEW".to_owned(),
                },
            })
            .expect("config update envelope"),
        )
        .expect("config update receipt");
    assert_eq!(
        receipt.dispatch_status,
        "provider_config_saved_restart_required"
    );

    match runtime
        .query_runtime(&UiCommand::QueryConfigStatus)
        .expect("config query")
        .expect("runtime-owned config result")
    {
        UiQueryResult::ConfigStatus(status) => {
            assert_eq!(status.provider_id, "new-provider");
            assert_eq!(status.provider_base_url_host, "new.example.test");
            assert_eq!(status.default_model, "new-model");
            assert_eq!(status.provider_auth_source, "env");
            assert!(status.restart_required_on_change);
            let encoded = serde_json::to_string(&status).expect("status json");
            assert!(!encoded.contains("new-secret"));
            assert!(!encoded.contains("old-secret"));
            assert!(!encoded.contains("api_key"));
        }
        other => panic!("unexpected query result: {other:?}"),
    }

    {
        let state = runtime.state.lock().expect("lock runtime state");
        assert_eq!(state.config.model, "old-model");
        assert_eq!(
            state
                .config
                .live
                .as_ref()
                .unwrap()
                .selected_agent
                .provider
                .id,
            "old"
        );
        assert_eq!(
            state
                .config
                .live
                .as_ref()
                .unwrap()
                .selected_agent
                .provider
                .default_model,
            "old-model"
        );
    }

    let raw = fs::read_to_string(&config_path).expect("read saved config");
    assert!(raw.contains("[providers.new-provider]"));
    assert!(raw.contains("default_model = \"new-model\""));
    assert!(raw.contains("api_key_env = \"FREEHAND_RUNTIME_PROVIDER_NEW\""));
    assert!(!raw.contains("new-secret"));
    assert!(!raw.contains("old-secret"));

    // SAFETY: undo the test environment mutation before exit.
    unsafe {
        std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_OLD");
        std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_NEW");
        std::env::remove_var("FREEHAND_RUNTIME_MASTER_TOKEN");
        std::env::remove_var("FREEHAND_RUNTIME_WORKER_TOKEN");
    }
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatch_upserts_provider_registry_without_switching_active_selection() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let config_path = runtime_home.join("config.toml");
    fs::write(
        &config_path,
        r#"
[providers.cc]
id = "cc"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://api.anyint.ai/openai/v1"
default_model = "gpt-5.5"

[providers.cc.auth]
type = "apikey"
api_key_env = "FREEHAND_RUNTIME_PROVIDER_UPSERT_CC"

[providers.minimax]
id = "minimax"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.minimaxi.com/anthropic"
default_model = "MiniMax-M3"

[providers.minimax.auth]
type = "apikey"
api_key = "sk-minimax-inline"

[agents.agent-live]
name = "agent-live"
mode = "master"
node_id = "agent-live-node"
paired_agents = ["agent-live-worker"]
pair_token = "FREEHAND_RUNTIME_PROVIDER_UPSERT_MASTER"
provider = "cc"
fallback_provider = "minimax"

[agents.agent-live-worker]
name = "agent-live-worker"
mode = "slave"
node_id = "agent-live-worker-node"
paired_agents = ["agent-live"]
pair_token = "FREEHAND_RUNTIME_PROVIDER_UPSERT_WORKER"
provider = "cc"
fallback_provider = "minimax"
"#,
    )
    .expect("write config");
    // SAFETY: this test owns these unique variable names and removes them before exit.
    unsafe {
        std::env::set_var("FREEHAND_RUNTIME_PROVIDER_UPSERT_CC", "cc-secret");
        std::env::set_var("FREEHAND_RUNTIME_PROVIDER_UPSERT_MASTER", "pair-token");
        std::env::set_var("FREEHAND_RUNTIME_PROVIDER_UPSERT_WORKER", "pair-token");
    }
    let selected = freehand_config::load_config_from_path(&config_path)
        .expect("load config")
        .select_agent("agent-live")
        .expect("select agent");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::UpsertProviderConfig {
                update: UiProviderConfigUpdate {
                    agent_name: "agent-live".to_owned(),
                    provider_id: "extra".to_owned(),
                    provider_type: "openai".to_owned(),
                    provider_protocol: "responses".to_owned(),
                    base_url: "https://extra.example.test/openai/v1".to_owned(),
                    default_model: "gpt-extra".to_owned(),
                    web_search: "auto".to_owned(),
                    api_key_env: "FREEHAND_RUNTIME_PROVIDER_UPSERT_EXTRA".to_owned(),
                },
            })
            .expect("provider upsert envelope"),
        )
        .expect("provider upsert receipt");
    assert_eq!(
        receipt.dispatch_status,
        "provider_config_upserted_restart_required"
    );

    match runtime
        .query_runtime(&UiCommand::QueryConfigStatus)
        .expect("config query")
        .expect("runtime-owned config result")
    {
        UiQueryResult::ConfigStatus(status) => {
            assert_eq!(status.provider_id, "cc");
            assert_eq!(status.fallback_provider_id.as_deref(), Some("minimax"));
            assert_eq!(status.provider_registry.len(), 3);
            assert!(
                status
                    .provider_registry
                    .iter()
                    .any(|provider| provider.provider_id == "extra"
                        && provider.provider_base_url == "https://extra.example.test/openai/v1")
            );
            let encoded = serde_json::to_string(&status).expect("status json");
            assert!(!encoded.contains("cc-secret"));
            assert!(!encoded.contains("sk-minimax-inline"));
            assert!(!encoded.contains("api_key"));
        }
        other => panic!("unexpected query result: {other:?}"),
    }

    {
        let state = runtime.state.lock().expect("lock runtime state");
        assert_eq!(
            state
                .config
                .live
                .as_ref()
                .unwrap()
                .selected_agent
                .provider
                .id,
            "cc"
        );
    }

    let raw = fs::read_to_string(&config_path).expect("read saved config");
    assert!(raw.contains("[providers.extra]"));
    assert!(raw.contains("provider = \"cc\""));
    assert!(raw.contains("fallback_provider = \"minimax\""));

    // SAFETY: undo the test environment mutation before exit.
    unsafe {
        std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_UPSERT_CC");
        std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_UPSERT_MASTER");
        std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_UPSERT_WORKER");
    }
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatch_upserts_and_selects_model_group_without_hot_reload() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let config_path = runtime_home.join("config.toml");
    fs::write(
        &config_path,
        r#"
[providers.cc]
id = "cc"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://api.anyint.ai/openai/v1"
default_model = "gpt-5.5"

[providers.cc.auth]
type = "apikey"
api_key_env = "FREEHAND_RUNTIME_MODEL_GROUP_CC"

[providers.minimax]
id = "minimax"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.minimaxi.com/anthropic"
default_model = "MiniMax-M3"

[providers.minimax.auth]
type = "apikey"
api_key = "sk-minimax-inline"

[agents.agent-live]
name = "agent-live"
mode = "master"
node_id = "agent-live-node"
paired_agents = ["agent-live-worker"]
pair_token = "FREEHAND_RUNTIME_MODEL_GROUP_MASTER"
provider = "minimax"
fallback_provider = "cc"

[agents.agent-live-worker]
name = "agent-live-worker"
mode = "slave"
node_id = "agent-live-worker-node"
paired_agents = ["agent-live"]
pair_token = "FREEHAND_RUNTIME_MODEL_GROUP_WORKER"
provider = "minimax"
"#,
    )
    .expect("write config");
    // SAFETY: this test owns these unique variable names and removes them before exit.
    unsafe {
        std::env::set_var("FREEHAND_RUNTIME_MODEL_GROUP_CC", "cc-secret");
        std::env::set_var("FREEHAND_RUNTIME_MODEL_GROUP_MASTER", "pair-token");
        std::env::set_var("FREEHAND_RUNTIME_MODEL_GROUP_WORKER", "pair-token");
    }
    let selected = freehand_config::load_config_from_path(&config_path)
        .expect("load config")
        .select_agent("agent-live")
        .expect("select agent");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::UpsertModelGroupConfig {
                group: UiModelGroupConfigUpdate {
                    agent_name: "agent-live".to_owned(),
                    group_id: "research".to_owned(),
                    enabled: true,
                    label: "Research".to_owned(),
                    primary: UiModelRouteUpdate {
                        provider_id: "cc".to_owned(),
                        model: "gpt-research-primary".to_owned(),
                    },
                    sub: Some(UiModelRouteUpdate {
                        provider_id: "cc".to_owned(),
                        model: "gpt-research-sub".to_owned(),
                    }),
                    search: Some(UiModelRouteUpdate {
                        provider_id: "cc".to_owned(),
                        model: "gpt-research-search".to_owned(),
                    }),
                    title: Some(UiModelRouteUpdate {
                        provider_id: "minimax".to_owned(),
                        model: "MiniMax-title".to_owned(),
                    }),
                    fallback: Some(UiModelRouteUpdate {
                        provider_id: "minimax".to_owned(),
                        model: "MiniMax-fallback".to_owned(),
                    }),
                    load_balance: vec![UiModelWeightedRouteUpdate {
                        provider_id: "cc".to_owned(),
                        model: "gpt-research-primary".to_owned(),
                        weight: 2,
                    }],
                },
            })
            .expect("model group upsert envelope"),
        )
        .expect("model group upsert receipt");
    assert_eq!(
        receipt.dispatch_status,
        "model_group_config_upserted_restart_required"
    );

    match runtime
        .query_runtime(&UiCommand::QueryConfigStatus)
        .expect("config query")
        .expect("runtime-owned config result")
    {
        UiQueryResult::ConfigStatus(status) => {
            assert_eq!(status.provider_id, "minimax");
            assert_eq!(status.model_group_id, None);
            assert_eq!(status.model_group_registry.len(), 1);
            assert_eq!(status.model_group_registry[0].group_id, "research");
            assert_eq!(
                status.model_group_registry[0]
                    .search
                    .as_ref()
                    .expect("search route")
                    .model,
                "gpt-research-search"
            );
        }
        other => panic!("unexpected query result: {other:?}"),
    }

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::UpdateAgentModelGroupSelection {
                selection: UiAgentModelGroupSelectionUpdate {
                    agent_name: "agent-live".to_owned(),
                    model_group_id: Some("research".to_owned()),
                },
            })
            .expect("model group selection envelope"),
        )
        .expect("model group selection receipt");
    assert_eq!(
        receipt.dispatch_status,
        "model_group_selection_saved_restart_required"
    );

    match runtime
        .query_runtime(&UiCommand::QueryConfigStatus)
        .expect("config query")
        .expect("runtime-owned config result")
    {
        UiQueryResult::ConfigStatus(status) => {
            assert_eq!(status.model_group_id.as_deref(), Some("research"));
            assert_eq!(status.provider_id, "cc");
            assert_eq!(status.default_model, "gpt-research-primary");
            assert_eq!(status.fallback_provider_id.as_deref(), Some("minimax"));
            let encoded = serde_json::to_string(&status).expect("status json");
            assert!(!encoded.contains("cc-secret"));
            assert!(!encoded.contains("sk-minimax-inline"));
            assert!(!encoded.contains("api_key"));
        }
        other => panic!("unexpected query result: {other:?}"),
    }

    {
        let state = runtime.state.lock().expect("lock runtime state");
        assert_eq!(
            state
                .config
                .live
                .as_ref()
                .unwrap()
                .selected_agent
                .provider
                .id,
            "minimax"
        );
    }

    let raw = fs::read_to_string(&config_path).expect("read saved config");
    assert!(raw.contains("[model_groups.research]"));
    assert!(raw.contains("model_group = \"research\""));
    assert!(raw.contains("model = \"gpt-research-search\""));

    // SAFETY: undo the test environment mutation before exit.
    unsafe {
        std::env::remove_var("FREEHAND_RUNTIME_MODEL_GROUP_CC");
        std::env::remove_var("FREEHAND_RUNTIME_MODEL_GROUP_MASTER");
        std::env::remove_var("FREEHAND_RUNTIME_MODEL_GROUP_WORKER");
    }
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatch_switches_agent_provider_selection_without_hot_reload() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let config_path = runtime_home.join("config.toml");
    fs::write(
        &config_path,
        r#"
[providers.cc]
id = "cc"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://api.anyint.ai/openai/v1"
default_model = "gpt-5.5"

[providers.cc.auth]
type = "apikey"
api_key_env = "FREEHAND_RUNTIME_PROVIDER_SWITCH_CC"

[providers.minimax]
id = "minimax"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.minimaxi.com/anthropic"
default_model = "MiniMax-M3"

[providers.minimax.auth]
type = "apikey"
api_key = "sk-minimax-inline"

[agents.agent-live]
name = "agent-live"
mode = "master"
node_id = "agent-live-node"
paired_agents = ["agent-live-worker"]
pair_token = "FREEHAND_RUNTIME_PROVIDER_SWITCH_MASTER"
provider = "cc"
fallback_provider = "minimax"

[agents.agent-live-worker]
name = "agent-live-worker"
mode = "slave"
node_id = "agent-live-worker-node"
paired_agents = ["agent-live"]
pair_token = "FREEHAND_RUNTIME_PROVIDER_SWITCH_WORKER"
provider = "cc"
fallback_provider = "minimax"
"#,
    )
    .expect("write config");
    // SAFETY: this test owns these unique variable names and removes them before exit.
    unsafe {
        std::env::set_var("FREEHAND_RUNTIME_PROVIDER_SWITCH_CC", "cc-secret");
        std::env::set_var("FREEHAND_RUNTIME_PROVIDER_SWITCH_MASTER", "pair-token");
        std::env::set_var("FREEHAND_RUNTIME_PROVIDER_SWITCH_WORKER", "pair-token");
    }
    let selected = freehand_config::load_config_from_path(&config_path)
        .expect("load config")
        .select_agent("agent-live")
        .expect("select agent");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::UpdateAgentProviderSelection {
                selection: UiAgentProviderSelectionUpdate {
                    agent_name: "agent-live".to_owned(),
                    provider_id: "minimax".to_owned(),
                    fallback_provider_id: None,
                },
            })
            .expect("provider selection envelope"),
        )
        .expect("provider selection receipt");
    assert_eq!(
        receipt.dispatch_status,
        "agent_provider_selection_saved_restart_required"
    );

    match runtime
        .query_runtime(&UiCommand::QueryConfigStatus)
        .expect("config query")
        .expect("runtime-owned config result")
    {
        UiQueryResult::ConfigStatus(status) => {
            assert_eq!(status.provider_id, "minimax");
            assert_eq!(status.provider_protocol, "messages");
            assert_eq!(
                status.provider_base_url,
                "https://api.minimaxi.com/anthropic"
            );
            assert_eq!(status.fallback_provider_id, None);
            assert_eq!(status.provider_registry.len(), 2);
        }
        other => panic!("unexpected query result: {other:?}"),
    }

    {
        let state = runtime.state.lock().expect("lock runtime state");
        assert_eq!(
            state
                .config
                .live
                .as_ref()
                .unwrap()
                .selected_agent
                .provider
                .id,
            "cc"
        );
    }

    let raw = fs::read_to_string(&config_path).expect("read saved config");
    assert!(raw.contains("[providers.cc]"));
    assert!(raw.contains("[providers.minimax]"));
    let master_start = raw.find("[agents.agent-live]\n").expect("master table");
    let master_rest = &raw[master_start..];
    let master_end = master_rest
        .find("\n[agents.agent-live-worker]")
        .unwrap_or(master_rest.len());
    let master_block = &master_rest[..master_end];
    assert!(master_block.contains("provider = \"minimax\""));
    assert!(!master_block.contains("fallback_provider = "));
    let worker_block = raw
        .split("[agents.agent-live-worker]")
        .nth(1)
        .expect("worker table");
    assert!(worker_block.contains("fallback_provider = \"minimax\""));

    // SAFETY: undo the test environment mutation before exit.
    unsafe {
        std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_SWITCH_CC");
        std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_SWITCH_MASTER");
        std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_SWITCH_WORKER");
    }
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatch_rejects_invalid_provider_config_without_overwrite() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let config_path = runtime_home.join("config.toml");
    fs::write(
        &config_path,
        r#"
[providers.old]
id = "old"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://old.example.test/v1"
default_model = "old-model"

[providers.old.auth]
type = "apikey"
api_key_env = "FREEHAND_RUNTIME_PROVIDER_OLD_INVALID"

[agents.agent-live]
name = "agent-live"
mode = "master"
node_id = "agent-live-node"
paired_agents = ["agent-live-worker"]
pair_token = "FREEHAND_RUNTIME_MASTER_TOKEN_INVALID"
provider = "old"

[agents.agent-live-worker]
name = "agent-live-worker"
mode = "slave"
node_id = "agent-live-worker-node"
paired_agents = ["agent-live"]
pair_token = "FREEHAND_RUNTIME_WORKER_TOKEN_INVALID"
provider = "old"
"#,
    )
    .expect("write config");
    // SAFETY: this test owns these unique variable names and removes them before exit.
    unsafe {
        std::env::set_var("FREEHAND_RUNTIME_PROVIDER_OLD_INVALID", "old-secret");
        std::env::set_var("FREEHAND_RUNTIME_MASTER_TOKEN_INVALID", "pair-token");
        std::env::set_var("FREEHAND_RUNTIME_WORKER_TOKEN_INVALID", "pair-token");
    }
    let selected = freehand_config::load_config_from_path(&config_path)
        .expect("load config")
        .select_agent("agent-live")
        .expect("select agent");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let before = fs::read_to_string(&config_path).expect("read before");
    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::UpdateProviderConfig {
                update: UiProviderConfigUpdate {
                    agent_name: "agent-live".to_owned(),
                    provider_id: "bad-provider".to_owned(),
                    provider_type: "anthropic".to_owned(),
                    provider_protocol: "messages".to_owned(),
                    base_url: "not-a-url".to_owned(),
                    default_model: "bad-model".to_owned(),
                    web_search: "auto".to_owned(),
                    api_key_env: "FREEHAND_RUNTIME_PROVIDER_NEW_INVALID".to_owned(),
                },
            })
            .expect("config update envelope"),
        )
        .expect_err("invalid update must fail");
    let err_text = err.to_string();
    assert!(
        err_text.contains("bad-provider") && err_text.contains("base_url"),
        "unexpected config update error: {err_text}"
    );
    let after = fs::read_to_string(&config_path).expect("read after");
    assert_eq!(after, before);
    assert!(
        runtime
            .query_runtime(&UiCommand::QueryConfigStatus)
            .expect("config query")
            .is_some()
    );

    // SAFETY: undo the test environment mutation before exit.
    unsafe {
        std::env::remove_var("FREEHAND_RUNTIME_PROVIDER_OLD_INVALID");
        std::env::remove_var("FREEHAND_RUNTIME_MASTER_TOKEN_INVALID");
        std::env::remove_var("FREEHAND_RUNTIME_WORKER_TOKEN_INVALID");
    }
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatch_updates_agent_resource_count_without_fabricating_live_agents() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let config_path = runtime_home.join("config.toml");
    fs::write(
        &config_path,
        r#"
[providers.primary]
id = "primary"
enabled = true
type = "openai"
protocol = "responses"
base_url = "https://primary.example.test/v1"
default_model = "primary-model"

[providers.primary.auth]
type = "apikey"
api_key = "primary-secret"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker"]
pair_token = "FREEHAND_RUNTIME_RESOURCE_MASTER_TOKEN"
provider = "primary"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agents = ["master"]
pair_token = "FREEHAND_RUNTIME_RESOURCE_WORKER_TOKEN"
provider = "primary"
"#,
    )
    .expect("write config");
    // SAFETY: this test owns these unique environment variables.
    unsafe {
        std::env::set_var("FREEHAND_RUNTIME_RESOURCE_MASTER_TOKEN", "pair-token");
        std::env::set_var("FREEHAND_RUNTIME_RESOURCE_WORKER_TOKEN", "pair-token");
    }
    let selected = freehand_config::load_config_from_path(&config_path)
        .expect("load config")
        .select_agent("master")
        .expect("select master");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::UpdateAgentResourceConfig {
                update: UiAgentResourceConfigUpdate {
                    agent_name: "master".to_owned(),
                    resource_count: 3,
                },
            })
            .expect("resource update envelope"),
        )
        .expect("resource update dispatch");
    assert_eq!(
        receipt.dispatch_status,
        "agent_resource_config_saved_restart_required:count=3"
    );
    match runtime
        .query_runtime(&UiCommand::QueryConfigStatus)
        .expect("config status")
        .expect("config projection")
    {
        UiQueryResult::ConfigStatus(status) => {
            assert_eq!(status.agent_resource_count, 3);
            assert_eq!(status.agent_resource_limit, 5);
            assert_eq!(status.agent_resource_provider_mode, "shared");
            assert_eq!(
                status.agent_resource_provider_id.as_deref(),
                Some("primary")
            );
            assert_eq!(status.paired_agents.len(), 3);
        }
        other => panic!("unexpected config status: {other:?}"),
    }
    let persisted =
        freehand_config::load_config_from_path(&config_path).expect("load updated config");
    assert_eq!(
        persisted
            .agents()
            .get("master")
            .expect("master")
            .paired_agent_names
            .len(),
        3
    );

    let before_invalid = fs::read_to_string(&config_path).expect("read before invalid");
    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::UpdateAgentResourceConfig {
                update: UiAgentResourceConfigUpdate {
                    agent_name: "worker".to_owned(),
                    resource_count: 2,
                },
            })
            .expect("invalid owner envelope"),
        )
        .expect_err("non-Master resource update must fail");
    assert!(err.to_string().contains("only for a master agent"));
    assert_eq!(
        fs::read_to_string(&config_path).expect("read after invalid"),
        before_invalid
    );

    // SAFETY: undo the test environment mutation before exit.
    unsafe {
        std::env::remove_var("FREEHAND_RUNTIME_RESOURCE_MASTER_TOKEN");
        std::env::remove_var("FREEHAND_RUNTIME_RESOURCE_WORKER_TOKEN");
    }
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_dispatch_failure_preserves_other_session_transcripts() {
    let runtime_home = temp_runtime_home();
    let preserved_session = SessionId::new("runtime-session-preserved");
    let (base_url_ok, rx_ok, handle_ok) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response("preserved answer")],
    );
    run_live_reason_turn(
        &live_selected_agent(base_url_ok, freehand_config::ProviderType::Anthropic),
        LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            session_id: preserved_session.clone(),
            turn_id: TurnId::new("runtime-turn-1"),
            trace_id: TraceId::new("runtime-trace-1"),
            prompt: "preserved prompt".to_owned(),
            attachments: Vec::new(),
            attachment_metadata: Vec::new(),
            cwd: None,
            execution_profile: LiveReasonExecutionProfile::Workspace,
            stream: false,
            cancel_token: None,
        },
    )
    .expect("persist preserved session");
    let _ = rx_ok.recv().expect("preserved provider request");
    handle_ok.join().expect("join preserved provider");

    let (base_url_fail, rx_fail, handle_fail) = spawn_status_sequence_server(
            (0..PROVIDER_EXECUTOR_RETRY_CAP)
                .map(|index| {
                    (
                        500,
                        "application/json",
                        format!(
                            r#"{{"type":"error","error":{{"type":"api_error","message":"failure {index}"}}}}"#
                        ),
                    )
                })
                .collect(),
        );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(base_url_fail, freehand_config::ProviderType::Anthropic),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let failed_session = SessionId::new("runtime-session-failed");

    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "failed prompt".to_owned(),
                session_id: Some(failed_session.clone()),
                cwd: None,
                metadata: None,
            })
            .expect("failed envelope"),
        )
        .expect_err("provider exhaustion must fail");
    for _ in 0..PROVIDER_EXECUTOR_RETRY_CAP {
        let _ = rx_fail.recv().expect("failed provider request");
    }
    handle_fail.join().expect("join failed provider");
    assert!(
        err.to_string().contains("anthropic_http_status_500"),
        "unexpected dispatch error: {err}"
    );

    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: preserved_session.clone(),
        })
        .expect("query preserved transcript")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 1);
            assert_eq!(
                transcript.turns[0].user_text.as_deref(),
                Some("preserved prompt")
            );
            assert!(
                transcript.turns[0]
                    .terminal_text
                    .as_deref()
                    .is_some_and(|text| text.contains("preserved answer"))
            );
        }
        other => panic!("unexpected preserved transcript query: {other:?}"),
    }

    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: failed_session.clone(),
        })
        .expect("query failed transcript")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert!(
                transcript
                    .turns
                    .iter()
                    .any(|turn| turn.terminal_status == Some(TerminalStatus::Failed)),
                "failed session should keep its own failed turn projection: {:?}",
                transcript.turns
            );
        }
        other => panic!("unexpected failed transcript query: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_dispatch_materializes_failed_turn_when_provider_fails_before_persistence() {
    let runtime_home = temp_runtime_home();
    let failed_session = SessionId::new("runtime-session-early-provider-failure");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent_with_protocol(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
            ConfigProviderProtocol::Responses,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "prompt must remain visible after early provider failure".to_owned(),
                session_id: Some(failed_session.clone()),
                cwd: None,
                metadata: None,
            })
            .expect("submit envelope"),
        )
        .expect_err("unsupported provider/protocol must fail");
    assert!(
        err.to_string().contains("not supported"),
        "unexpected dispatch error: {err}"
    );

    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: failed_session.clone(),
        })
        .expect("query failed transcript")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 1);
            let turn = &transcript.turns[0];
            assert_eq!(
                turn.user_text.as_deref(),
                Some("prompt must remain visible after early provider failure")
            );
            assert_eq!(turn.terminal_status, Some(TerminalStatus::Failed));
            assert!(
                turn.terminal_text
                    .as_deref()
                    .is_some_and(|text| text.contains("not supported")),
                "failed turn should expose the provider failure: {turn:?}"
            );
        }
        other => panic!("unexpected failed transcript query: {other:?}"),
    }

    let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
        .restore(&failed_session)
        .expect("failed turn should be persisted");
    assert!(restored.active_turn.is_none());
    assert_eq!(restored.closed_turns.len(), 1);
    assert_eq!(
        restored.closed_turns[0]
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Failed)
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_dispatch_recovers_dead_owner_master_active_work_before_new_turn() {
    let runtime_home = temp_runtime_home();
    let stale_session = SessionId::new("runtime-session-stale-master-work");
    let stale_turn = TurnId::new("runtime-turn-1");
    let stale_trace = TraceId::new("runtime-trace-1");
    let persistence = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"));
    let engine = ReasonTurnEngine::new();
    let mut history = SessionHistory::new(stale_session.clone(), Vec::new()).expect("history");
    let stale = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: stale_session.clone(),
                turn_id: stale_turn.clone(),
                trace_id: stale_trace.clone(),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-live"),
                user_text: "stale active Master prompt".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "stale-model".to_owned(),
            },
        )
        .expect("start stale turn");
    persistence
        .record_turn_started(&history, &stale, 0)
        .expect("persist stale active turn");
    master_runner::register_master_active_work(
        &runtime_home,
        &AgentId::new("agent-live"),
        &stale_session,
        &stale_turn,
        &stale_trace,
    )
    .expect("register stale active work");
    let mut active =
        master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
            .expect("load active work")
            .expect("active work");
    active.owner_process_id = Some(999_999);
    active.state = master_runner::MasterActiveWorkState::SuspendedByAttention;
    active.safe_point = master_runner::MasterWorkSafePoint::BeforeProviderRequest;
    let active_path = runtime_home
        .join("state")
        .join("master-loop")
        .join("agent-live.active-work.json");
    fs::write(
        active_path,
        serde_json::to_string_pretty(&active).expect("render active work"),
    )
    .expect("write dead-owner active work");

    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response("fresh turn completed")],
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let fresh_session = SessionId::new("runtime-session-after-stale-work");
    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "fresh prompt after stale active work".to_owned(),
                session_id: Some(fresh_session.clone()),
                cwd: None,
                metadata: None,
            })
            .expect("submit envelope"),
        )
        .expect("fresh submit must not be blocked by stale active work");
    assert!(
        receipt
            .dispatch_status
            .contains("reason_live_turn_completed")
    );
    let provider_request = rx.recv().expect("fresh provider request");
    assert!(provider_request.contains("fresh prompt after stale active work"));
    handle.join().expect("join provider");

    let recovered_stale = persistence
        .restore(&stale_session)
        .expect("restore stale session");
    assert!(recovered_stale.active_turn.is_none());
    assert_eq!(recovered_stale.closed_turns.len(), 1);
    let stale_terminal = recovered_stale.closed_turns[0]
        .terminal_event
        .as_ref()
        .expect("stale terminal");
    assert_eq!(stale_terminal.status, TerminalStatus::Interrupted);
    assert!(
        stale_terminal
            .summary
            .contains("interrupted during daemon recovery")
    );
    assert!(
        master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
            .expect("load active work after submit")
            .is_none(),
        "new successful live submit must clear its own active-work checkpoint"
    );

    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: stale_session,
        })
        .expect("query stale transcript")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 1);
            assert_eq!(
                transcript.turns[0].terminal_status,
                Some(TerminalStatus::Interrupted)
            );
        }
        other => panic!("unexpected stale transcript: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bootstrap_clears_dead_owner_master_active_work_without_active_snapshot() {
    let runtime_home = temp_runtime_home();
    let stale_session = SessionId::new("runtime-session-stale-checkpoint-only");
    let persistence = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"));
    let engine = ReasonTurnEngine::new();
    let mut history = SessionHistory::new(stale_session.clone(), Vec::new()).expect("history");
    let mut closed = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: stale_session.clone(),
                turn_id: TurnId::new("runtime-turn-1"),
                trace_id: TraceId::new("runtime-trace-1"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-live"),
                user_text: "already completed turn".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model-a".to_owned(),
            },
        )
        .expect("start closed turn");
    closed.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: stale_session.clone(),
        turn_id: TurnId::new("runtime-turn-1"),
        trace_id: TraceId::new("runtime-trace-1"),
        feature_id: FeatureId::new("provider.reason-live-bridge"),
        agent_id: AgentId::new("agent-live"),
        status: TerminalStatus::Success,
        summary: "already completed".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &closed, 0)
        .expect("persist closed turn");

    let stale_turn = TurnId::new("runtime-turn-99");
    let stale_trace = TraceId::new("runtime-trace-99");
    master_runner::register_master_active_work(
        &runtime_home,
        &AgentId::new("agent-live"),
        &stale_session,
        &stale_turn,
        &stale_trace,
    )
    .expect("register stale checkpoint-only active work");
    let mut active =
        master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
            .expect("load active work")
            .expect("active work");
    active.owner_process_id = Some(999_999);
    let active_path = runtime_home
        .join("state")
        .join("master-loop")
        .join("agent-live.active-work.json");
    fs::write(
        active_path,
        serde_json::to_string_pretty(&active).expect("render active work"),
    )
    .expect("write dead-owner active work");

    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap clears stale checkpoint-only work");

    assert!(
        master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
            .expect("load active work after bootstrap")
            .is_none(),
        "bootstrap must clear a dead active-work checkpoint when the active turn snapshot is gone"
    );
    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: stale_session,
        })
        .expect("query retained transcript")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 1);
            assert_eq!(
                transcript.turns[0].terminal_status,
                Some(TerminalStatus::Success)
            );
        }
        other => panic!("unexpected stale transcript: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bootstrap_clears_dead_owner_master_active_work_without_session_truth() {
    let runtime_home = temp_runtime_home();
    let stale_session = SessionId::new("runtime-session-missing-stale-work");
    let stale_turn = TurnId::new("runtime-turn-404");
    let stale_trace = TraceId::new("runtime-trace-404");
    master_runner::register_master_active_work(
        &runtime_home,
        &AgentId::new("agent-live"),
        &stale_session,
        &stale_turn,
        &stale_trace,
    )
    .expect("register stale active work");
    let mut active =
        master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
            .expect("load active work")
            .expect("active work");
    active.owner_process_id = Some(999_999);
    let active_path = runtime_home
        .join("state")
        .join("master-loop")
        .join("agent-live.active-work.json");
    fs::write(
        active_path,
        serde_json::to_string_pretty(&active).expect("render active work"),
    )
    .expect("write dead-owner active work");

    let _runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap clears stale missing-session work");

    assert!(
        master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
            .expect("load active work after bootstrap")
            .is_none(),
        "bootstrap must clear a dead active-work checkpoint when no session truth exists"
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bootstrap_closes_stale_toolpending_without_lifecycle_owner() {
    let runtime_home = temp_runtime_home();
    let session_id = SessionId::new("runtime-session-stale-toolpending");
    let turn_id = TurnId::new("runtime-turn-1");
    let agent_id = AgentId::new("agent-live");
    let persistence = ReasonPersistence::new(&runtime_home, agent_id.clone());
    persistence
        .create_session_metadata(
            session_id.clone(),
            Some("Stale toolpending session".to_owned()),
            None,
        )
        .expect("persist session metadata");
    let engine = ReasonTurnEngine::new();
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("history");
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: TraceId::new("runtime-trace-1"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: agent_id.clone(),
                user_text: "choose a path".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model-a".to_owned(),
            },
        )
        .expect("start turn");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: turn_id.clone(),
        trace_id: TraceId::new("runtime-trace-1"),
        feature_id: FeatureId::new("provider.reason-live-bridge"),
        agent_id: agent_id.clone(),
        status: TerminalStatus::ToolPending,
        summary: "Waiting for lifecycle: user must pick option 1 or 2".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .expect("persist stale toolpending");

    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("bootstrap must close stale waiting truth");

    let restored = persistence
        .restore(&session_id)
        .expect("restore recovered session");
    assert!(restored.active_turn.is_none());
    assert_eq!(restored.closed_turns.len(), 1);
    let recovered = restored.closed_turns.last().expect("closed turn");
    assert_eq!(
        recovered
            .terminal_event
            .as_ref()
            .map(|terminal| terminal.status.clone()),
        Some(TerminalStatus::Blocked)
    );
    assert!(
        recovered
            .terminal_event
            .as_ref()
            .expect("terminal")
            .summary
            .contains("Startup lifecycle reconciliation closed stale waiting state")
    );

    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionList)
        .expect("session list")
    {
        UiQueryResult::SessionList(list) => {
            let summary = list
                .sessions
                .iter()
                .find(|summary| summary.session_id == session_id)
                .expect("session summary");
            assert_eq!(summary.active_turn_id, None);
            assert_eq!(summary.latest_status, "blocked");
        }
        other => panic!("unexpected session list: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bootstrap_keeps_toolpending_when_child_task_can_wake_parent() {
    let runtime_home = temp_runtime_home();
    let session_id = SessionId::new("runtime-session-owner-open-toolpending");
    let turn_id = TurnId::new("runtime-turn-7");
    let agent_id = AgentId::new("agent-live");
    let persistence = ReasonPersistence::new(&runtime_home, agent_id.clone());
    persistence
        .create_session_metadata(
            session_id.clone(),
            Some("Owner open toolpending session".to_owned()),
            None,
        )
        .expect("persist session metadata");
    let engine = ReasonTurnEngine::new();
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("history");
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: TraceId::new("runtime-trace-7"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: agent_id.clone(),
                user_text: "waiting for child".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model-a".to_owned(),
            },
        )
        .expect("start turn");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: turn_id.clone(),
        trace_id: TraceId::new("runtime-trace-7"),
        feature_id: FeatureId::new("provider.reason-live-bridge"),
        agent_id: agent_id.clone(),
        status: TerminalStatus::ToolPending,
        summary: "Waiting for lifecycle: child task still open".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .expect("persist owner-backed toolpending");

    let task_runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("task runtime");
    task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(TaskId::new("task-owner-open-1")),
            title: "open child".to_owned(),
            content: "owner wake fixture".to_owned(),
            goal: "keep parent waiting".to_owned(),
            deliverables: vec!["child progress".to_owned()],
            acceptance: vec!["child remains open".to_owned()],
            priority: 50,
            target_cwd: Some(std::env::temp_dir().display().to_string()),
            execution_profile: TaskExecutionProfile::Workspace,
            dispatch: TaskDispatchRequest::None,
            parent: TaskParentRef {
                session_id: Some(session_id.clone()),
                turn_id: Some(turn_id.clone()),
                trace_id: Some(TraceId::new("runtime-trace-7")),
            },
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("create-open-child"),
        })
        .expect("create open child");

    let _runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("bootstrap must keep owner-backed waiting truth");

    let restored = persistence
        .restore(&session_id)
        .expect("restore owner session");
    assert_eq!(restored.closed_turns.len(), 1);
    assert_eq!(
        restored.closed_turns[0]
            .terminal_event
            .as_ref()
            .map(|terminal| terminal.status.clone()),
        Some(TerminalStatus::ToolPending)
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatches_session_rollback_into_effective_ui_projection() {
    let runtime_home = temp_runtime_home();
    let session_id = SessionId::new("session-rollback-runtime");
    let persistence = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"));
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("history");
    for (turn_id, trace_id, prompt, summary) in [
        ("runtime-turn-1", "trace-1", "first prompt", "first done"),
        ("runtime-turn-2", "trace-2", "second prompt", "second done"),
    ] {
        let mut turn = ReasonTurnEngine::new()
            .start_turn(
                &mut history,
                TurnStartInput {
                    session_id: session_id.clone(),
                    turn_id: TurnId::new(turn_id),
                    trace_id: TraceId::new(trace_id),
                    feature_id: FeatureId::new("runtime.ui-command-dispatch"),
                    agent_id: AgentId::new("agent-live"),
                    user_text: prompt.to_owned(),
                    planned_context_segments: Vec::new(),
                    tool_schema_fingerprint: None,
                    model: "model-a".to_owned(),
                },
            )
            .expect("turn");
        turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
            session_id: session_id.clone(),
            turn_id: TurnId::new(turn_id),
            trace_id: TraceId::new(trace_id),
            feature_id: FeatureId::new("runtime.ui-command-dispatch"),
            agent_id: AgentId::new("agent-live"),
            status: TerminalStatus::Success,
            summary: summary.to_owned(),
        });
        persistence
            .record_turn_closed(&history, &turn, 0)
            .expect("persist turn");
    }

    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    let rolled_back_child = task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(TaskId::new("rollback-child-turn-2")),
            title: "Rolled back child".to_owned(),
            content: "child created by the rolled back turn".to_owned(),
            goal: "must not survive session rollback as active truth".to_owned(),
            deliverables: vec!["rolled-back.md".to_owned()],
            acceptance: vec!["child cancelled on rollback".to_owned()],
            priority: 90,
            target_cwd: Some(runtime_home.display().to_string()),
            execution_profile: TaskExecutionProfile::Workspace,
            dispatch: TaskDispatchRequest::None,
            parent: TaskParentRef {
                session_id: Some(session_id.clone()),
                turn_id: Some(TurnId::new("runtime-turn-2")),
                trace_id: None,
            },
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("rollback-child-turn-2"),
        })
        .expect("create rolled back child")
        .task;
    let retained_child = task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(TaskId::new("retained-child-turn-1")),
            title: "Retained child".to_owned(),
            content: "child created by the retained turn".to_owned(),
            goal: "must remain because its parent turn was not rolled back".to_owned(),
            deliverables: vec!["retained.md".to_owned()],
            acceptance: vec!["child remains attached".to_owned()],
            priority: 80,
            target_cwd: Some(runtime_home.display().to_string()),
            execution_profile: TaskExecutionProfile::Workspace,
            dispatch: TaskDispatchRequest::None,
            parent: TaskParentRef {
                session_id: Some(session_id.clone()),
                turn_id: Some(TurnId::new("runtime-turn-1")),
                trace_id: None,
            },
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("retained-child-turn-1"),
        })
        .expect("create retained child")
        .task;

    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let rollback = build_command_dispatch_envelope(&UiCommand::RollbackLatestSessionTurn {
        session_id: session_id.clone(),
    })
    .expect("rollback envelope");
    let receipt = runtime.dispatch(rollback).expect("rollback dispatch");
    assert_eq!(receipt.target_feature_id, "reason.persistence");
    assert!(
        receipt
            .dispatch_status
            .contains("session_turn_rolled_back:runtime-turn-2")
    );
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("reload task runtime");
    assert_eq!(
        task_runtime
            .query_task(&rolled_back_child.task_id)
            .expect("rolled back child")
            .status,
        TaskStatus::Cancelled
    );
    assert_eq!(
        task_runtime
            .query_task(&retained_child.task_id)
            .expect("retained child")
            .status,
        TaskStatus::WaitingAgent
    );

    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("session turns")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.turns.len(), 1);
            assert_eq!(transcript.turns[0].turn_id, TurnId::new("runtime-turn-1"));
            assert_eq!(
                transcript.turns[0].user_text.as_deref(),
                Some("first prompt")
            );
        }
        other => panic!("unexpected session turns: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bootstrap_restores_multiround_turns_as_separate_ui_cards() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    fs::write(runtime_home.join("restore.txt"), "restored tool content\n")
        .expect("write restore fixture");
    with_temp_workspace(|_| {
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_named_response(
                    "toolu_restore_read",
                    "read_file",
                    json!({"path":"restore.txt","offset":0,"limit":2}),
                ),
                complete_single_response("final after tool"),
            ],
        );
        run_live_reason_turn(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            live_request_for(&runtime_home, "runtime-session-tool-restore", 9),
        )
        .expect("persist multi-round session");
        let _ = rx.recv().expect("provider request round 1");
        let _ = rx.recv().expect("provider request round 2");
        handle.join().expect("join provider");

        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime bootstrap");

        let transcript = runtime
            .query_runtime(&UiCommand::QuerySessionTurns {
                session_id: SessionId::new("runtime-session-tool-restore"),
            })
            .expect("session turns query")
            .expect("session turns result");
        match transcript {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 2);
                let tool_turn = &transcript.turns[0];
                assert_eq!(tool_turn.turn_id, TurnId::new("runtime-turn-9"));
                assert_eq!(
                    tool_turn.user_text.as_deref(),
                    Some("prompt for runtime-session-tool-restore")
                );
                let read_file_tool = tool_turn
                    .tool_activities
                    .iter()
                    .find(|tool| tool.tool_name == "read_file")
                    .unwrap_or_else(|| {
                        panic!(
                            "restored first round must retain its own file-read activity: {:?}",
                            tool_turn.tool_activities
                        )
                    });
                assert_ne!(
                    read_file_tool.status,
                    freehand_ui_protocol::UiToolActivityStatus::Waiting,
                    "restored first-round tool activity must keep its terminal status"
                );
                assert!(
                    read_file_tool.display.is_some(),
                    "restored first-round tool activity must keep semantic display truth"
                );
                assert!(tool_turn.terminal_text.is_none());

                let final_turn = &transcript.turns[1];
                assert_eq!(final_turn.turn_id, TurnId::new("runtime-turn-9-r2"));
                assert_eq!(
                    final_turn.user_text, None,
                    "continuation rounds must not render the external user prompt again"
                );
                assert!(
                    final_turn.tool_activities.is_empty(),
                    "final round must not aggregate earlier-round tool activity: {:?}",
                    final_turn.tool_activities
                );
                assert!(
                    final_turn
                        .terminal_text
                        .as_deref()
                        .is_some_and(|text| text.contains("final after tool"))
                );
            }
            other => panic!("unexpected session turns query: {other:?}"),
        }
    });

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bootstrap_does_not_replay_incomplete_historical_reason_ledgers() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let session_id = "runtime-session-bootstrap-no-ledger-replay";
    with_temp_workspace(|_| {
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_named_response(
                    "toolu_bootstrap_no_replay",
                    "read_file",
                    json!({"path":"missing.txt","offset":0,"limit":2}),
                ),
                complete_single_response("final historical round"),
            ],
        );
        let selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
        run_live_reason_turn(&selected, live_request_for(&runtime_home, session_id, 29))
            .expect("persist multi-round historical session");
        let _ = rx.recv().expect("provider request round 1");
        let _ = rx.recv().expect("provider request round 2");
        handle.join().expect("join provider");

        let first_round_path = runtime_home
            .join("state")
            .join("turns")
            .join(&selected.name)
            .join(session_id)
            .join("turns")
            .join("runtime-turn-29.json");
        if first_round_path.is_file() {
            fs::remove_file(&first_round_path)
                .expect("remove first authoritative round to simulate old incomplete snapshots");
        }
        assert!(
            runtime_home
                .join("state")
                .join("turns")
                .join(&selected.name)
                .join(session_id)
                .join("turns")
                .join("runtime-turn-29-r2.json")
                .is_file(),
            "fixture must retain the final authoritative continuation snapshot"
        );
        let ledger_path = runtime_home
            .join("ledgers")
            .join("reason")
            .join(&selected.name)
            .join(format!("{session_id}.jsonl"));
        fs::write(&ledger_path, "{poisoned historical ledger}\n")
            .expect("poison historical ledger");

        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("bootstrap must not replay every historical reason ledger");

        match runtime
            .ui_state()
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QuerySessionTurns {
                session_id: SessionId::new(session_id),
            })
            .expect("bootstrapped authoritative transcript")
        {
            UiQueryResult::SessionTurns(transcript) => {
                assert_eq!(transcript.turns.len(), 1);
                assert_eq!(
                    transcript.turns[0].turn_id,
                    TurnId::new("runtime-turn-29-r2")
                );
                assert!(
                    transcript.turns[0]
                        .terminal_text
                        .as_deref()
                        .is_some_and(|text| text.contains("final historical round"))
                );
            }
            other => panic!("unexpected session turns query: {other:?}"),
        }
    });

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bootstrap_tolerates_incomplete_authoritative_history_with_empty_ledger() {
    let runtime_home = temp_runtime_home();
    let session_id = SessionId::new("runtime-session-incomplete-empty-ledger");
    let turn_id = TurnId::new("runtime-turn-7-r3");
    let agent_id = AgentId::new("agent-live");
    let persistence = ReasonPersistence::new(&runtime_home, agent_id.clone());
    persistence
        .create_session_metadata(
            session_id.clone(),
            Some("Incomplete empty ledger session".to_owned()),
            None,
        )
        .expect("persist session metadata");
    let engine = ReasonTurnEngine::new();
    let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("history");
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                trace_id: TraceId::new("runtime-trace-incomplete-empty"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: agent_id.clone(),
                user_text: "historical incomplete residue".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model-a".to_owned(),
            },
        )
        .expect("start turn");
    turn.terminal_event = Some(freehand_contracts::ReasonResp03TerminalEvent {
        session_id: session_id.clone(),
        turn_id: turn_id.clone(),
        trace_id: TraceId::new("runtime-trace-incomplete-empty"),
        feature_id: FeatureId::new("provider.reason-live-bridge"),
        agent_id: agent_id.clone(),
        status: TerminalStatus::Success,
        summary: "only final repair round remains".to_owned(),
    });
    persistence
        .record_turn_closed(&history, &turn, 0)
        .expect("persist incomplete final-round snapshot");

    // Remove the only closed turn file? No: leave final -r3 snapshot and delete ledger to
    // force incomplete-round + empty ledger path. First remove any ledger if present.
    let ledger_path = runtime_home
        .join("ledgers")
        .join("reason")
        .join(agent_id.as_str())
        .join(format!("{}.jsonl", session_id.as_str()));
    if ledger_path.is_file() {
        fs::remove_file(&ledger_path).expect("remove ledger");
    }

    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("bootstrap must tolerate incomplete historical residue with empty ledger");

    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("query turns")
    {
        UiQueryResult::SessionTurns(turns) => {
            assert_eq!(
                turns.turns.len(),
                1,
                "authoritative final-round snapshot must still project"
            );
            assert_eq!(turns.turns[0].turn_id.as_str(), "runtime-turn-7-r3");
        }
        other => panic!("unexpected query result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn provider_request_built_debug_projects_model_waiting_ui_state() {
    let ui_state = Arc::new(Mutex::new(UiProtocolState::default()));
    let session_id = SessionId::new("session-model-request");
    let turn_id = TurnId::new("runtime-turn-77");
    let trace_id = TraceId::new("trace-model-request");
    let semantic = DebugSemanticPosition {
        feature_id: FeatureId::new("provider.reason-live-bridge"),
        session_id: session_id.clone(),
        turn_id: turn_id.clone(),
        trace_id: trace_id.clone(),
        agent_id: Some(AgentId::new("agent-1")),
        pipeline_node: Some("RuntimeLive02ProviderRequestBuilt".to_owned()),
    };
    let scene = DebugScenePosition {
        crate_name: "freehand-runtime".to_owned(),
        file: "src/lib.rs".to_owned(),
        function: "test".to_owned(),
        line: None,
        artifact_path: None,
        raw_exchange_id: None,
    };
    let event = DebugEvent {
        envelope: DebugTraceEnvelope {
            semantic: semantic.clone(),
            scene: scene.clone(),
            input_hash: None,
            output_hash: None,
            artifact_path: None,
            timestamp: "1".to_owned(),
        },
        snapshot: Some(DebugStateSnapshot::new(
            semantic,
            scene,
            "provider request built",
            vec!["model=MiniMax-M2.7".to_owned()],
        )),
    };

    apply_runtime_debug_event(&ui_state, &AgentId::new("agent-1"), "node-1", &event);
    let query = ui_state
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QueryTurn {
            turn_id: turn_id.clone(),
        })
        .expect("query turn");
    match query {
        UiQueryResult::Turn(Some(turn)) => {
            assert_eq!(turn.session_id, session_id);
            assert_eq!(turn.turn_id, turn_id);
            assert_eq!(
                turn.model_request
                    .as_ref()
                    .and_then(|activity| activity.detail.as_deref()),
                Some("provider request built")
            );
            assert_eq!(
                turn.model_request.as_ref().map(|activity| activity.kind),
                Some(UiModelRequestKind::Thinking)
            );
        }
        other => panic!("unexpected query result: {other:?}"),
    }
}

#[test]
fn context_planning_debug_projects_model_waiting_ui_state() {
    let ui_state = Arc::new(Mutex::new(UiProtocolState::default()));
    let session_id = SessionId::new("session-context-planning");
    let turn_id = TurnId::new("runtime-turn-context-planning");
    let trace_id = TraceId::new("trace-context-planning");

    for (pipeline_node, status_text) in [
        (
            "RuntimeLive01ContextPlanningStarted",
            "preparing request context",
        ),
        (
            "RuntimeLive01ContextSegmentStarted",
            "request context segment started",
        ),
        (
            "RuntimeLive01ContextSegmentCompleted",
            "request context segment ready",
        ),
        (
            "RuntimeLive01ContextSegmentFailed",
            "request context segment failed",
        ),
        (
            "RuntimeLive01ContextPlanningCompleted",
            "request context ready",
        ),
    ] {
        let semantic = DebugSemanticPosition {
            feature_id: FeatureId::new("provider.reason-live-bridge"),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            trace_id: trace_id.clone(),
            agent_id: Some(AgentId::new("agent-1")),
            pipeline_node: Some(pipeline_node.to_owned()),
        };
        let scene = DebugScenePosition {
            crate_name: "freehand-runtime".to_owned(),
            file: "src/lib.rs".to_owned(),
            function: "test".to_owned(),
            line: None,
            artifact_path: None,
            raw_exchange_id: None,
        };
        let event = DebugEvent {
            envelope: DebugTraceEnvelope {
                semantic: semantic.clone(),
                scene: scene.clone(),
                input_hash: None,
                output_hash: None,
                artifact_path: None,
                timestamp: "1".to_owned(),
            },
            snapshot: Some(DebugStateSnapshot::new(
                semantic,
                scene,
                status_text,
                Vec::new(),
            )),
        };

        apply_runtime_debug_event(&ui_state, &AgentId::new("agent-1"), "node-1", &event);
        let query = ui_state
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QueryTurn {
                turn_id: turn_id.clone(),
            })
            .expect("query turn");
        match query {
            UiQueryResult::Turn(Some(turn)) => {
                assert_eq!(turn.session_id, session_id);
                assert_eq!(turn.turn_id, turn_id);
                assert_eq!(
                    turn.model_request
                        .as_ref()
                        .and_then(|activity| activity.detail.as_deref()),
                    Some(status_text)
                );
                assert_eq!(
                    turn.model_request.as_ref().map(|activity| activity.kind),
                    Some(UiModelRequestKind::Thinking)
                );
            }
            other => panic!("unexpected query result: {other:?}"),
        }
    }
}

#[test]
fn provider_recovery_debug_updates_same_turn_activity() {
    let ui_state = Arc::new(Mutex::new(UiProtocolState::default()));
    let session_id = SessionId::new("session-provider-recovery");
    let turn_id = TurnId::new("runtime-turn-provider-recovery");
    let trace_id = TraceId::new("trace-provider-recovery");

    for (pipeline_node, status_text, expected_transport_kind) in [
        (
            "RuntimeLive05ProviderError",
            "provider retry 1/10: anthropic_http_status_500; wait 0ms before internal resend; error: internal_error: server exploded; raw_hash=hash",
            UiModelTransportKind::ProviderRetry,
        ),
        (
            "RuntimeLive05ProviderFailover",
            "provider route switched to fallback",
            UiModelTransportKind::ProviderFailover,
        ),
    ] {
        let semantic = DebugSemanticPosition {
            feature_id: FeatureId::new("provider.reason-live-bridge"),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            trace_id: trace_id.clone(),
            agent_id: Some(AgentId::new("agent-1")),
            pipeline_node: Some(pipeline_node.to_owned()),
        };
        let scene = DebugScenePosition {
            crate_name: "freehand-runtime".to_owned(),
            file: "src/lib.rs".to_owned(),
            function: "test".to_owned(),
            line: None,
            artifact_path: None,
            raw_exchange_id: None,
        };
        let event = DebugEvent {
            envelope: DebugTraceEnvelope {
                semantic: semantic.clone(),
                scene: scene.clone(),
                input_hash: None,
                output_hash: None,
                artifact_path: None,
                timestamp: "1".to_owned(),
            },
            snapshot: Some(DebugStateSnapshot::new(
                semantic,
                scene,
                status_text,
                Vec::new(),
            )),
        };

        apply_runtime_debug_event(&ui_state, &AgentId::new("agent-1"), "node-1", &event);
        let query = ui_state
            .lock()
            .expect("lock ui")
            .query(&UiCommand::QueryTurn {
                turn_id: turn_id.clone(),
            })
            .expect("query turn");
        match query {
            UiQueryResult::Turn(Some(turn)) => {
                let activity = turn.model_request.expect("provider recovery activity");
                assert_eq!(activity.kind, UiModelRequestKind::Thinking);
                let transport = activity.transport.expect("provider transport activity");
                assert_eq!(transport.kind, expected_transport_kind);
                assert_eq!(transport.detail.as_deref(), Some(status_text));
                assert!(turn.errors.is_empty());
            }
            other => panic!("unexpected query result: {other:?}"),
        }
    }
}

#[test]
fn live_dispatch_projects_schema_polishing_feedback_to_client_before_mismatch_completes() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            invalid_complete_response(),
            complete_single_response("schema polished"),
        ],
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "trigger schema polishing".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("envelope"),
        )
        .expect("submit should complete after schema polishing");
    assert!(
        receipt
            .dispatch_status
            .contains("reason_live_turn_completed")
    );
    let _first_request = rx.recv().expect("first provider request");
    let second_request = rx.recv().expect("schema polishing provider request");
    handle.join().expect("join provider");

    assert!(second_request.contains("`completion_reason`: is required"));
    assert!(second_request.contains("`evidence`: is required"));
    assert!(second_request.contains("`learned`: is required"));

    let transcript = runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: SessionId::new("runtime-session-agent-live"),
        })
        .expect("query transcript");
    match transcript {
        UiQueryResult::SessionTurns(transcript) => {
            let retry_round = transcript
                .turns
                .iter()
                .find(|turn| turn.turn_id == TurnId::new("runtime-turn-1"))
                .expect("schema mismatch round");
            let activity = retry_round
                .model_request
                .as_ref()
                .expect("schema polishing must be client-visible");
            assert_eq!(activity.kind, UiModelRequestKind::SchemaRetry);
            let detail = activity.detail.as_deref().expect("schema detail");
            assert!(detail.contains("schema polishing #1"));
            assert!(detail.contains("completion_reason is required"));
            assert!(detail.contains("evidence is required"));
            assert!(detail.contains("learned is required"));

            let final_round = transcript
                .turns
                .iter()
                .find(|turn| turn.turn_id == TurnId::new("runtime-turn-1-r2"))
                .expect("polishing final round");
            assert_eq!(final_round.terminal_status, Some(TerminalStatus::Success));
            assert!(final_round.model_request.is_none());
        }
        other => panic!("unexpected transcript query: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_dispatch_projects_missing_schema_polishing_feedback_to_client() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            missing_completion_schema_response(),
            complete_single_response("schema polished"),
        ],
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "trigger missing schema polishing".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("envelope"),
        )
        .expect("submit should complete after missing schema polishing");
    assert!(
        receipt
            .dispatch_status
            .contains("reason_live_turn_completed")
    );
    let _first_request = rx.recv().expect("first provider request");
    let second_request = rx.recv().expect("schema polishing provider request");
    handle.join().expect("join provider");

    assert!(second_request.contains("`freehand_completion`: missing"));
    assert!(second_request.contains("<freehand_completion>"));

    let transcript = runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: SessionId::new("runtime-session-agent-live"),
        })
        .expect("query transcript");
    match transcript {
        UiQueryResult::SessionTurns(transcript) => {
            let retry_round = transcript
                .turns
                .iter()
                .find(|turn| turn.turn_id == TurnId::new("runtime-turn-1"))
                .expect("schema mismatch round");
            let activity = retry_round
                .model_request
                .as_ref()
                .expect("schema polishing must be client-visible");
            assert_eq!(activity.kind, UiModelRequestKind::SchemaRetry);
            let detail = activity.detail.as_deref().expect("schema detail");
            assert!(detail.contains("schema polishing #1"));
            assert!(detail.contains("freehand_completion missing"));
            assert!(detail.contains("<freehand_completion>"));
        }
        other => panic!("unexpected transcript query: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_master_rejects_complete_while_parent_child_task_open() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    let child = task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(TaskId::new("open-child-task")),
            title: "Open child task".to_owned(),
            content: "child work is still running".to_owned(),
            goal: "prove parent completion is gated".to_owned(),
            deliverables: vec!["child result".to_owned()],
            acceptance: vec!["child task closed".to_owned()],
            priority: 90,
            target_cwd: Some(runtime_home.display().to_string()),
            execution_profile: TaskExecutionProfile::Workspace,
            dispatch: TaskDispatchRequest::None,
            parent: TaskParentRef {
                session_id: Some(SessionId::new("session-live")),
                turn_id: None,
                trace_id: None,
            },
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("open-child"),
        })
        .expect("create open child")
        .task;
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            complete_single_response("premature final answer"),
            waiting_single_response("wait for open-child-task to close, then synthesize"),
        ],
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.cwd = Some(runtime_home.clone());

    let outcome = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
        |_| {},
        |_| {},
        |_| {},
    )
    .expect("master completion gate should repair to waiting");
    let requests = collect_provider_requests(&rx, 2);
    handle.join().expect("join provider");

    assert_eq!(outcome.rounds, 2);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::ToolPending)
    );
    assert_eq!(outcome.schema_rejections.len(), 1);
    assert_eq!(outcome.schema_rejections[0].issues[0].field, "claim");
    assert!(
        outcome.schema_rejections[0].issues[0]
            .message
            .contains("open-child-task")
    );
    assert!(requests[1].contains("cannot be `complete` while child Worker tasks"));
    assert_eq!(
        task_runtime
            .query_task(&child.task_id)
            .expect("child unchanged")
            .status,
        TaskStatus::WaitingAgent
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_master_rejects_waiting_when_child_tasks_are_terminal_and_no_owner_will_wake() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    let child = task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(TaskId::new("closed-child-task")),
            title: "Closed child task".to_owned(),
            content: "child work is already terminal".to_owned(),
            goal: "prove stale parent waiting is rejected".to_owned(),
            deliverables: vec!["child result".to_owned()],
            acceptance: vec!["child task closed".to_owned()],
            priority: 90,
            target_cwd: Some(runtime_home.display().to_string()),
            execution_profile: TaskExecutionProfile::Workspace,
            dispatch: TaskDispatchRequest::None,
            parent: TaskParentRef {
                session_id: Some(SessionId::new("session-live")),
                turn_id: Some(TurnId::new("runtime-turn-1")),
                trace_id: Some(TraceId::new("runtime-trace-1")),
            },
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("closed-child"),
        })
        .expect("create child")
        .task;
    task_runtime
        .cancel_task(TaskMutationRequest {
            task_id: child.task_id.clone(),
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("cancel-closed-child"),
        })
        .expect("terminal child");
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            waiting_single_response("Waiting for the user to pick option A or option B"),
            blocked_single_response(
                "needs user choice",
                "waiting for user choice after child task is already terminal",
            ),
        ],
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.cwd = Some(runtime_home.clone());

    let outcome = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
        |_| {},
        |_| {},
        |_| {},
    )
    .expect("stale waiting must repair to non-running terminal state");
    let requests = [
        rx.recv_timeout(Duration::from_secs(2))
            .expect("initial provider request"),
        rx.recv_timeout(Duration::from_secs(2))
            .expect("schema repair provider request after rejected stale waiting"),
    ];
    handle.join().expect("join provider");

    assert_eq!(outcome.rounds, 2);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Blocked)
    );
    assert_eq!(outcome.schema_rejections.len(), 1);
    assert_eq!(outcome.schema_rejections[0].issues[0].field, "claim");
    assert!(
        outcome.schema_rejections[0].issues[0]
            .message
            .contains("claim=`waiting` requires open Task Center or timer owner truth"),
        "unexpected rejection: {:?}",
        outcome.schema_rejections[0].issues[0]
    );
    assert!(requests[1].contains("claim=`waiting` requires open Task Center or timer owner truth"));
    assert_eq!(
        task_runtime
            .query_task(&child.task_id)
            .expect("child unchanged")
            .status,
        TaskStatus::Cancelled
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_master_allows_complete_with_terminal_cancelled_child_tasks() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    for task_id in ["cancelled-old-child-a", "cancelled-old-child-b"] {
        task_runtime
            .create_task(TaskCreateRequest {
                task_id: Some(TaskId::new(task_id)),
                title: format!("{task_id} title"),
                content: "historical wrong-path child task".to_owned(),
                goal: "prove terminal historical child tasks do not block final synthesis"
                    .to_owned(),
                deliverables: vec!["terminal child state".to_owned()],
                acceptance: vec!["cancelled child does not keep parent waiting".to_owned()],
                priority: 90,
                target_cwd: Some(runtime_home.display().to_string()),
                execution_profile: TaskExecutionProfile::Workspace,
                dispatch: TaskDispatchRequest::None,
                parent: TaskParentRef {
                    session_id: Some(SessionId::new("session-live")),
                    turn_id: Some(TurnId::new("runtime-turn-previous")),
                    trace_id: None,
                },
                actor: lifecycle_test_actor(),
                watermark: lifecycle_test_watermark(task_id),
            })
            .expect("create cancelled child");
        task_runtime
            .cancel_task(TaskMutationRequest {
                task_id: TaskId::new(task_id),
                actor: lifecycle_test_actor(),
                watermark: lifecycle_test_watermark(&format!("cancel-{task_id}")),
            })
            .expect("cancel child");
    }
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response(
            "final answer after terminal historical children",
        )],
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.cwd = Some(runtime_home.clone());

    let outcome = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
        |_| {},
        |_| {},
        |_| {},
    )
    .expect("terminal historical children should not reject complete");
    let requests = collect_provider_requests(&rx, 1);
    handle.join().expect("join provider");

    assert_eq!(outcome.rounds, 1);
    assert!(outcome.schema_rejections.is_empty());
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].contains("cannot be complete while child Worker tasks"));
    for task_id in ["cancelled-old-child-a", "cancelled-old-child-b"] {
        assert_eq!(
            task_runtime
                .query_task(&TaskId::new(task_id))
                .expect("child persisted")
                .status,
            TaskStatus::Cancelled
        );
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_submit_uses_requested_session_id_for_new_webui_session() {
    let runtime_home = temp_runtime_home();
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response("new session answer")],
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");

    let requested_session = SessionId::new("webui-session-test");
    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "hello from new session".to_owned(),
                session_id: Some(requested_session.clone()),
                cwd: None,
                metadata: None,
            })
            .expect("envelope"),
        )
        .expect("submit receipt");
    assert!(
        receipt
            .dispatch_status
            .contains("reason_live_turn_completed")
    );
    let _ = rx.recv().expect("provider request");
    handle.join().expect("join provider");

    let transcript = runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: requested_session.clone(),
        })
        .expect("query transcript");
    match transcript {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.session_id, requested_session);
            assert_eq!(transcript.turns.len(), 1);
            assert_eq!(
                transcript.turns[0].user_text.as_deref(),
                Some("hello from new session")
            );
            assert_eq!(
                transcript.turns[0].terminal_status,
                Some(TerminalStatus::Success)
            );
        }
        other => panic!("unexpected transcript: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_retries_recoverable_provider_errors_then_succeeds() {
    let runtime_home = temp_runtime_home();
    let mut debug_events = Vec::new();
    let (base_url, rx, handle) = spawn_status_sequence_server(vec![
        (
            500,
            "application/json",
            r#"{"type":"error","error":{"type":"api_error","message":"first upstream failure"}}"#
                .to_owned(),
        ),
        (
            500,
            "application/json",
            r#"{"type":"error","error":{"type":"api_error","message":"second upstream failure"}}"#
                .to_owned(),
        ),
        (
            200,
            "application/json",
            complete_single_response("retry ok"),
        ),
    ]);

    let outcome = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            ..live_request(false)
        },
        |_| {},
        |event| debug_events.push(event.clone()),
        |_| {},
    )
    .expect("provider retry should recover");

    assert_eq!(
        outcome.rounds, 1,
        "provider retries must not start a new reason round"
    );
    assert_eq!(
        outcome.turns.len(),
        1,
        "provider retries must stay inside one turn"
    );
    assert_eq!(outcome.turns[0].request.turn_id, TurnId::new("turn-live"));
    assert_eq!(
        outcome.turns[0].request.user_text, "reply exactly pong",
        "provider resend must not synthesize another user input"
    );
    assert!(
        outcome
            .turn
            .terminal_event
            .expect("terminal")
            .summary
            .contains("retry ok")
    );
    assert!(outcome.turn.error_events.is_empty());
    assert!(debug_events.iter().any(|event| {
        event.envelope.semantic.pipeline_node.as_deref() == Some("RuntimeLive05ProviderError")
            && event.snapshot.as_ref().is_some_and(|snapshot| {
                snapshot.status_text.contains("provider retry 1/10")
                    && snapshot.status_text.contains("anthropic_http_status_500")
                    && snapshot
                        .status_text
                        .contains("wait 0ms before internal resend")
                    && snapshot.status_text.contains("api_error")
            })
    }));
    assert_eq!(rx.iter().take(3).count(), 3);
    handle.join().expect("join provider");
    let metadata =
        metadata_ledger_records(&runtime_home, "agent-live", &SessionId::new("session-live"));
    let retry_actions = metadata
        .iter()
        .filter_map(|row| metadata_entry_string(row, "error.recovery_action"))
        .collect::<Vec<_>>();
    assert!(retry_actions.contains(&"retry_same_step".to_owned()));
    assert!(!retry_actions.contains(&"fail_turn".to_owned()));

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_publishes_provider_retry_before_next_attempt() {
    let runtime_home = temp_runtime_home();
    let (base_url, request_rx, release_second_attempt, provider_handle) = spawn_retry_gate_server();
    let (debug_tx, debug_rx) = mpsc::channel::<String>();
    let runner_home = runtime_home.clone();
    let runner = thread::spawn(move || {
        run_live_reason_turn_with_hooks(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            LiveReasonTurnRequest {
                runtime_home: runner_home,
                ..live_request(false)
            },
            |_| {},
            |event| {
                if event.envelope.semantic.pipeline_node.as_deref()
                    == Some("RuntimeLive05ProviderError")
                {
                    let _ = debug_tx.send(
                        event
                            .snapshot
                            .as_ref()
                            .map(|snapshot| snapshot.status_text.clone())
                            .unwrap_or_default(),
                    );
                }
            },
            |_| {},
        )
        .expect("provider retry should recover")
    });

    let first_request = request_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("first provider request");
    assert!(first_request.starts_with("POST /v1/messages "));
    let retry_status = debug_rx.recv_timeout(Duration::from_secs(2));
    release_second_attempt
        .send(())
        .expect("release second provider attempt");
    let second_request = request_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("second provider request");
    assert!(second_request.starts_with("POST /v1/messages "));
    let outcome = runner.join().expect("join live runner");
    provider_handle.join().expect("join provider");

    let retry_status = retry_status.expect("provider retry status must be published");
    assert!(
        retry_status.contains("provider retry 1/10")
            && retry_status.contains("anthropic_http_status_500")
            && retry_status.contains("wait 0ms before internal resend")
            && retry_status.contains("api_error"),
        "provider retry status must expose attempt, code, wait, and error summary while pending: {retry_status}"
    );
    assert!(outcome.turn.error_events.is_empty());
    assert!(
        outcome
            .turn
            .terminal_event
            .expect("terminal")
            .summary
            .contains("retry gate ok")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn provider_retry_backoff_sleep_observes_live_cancel_token() {
    let token = Arc::new(AtomicBool::new(false));
    let mut request = live_request(false);
    request.cancel_token = Some(Arc::clone(&token));
    let (started_tx, started_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        started_tx.send(()).expect("send started");
        let started_at = Instant::now();
        let result = sleep_provider_retry(&request, Duration::from_secs(5));
        (result, started_at.elapsed())
    });

    started_rx.recv().expect("sleep started");
    thread::sleep(Duration::from_millis(100));
    token.store(true, Ordering::SeqCst);
    let (result, elapsed) = handle.join().expect("join sleep");

    assert_eq!(result, Err(RuntimeLiveBridgeError::Cancelled));
    assert!(
        elapsed < Duration::from_secs(1),
        "provider retry backoff must not block cancel for the full retry window: {elapsed:?}"
    );
}

#[test]
fn live_bridge_failover_from_openai_http_402_to_anthropic_success() {
    let runtime_home = temp_runtime_home();
    let (primary_url, primary_rx, primary_handle) = spawn_status_sequence_server(vec![(
        402,
        "application/json",
        r#"{"error":{"message":"insufficient credits"}}"#.to_owned(),
    )]);
    let (fallback_url, fallback_rx, fallback_handle) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response("fallback completed")],
    );
    let selected = live_selected_agent_with_fallback(primary_url, fallback_url);

    let outcome = run_live_reason_turn_with_hooks(
        &selected,
        LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            ..live_request(false)
        },
        |_| {},
        |_| {},
        |_| {},
    )
    .expect("402 should fail over to the configured fallback");

    let primary_request = primary_rx.recv().expect("primary request");
    let fallback_request = fallback_rx.recv().expect("fallback request");
    primary_handle.join().expect("join primary");
    fallback_handle.join().expect("join fallback");
    assert!(primary_request.starts_with("POST /responses "));
    assert!(fallback_request.contains("\"model\":\"MiniMax-M3\""));
    assert_eq!(outcome.turn.provider_payload.model, "MiniMax-M3");
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|terminal| terminal.status.clone()),
        Some(TerminalStatus::Success)
    );
    assert!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .expect("terminal")
            .summary
            .contains("fallback completed")
    );
    assert!(outcome.turn.error_events.is_empty());

    let metadata =
        metadata_ledger_records(&runtime_home, "agent-live", &SessionId::new("session-live"));
    assert!(metadata.iter().any(|record| {
        record.write_node.pipeline_node == "RuntimeLive05ProviderFailover"
            && metadata_entry_string(record, "error.code").as_deref()
                == Some("openai_http_status_402")
            && metadata_entry_string(record, "error.recovery_action").as_deref()
                == Some("failover_provider")
    }));
    assert!(metadata.iter().any(|record| {
        record.write_node.pipeline_node == "RuntimeLive05ProviderFailover"
            && metadata_entry_string(record, "provider.route").as_deref() == Some("fallback")
            && metadata_entry_string(record, "provider.failover_from").as_deref() == Some("cc")
            && metadata_entry_string(record, "provider.failover_to").as_deref() == Some("minimax")
            && metadata_entry_string(record, "provider.failover_error_code").as_deref()
                == Some("openai_http_status_402")
            && metadata_entry_string(record, "reason.model").as_deref() == Some("MiniMax-M3")
    }));
    assert!(!metadata.iter().any(|record| {
        metadata_entry_string(record, "error.code").as_deref() == Some("openai_http_status_402")
            && metadata_entry_string(record, "error.recovery_action").as_deref()
                == Some("fail_turn")
    }));

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_failover_after_primary_retry_exhaustion() {
    let runtime_home = temp_runtime_home();
    let primary_responses = (0..PROVIDER_EXECUTOR_RETRY_CAP)
        .map(|attempt| {
            (
                500,
                "application/json",
                format!(r#"{{"error":{{"message":"primary failure {attempt}"}}}}"#),
            )
        })
        .collect();
    let (primary_url, primary_rx, primary_handle) = spawn_status_sequence_server(primary_responses);
    let (fallback_url, fallback_rx, fallback_handle) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response("fallback after retries")],
    );
    let selected = live_selected_agent_with_fallback(primary_url, fallback_url);

    let outcome = run_live_reason_turn_with_hooks(
        &selected,
        LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            ..live_request(false)
        },
        |_| {},
        |_| {},
        |_| {},
    )
    .expect("retry exhaustion should fail over");

    assert_eq!(
        primary_rx
            .iter()
            .take(PROVIDER_EXECUTOR_RETRY_CAP as usize)
            .count(),
        PROVIDER_EXECUTOR_RETRY_CAP as usize
    );
    assert_eq!(fallback_rx.iter().take(1).count(), 1);
    primary_handle.join().expect("join primary");
    fallback_handle.join().expect("join fallback");
    assert_eq!(outcome.turn.provider_payload.model, "MiniMax-M3");
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|terminal| terminal.status.clone()),
        Some(TerminalStatus::Success)
    );
    assert!(outcome.turn.error_events.is_empty());

    let metadata =
        metadata_ledger_records(&runtime_home, "agent-live", &SessionId::new("session-live"));
    assert!(metadata.iter().any(|record| {
        record.write_node.pipeline_node == "RuntimeLive05ProviderFailover"
            && metadata_entry_string(record, "provider.failover_error_code").as_deref()
                == Some("openai_http_status_500")
    }));
    assert!(metadata.iter().any(|record| {
        record.write_node.pipeline_node == "RuntimeLive05ProviderFailover"
            && metadata_entry_string(record, "error.code").as_deref()
                == Some("openai_http_status_500")
            && metadata_entry_string(record, "error.recovery_action").as_deref()
                == Some("failover_provider")
    }));

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_primary_success_does_not_activate_fallback() {
    let runtime_home = temp_runtime_home();
    let (primary_url, primary_rx, primary_handle) = spawn_sequence_server(
        "application/json",
        vec![openai_responses_complete_response("primary completed")],
    );
    let selected = live_selected_agent_with_fallback(primary_url, "http://127.0.0.1:1".to_owned());

    let outcome = run_live_reason_turn_with_hooks(
        &selected,
        LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            ..live_request(false)
        },
        |_| {},
        |_| {},
        |_| {},
    )
    .expect("primary success should not call fallback");

    assert_eq!(primary_rx.iter().take(1).count(), 1);
    primary_handle.join().expect("join primary");
    assert_eq!(outcome.turn.provider_payload.model, "gpt-5.5");
    let metadata =
        metadata_ledger_records(&runtime_home, "agent-live", &SessionId::new("session-live"));
    assert!(
        !metadata
            .iter()
            .any(|record| record.write_node.pipeline_node == "RuntimeLive05ProviderFailover")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_adapter_failure_does_not_activate_fallback() {
    let runtime_home = temp_runtime_home();
    let (primary_url, primary_rx, primary_handle) =
        spawn_sequence_server("application/json", vec!["{not-json}".to_owned()]);
    let selected = live_selected_agent_with_fallback(primary_url, "http://127.0.0.1:1".to_owned());

    let err = run_live_reason_turn_with_hooks(
        &selected,
        LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            ..live_request(false)
        },
        |_| {},
        |_| {},
        |_| {},
    )
    .expect_err("adapter failure must not activate fallback");

    assert_eq!(primary_rx.iter().take(1).count(), 1);
    primary_handle.join().expect("join primary");
    assert!(err.to_string().contains("openai_adapter_failed"));
    let metadata =
        metadata_ledger_records(&runtime_home, "agent-live", &SessionId::new("session-live"));
    assert!(
        !metadata
            .iter()
            .any(|record| record.write_node.pipeline_node == "RuntimeLive05ProviderFailover")
    );
    let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
        .restore(&SessionId::new("session-live"))
        .expect("restore failed turn");
    assert!(restored.active_turn.is_none());
    assert_eq!(restored.closed_turns.len(), 1);

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_fallback_exhaustion_materializes_one_failed_turn() {
    let runtime_home = temp_runtime_home();
    let (primary_url, primary_rx, primary_handle) = spawn_status_sequence_server(vec![(
        402,
        "application/json",
        r#"{"error":{"message":"insufficient credits"}}"#.to_owned(),
    )]);
    let fallback_responses = (0..PROVIDER_EXECUTOR_RETRY_CAP)
        .map(|attempt| {
            (
                500,
                "application/json",
                format!(r#"{{"type":"error","error":{{"message":"fallback failure {attempt}"}}}}"#),
            )
        })
        .collect();
    let (fallback_url, fallback_rx, fallback_handle) =
        spawn_status_sequence_server(fallback_responses);
    let selected = live_selected_agent_with_fallback(primary_url, fallback_url);

    let err = run_live_reason_turn_with_hooks(
        &selected,
        LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            ..live_request(false)
        },
        |_| {},
        |_| {},
        |_| {},
    )
    .expect_err("fallback exhaustion must fail the turn once");

    assert_eq!(primary_rx.iter().take(1).count(), 1);
    assert_eq!(
        fallback_rx
            .iter()
            .take(PROVIDER_EXECUTOR_RETRY_CAP as usize)
            .count(),
        PROVIDER_EXECUTOR_RETRY_CAP as usize
    );
    primary_handle.join().expect("join primary");
    fallback_handle.join().expect("join fallback");
    assert!(err.to_string().contains("anthropic_http_status_500"));
    let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
        .restore(&SessionId::new("session-live"))
        .expect("restore failed turn");
    assert!(restored.active_turn.is_none());
    assert_eq!(restored.closed_turns.len(), 1);
    let failed_turn = restored.closed_turns.last().expect("failed turn");
    assert_eq!(failed_turn.provider_payload.model, "MiniMax-M3");
    assert_eq!(
        failed_turn
            .terminal_event
            .as_ref()
            .map(|terminal| terminal.status.clone()),
        Some(TerminalStatus::Failed)
    );
    assert_eq!(
        failed_turn
            .error_events
            .last()
            .map(|event| event.error.code.as_str()),
        Some("anthropic_http_status_500")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_fails_after_ten_provider_retries_with_error_code() {
    let runtime_home = temp_runtime_home();
    let responses = (0..PROVIDER_EXECUTOR_RETRY_CAP)
            .map(|index| {
                (
                    500,
                    "application/json",
                    format!(
                        r#"{{"type":"error","error":{{"type":"api_error","message":"upstream failure {index}"}}}}"#
                    ),
                )
            })
            .collect::<Vec<_>>();
    let (base_url, rx, handle) = spawn_status_sequence_server(responses);

    let err = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            ..live_request(false)
        },
        |_| {},
        |_| {},
        |_| {},
    )
    .expect_err("provider retry exhaustion should fail");

    assert!(err.to_string().contains("anthropic_http_status_500"));
    assert_eq!(
        rx.iter().take(PROVIDER_EXECUTOR_RETRY_CAP as usize).count(),
        PROVIDER_EXECUTOR_RETRY_CAP as usize
    );
    handle.join().expect("join provider");
    let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
        .restore(&SessionId::new("session-live"))
        .expect("restore failed turn");
    assert!(restored.active_turn.is_none());
    let closed = restored.closed_turns.last().expect("closed turn");
    let error = closed.error_events.last().expect("error event");
    assert_eq!(error.error.code, "anthropic_http_status_500");
    assert!(
        closed
            .terminal_event
            .as_ref()
            .expect("terminal")
            .summary
            .contains("anthropic_http_status_500")
    );
    let metadata =
        metadata_ledger_records(&runtime_home, "agent-live", &SessionId::new("session-live"));
    let retry_indexes = metadata
        .iter()
        .filter_map(|row| metadata_entry_u64(row, "error.retry_index"))
        .collect::<Vec<_>>();
    assert!(retry_indexes.contains(&1));
    assert!(retry_indexes.contains(&(PROVIDER_EXECUTOR_RETRY_CAP as u64)));
    let recovery_actions = metadata
        .iter()
        .filter_map(|row| metadata_entry_string(row, "error.recovery_action"))
        .collect::<Vec<_>>();
    assert!(recovery_actions.contains(&"retry_same_step".to_owned()));
    assert!(recovery_actions.contains(&"fail_turn".to_owned()));

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn submit_cwd_is_projected_and_inherited_by_session() {
    let root = temp_runtime_home();
    fs::create_dir_all(&root).expect("create cwd");
    let runtime = runtime();
    let session_id = SessionId::new("webui-session-cwd-runtime");
    let cwd = fs::canonicalize(&root)
        .expect("canonical cwd")
        .to_string_lossy()
        .into_owned();

    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "first cwd turn".to_owned(),
                session_id: Some(session_id.clone()),
                cwd: Some(cwd.clone()),
                metadata: None,
            })
            .expect("first envelope"),
        )
        .expect("first receipt");
    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "second cwd turn".to_owned(),
                session_id: Some(session_id.clone()),
                cwd: None,
                metadata: None,
            })
            .expect("second envelope"),
        )
        .expect("second receipt");

    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("query transcript")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.cwd.as_deref(), Some(cwd.as_str()));
            assert_eq!(transcript.turns.len(), 2);
            assert!(
                transcript
                    .turns
                    .iter()
                    .all(|turn| turn.cwd.as_deref() == Some(cwd.as_str()))
            );
        }
        other => panic!("unexpected transcript: {other:?}"),
    }

    fs::remove_dir_all(root).expect("cleanup cwd");
}

#[test]
fn live_master_tool_execution_allows_external_session_cwd_read() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let workspace = temp_runtime_home();
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(workspace.join("session-cwd.txt"), "session cwd content\n")
        .expect("write workspace file");
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_named_response(
                "toolu_session_cwd",
                "read_file",
                json!({"path":"session-cwd.txt","offset":0,"limit":5}),
            ),
            complete_single_response("read session cwd"),
        ],
    );
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        runtime_home.clone(),
        false,
    )
    .expect("runtime bootstrap");
    let session_id = SessionId::new("webui-session-tool-cwd");
    let cwd = fs::canonicalize(&workspace)
        .expect("canonical workspace")
        .to_string_lossy()
        .into_owned();

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "read the session cwd file".to_owned(),
                session_id: Some(session_id.clone()),
                cwd: Some(cwd.clone()),
                metadata: None,
            })
            .expect("envelope"),
        )
        .expect("submit receipt");
    let _first_request = rx.recv().expect("first provider request");
    let reentry_request = rx.recv().expect("tool reentry provider request");
    handle.join().expect("join provider");

    assert!(
        receipt
            .dispatch_status
            .contains("reason_live_turn_completed")
    );
    assert!(reentry_request.contains("\"type\":\"tool_result\""));
    assert!(!reentry_request.contains("\"is_error\":true"));
    assert!(reentry_request.contains("session cwd content"));
    match runtime
        .ui_state()
        .lock()
        .expect("lock ui")
        .query(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("query transcript")
    {
        UiQueryResult::SessionTurns(transcript) => {
            assert_eq!(transcript.cwd.as_deref(), Some(cwd.as_str()));
            assert_eq!(transcript.turns[0].cwd.as_deref(), Some(cwd.as_str()));
        }
        other => panic!("unexpected transcript: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

fn selected_master_agent() -> SelectedAgentConfig {
    SelectedAgentConfig {
        name: "master".to_owned(),
        mode: AgentMode::Master,
        node_id: "master-node".to_owned(),
        paired_agents: vec![SelectedPeerAgentConfig {
            name: "worker".to_owned(),
            mode: AgentMode::Slave,
            node_id: "worker-node".to_owned(),
            allowed_pair_ip: Some("127.0.0.1".parse().expect("ip")),
            pair_token_env: "FREEHAND_PAIR_TOKEN_WORKER".to_owned(),
            provider_id: "provider-worker".to_owned(),
            fallback_provider_id: None,
            model_group_id: None,
        }],
        allowed_pair_ip: None,
        pair_token_env: "FREEHAND_PAIR_TOKEN_MASTER".to_owned(),
        pair_token: "pair-token".to_owned(),
        provider: freehand_config::SelectedProviderConfig {
            id: "provider-master".to_owned(),
            provider_type: freehand_config::ProviderType::Anthropic,
            protocol: freehand_config::ProviderProtocol::Messages,
            base_url: "https://example.invalid".to_owned(),
            default_model: "model-master".to_owned(),
            web_search: freehand_config::ProviderWebSearchMode::Auto,
            web_search_wire: freehand_config::ProviderWebSearchWire::WebSearch,
            auth_type: freehand_config::ProviderAuthType::ApiKey,
            auth_source: freehand_config::ProviderAuthSourceKind::Inline,
            api_key: "secret".to_owned(),
        },
        fallback_provider: None,
        model_group_id: None,
        restart_required_on_change: true,
        relay_connection: None,
    }
}

fn live_selected_agent(
    base_url: String,
    provider_type: freehand_config::ProviderType,
) -> SelectedAgentConfig {
    let protocol = match provider_type {
        freehand_config::ProviderType::Anthropic => ConfigProviderProtocol::Messages,
        freehand_config::ProviderType::OpenAi => ConfigProviderProtocol::ChatCompletions,
    };
    SelectedAgentConfig {
        name: "agent-live".to_owned(),
        mode: AgentMode::Master,
        node_id: "agent-live-node".to_owned(),
        paired_agents: vec![SelectedPeerAgentConfig {
            name: "agent-live-worker".to_owned(),
            mode: AgentMode::Slave,
            node_id: "agent-live-worker-node".to_owned(),
            allowed_pair_ip: None,
            pair_token_env: "FREEHAND_WORKER_TOKEN".to_owned(),
            provider_id: "provider-live".to_owned(),
            fallback_provider_id: None,
            model_group_id: None,
        }],
        allowed_pair_ip: None,
        pair_token_env: "FREEHAND_MASTER_TOKEN".to_owned(),
        pair_token: "pair-token".to_owned(),
        provider: freehand_config::SelectedProviderConfig {
            id: "provider-live".to_owned(),
            provider_type,
            protocol,
            base_url,
            default_model: "MiniMax-M2.7".to_owned(),
            web_search: freehand_config::ProviderWebSearchMode::Auto,
            web_search_wire: freehand_config::ProviderWebSearchWire::WebSearch,
            auth_type: freehand_config::ProviderAuthType::ApiKey,
            auth_source: freehand_config::ProviderAuthSourceKind::Env,
            api_key: "test-api-key".to_owned(),
        },
        fallback_provider: None,
        model_group_id: None,
        restart_required_on_change: true,
        relay_connection: None,
    }
}

fn selected_peer(
    name: impl Into<String>,
    mode: AgentMode,
    node_id: impl Into<String>,
    pair_token_env: impl Into<String>,
) -> SelectedPeerAgentConfig {
    SelectedPeerAgentConfig {
        name: name.into(),
        mode,
        node_id: node_id.into(),
        allowed_pair_ip: None,
        pair_token_env: pair_token_env.into(),
        provider_id: "worker-provider".to_owned(),
        fallback_provider_id: None,
        model_group_id: None,
    }
}

fn set_single_worker_peer(selected: &mut SelectedAgentConfig, worker_id: &str) {
    set_worker_peers(selected, &[worker_id]);
}

fn set_worker_peers(selected: &mut SelectedAgentConfig, worker_ids: &[&str]) {
    selected.paired_agents = worker_ids
        .iter()
        .map(|worker_id| {
            selected_peer(
                *worker_id,
                AgentMode::Slave,
                format!("{worker_id}-node"),
                "FREEHAND_WORKER_TOKEN",
            )
        })
        .collect();
}

fn set_single_master_peer(selected: &mut SelectedAgentConfig, master_id: &str) {
    selected.paired_agents = vec![selected_peer(
        master_id,
        AgentMode::Master,
        format!("{master_id}-node"),
        "FREEHAND_MASTER_TOKEN",
    )];
}

fn live_selected_agent_with_protocol(
    base_url: String,
    provider_type: freehand_config::ProviderType,
    protocol: ConfigProviderProtocol,
) -> SelectedAgentConfig {
    let mut selected = live_selected_agent(base_url, provider_type);
    selected.provider.protocol = protocol;
    selected
}

fn live_selected_agent_with_fallback(
    primary_base_url: String,
    fallback_base_url: String,
) -> SelectedAgentConfig {
    let mut selected = live_selected_agent_with_protocol(
        primary_base_url,
        freehand_config::ProviderType::OpenAi,
        ConfigProviderProtocol::Responses,
    );
    selected.provider.id = "cc".to_owned();
    selected.provider.default_model = "gpt-5.5".to_owned();
    selected.fallback_provider = Some(freehand_config::SelectedProviderConfig {
        id: "minimax".to_owned(),
        provider_type: freehand_config::ProviderType::Anthropic,
        protocol: ConfigProviderProtocol::Messages,
        base_url: fallback_base_url,
        default_model: "MiniMax-M3".to_owned(),
        web_search: freehand_config::ProviderWebSearchMode::Auto,
        web_search_wire: freehand_config::ProviderWebSearchWire::WebSearch,
        auth_type: freehand_config::ProviderAuthType::ApiKey,
        auth_source: freehand_config::ProviderAuthSourceKind::Env,
        api_key: "fallback-test-api-key".to_owned(),
    });
    selected
}

fn live_selected_worker_agent(
    base_url: String,
    provider_type: freehand_config::ProviderType,
) -> SelectedAgentConfig {
    let mut selected = live_selected_agent(base_url, provider_type);
    selected.name = "worker-live".to_owned();
    selected.mode = AgentMode::Slave;
    set_single_master_peer(&mut selected, "master-live");
    selected
}

fn temp_runtime_home() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let process_id = std::process::id();
    let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "freehand-runtime-live-{process_id}-{stamp}-{counter}"
    ))
}

#[cfg(unix)]
fn create_dir_symlink(source: &Path, link: &Path) {
    std::os::unix::fs::symlink(source, link).expect("create dir symlink");
}

#[cfg(windows)]
fn create_dir_symlink(source: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(source, link).expect("create dir symlink");
}

fn live_request(stream: bool) -> LiveReasonTurnRequest {
    LiveReasonTurnRequest {
        runtime_home: temp_runtime_home(),
        session_id: SessionId::new("session-live"),
        turn_id: TurnId::new("turn-live"),
        trace_id: TraceId::new("trace-live"),
        prompt: "reply exactly pong".to_owned(),
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: None,
        execution_profile: LiveReasonExecutionProfile::Workspace,
        stream,
        cancel_token: None,
    }
}

fn live_request_for(runtime_home: &Path, session_id: &str, ordinal: u64) -> LiveReasonTurnRequest {
    LiveReasonTurnRequest {
        runtime_home: runtime_home.to_path_buf(),
        session_id: SessionId::new(session_id),
        turn_id: TurnId::new(format!("runtime-turn-{ordinal}")),
        trace_id: TraceId::new(format!("runtime-trace-{ordinal}")),
        prompt: format!("prompt for {session_id}"),
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: None,
        execution_profile: LiveReasonExecutionProfile::Workspace,
        stream: false,
        cancel_token: None,
    }
}

fn inject_live_master_attention_resolution(
    runtime_home: &Path,
    session_id: &SessionId,
    turn_id: &TurnId,
    trace_id: &TraceId,
    attention_event_id: &str,
) {
    let agent_id = AgentId::new("agent-live");
    let mut active = master_runner::load_master_active_work(runtime_home, &agent_id)
        .expect("load active work")
        .expect("active work");
    assert_eq!(&active.session_id, session_id);
    assert_eq!(&active.logical_turn_id, turn_id);
    assert_eq!(&active.trace_id, trace_id);
    active.state = master_runner::MasterActiveWorkState::Running;
    active.attention_resolution = Some(master_runner::MasterAttentionResolution {
        attention_event_id: attention_event_id.to_owned(),
        decision_kind: "task_advanced".to_owned(),
        changed_task_ids: vec![TaskId::new("task-attention-changed")],
        changed_constraints: vec!["acceptance changed".to_owned()],
        resume_from: master_runner::MasterWorkReference {
            work_id: active.work_id.clone(),
            session_id: active.session_id.clone(),
            logical_turn_id: active.logical_turn_id.clone(),
            trace_id: active.trace_id.clone(),
        },
    });
    let path = runtime_home
        .join("state")
        .join("master-loop")
        .join("agent-live.active-work.json");
    fs::write(
        path,
        serde_json::to_string_pretty(&active).expect("serialize active work"),
    )
    .expect("write active work resolution");
}

fn lifecycle_live_request(runtime_home: &Path, event_id: &str) -> LiveReasonTurnRequest {
    LiveReasonTurnRequest {
        runtime_home: runtime_home.to_path_buf(),
        session_id: SessionId::new(format!("master-lifecycle-{event_id}")),
        turn_id: TurnId::new(format!("master-lifecycle-{event_id}-decision")),
        trace_id: TraceId::new(format!("master-lifecycle-trace-{event_id}")),
        prompt: format!("make one Task Center decision for {event_id}"),
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: Some(runtime_home.to_path_buf()),
        execution_profile: LiveReasonExecutionProfile::Workspace,
        stream: false,
        cancel_token: None,
    }
}

fn create_lifecycle_test_worker(runtime: &TaskRuntime) {
    runtime
        .create_agent(AgentCreateRequest {
            agent_id: AgentId::new("worker"),
            capabilities: vec!["workspace".to_owned()],
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("create-worker"),
        })
        .expect("create worker");
}

fn create_lifecycle_test_task(runtime: &TaskRuntime, task_id: &str) -> TaskSnapshot {
    runtime
        .create_task(TaskCreateRequest {
            task_id: Some(TaskId::new(task_id)),
            title: format!("{task_id} title"),
            content: "lifecycle decision fixture".to_owned(),
            goal: "persist one target task decision".to_owned(),
            deliverables: vec!["decision evidence".to_owned()],
            acceptance: vec!["target task changes".to_owned()],
            priority: 90,
            target_cwd: Some(std::env::temp_dir().display().to_string()),
            execution_profile: TaskExecutionProfile::Workspace,
            dispatch: TaskDispatchRequest::None,
            parent: TaskParentRef {
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            actor: lifecycle_test_actor(),
            watermark: lifecycle_test_watermark("create-task"),
        })
        .expect("create lifecycle task")
        .task
}

fn lifecycle_test_actor() -> TaskActor {
    TaskActor {
        agent_id: AgentId::new("agent-live"),
        source: "runtime.master-worker-loop.test".to_owned(),
        session_id: None,
        turn_id: None,
        trace_id: None,
    }
}

fn lifecycle_test_watermark(hook: &str) -> TaskWatermark {
    TaskWatermark {
        metadata_id: None,
        hook: Some(hook.to_owned()),
        action_tool_call_id: None,
    }
}

fn with_temp_workspace<F>(test: F)
where
    F: FnOnce(&Path),
{
    with_locked_cwd(|| {
        let original = std::env::current_dir().expect("current dir");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "freehand-runtime-tools-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp workspace");
        std::env::set_current_dir(&root).expect("set cwd");
        let restore = RestoreCwd { original };
        test(&root);
        drop(restore);
        fs::remove_dir_all(&root).expect("cleanup temp workspace");
    });
}

fn with_locked_cwd<F, R>(test: F) -> R
where
    F: FnOnce() -> R,
{
    let _lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    test()
}

fn cwd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn fnv1a_hex_for_test(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

struct RestoreCwd {
    original: PathBuf,
}

impl Drop for RestoreCwd {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

fn checkpoint_ledger_rows(
    runtime_home: &Path,
    agent_id: &str,
    session_id: &SessionId,
) -> Vec<RuntimeCheckpointLedgerRow> {
    let path = runtime_home
        .join("ledgers")
        .join("checkpoints")
        .join(agent_id)
        .join(format!("{}.jsonl", session_id.as_str()));
    let raw = fs::read_to_string(path).expect("read checkpoint ledger");
    raw.lines()
        .map(|line| serde_json::from_str(line).expect("decode ledger row"))
        .collect()
}

fn metadata_ledger_records(
    runtime_home: &Path,
    agent_id: &str,
    session_id: &SessionId,
) -> Vec<MetadataEnvelope> {
    let path = runtime_home
        .join("ledgers")
        .join("metadata")
        .join(agent_id)
        .join(format!("{}.jsonl", session_id.as_str()));
    let raw = fs::read_to_string(path).expect("read metadata ledger");
    raw.lines()
        .map(|line| serde_json::from_str(line).expect("decode metadata ledger row"))
        .collect()
}

fn provider_raw_ledger_rows(
    runtime_home: &Path,
    provider_family: &str,
    agent_id: &str,
    session_id: &SessionId,
    turn_id: &str,
) -> Vec<ProviderRawLedgerRow> {
    let path = runtime_home
        .join("ledgers")
        .join("providers")
        .join(provider_family)
        .join(agent_id)
        .join(session_id.as_str())
        .join(format!("{turn_id}.jsonl"));
    let raw = fs::read_to_string(path).expect("read provider raw ledger");
    raw.lines()
        .map(|line| serde_json::from_str(line).expect("decode provider raw ledger row"))
        .collect()
}

fn runtime_debug_events<'a>(events: &'a [DebugEvent], pipeline_node: &str) -> Vec<&'a DebugEvent> {
    events
        .iter()
        .filter(|event| {
            event
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.scene.crate_name == "freehand-runtime")
                && event.envelope.semantic.feature_id.as_str() == "provider.reason-live-bridge"
                && event.envelope.semantic.pipeline_node.as_deref() == Some(pipeline_node)
        })
        .collect()
}

fn spawn_mock_server(
    status: u16,
    content_type: &'static str,
    response_body: String,
) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("addr"));
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        let mut raw = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).expect("read");
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&buffer[..read]);
            if request_is_complete(&raw) {
                break;
            }
        }
        let request = String::from_utf8(raw).expect("utf8");
        tx.send(request).expect("send");
        let response = format!(
            "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
            response_body.len()
        );
        stream.write_all(response.as_bytes()).expect("write");
    });
    (base_url, rx, handle)
}

fn spawn_incremental_stream_server(
    first_chunk: String,
    remaining_chunks: String,
) -> (
    String,
    mpsc::Receiver<String>,
    mpsc::Receiver<bool>,
    mpsc::Sender<()>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("addr"));
    let (request_tx, request_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (continue_tx, continue_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        let mut raw = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).expect("read");
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&buffer[..read]);
            if request_is_complete(&raw) {
                break;
            }
        }
        request_tx
            .send(String::from_utf8(raw).expect("utf8"))
            .expect("send");
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n",
            )
            .expect("write headers");
        stream
            .write_all(first_chunk.as_bytes())
            .expect("write first chunk");
        stream.flush().expect("flush first chunk");

        let released = continue_rx.recv_timeout(Duration::from_secs(2)).is_ok();
        release_tx.send(released).expect("send release");
        if released {
            stream
                .write_all(remaining_chunks.as_bytes())
                .expect("write remaining chunks");
            stream.flush().expect("flush remaining chunks");
        }
    });
    (base_url, request_rx, release_rx, continue_tx, handle)
}

fn spawn_sequence_server(
    content_type: &'static str,
    response_bodies: Vec<String>,
) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
    spawn_status_sequence_server(
        response_bodies
            .into_iter()
            .map(|body| (200, content_type, body))
            .collect(),
    )
}

fn spawn_status_sequence_server(
    responses: Vec<(u16, &'static str, String)>,
) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("addr"));
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        for (status, content_type, response_body) in responses {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("timeout");
            let mut raw = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("read");
                if read == 0 {
                    break;
                }
                raw.extend_from_slice(&buffer[..read]);
                if request_is_complete(&raw) {
                    break;
                }
            }
            tx.send(String::from_utf8(raw).expect("utf8"))
                .expect("send");
            let response = format!(
                "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
                response_body.len()
            );
            stream.write_all(response.as_bytes()).expect("write");
        }
    });
    (base_url, rx, handle)
}

fn spawn_retry_gate_server() -> (
    String,
    mpsc::Receiver<String>,
    mpsc::Sender<()>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("addr"));
    let (request_tx, request_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let responses = [
                (
                    500,
                    "application/json",
                    r#"{"type":"error","error":{"type":"api_error","message":"first upstream failure"}}"#
                        .to_owned(),
                ),
                (
                    200,
                    "application/json",
                    complete_single_response("retry gate ok"),
                ),
            ];
        for (index, (status, content_type, response_body)) in responses.into_iter().enumerate() {
            if index == 1 {
                release_rx
                    .recv_timeout(Duration::from_secs(3))
                    .expect("release second attempt");
            }
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("timeout");
            let mut raw = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("read");
                if read == 0 {
                    break;
                }
                raw.extend_from_slice(&buffer[..read]);
                if request_is_complete(&raw) {
                    break;
                }
            }
            request_tx
                .send(String::from_utf8(raw).expect("utf8"))
                .expect("send request");
            let response = format!(
                "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\r\n{response_body}",
                response_body.len()
            );
            stream.write_all(response.as_bytes()).expect("write");
        }
    });
    (base_url, request_rx, release_tx, handle)
}

fn request_is_complete(raw: &[u8]) -> bool {
    let text = String::from_utf8_lossy(raw);
    let Some(header_end) = text.find("\r\n\r\n") else {
        return false;
    };
    let content_length = text[..header_end]
        .lines()
        .find_map(|line| {
            line.strip_prefix("content-length: ")
                .or_else(|| line.strip_prefix("Content-Length: "))
        })
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    raw.len() >= header_end + 4 + content_length
}

fn http_request_body_json(raw: &str) -> Value {
    let (_, body) = raw
        .split_once("\r\n\r\n")
        .expect("HTTP request should contain a body separator");
    serde_json::from_str(body).expect("HTTP request body json")
}

fn tagged_completion_json(body: &str) -> String {
    format!("<freehand_completion>\n{body}\n</freehand_completion>")
}

fn complete_single_response(visible_text: &str) -> String {
    let tagged = tagged_completion_json(&format!(
        r#"{{"claim":"complete","completion_reason":"done","evidence":"provider returned {visible_text}","summary":"{visible_text}","learned":"keep tagged completion strict"}}"#
    ));
    format!(
        r#"{{"content":[{{"type":"text","text":"{visible}\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":82}},"stop_reason":"end_turn"}}"#,
        visible = visible_text,
        tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
    )
}

fn blocked_single_response(visible_text: &str, blocked_reason: &str) -> String {
    let tagged = tagged_completion_json(&format!(
        r#"{{"claim":"blocked","blocked_reason":"{blocked_reason}"}}"#
    ));
    format!(
        r#"{{"content":[{{"type":"text","text":"{visible}\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":82}},"stop_reason":"end_turn"}}"#,
        visible = visible_text,
        tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
    )
}

fn openai_responses_complete_response(visible_text: &str) -> String {
    let tagged = tagged_completion_json(&format!(
        r#"{{"claim":"complete","completion_reason":"done","evidence":"provider returned {visible_text}","summary":"{visible_text}","learned":"keep tagged completion strict"}}"#
    ));
    json!({
        "id": "resp-test",
        "object": "response",
        "status": "completed",
        "output": [{
            "type": "message",
            "id": "msg-test",
            "role": "assistant",
            "status": "completed",
            "content": [{
                "type": "output_text",
                "text": format!("{visible_text}\n{tagged}"),
                "annotations": []
            }]
        }],
        "usage": {
            "input_tokens": 14,
            "output_tokens": 82,
            "total_tokens": 96
        }
    })
    .to_string()
}

fn status_stop_single_response(visible_text: &str) -> String {
    let status = r#"<<<freehand_status>>>
{"schema_version":1,"status":{"simple_question":true}}
<</freehand_status>>>"#;
    json!({
        "content": [{
            "type": "text",
            "text": format!("{visible_text}\n{status}")
        }],
        "usage": {"input_tokens": 14, "output_tokens": 40},
        "stop_reason": "end_turn"
    })
    .to_string()
}

fn continue_single_response(next_step: &str) -> String {
    let tagged = tagged_completion_json(&format!(
        r#"{{"claim":"continue","next_step":"{next_step}"}}"#
    ));
    format!(
        r#"{{"content":[{{"type":"text","text":"working\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":40}},"stop_reason":"end_turn"}}"#,
        tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
    )
}

fn waiting_single_response(next_step: &str) -> String {
    let tagged = tagged_completion_json(&format!(
        r#"{{"claim":"waiting","next_step":"{next_step}"}}"#
    ));
    format!(
        r#"{{"content":[{{"type":"text","text":"waiting\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":40}},"stop_reason":"end_turn"}}"#,
        tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
    )
}

fn continue_with_visible_response(visible_text: &str, next_step: &str) -> String {
    let tagged = tagged_completion_json(&format!(
        r#"{{"claim":"continue","next_step":"{next_step}"}}"#
    ));
    json!({
        "content": [{
            "type": "text",
            "text": format!("{visible_text}\n{tagged}")
        }],
        "usage": {"input_tokens": 14, "output_tokens": 40},
        "stop_reason": "end_turn"
    })
    .to_string()
}

fn invalid_complete_response() -> String {
    let tagged = tagged_completion_json(r#"{"claim":"complete","summary":"pong"}"#);
    format!(
        r#"{{"content":[{{"type":"text","text":"draft\n{tagged}"}}],"usage":{{"input_tokens":14,"output_tokens":40}},"stop_reason":"end_turn"}}"#,
        tagged = tagged.replace('\n', "\\n").replace('"', "\\\""),
    )
}

fn missing_completion_schema_response() -> String {
    json!({
        "content": [{
            "type": "text",
            "text": "draft without the required Freehand completion block"
        }],
        "usage": {"input_tokens": 14, "output_tokens": 40},
        "stop_reason": "end_turn"
    })
    .to_string()
}

fn max_tokens_text_response() -> String {
    json!({
        "content": [{
            "type": "text",
            "text": "partial response without a completion schema"
        }],
        "usage": {"input_tokens": 14, "output_tokens": 512},
        "stop_reason": "max_tokens"
    })
    .to_string()
}

fn task_tool_call(arguments: Vec<(&str, Value)>) -> ReasonReq04ToolCall {
    ReasonReq04ToolCall {
        session_id: SessionId::new("session-task"),
        turn_id: TurnId::new("turn-task"),
        trace_id: TraceId::new("trace-task"),
        feature_id: FeatureId::new("provider.reason-live-bridge"),
        agent_id: AgentId::new("agent-task"),
        tool_call: ToolCallContract {
            tool_call_id: ToolCallId::new("toolu_task_1"),
            tool_name: "task".to_owned(),
            arguments: arguments
                .into_iter()
                .map(|(name, value)| ToolArgument {
                    name: name.to_owned(),
                    value,
                })
                .collect(),
            arguments_complete: true,
        },
    }
}

fn timer_tool_call(arguments: Vec<(&str, Value)>) -> ReasonReq04ToolCall {
    ReasonReq04ToolCall {
        session_id: SessionId::new("session-timer"),
        turn_id: TurnId::new("turn-timer"),
        trace_id: TraceId::new("trace-timer"),
        feature_id: FeatureId::new("provider.reason-live-bridge"),
        agent_id: AgentId::new("agent-live"),
        tool_call: ToolCallContract {
            tool_call_id: ToolCallId::new("toolu_timer_1"),
            tool_name: "timer".to_owned(),
            arguments: arguments
                .into_iter()
                .map(|(name, value)| ToolArgument {
                    name: name.to_owned(),
                    value,
                })
                .collect(),
            arguments_complete: true,
        },
    }
}

fn tool_use_named_response(tool_call_id: &str, tool_name: &str, input: Value) -> String {
    json!({
        "content": [{
            "type": "tool_use",
            "id": tool_call_id,
            "name": tool_name,
            "input": input
        }],
        "usage": {"input_tokens": 20, "output_tokens": 16},
        "stop_reason": "tool_use"
    })
    .to_string()
}

fn task_tool_use_response(tool_call_id: &str, input: Value) -> String {
    tool_use_named_response(tool_call_id, "task", input)
}

fn master_autonomy_prompt(sentinel: &str) -> String {
    format!(
            "{}\n{sentinel}",
            (0..80)
                .map(|index| format!(
                    "step-{index}: master must create a worker task, dispatch it, inspect worker result, handle success, execution error, and incomplete review retry without losing this instruction."
                ))
                .collect::<Vec<_>>()
                .join("\n")
        )
}

fn collect_provider_requests(rx: &mpsc::Receiver<String>, expected: usize) -> Vec<String> {
    (0..expected)
        .map(|index| {
            rx.recv()
                .unwrap_or_else(|err| panic!("provider request {index}: {err}"))
        })
        .collect()
}

fn assert_master_task_request_contract(
    raw_request: &str,
    sentinel: &str,
    configured_worker_set: &str,
) {
    assert!(raw_request.contains(sentinel));
    assert!(raw_request.contains("Master task orchestration policy"));
    assert!(
        raw_request
            .contains("`continue` means Freehand should immediately run another model round")
    );
    assert!(raw_request.contains("Do not use `continue` to wait for a Worker, timer"));
    assert!(raw_request.contains("you are the master agent"));
    assert!(raw_request.contains("Dispatch when"));
    assert!(raw_request.contains("Master local tool surface"));
    assert!(raw_request.contains("Master network tool surface"));
    assert!(raw_request.contains("current selected session cwd"));
    assert!(raw_request.contains("Use them directly for local repository analysis"));
    assert!(raw_request.contains("Do not dispatch when"));
    assert!(raw_request.contains("`web_fetch` fetches known HTTP/HTTPS URLs"));
    assert!(raw_request.contains("Configured Worker capability surface"));
    assert!(raw_request.contains("configured_worker_capabilities"));
    assert!(raw_request.contains("network_tools"));
    assert!(raw_request.contains("If your own Master surface cannot complete a slice directly"));
    assert!(raw_request.contains(
        "Finish blocked only when neither Master nor any configured Worker/provider route has the required search capability"
    ));
    assert!(!raw_request.contains("no web tool is exposed"));
    assert!(
        !raw_request.contains("Do not create a Worker task for pure web/current-news research")
    );
    assert!(raw_request.contains("Multi-agent dispatch"));
    assert!(raw_request.contains("Concurrency control"));
    assert!(raw_request.contains("Flow control"));
    assert!(raw_request.contains("Task tool workflow"));
    assert!(raw_request.contains("Timer workflow"));
    assert!(raw_request.contains("Ordinary responses must omit it"));
    assert!(raw_request.contains("include only the required <freehand_completion> block"));
    assert!(raw_request.contains(&format!("Configured Worker ids: `{configured_worker_set}`")));
    assert!(raw_request.contains("Historical agents returned by list_agents"));
    assert!(raw_request.contains("never put task(...)"));
    assert!(raw_request.contains("The Worker does not receive the task tool"));
    assert!(
        raw_request.contains(
            "converts the Worker completion schema into TaskReviewSubmitted or TaskBlocked"
        )
    );
    assert!(raw_request.contains("Master framework tool surface"));
    assert!(raw_request.contains("Do not call shell/bash"));
    assert!(raw_request.contains("Workspace boundary"));
    assert!(raw_request.contains("top-level JSON object must include \\\"op\\\""));
    assert!(raw_request.contains("Never omit op"));
    assert!(raw_request.contains("expanded absolute path"));
    assert!(raw_request.contains("leading-~/symlink aliases are valid target_cwd values"));
    assert!(raw_request.contains("target_cwd_path_diagnostic"));
    assert!(raw_request.contains("\\\"target_cwd\\\":\\\"/absolute/existing/workspace"));
    assert!(raw_request.contains("assign only useful independent subtasks"));
    assert!(raw_request.contains("{\\\"op\\\":\\\"list_agents\\\"}"));
    assert!(raw_request.contains("{\\\"op\\\":\\\"schedule\\\""));
    assert!(raw_request.contains("A timer is not scheduled until the timer tool returns"));
    assert!(raw_request.contains("do not claim or imply that a timer was scheduled"));
    assert!(raw_request.contains(
        "If no other work is ready and the user's requested final outcome is not yet delivered"
    ));
    assert!(raw_request.contains("finish the current turn with `claim=\\\"waiting\\\"`"));
    assert!(raw_request.contains("next useful wait exceeds 3 minutes"));
    assert!(raw_request.contains("dead-waiting in the current turn"));
    assert!(raw_request.contains("continue any other ready Master-side work"));
    assert!(raw_request.contains("what waited condition to revisit"));
    assert!(raw_request.contains("task-space-snapshot"));
    assert!(raw_request.contains("<freehand_task_space>"));
    assert!(raw_request.contains("valid_task_status_filters"));
    assert!(raw_request.contains("known_tasks"));
    assert!(raw_request.contains("Do not call status=\\\"all\\\""));
    assert!(raw_request.contains("Master task orchestration examples"));
    assert!(raw_request.contains("Local workspace sample"));
    assert!(raw_request.contains("Web fetch sample"));
    assert!(raw_request.contains("Cross-workspace sample"));
    assert!(raw_request.contains("~/work/repo-a"));
    assert!(raw_request.contains("~/work/repo-b"));
    assert!(raw_request.contains("target_cwd"));
    assert!(raw_request.contains("Worker success sample"));
    assert!(raw_request.contains("Worker execution error sample"));
    assert!(raw_request.contains("Worker retry sample"));
    assert!(raw_request.contains("\"name\":\"task\""));
    assert!(raw_request.contains("\"name\":\"timer\""));
    assert!(raw_request.contains("\"name\":\"web_fetch\""));
    for local_tool in [
        "\"name\":\"read_file\"",
        "\"name\":\"ls\"",
        "\"name\":\"grep\"",
        "\"name\":\"glob\"",
        "\"name\":\"write_file\"",
        "\"name\":\"edit_file\"",
        "\"name\":\"multi_edit\"",
        "\"name\":\"delete_range\"",
    ] {
        assert!(
            raw_request.contains(local_tool),
            "master request must expose local workspace tool schema {local_tool}"
        );
    }
    for forbidden in [
        "\"name\":\"complete_step\"",
        "\"name\":\"todo_write\"",
        "\"name\":\"bash\"",
    ] {
        assert!(
            !raw_request.contains(forbidden),
            "master request must not expose forbidden tool schema {forbidden}"
        );
    }
    assert!(raw_request.contains("\"record_execution\""));
    assert!(raw_request.contains("\"retry_count\""));
    assert!(raw_request.contains("create_agent"));
    assert!(raw_request.contains("review_ready"));
}

fn task_truth(runtime_home: &Path, task_id: &str) -> (TaskSnapshot, Vec<String>) {
    let task_runtime =
        TaskRuntime::boot(runtime_home, AgentId::new("agent-live")).expect("task runtime");
    let task = task_runtime
        .query_task(&TaskId::new(task_id))
        .expect("query task truth");
    let event_types = task_runtime
        .task_history(&TaskId::new(task_id))
        .expect("task history")
        .into_iter()
        .map(|event| event.event_type)
        .collect::<Vec<_>>();
    (task, event_types)
}

fn event_index(events: &[String], event_type: &str) -> usize {
    events
        .iter()
        .position(|event| event == event_type)
        .unwrap_or_else(|| panic!("missing event {event_type}: {events:?}"))
}

fn tool_use_single_response() -> String {
    tool_use_named_response(
        "toolu_read_1",
        "read_file",
        json!({"path":"Cargo.toml","offset":0,"limit":2}),
    )
}

fn incomplete_tool_use_stream_response() -> String {
    concat!(
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_incomplete_1\",\"name\":\"read_file\",\"input\":{}}}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"input_tokens\":20,\"output_tokens\":8}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        )
        .to_owned()
}

fn tool_use_missing_read_response() -> String {
    tool_use_named_response(
        "toolu_missing_read_1",
        "read_file",
        json!({"path":"definitely-missing-freehand-file.txt","offset":0,"limit":2}),
    )
}

fn tool_use_unknown_response() -> String {
    tool_use_named_response(
        "toolu_unknown_1",
        "totally_unknown_tool",
        json!({"path":"Cargo.toml"}),
    )
}

fn tool_use_write_file_response(path: &str, content: &str) -> String {
    tool_use_named_response(
        "toolu_write_1",
        "write_file",
        json!({
            "path": path,
            "content": content
        }),
    )
}

fn tool_use_edit_file_response(path: &str, old_string: &str, new_string: &str) -> String {
    tool_use_named_response(
        "toolu_edit_1",
        "edit_file",
        json!({
            "path": path,
            "old_string": old_string,
            "new_string": new_string
        }),
    )
}

fn tool_use_bash_response(command: &str) -> String {
    tool_use_named_response(
        "toolu_bash_1",
        "bash",
        json!({
            "command": command
        }),
    )
}

fn complete_stream_response(visible_text: &str) -> String {
    let tagged = tagged_completion_json(
        r#"{"claim":"complete","completion_reason":"done","evidence":"provider returned pong","summary":"pong","learned":"keep tagged completion strict"}"#,
    );
    format!(
        concat!(
            "event: content_block_start\n",
            "data: {{\"type\":\"content_block_start\",\"index\":0,\"content_block\":{{\"type\":\"thinking\",\"thinking\":\"\"}}}}\n\n",
            "event: content_block_delta\n",
            "data: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"thinking_delta\",\"thinking\":\"thinking\"}}}}\n\n",
            "event: content_block_stop\n",
            "data: {{\"type\":\"content_block_stop\",\"index\":0}}\n\n",
            "event: content_block_start\n",
            "data: {{\"type\":\"content_block_start\",\"index\":1,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n",
            "event: content_block_delta\n",
            "data: {{\"type\":\"content_block_delta\",\"index\":1,\"delta\":{{\"type\":\"text_delta\",\"text\":\"{text}\"}}}}\n\n",
            "event: content_block_stop\n",
            "data: {{\"type\":\"content_block_stop\",\"index\":1}}\n\n",
            "event: message_delta\n",
            "data: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"input_tokens\":14,\"output_tokens\":82}}}}\n\n",
            "event: message_stop\n",
            "data: {{\"type\":\"message_stop\"}}\n\n"
        ),
        text = format!("{visible_text}\\n{tagged}")
            .replace('\n', "\\n")
            .replace('"', "\\\"")
    )
}

#[test]
fn live_bridge_runs_single_shot_anthropic_provider_into_turn_truth() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) =
        spawn_mock_server(200, "application/json", complete_single_response("pong"));
    let request = live_request(false);
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();
    let mut debug_events = Vec::<DebugEvent>::new();

    let outcome = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
        |_| {},
        |event| debug_events.push(event.clone()),
        |_| {},
    )
    .expect("live bridge");
    let raw_request = rx.recv().expect("request");
    handle.join().expect("join");

    assert!(raw_request.starts_with("POST /v1/messages HTTP/1.1"));
    assert!(raw_request.contains("x-api-key: test-api-key"));
    assert!(raw_request.contains("\"stream\":false"));
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|e| e.status.clone()),
        Some(TerminalStatus::Success)
    );
    assert!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .is_some_and(|event| event.summary.contains("Summary: pong"))
    );
    assert_eq!(
        strip_completion_submission_block(&collect_turn_text(&outcome.turn)),
        "pong"
    );
    assert!(
        outcome
            .broadcasts
            .iter()
            .any(|event| matches!(event, ReasonBroadcastEvent::Usage(_)))
    );

    let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "provider.reason-live-bridge"
            && record.write_node.pipeline_node == "RuntimeLive01RestoreResolved"
    }));
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "provider.reason-live-bridge"
            && record.write_node.pipeline_node == "RuntimeLive01ContextPlanningStarted"
    }));
    assert_eq!(
        metadata
            .iter()
            .filter(|record| {
                record.owner.feature_id.as_str() == "provider.reason-live-bridge"
                    && record.write_node.pipeline_node == "RuntimeLive01ContextSegmentStarted"
            })
            .count(),
        6
    );
    assert_eq!(
        metadata
            .iter()
            .filter(|record| {
                record.owner.feature_id.as_str() == "provider.reason-live-bridge"
                    && record.write_node.pipeline_node == "RuntimeLive01ContextSegmentCompleted"
            })
            .count(),
        6
    );
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "provider.reason-live-bridge"
            && record.write_node.pipeline_node == "RuntimeLive01ContextSegmentCompleted"
            && metadata_entry_string(record, "context.segment_id").as_deref()
                == Some("instruction-capability")
            && metadata_entry_string(record, "context.segment_status").as_deref()
                == Some("completed")
            && metadata_entry_u64(record, "context.segment_elapsed_ms").is_some()
    }));
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "provider.reason-live-bridge"
            && record.write_node.pipeline_node == "RuntimeLive01ContextPlanningCompleted"
            && metadata_entry_u64(record, "context.segment_count").is_some()
            && metadata_entry_u64(record, "context.estimated_token_budget").is_some()
    }));
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "provider.reason-live-bridge"
            && record.write_node.pipeline_node == "RuntimeLive02ProviderRequestBuilt"
    }));
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "provider.reason-live-bridge"
            && record.write_node.pipeline_node == "RuntimeLive04TurnClosed"
    }));
    assert!(metadata.iter().all(
        |record| serde_json::to_string(record).expect("encode metadata")
            != outcome.turn.request.user_text
    ));
    assert!(metadata.iter().all(|record| {
        let encoded = serde_json::to_string(record).expect("encode metadata");
        !encoded.contains("reply exactly pong")
    }));
    let provider_raw = provider_raw_ledger_rows(
        &runtime_home,
        "anthropic",
        "agent-live",
        &session_id,
        "turn-live",
    );
    assert_eq!(provider_raw.len(), 1);
    assert_eq!(provider_raw[0].raw_kind, "response_body");
    assert!(
        provider_raw[0]
            .body
            .contains("\"stop_reason\":\"end_turn\"")
    );
    assert_eq!(
        runtime_debug_events(&debug_events, "RuntimeLive01RestoreResolved").len(),
        1
    );
    assert_eq!(
        runtime_debug_events(&debug_events, "RuntimeLive01ContextPlanningStarted").len(),
        1
    );
    assert_eq!(
        runtime_debug_events(&debug_events, "RuntimeLive01ContextSegmentStarted").len(),
        6
    );
    assert_eq!(
        runtime_debug_events(&debug_events, "RuntimeLive01ContextSegmentCompleted").len(),
        6
    );
    assert_eq!(
        runtime_debug_events(&debug_events, "RuntimeLive01ContextPlanningCompleted").len(),
        1
    );
    assert_eq!(
        runtime_debug_events(&debug_events, "RuntimeLive02ProviderRequestBuilt").len(),
        1
    );
    assert_eq!(
        runtime_debug_events(&debug_events, "RuntimeLive04TurnClosed").len(),
        1
    );
    let expected_tool_count = BuiltinToolRegistry::reasonix_aligned()
        .master_implemented_definitions()
        .len();
    assert!(
        runtime_debug_events(&debug_events, "RuntimeLive02ProviderRequestBuilt")
            .into_iter()
            .flat_map(|event| {
                event
                    .snapshot
                    .as_ref()
                    .expect("runtime snapshot")
                    .detail_lines
                    .iter()
            })
            .any(|line| line == &format!("tool_definition_count={expected_tool_count}"))
    );
    assert!(debug_events.iter().all(|event| {
        let encoded = serde_json::to_string(event).expect("encode debug event");
        !encoded.contains("reply exactly pong")
    }));
}

#[test]
fn worker_live_bridge_excludes_shell_task_and_locks_task_workspace() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_mock_server(
        200,
        "application/json",
        complete_single_response("worker complete"),
    );
    let runtime_home = temp_runtime_home();
    let workspace = temp_runtime_home();
    fs::create_dir_all(&workspace).expect("create worker workspace");
    let canonical_workspace = fs::canonicalize(&workspace).expect("canonical workspace");
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.cwd = Some(canonical_workspace.clone());
    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    selected.name = "worker-live".to_owned();
    selected.mode = AgentMode::Slave;
    set_single_master_peer(&mut selected, "master-live");

    let outcome = run_worker_live_reason_turn(&selected, request).expect("worker live bridge");
    let raw_request = rx.recv().expect("provider request");
    handle.join().expect("join provider");

    assert!(!raw_request.contains("\"name\":\"bash\""));
    assert!(raw_request.contains("\"name\":\"read_file\""));
    assert!(!raw_request.contains("\"name\":\"task\""));
    assert!(raw_request.contains("Worker execution policy"));
    assert_eq!(
        outcome.turn.cwd.as_deref(),
        Some(canonical_workspace.to_string_lossy().as_ref())
    );
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn worker_live_bridge_returns_injected_shell_as_failed_tool_result() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let shell_target = temp_runtime_home().join("must-not-run");
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_bash_response(&format!("touch {}", shell_target.display())),
            complete_single_response("worker recovered after forbidden shell"),
        ],
    );
    let runtime_home = temp_runtime_home();
    let workspace = temp_runtime_home();
    fs::create_dir_all(&workspace).expect("create worker workspace");
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.cwd = Some(fs::canonicalize(&workspace).expect("canonical workspace"));
    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    selected.name = "worker-live".to_owned();
    selected.mode = AgentMode::Slave;
    set_single_master_peer(&mut selected, "master-live");

    let outcome = run_worker_live_reason_turn(&selected, request).expect("worker live bridge");
    let _first_request = rx.recv().expect("first provider request");
    let second_request = rx.recv().expect("second provider request");
    handle.join().expect("join provider");

    assert!(second_request.contains("\"type\":\"tool_result\""));
    assert!(second_request.contains("\"tool_use_id\":\"toolu_bash_1\""));
    assert!(second_request.contains("\"is_error\":true"));
    assert!(second_request.contains("shell execution is not available"));
    assert!(second_request.contains("Available Worker tools are exactly"));
    assert!(second_request.contains("Do not call shell, bash, readlink"));
    assert!(
        !second_request.contains("external inspection"),
        "failed shell result must not invite external probing"
    );
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.tool_executions, 1);
    assert!(
        !shell_target.exists(),
        "forbidden shell command must not execute"
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
    fs::remove_dir_all(workspace).expect("cleanup workspace");
}

#[test]
fn worker_live_bridge_rejects_master_mode_and_missing_workspace() {
    let selected = selected_master_agent();
    let mut request = live_request(false);
    request.cwd = Some(temp_runtime_home());
    assert!(matches!(
        run_worker_live_reason_turn(&selected, request),
        Err(RuntimeLiveBridgeError::AgentModeMismatch { .. })
    ));

    let mut worker = selected;
    worker.mode = AgentMode::Slave;
    set_single_master_peer(&mut worker, "master");
    assert_eq!(
        run_worker_live_reason_turn(&worker, live_request(false)),
        Err(RuntimeLiveBridgeError::WorkerWorkspaceRequired)
    );
}

#[test]
fn live_bridge_accepts_simple_status_stop_hook_without_completion_schema() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) =
        spawn_mock_server(200, "application/json", status_stop_single_response("pong"));
    let request = live_request(false);
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();

    let outcome = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
        |_| {},
        |_| {},
        |_| {},
    )
    .expect("status stop hook");
    let _raw_request = rx.recv().expect("request");
    handle.join().expect("join");

    assert_eq!(outcome.rounds, 1);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
    assert!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .is_some_and(|event| event.summary.contains("Summary: pong"))
    );

    let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "control.center"
            && record.write_node.pipeline_node == "ControlHook03AfterModelResponse"
            && record.entries.iter().any(|entry| {
                entry.key == "control.decision" && entry.value == json!("allow_natural_stop")
            })
    }));
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "control.center"
            && record.write_node.pipeline_node == "ControlHook04BeforeClientReturn"
            && record.entries.iter().any(|entry| {
                entry.key == "control.public_projection_stripped" && entry.value == json!(true)
            })
    }));
    assert!(metadata.iter().all(|record| {
        let encoded = serde_json::to_string(record).expect("metadata json");
        !encoded.contains("<<<freehand_status>>>") && !encoded.contains("pong")
    }));
}

#[test]
fn live_bridge_polishes_invalid_control_status_without_provider_failure() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let invalid_status = json!({
            "content": [{
                "type": "text",
                "text": "working\n<<<freehand_status>>>\n{\"schema_version\":1,\"status\":{\"simple_question\":true,\"next_step\":42}}\n<</freehand_status>>>"
            }],
            "usage": {"input_tokens": 14, "output_tokens": 40},
            "stop_reason": "end_turn"
        })
        .to_string();
    let corrected_status = json!({
            "content": [{
                "type": "text",
                "text": "pong\n<<<freehand_status>>>\n{\"schema_version\":1,\"status\":{\"simple_question\":true,\"next_step\":null,\"blocked_reason\":null}}\n<</freehand_status>>>"
            }],
            "usage": {"input_tokens": 14, "output_tokens": 40},
            "stop_reason": "end_turn"
        })
        .to_string();
    let (base_url, rx, handle) =
        spawn_sequence_server("application/json", vec![invalid_status, corrected_status]);
    let request = live_request(false);
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();
    let broadcasts = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&broadcasts);

    let outcome = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
        move |event| {
            captured.lock().expect("broadcast lock").push(event.clone());
        },
        |_| {},
        |_| {},
    )
    .expect("control status should polish and continue");
    let first_request = rx.recv().expect("first request");
    let second_request = rx.recv().expect("second request");
    handle.join().expect("join");

    assert!(!first_request.contains("status schema was rejected"));
    assert!(second_request.contains("status schema was rejected"));
    assert!(second_request.contains("next_step"));
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.schema_rejections.len(), 1);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
    assert!(
        broadcasts
            .lock()
            .expect("broadcast lock")
            .iter()
            .any(|event| matches!(
                event,
                ReasonBroadcastEvent::CompletionSchemaRejected(rejection)
                    if rejection.feedback.contains("next_step")
            ))
    );
    let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "control.center"
            && record.write_node.pipeline_node == "ControlHook03AfterModelResponse"
            && record.entries.iter().any(|entry| {
                entry.key == "control.status_validation" && entry.value == json!("rejected")
            })
    }));
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "error.center"
            && record.entries.iter().any(|entry| {
                entry.key == "error.code" && entry.value == json!("control_status_schema_rejected")
            })
            && record.entries.iter().any(|entry| {
                entry.key == "error.recovery_action" && entry.value == json!("repair_schema")
            })
    }));
}

#[test]
fn live_bridge_blocks_after_three_consecutive_invalid_control_statuses() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let invalid_status = || {
        json!({
                "content": [{
                    "type": "text",
                    "text": "working\n<<<freehand_status>>>\n{\"schema_version\":1,\"status\":{\"simple_question\":true,\"next_step\":42}}\n<</freehand_status>>>"
                }],
                "usage": {"input_tokens": 14, "output_tokens": 40},
                "stop_reason": "end_turn"
            })
            .to_string()
    };
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![invalid_status(), invalid_status(), invalid_status()],
    );

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        live_request(false),
    )
    .expect("schema mismatch exhaustion is blocked truth");
    for _ in 0..3 {
        rx.recv().expect("provider request");
    }
    handle.join().expect("join");

    assert_eq!(outcome.rounds, 3);
    assert_eq!(outcome.schema_rejections.len(), 3);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Blocked)
    );
    assert!(outcome.turn.terminal_event.as_ref().is_some_and(|event| {
        event.summary.contains("3 polishing attempts") && event.summary.contains("next_step")
    }));
    assert_eq!(
        outcome
            .broadcasts
            .iter()
            .filter(|event| matches!(event, ReasonBroadcastEvent::CompletionSchemaRejected(_)))
            .count(),
        2
    );
}

#[test]
fn task_tool_create_persists_and_queries_task() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "create a task".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    let create_call = task_tool_call(vec![
        ("op", json!("create")),
        ("task_id", json!("task-runtime-test")),
        ("title", json!("Task persistence")),
        ("content", json!("Persist and recover task")),
        ("goal", json!("Task query survives runtime reboot")),
        ("deliverables", json!(["ledger", "snapshot"])),
        ("acceptance", json!(["query returns assigned task"])),
        ("dispatch", json!({"mode":"self"})),
    ]);

    let create_output = execute_task_tool(&runtime_home, &turn, &create_call).expect("create task");

    assert!(create_output.contains("task_id=task-runtime-test"));
    assert!(create_output.contains("status=Assigned"));

    let query_call = task_tool_call(vec![
        ("op", json!("query")),
        ("task_id", json!("task-runtime-test")),
    ]);
    let query_output = execute_task_tool(&runtime_home, &turn, &query_call).expect("query task");

    assert!(query_output.contains("\"task_id\":\"task-runtime-test\""));
    assert!(query_output.contains("\"status\":\"assigned\""));

    let agents_call = task_tool_call(vec![("op", json!("list_agents"))]);
    let agents_output = execute_task_tool(&runtime_home, &turn, &agents_call).expect("list agents");

    assert!(agents_output.contains("\"agent_id\":\"agent-task\""));
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn task_tool_create_clean_search_task_without_target_cwd() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "create a clean search task".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    let create_call = task_tool_call(vec![
        ("op", json!("create")),
        ("task_id", json!("task-clean-search")),
        ("title", json!("Clean search")),
        ("content", json!("Search current provider docs")),
        ("goal", json!("Return search evidence")),
        ("deliverables", json!(["search summary"])),
        ("acceptance", json!(["sources and gaps are reported"])),
        ("execution_profile", json!("clean_search")),
        ("dispatch", json!({"mode":"none"})),
    ]);

    let create_output =
        execute_task_tool(&runtime_home, &turn, &create_call).expect("create clean search task");
    assert!(create_output.contains("task_id=task-clean-search"));
    assert!(!create_output.contains("target_cwd_path_diagnostic"));

    let query_output = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("query")),
            ("task_id", json!("task-clean-search")),
        ]),
    )
    .expect("query clean search task");
    assert!(query_output.contains("\"execution_profile\":\"clean_search\""));
    assert!(query_output.contains("\"target_cwd\":null"));
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn task_tool_rejects_unknown_execution_profile() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "create invalid profile task".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    let err = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("create")),
            ("task_id", json!("task-invalid-profile")),
            ("title", json!("Invalid profile")),
            ("content", json!("Invalid")),
            ("goal", json!("Reject invalid profile")),
            ("deliverables", json!(["none"])),
            ("acceptance", json!(["explicit failure"])),
            ("execution_profile", json!("search")),
            ("dispatch", json!({"mode":"none"})),
        ]),
    )
    .expect_err("invalid execution profile must fail");
    assert!(err.contains(
        "unsupported execution_profile `search`; expected `workspace` or `clean_search`"
    ));
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn task_tool_create_returns_symlink_parent_path_diagnostic() {
    let base = temp_runtime_home();
    let runtime_home = base.join("runtime");
    let canonical_parent = base.join("Documents").join("workspace-parent");
    let symlink_parent = base.join("workspace-link");
    fs::create_dir_all(&canonical_parent).expect("canonical parent");
    let canonical_parent_resolved =
        fs::canonicalize(&canonical_parent).expect("canonical parent resolved");
    create_dir_symlink(&canonical_parent, &symlink_parent);
    let requested_workspace = symlink_parent.join("missing-workspace");
    let requested_workspace_text = requested_workspace.to_string_lossy().to_string();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "create a task for a symlinked path".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    let create_call = task_tool_call(vec![
        ("op", json!("create")),
        ("task_id", json!("task-symlink-diagnostic")),
        ("title", json!("Symlink diagnostic")),
        ("content", json!("Inspect the requested symlink path")),
        ("goal", json!("Return path diagnostic evidence")),
        ("deliverables", json!(["diagnostic"])),
        ("acceptance", json!(["diagnostic includes symlink parent"])),
        ("target_cwd", json!(requested_workspace_text)),
        ("dispatch", json!({"mode":"none"})),
    ]);

    let create_output = execute_task_tool(&runtime_home, &turn, &create_call).expect("create task");

    assert!(create_output.contains("target_cwd_path_diagnostic"));
    assert!(create_output.contains("exists=false"));
    assert!(create_output.contains(&format!("nearest_existing=`{}`", symlink_parent.display())));
    assert!(create_output.contains(&format!(
        "nearest_existing_canonical=`{}`",
        canonical_parent_resolved.display()
    )));
    assert!(create_output.contains("missing_suffix=`missing-workspace`"));
    assert!(create_output.contains(&format!(
        "`{}` -> `{}`",
        symlink_parent.display(),
        canonical_parent.display()
    )));

    fs::remove_dir_all(base).expect("cleanup base");
}

#[test]
fn task_tool_query_attaches_existing_task_to_current_visible_session() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut origin_history =
        SessionHistory::new(SessionId::new("origin-session"), Vec::new()).expect("history");
    let origin_turn = engine
        .start_turn(
            &mut origin_history,
            TurnStartInput {
                session_id: SessionId::new("origin-session"),
                turn_id: TurnId::new("runtime-turn-1"),
                trace_id: TraceId::new("runtime-trace-1"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("master"),
                user_text: "create".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("origin turn");
    execute_task_tool(
        &runtime_home,
        &origin_turn,
        &task_tool_call(vec![
            ("op", json!("create")),
            ("task_id", json!("task-attach-proof")),
            ("title", json!("Attachment proof")),
            ("content", json!("Keep original parent")),
            ("goal", json!("Expose in observing session")),
            ("deliverables", json!(["visible task projection"])),
            ("acceptance", json!(["original parent remains unchanged"])),
            ("dispatch", json!({"mode":"self"})),
        ]),
    )
    .expect("create task");

    let mut observer_history =
        SessionHistory::new(SessionId::new("observer-session"), Vec::new()).expect("history");
    let observer_turn = engine
        .start_turn(
            &mut observer_history,
            TurnStartInput {
                session_id: SessionId::new("observer-session"),
                turn_id: TurnId::new("runtime-turn-1"),
                trace_id: TraceId::new("observer-trace-1"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("master"),
                user_text: "inspect existing task".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("observer turn");
    execute_task_tool(
        &runtime_home,
        &observer_turn,
        &task_tool_call(vec![
            ("op", json!("query")),
            ("task_id", json!("task-attach-proof")),
        ]),
    )
    .expect("query and attach");

    let task = TaskRuntime::boot(&runtime_home, AgentId::new("master"))
        .expect("runtime")
        .query_task(&TaskId::new("task-attach-proof"))
        .expect("task");
    assert_eq!(
        task.parent.session_id.as_ref().map(SessionId::as_str),
        Some("origin-session")
    );
    assert_eq!(
        task.attached_session_ids
            .iter()
            .map(SessionId::as_str)
            .collect::<Vec<_>>(),
        vec!["observer-session"]
    );
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn timer_tool_schedules_independent_internal_wakeups() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-timer"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-timer"),
                turn_id: TurnId::new("turn-timer"),
                trace_id: TraceId::new("trace-timer"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-live"),
                user_text: "schedule a wakeup".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");

    let output = execute_timer_tool(
        &runtime_home,
        &turn,
        &timer_tool_call(vec![
            ("op", json!("schedule")),
            ("timer_id", json!("timer-relative-proof")),
            ("reason", json!("check delegated work")),
            ("prompt", json!("Read TaskBoard and continue.")),
            ("mode", json!("relative")),
            ("delay_seconds", json!(60)),
        ]),
    )
    .expect("schedule relative timer");
    assert!(output.contains("timer_id=timer-relative-proof"));

    let store = TimerStore::new(&runtime_home, &AgentId::new("agent-live"));
    let schedules = store.active_schedules().expect("active schedules");
    assert_eq!(schedules.len(), 1);
    assert_eq!(schedules[0].timer_id, "timer-relative-proof");
    assert_eq!(schedules[0].prompt, "Read TaskBoard and continue.");
    assert!(schedules[0].next_due_at >= now_unix_seconds());
    assert!(
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live"))
            .expect("task runtime")
            .list_tasks(Default::default())
            .expect("list tasks")
            .is_empty(),
        "timer tool must not create task truth"
    );
    let events = store.load_events().expect("timer events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "TimerScheduled");

    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn timer_tool_default_ids_do_not_overwrite_same_turn_schedules() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-timer"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-timer"),
                turn_id: TurnId::new("turn-timer"),
                trace_id: TraceId::new("trace-timer"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-live"),
                user_text: "schedule two wakeups".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");

    execute_timer_tool(
        &runtime_home,
        &turn,
        &timer_tool_call(vec![
            ("op", json!("schedule")),
            ("reason", json!("check first condition")),
            ("prompt", json!("Read TaskBoard for first condition.")),
            ("mode", json!("relative")),
            ("delay_seconds", json!(60)),
        ]),
    )
    .expect("schedule first timer");
    execute_timer_tool(
        &runtime_home,
        &turn,
        &timer_tool_call(vec![
            ("op", json!("schedule")),
            ("reason", json!("check second condition")),
            ("prompt", json!("Read TaskBoard for second condition.")),
            ("mode", json!("relative")),
            ("delay_seconds", json!(120)),
        ]),
    )
    .expect("schedule second timer");

    let schedules = TimerStore::new(&runtime_home, &AgentId::new("agent-live"))
        .active_schedules()
        .expect("active schedules");
    assert_eq!(schedules.len(), 2);
    assert_ne!(schedules[0].timer_id, schedules[1].timer_id);
    assert!(
        schedules
            .iter()
            .all(|schedule| schedule.timer_id.contains("session-timer-turn-timer"))
    );

    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn timer_tool_validates_recurring_absolute_shapes() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-timer"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-timer"),
                turn_id: TurnId::new("turn-timer"),
                trace_id: TraceId::new("trace-timer"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-live"),
                user_text: "schedule recurring wakeup".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");

    execute_timer_tool(
        &runtime_home,
        &turn,
        &timer_tool_call(vec![
            ("op", json!("schedule")),
            ("timer_id", json!("timer-weekly-proof")),
            ("reason", json!("weekly check")),
            ("prompt", json!("Run weekly status check.")),
            ("mode", json!("recurring")),
            (
                "repeat",
                json!({
                    "kind": "weekly",
                    "weekdays": [1, 3, 5],
                    "time_of_day_seconds_local": 3600,
                    "max_runs": 3
                }),
            ),
        ]),
    )
    .expect("schedule weekly timer");

    let err = execute_timer_tool(
        &runtime_home,
        &turn,
        &timer_tool_call(vec![
            ("op", json!("schedule")),
            ("timer_id", json!("timer-invalid-proof")),
            ("reason", json!("bad weekly check")),
            ("prompt", json!("Should reject.")),
            ("mode", json!("recurring")),
            (
                "repeat",
                json!({
                    "kind": "weekly",
                    "weekdays": [7],
                    "time_of_day_seconds_local": 3600
                }),
            ),
        ]),
    )
    .expect_err("invalid weekday must fail");
    assert!(err.contains("weekdays must be integers 0..6"));

    let schedules = TimerStore::new(&runtime_home, &AgentId::new("agent-live"))
        .active_schedules()
        .expect("active schedules");
    assert_eq!(schedules.len(), 1);
    assert_eq!(schedules[0].timer_id, "timer-weekly-proof");
    assert_eq!(schedules[0].max_runs, 3);

    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn timer_tool_accepts_local_time_cron_repeat() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-timer"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-timer"),
                turn_id: TurnId::new("turn-timer"),
                trace_id: TraceId::new("trace-timer"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-live"),
                user_text: "schedule cron wakeup".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");

    execute_timer_tool(
        &runtime_home,
        &turn,
        &timer_tool_call(vec![
            ("op", json!("schedule")),
            ("timer_id", json!("timer-cron-proof")),
            ("reason", json!("cron check")),
            ("prompt", json!("Run local cron status check.")),
            ("mode", json!("recurring")),
            (
                "repeat",
                json!({
                    "kind": "cron",
                    "expression": "*/15 9-17 * * 1-5",
                    "max_runs": 4
                }),
            ),
        ]),
    )
    .expect("schedule cron timer");

    let schedules = TimerStore::new(&runtime_home, &AgentId::new("agent-live"))
        .active_schedules()
        .expect("active schedules");
    assert_eq!(schedules.len(), 1);
    assert_eq!(schedules[0].timer_id, "timer-cron-proof");
    assert!(matches!(
        schedules[0].repeat,
        Some(TimerRepeatRule::Cron { ref expression, .. }) if expression == "*/15 9-17 * * 1-5"
    ));

    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn timer_tool_rejects_invalid_cron_repeat() {
    let err = parse_cron_expression("*/0 9 * * 1-5").expect_err("zero step must fail");
    assert!(err.to_string().contains("step must be greater than zero"));
    let err = parse_cron_expression("0 25 * * 1-5").expect_err("invalid hour must fail");
    assert!(err.to_string().contains("outside 0..23"));
    let parsed = parse_cron_expression("*/15 9-17 * * 1-5").expect("valid cron");
    assert!(parsed.minutes.contains(&0));
    assert!(parsed.minutes.contains(&45));
    assert!(parsed.hours.contains(&9));
    assert!(parsed.hours.contains(&17));
    assert!(parsed.weekdays.contains(&1));
    assert!(parsed.weekdays.contains(&5));
}

#[test]
fn local_time_daily_and_weekly_due_use_local_weekday() {
    let now = now_unix_seconds();
    let local_now = local_datetime(now).expect("local datetime");
    let next_local_hour = (local_now.hour() + 1) % 24;
    let local_second = next_local_hour * 3600;
    let next_daily = next_daily_due(now, local_second, false).expect("daily due");
    let due_local = local_datetime(next_daily).expect("daily local datetime");
    assert_eq!(due_local.hour(), next_local_hour);

    let weekday = due_local.weekday().num_days_from_sunday() as u8;
    let next_weekly =
        next_weekly_due(now, local_second, &[weekday]).expect("weekly due for local weekday");
    let weekly_local = local_datetime(next_weekly).expect("weekly local datetime");
    assert_eq!(weekly_local.weekday().num_days_from_sunday() as u8, weekday);
    assert_eq!(weekly_local.hour(), next_local_hour);
}

#[test]
fn task_tool_review_lifecycle_rejects_early_close_and_closes_after_approval() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "create a task".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("create")),
            ("task_id", json!("task-runtime-review")),
            ("title", json!("Review lifecycle")),
            ("content", json!("Exercise review lifecycle")),
            ("goal", json!("Close only after approval")),
            ("deliverables", json!(["code"])),
            ("acceptance", json!(["approval required"])),
            ("dispatch", json!({"mode":"self"})),
        ]),
    )
    .expect("create task");

    let early_close = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("close")),
            ("task_id", json!("task-runtime-review")),
        ]),
    )
    .expect_err("early close must fail");
    assert!(early_close.contains("invalid task transition"));

    for call in [
        task_tool_call(vec![
            ("op", json!("resume")),
            ("task_id", json!("task-runtime-review")),
        ]),
        task_tool_call(vec![
            ("op", json!("submit_review")),
            ("task_id", json!("task-runtime-review")),
            ("summary", json!("ready")),
            ("deliverables", json!(["code"])),
            ("evidence", json!(["tests passed"])),
        ]),
        task_tool_call(vec![
            ("op", json!("approve")),
            ("task_id", json!("task-runtime-review")),
        ]),
    ] {
        execute_task_tool(&runtime_home, &turn, &call).expect("lifecycle op");
    }
    let close = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("close")),
            ("task_id", json!("task-runtime-review")),
        ]),
    )
    .expect("close after approval");

    assert!(close.contains("status=Closed"));
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn task_tool_resume_and_heartbeat_persist_running_lease() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "run a task".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("create")),
            ("task_id", json!("task-runtime-heartbeat")),
            ("title", json!("Heartbeat lifecycle")),
            ("content", json!("Exercise task heartbeat")),
            ("goal", json!("Running task keeps a lease")),
            ("deliverables", json!(["lease"])),
            ("acceptance", json!(["heartbeat accepted"])),
            ("dispatch", json!({"mode":"self"})),
        ]),
    )
    .expect("create task");

    let resume = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("resume")),
            ("task_id", json!("task-runtime-heartbeat")),
        ]),
    )
    .expect("resume");
    assert!(resume.contains("status=Running"));

    let heartbeat = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("heartbeat")),
            ("task_id", json!("task-runtime-heartbeat")),
            ("ttl_seconds", json!(600)),
        ]),
    )
    .expect("heartbeat");
    assert!(heartbeat.contains("event=TaskHeartbeat"));

    let query = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("query")),
            ("task_id", json!("task-runtime-heartbeat")),
        ]),
    )
    .expect("query");
    assert!(query.contains("\"status\":\"running\""));
    assert!(
        runtime_home
            .join("state/task-runtime/agent-task/leases.json")
            .is_file()
    );
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn task_tool_agent_assign_cancel_close_lifecycle() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "assign a task".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    let create_agent = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("create_agent")),
            ("agent_id", json!("worker-runtime")),
            ("capabilities", json!(["code_edit"])),
        ]),
    )
    .expect("create agent");
    assert!(create_agent.contains("status=Available"));

    execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("create")),
            ("task_id", json!("task-runtime-assign")),
            ("title", json!("Assign lifecycle")),
            ("content", json!("Exercise assign and cancel")),
            ("goal", json!("Assigned task can be cancelled")),
            ("deliverables", json!(["task"])),
            ("acceptance", json!(["agent released"])),
            ("dispatch", json!({"mode":"none"})),
        ]),
    )
    .expect("create waiting task");

    let assigned = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("assign")),
            ("task_id", json!("task-runtime-assign")),
            ("agent_id", json!("worker-runtime")),
        ]),
    )
    .expect("assign");
    assert!(assigned.contains("status=Assigned"));

    let busy_close = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("close_agent")),
            ("agent_id", json!("worker-runtime")),
        ]),
    )
    .expect_err("busy worker cannot close");
    assert!(busy_close.contains("invalid agent transition"));

    let cancelled = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("cancel")),
            ("task_id", json!("task-runtime-assign")),
        ]),
    )
    .expect("cancel");
    assert!(cancelled.contains("status=Cancelled"));

    let closed = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("close_agent")),
            ("agent_id", json!("worker-runtime")),
        ]),
    )
    .expect("close idle worker");
    assert!(closed.contains("status=Closed"));
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn task_tool_claim_next_runs_highest_priority_task() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "claim highest priority task".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    for (task_id, priority) in [("task-low", 10), ("task-high", 90)] {
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("create")),
                ("task_id", json!(task_id)),
                ("title", json!(format!("Claim {task_id}"))),
                ("content", json!("Exercise priority claim")),
                ("goal", json!("Claim highest priority task")),
                ("deliverables", json!(["task"])),
                ("acceptance", json!(["highest priority claimed"])),
                ("priority", json!(priority)),
                ("dispatch", json!({"mode":"none"})),
            ]),
        )
        .expect("create task");
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("assign")),
                ("task_id", json!(task_id)),
                ("agent_id", json!("agent-task")),
            ]),
        )
        .expect("assign task");
    }

    let claimed = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("claim_next")),
            ("agent_id", json!("agent-task")),
            ("execution_id", json!("exec-task-high")),
            ("ttl_seconds", json!(600)),
        ]),
    )
    .expect("claim next");
    assert!(claimed.contains("task_id=task-high"));
    assert!(claimed.contains("status=Running"));
    assert!(claimed.contains("execution_id=exec-task-high"));

    let low = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![("op", json!("query")), ("task_id", json!("task-low"))]),
    )
    .expect("query low");
    assert!(low.contains("\"status\":\"assigned\""));
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn task_tool_record_execution_requires_running_task() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "record worker progress".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("create")),
            ("task_id", json!("task-runtime-execution")),
            ("title", json!("Execution progress")),
            ("content", json!("Record execution progress")),
            ("goal", json!("Progress enters task ledger")),
            ("deliverables", json!(["event"])),
            ("acceptance", json!(["running only"])),
            ("dispatch", json!({"mode":"self"})),
        ]),
    )
    .expect("create task");
    let rejected = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("record_execution")),
            ("task_id", json!("task-runtime-execution")),
            ("phase", json!("debug")),
            ("summary", json!("should fail before running")),
            ("evidence", json!(["assigned only"])),
        ]),
    )
    .expect_err("assigned task cannot record execution");
    assert!(rejected.contains("invalid task transition"));

    execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("resume")),
            ("task_id", json!("task-runtime-execution")),
        ]),
    )
    .expect("resume task");
    let recorded = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("record_execution")),
            ("task_id", json!("task-runtime-execution")),
            ("phase", json!("debug")),
            ("summary", json!("read function map")),
            (
                "evidence",
                json!(["docs/function-maps/task.orchestration.md"]),
            ),
        ]),
    )
    .expect("record execution");
    assert!(recorded.contains("status=Running"));
    assert!(recorded.contains("event=TaskExecutionRecorded"));
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn task_tool_structured_execution_status_requires_execution_identity() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "record structured worker state".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("create")),
            ("task_id", json!("task-runtime-structured-status")),
            ("title", json!("Structured execution status")),
            ("content", json!("Reject missing execution identity")),
            ("goal", json!("No implicit execution id fallback")),
            ("deliverables", json!(["explicit error"])),
            ("acceptance", json!(["task remains running"])),
            ("dispatch", json!({"mode":"self"})),
        ]),
    )
    .expect("create task");
    execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("resume")),
            ("task_id", json!("task-runtime-structured-status")),
        ]),
    )
    .expect("resume task");

    let err = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("record_execution")),
            ("task_id", json!("task-runtime-structured-status")),
            ("agent_id", json!("agent-task")),
            ("status", json!("blocked")),
            ("phase", json!("execution_error")),
            (
                "summary",
                json!("worker failed but execution id is missing"),
            ),
            ("evidence", json!(["missing execution id"])),
        ]),
    )
    .expect_err("structured execution status requires execution id");
    assert!(err.contains("`execution_id` is required"));

    let query = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("query")),
            ("task_id", json!("task-runtime-structured-status")),
        ]),
    )
    .expect("query");
    assert!(query.contains("\"status\":\"running\""));
    let history_output = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("history")),
            ("task_id", json!("task-runtime-structured-status")),
        ]),
    )
    .expect("history");
    assert!(!history_output.contains("TaskBlocked"));
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn task_tool_history_returns_ordered_execution_timeline() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "query task timeline".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("create")),
            ("task_id", json!("task-runtime-history")),
            ("title", json!("History")),
            ("content", json!("Query task history")),
            ("goal", json!("Timeline is queryable")),
            ("deliverables", json!(["history"])),
            ("acceptance", json!(["ordered events"])),
            ("dispatch", json!({"mode":"self"})),
        ]),
    )
    .expect("create task");
    execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("resume")),
            ("task_id", json!("task-runtime-history")),
        ]),
    )
    .expect("resume task");
    execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("record_execution")),
            ("task_id", json!("task-runtime-history")),
            ("phase", json!("debug")),
            ("summary", json!("inspect timeline")),
            ("evidence", json!(["ledger query"])),
        ]),
    )
    .expect("record execution");

    let timeline = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("history")),
            ("task_id", json!("task-runtime-history")),
        ]),
    )
    .expect("history");

    assert!(timeline.contains("\"event_type\":\"TaskCreated\""));
    assert!(timeline.contains("\"event_type\":\"TaskExecutionRecorded\""));
    assert!(timeline.contains("\"seq\":1"));
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn task_tool_list_tasks_filters_queue_projection() {
    let runtime_home = temp_runtime_home();
    let engine = ReasonTurnEngine::new();
    let mut history =
        SessionHistory::new(SessionId::new("session-task"), Vec::new()).expect("history");
    let turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: SessionId::new("session-task"),
                turn_id: TurnId::new("turn-task"),
                trace_id: TraceId::new("trace-task"),
                feature_id: FeatureId::new("provider.reason-live-bridge"),
                agent_id: AgentId::new("agent-task"),
                user_text: "list assigned tasks".to_owned(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: "model".to_owned(),
            },
        )
        .expect("turn");
    for (task_id, priority) in [("task-list-low", 10), ("task-list-high", 90)] {
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("create")),
                ("task_id", json!(task_id)),
                ("title", json!(format!("List {task_id}"))),
                ("content", json!("List task queue")),
                ("goal", json!("Filter by assigned state")),
                ("deliverables", json!(["list"])),
                ("acceptance", json!(["filtered"])),
                ("priority", json!(priority)),
                ("dispatch", json!({"mode":"none"})),
            ]),
        )
        .expect("create task");
        execute_task_tool(
            &runtime_home,
            &turn,
            &task_tool_call(vec![
                ("op", json!("assign")),
                ("task_id", json!(task_id)),
                ("agent_id", json!("agent-task")),
            ]),
        )
        .expect("assign task");
    }

    let tasks = execute_task_tool(
        &runtime_home,
        &turn,
        &task_tool_call(vec![
            ("op", json!("list_tasks")),
            ("status", json!("assigned")),
            ("agent_id", json!("agent-task")),
        ]),
    )
    .expect("list tasks");

    let high_pos = tasks.find("\"task_id\":\"task-list-high\"").expect("high");
    let low_pos = tasks.find("\"task_id\":\"task-list-low\"").expect("low");
    assert!(high_pos < low_pos);
    assert!(tasks.contains("\"status\":\"assigned\""));
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn live_bridge_stamps_tool_schema_fingerprint_into_planner_diagnostics() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, _rx, handle) =
        spawn_mock_server(200, "application/json", complete_single_response("pong"));

    let request = live_request(false);

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("live bridge");
    handle.join().expect("join");

    let registry = BuiltinToolRegistry::reasonix_aligned();
    let expected = fnv1a_hex_for_test(&registry.master_implemented_schema_fingerprint());
    let empty = fnv1a_hex_for_test("");

    assert_eq!(
        outcome.turn.planned_context.diagnostics.tool_schema_hash,
        expected
    );
    assert_ne!(
        outcome.turn.planned_context.diagnostics.tool_schema_hash,
        empty
    );
}

#[test]
fn live_bridge_admits_long_operator_task_without_semantic_truncation() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_mock_server(
        200,
        "application/json",
        complete_single_response("accepted"),
    );
    let mut request = live_request(false);
    request.prompt = format!(
            "{}\nSENTINEL_MASTER_AUTONOMY_LONG_PROMPT_END",
            (0..80)
                .map(|index| format!(
                    "step-{index}: master must create a worker task, dispatch it, inspect worker status, handle rejection, retry, approve, and close without losing this instruction."
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("long operator task must reach provider request");
    let raw_request = rx.recv().expect("request");
    handle.join().expect("join");

    assert!(raw_request.contains("step-79"));
    assert!(raw_request.contains("SENTINEL_MASTER_AUTONOMY_LONG_PROMPT_END"));
    assert_master_task_request_contract(
        &raw_request,
        "SENTINEL_MASTER_AUTONOMY_LONG_PROMPT_END",
        "agent-live-worker",
    );
    let original_task = outcome
        .turn
        .planned_context
        .ordered_segments
        .iter()
        .find(|segment| segment.segment_id.as_str() == "original-task")
        .expect("original task segment");
    assert_eq!(original_task.kind, ContextSegmentKind::TaskContract);
    let original_task_cost = outcome
        .turn
        .planned_context
        .diagnostics
        .segment_token_costs
        .iter()
        .find(|cost| cost.segment_id.as_str() == "original-task")
        .expect("original task token cost");
    assert!(original_task.token_budget >= original_task_cost.estimated_tokens);
    assert!(original_task.token_budget > 128);
}

#[test]
fn live_bridge_admits_instruction_capability_manifest_as_typed_context() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(runtime_home.join("skills/global-skill")).expect("global skill dir");
    fs::write(
        runtime_home.join("AGENTS.md"),
        "Global instruction sentinel FH-INSTRUCTION-GLOBAL",
    )
    .expect("write global agents");
    fs::write(
        runtime_home.join("skills/global-skill/SKILL.md"),
        "---\nname: global-skill\ndescription: Global instruction skill sentinel\n---\n# Skill\n",
    )
    .expect("write global skill");
    let workspace = runtime_home.join("workspace");
    fs::create_dir_all(workspace.join(".agents/skills/local-skill")).expect("workspace dirs");
    fs::write(
        workspace.join("Cargo.toml"),
        "[package]\nname=\"instruction-fixture\"\nversion=\"0.0.0\"\nedition=\"2021\"\n",
    )
    .expect("write marker");
    fs::write(
        workspace.join("AGENTS.md"),
        "Local instruction sentinel FH-INSTRUCTION-LOCAL",
    )
    .expect("write local agents");
    fs::write(
            workspace.join(".agents/skills/local-skill/SKILL.md"),
            "---\nname: local-skill\ndescription: Local instruction skill sentinel\n---\n# Local Skill\n",
        )
        .expect("write local skill");

    let (base_url, rx, handle) = spawn_mock_server(
        200,
        "application/json",
        complete_single_response("accepted"),
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home;
    request.cwd = Some(workspace);

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("instruction capability context reaches provider request");
    let raw_request = rx.recv().expect("request");
    handle.join().expect("join");

    assert!(raw_request.contains("kind=\\\"instruction_capability\\\""));
    assert!(raw_request.contains("<freehand_instruction_capability>"));
    assert!(raw_request.contains("Global instruction skill sentinel"));
    assert!(raw_request.contains("Local instruction skill sentinel"));
    assert!(raw_request.contains("FH-INSTRUCTION-GLOBAL"));
    assert!(raw_request.contains("FH-INSTRUCTION-LOCAL"));
    let instruction_segment = outcome
        .turn
        .planned_context
        .ordered_segments
        .iter()
        .find(|segment| segment.segment_id.as_str() == "instruction-capability")
        .expect("instruction capability segment");
    assert_eq!(
        instruction_segment.kind,
        ContextSegmentKind::InstructionCapability
    );
    assert_eq!(
        instruction_segment.provenance.source,
        "instruction_capability"
    );
}

#[test]
fn live_bridge_admits_long_previous_visible_output_without_fixed_cap() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let visible_text = format!(
            "{}\nSENTINEL_PREVIOUS_VISIBLE_OUTPUT_LONG_END",
            (0..180)
                .map(|index| format!(
                    "round-one-visible-{index}: keep this model-visible repair context for the next round without a short fixed cap."
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            continue_with_visible_response(&visible_text, "finish after carrying prior output"),
            complete_single_response("final after long visible output"),
        ],
    );
    let request = live_request(false);

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("long prior visible output must reach next provider request");
    let _first_request = rx.recv().expect("first request");
    let second_request = rx.recv().expect("second request");
    handle.join().expect("join");

    assert_eq!(outcome.rounds, 2);
    assert!(second_request.contains("SENTINEL_PREVIOUS_VISIBLE_OUTPUT_LONG_END"));
    assert!(second_request.contains("<<<freehand_status>>>"));
    assert!(second_request.contains("Master task orchestration examples"));
    let previous_output = outcome
        .turn
        .planned_context
        .ordered_segments
        .iter()
        .find(|segment| segment.segment_id.as_str() == "previous-visible-output")
        .expect("previous visible output segment");
    let previous_output_cost = outcome
        .turn
        .planned_context
        .diagnostics
        .segment_token_costs
        .iter()
        .find(|cost| cost.segment_id.as_str() == "previous-visible-output")
        .expect("previous visible output token cost");
    assert!(previous_output.token_budget >= previous_output_cost.estimated_tokens);
    assert!(previous_output.token_budget > 512);
}

#[test]
fn live_bridge_master_autonomy_success_dispatches_worker_and_closes_task() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let sentinel = "SENTINEL_MASTER_AUTONOMY_SUCCESS_END";
    let task_id = "task-master-autonomy-success";
    let worker_id = "worker-master-autonomy-success";
    let execution_id = "exec-master-autonomy-success";
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            task_tool_use_response(
                "toolu_success_agent",
                json!({
                    "op":"create_agent",
                    "agent_id":worker_id,
                    "capabilities":["code_edit","test_run"]
                }),
            ),
            task_tool_use_response(
                "toolu_success_create",
                json!({
                    "op":"create",
                    "task_id":task_id,
                    "title":"Autonomy success task",
                    "content":"Worker should complete the delegated task successfully.",
                    "goal":"Prove master can dispatch and close a successful worker task.",
                    "deliverables":["success report"],
                    "acceptance":["task closes after approval"],
                    "dispatch":{"mode":"none"},
                    "priority":90
                }),
            ),
            task_tool_use_response(
                "toolu_success_assign",
                json!({
                    "op":"assign",
                    "task_id":task_id,
                    "agent_id":worker_id
                }),
            ),
            task_tool_use_response(
                "toolu_success_claim",
                json!({
                    "op":"claim_next",
                    "agent_id":worker_id,
                    "execution_id":execution_id,
                    "ttl_seconds":600
                }),
            ),
            task_tool_use_response(
                "toolu_success_running",
                json!({
                    "op":"record_execution",
                    "status":"running",
                    "task_id":task_id,
                    "agent_id":worker_id,
                    "execution_id":execution_id,
                    "phase":"implementation",
                    "summary":"worker implemented the requested change",
                    "evidence":["changed files inspected"]
                }),
            ),
            task_tool_use_response(
                "toolu_success_review_ready",
                json!({
                    "op":"record_execution",
                    "status":"review_ready",
                    "task_id":task_id,
                    "agent_id":worker_id,
                    "execution_id":execution_id,
                    "phase":"review",
                    "summary":"worker completed all acceptance checks",
                    "deliverables":["success report"],
                    "evidence":["unit test passed","owner truth updated"]
                }),
            ),
            task_tool_use_response(
                "toolu_success_approve",
                json!({
                    "op":"approve",
                    "task_id":task_id
                }),
            ),
            task_tool_use_response(
                "toolu_success_close",
                json!({
                    "op":"close",
                    "task_id":task_id
                }),
            ),
            complete_single_response("master closed successful worker task"),
        ],
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.prompt = master_autonomy_prompt(sentinel);

    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    set_single_worker_peer(&mut selected, worker_id);
    let outcome = run_live_reason_turn(&selected, request).expect("master autonomy success path");
    let requests = collect_provider_requests(&rx, 9);
    handle.join().expect("join provider");

    assert_master_task_request_contract(&requests[0], sentinel, worker_id);
    assert!(requests[1].contains("Agent created"));
    assert!(requests.iter().any(|request| {
        request.contains("\"tool_use_id\":\"toolu_success_review_ready\"")
            && request.contains("Task review submitted")
    }));
    assert!(requests.iter().any(|request| {
        request.contains("\"tool_use_id\":\"toolu_success_close\"")
            && request.contains("Task closed")
    }));
    assert_eq!(outcome.tool_executions, 8);
    assert_eq!(outcome.rounds, 9);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );

    let (task, event_types) = task_truth(&runtime_home, task_id);
    assert_eq!(task.status, TaskStatus::Closed);
    for required in [
        "TaskCreated",
        "TaskAssigned",
        "TaskResumed",
        "TaskExecutionRecorded",
        "TaskReviewSubmitted",
        "TaskReviewApproved",
        "TaskClosed",
    ] {
        assert!(
            event_types.iter().any(|event| event == required),
            "missing {required}: {event_types:?}"
        );
    }
    assert!(
        !event_types
            .iter()
            .any(|event| event == "TaskReviewRejected")
    );
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_master_autonomy_execution_error_blocks_without_success_close() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let sentinel = "SENTINEL_MASTER_AUTONOMY_EXECUTION_ERROR_END";
    let task_id = "task-master-autonomy-error";
    let worker_id = "worker-master-autonomy-error";
    let execution_id = "exec-master-autonomy-error";
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            task_tool_use_response(
                "toolu_error_agent",
                json!({
                    "op":"create_agent",
                    "agent_id":worker_id,
                    "capabilities":["code_edit"]
                }),
            ),
            task_tool_use_response(
                "toolu_error_create",
                json!({
                    "op":"create",
                    "task_id":task_id,
                    "title":"Autonomy execution error task",
                    "content":"Worker should report an execution error.",
                    "goal":"Prove master keeps errored worker task blocked instead of closing it.",
                    "deliverables":["error report"],
                    "acceptance":["blocked state is visible"],
                    "dispatch":{"mode":"none"},
                    "priority":80
                }),
            ),
            task_tool_use_response(
                "toolu_error_assign",
                json!({
                    "op":"assign",
                    "task_id":task_id,
                    "agent_id":worker_id
                }),
            ),
            task_tool_use_response(
                "toolu_error_claim",
                json!({
                    "op":"claim_next",
                    "agent_id":worker_id,
                    "execution_id":execution_id,
                    "ttl_seconds":600
                }),
            ),
            task_tool_use_response(
                "toolu_error_running",
                json!({
                    "op":"record_execution",
                    "status":"running",
                    "task_id":task_id,
                    "agent_id":worker_id,
                    "execution_id":execution_id,
                    "phase":"implementation",
                    "summary":"worker started execution",
                    "evidence":["worker heartbeat observed"]
                }),
            ),
            task_tool_use_response(
                "toolu_error_blocked",
                json!({
                    "op":"record_execution",
                    "status":"blocked",
                    "task_id":task_id,
                    "agent_id":worker_id,
                    "execution_id":execution_id,
                    "phase":"execution_error",
                    "summary":"worker hit provider_error_500 and cannot continue without master decision",
                    "evidence":["provider_error_500","no deliverable produced"]
                }),
            ),
            blocked_single_response(
                "master left errored worker task blocked",
                "worker task is blocked after execution_error and needs Master/user decision",
            ),
        ],
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.prompt = master_autonomy_prompt(sentinel);

    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    set_single_worker_peer(&mut selected, worker_id);
    let outcome =
        run_live_reason_turn(&selected, request).expect("master autonomy execution error path");
    let requests = collect_provider_requests(&rx, 7);
    handle.join().expect("join provider");

    assert_master_task_request_contract(&requests[0], sentinel, worker_id);
    assert!(requests.iter().any(|request| {
        request.contains("\"tool_use_id\":\"toolu_error_blocked\"")
            && request.contains("TaskBlocked")
            && request.contains("status=Blocked")
    }));
    assert_eq!(outcome.tool_executions, 6);
    assert_eq!(outcome.rounds, 7);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Blocked)
    );

    let (task, event_types) = task_truth(&runtime_home, task_id);
    assert_eq!(task.status, TaskStatus::Blocked);
    assert!(event_types.iter().any(|event| event == "TaskBlocked"));
    assert!(
        !event_types
            .iter()
            .any(|event| event == "TaskReviewSubmitted")
    );
    assert!(
        !event_types
            .iter()
            .any(|event| event == "TaskReviewApproved")
    );
    assert!(!event_types.iter().any(|event| event == "TaskClosed"));
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_master_autonomy_rejected_review_retries_and_closes() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let sentinel = "SENTINEL_MASTER_AUTONOMY_REJECT_RETRY_END";
    let task_id = "task-master-autonomy-retry";
    let worker_id = "worker-master-autonomy-retry";
    let execution_id = "exec-master-autonomy-retry";
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            task_tool_use_response(
                "toolu_retry_agent",
                json!({
                    "op":"create_agent",
                    "agent_id":worker_id,
                    "capabilities":["code_edit","test_run"]
                }),
            ),
            task_tool_use_response(
                "toolu_retry_create",
                json!({
                    "op":"create",
                    "task_id":task_id,
                    "title":"Autonomy retry task",
                    "content":"Worker first submits incomplete work, then fixes it.",
                    "goal":"Prove master rejects incomplete worker submission and closes only after retry.",
                    "deliverables":["complete report"],
                    "acceptance":["review rejection precedes retry close"],
                    "dispatch":{"mode":"none"},
                    "priority":85
                }),
            ),
            task_tool_use_response(
                "toolu_retry_assign",
                json!({
                    "op":"assign",
                    "task_id":task_id,
                    "agent_id":worker_id
                }),
            ),
            task_tool_use_response(
                "toolu_retry_claim",
                json!({
                    "op":"claim_next",
                    "agent_id":worker_id,
                    "execution_id":execution_id,
                    "ttl_seconds":600
                }),
            ),
            task_tool_use_response(
                "toolu_retry_incomplete_review",
                json!({
                    "op":"record_execution",
                    "status":"review_ready",
                    "task_id":task_id,
                    "agent_id":worker_id,
                    "execution_id":execution_id,
                    "phase":"review",
                    "summary":"worker submitted partial implementation without regression proof",
                    "deliverables":["partial report"],
                    "evidence":["no regression evidence"]
                }),
            ),
            task_tool_use_response(
                "toolu_retry_reject",
                json!({
                    "op":"reject",
                    "task_id":task_id,
                    "reject_reason":"missing regression proof",
                    "next_requirements":["run regression evidence","resubmit complete deliverable"]
                }),
            ),
            task_tool_use_response(
                "toolu_retry_recovering",
                json!({
                    "op":"record_execution",
                    "status":"recovering",
                    "task_id":task_id,
                    "agent_id":worker_id,
                    "execution_id":execution_id,
                    "phase":"retry",
                    "summary":"worker is fixing rejected submission",
                    "evidence":["rejection reason acknowledged"],
                    "retry_count":1
                }),
            ),
            task_tool_use_response(
                "toolu_retry_complete_review",
                json!({
                    "op":"record_execution",
                    "status":"review_ready",
                    "task_id":task_id,
                    "agent_id":worker_id,
                    "execution_id":execution_id,
                    "phase":"review",
                    "summary":"worker resubmitted complete implementation with regression proof",
                    "deliverables":["complete report"],
                    "evidence":["regression passed","missing proof supplied"]
                }),
            ),
            task_tool_use_response(
                "toolu_retry_approve",
                json!({
                    "op":"approve",
                    "task_id":task_id
                }),
            ),
            task_tool_use_response(
                "toolu_retry_close",
                json!({
                    "op":"close",
                    "task_id":task_id
                }),
            ),
            complete_single_response("master closed retried worker task"),
        ],
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.prompt = master_autonomy_prompt(sentinel);

    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    set_single_worker_peer(&mut selected, worker_id);
    let outcome = run_live_reason_turn(&selected, request)
        .expect("master autonomy rejected-review retry path");
    let requests = collect_provider_requests(&rx, 11);
    handle.join().expect("join provider");

    assert_master_task_request_contract(&requests[0], sentinel, worker_id);
    assert!(requests.iter().any(|request| {
        request.contains("\"tool_use_id\":\"toolu_retry_reject\"")
            && request.contains("Task rejected")
    }));
    assert!(requests.iter().any(|request| {
        request.contains("\"tool_use_id\":\"toolu_retry_recovering\"")
            && request.contains("TaskExecutionRecovering")
    }));
    assert!(requests.iter().any(|request| {
        request.contains("\"tool_use_id\":\"toolu_retry_close\"") && request.contains("Task closed")
    }));
    assert_eq!(outcome.tool_executions, 10);
    assert_eq!(outcome.rounds, 11);

    let (task, event_types) = task_truth(&runtime_home, task_id);
    assert_eq!(task.status, TaskStatus::Closed);
    let first_review = event_index(&event_types, "TaskReviewSubmitted");
    let rejected = event_index(&event_types, "TaskReviewRejected");
    let recovering = event_index(&event_types, "TaskExecutionRecovering");
    let second_review = event_types
        .iter()
        .enumerate()
        .skip(rejected.saturating_add(1))
        .find(|(_, event)| event.as_str() == "TaskReviewSubmitted")
        .map(|(index, _)| index)
        .expect("second review submission after rejection");
    let approved = event_index(&event_types, "TaskReviewApproved");
    let closed = event_index(&event_types, "TaskClosed");
    assert!(first_review < rejected);
    assert!(rejected < recovering);
    assert!(recovering < second_review);
    assert!(second_review < approved);
    assert!(approved < closed);
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_runs_streaming_anthropic_provider_into_broadcasts() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) =
        spawn_mock_server(200, "text/event-stream", complete_stream_response("pong"));
    let request = live_request(true);
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("live bridge");
    let raw_request = rx.recv().expect("request");
    handle.join().expect("join");

    assert!(raw_request.contains("\"stream\":true"));
    assert_eq!(outcome.rounds, 1);
    let text = strip_completion_submission_block(&collect_turn_text(&outcome.turn));
    assert_eq!(text.trim(), "pong");
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|e| e.status.clone()),
        Some(TerminalStatus::Success)
    );
    let provider_raw = provider_raw_ledger_rows(
        &runtime_home,
        "anthropic",
        "agent-live",
        &session_id,
        "turn-live",
    );
    assert!(!provider_raw.is_empty());
    assert!(
        provider_raw
            .iter()
            .all(|row| row.raw_kind == "stream_event_body")
    );
    assert!(
        provider_raw
            .iter()
            .any(|row| row.body.contains("\"type\":\"message_stop\""))
    );
    assert!(outcome.broadcasts.iter().any(
            |event| matches!(event, ReasonBroadcastEvent::Semantic(event) if event.kind == SemanticEventKind::Reasoning)
        ));
}

#[test]
fn live_bridge_applies_stream_outputs_before_provider_finishes() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tagged = tagged_completion_json(
        r#"{"claim":"complete","completion_reason":"done","evidence":"provider returned pong","summary":"pong","learned":"keep tagged completion strict"}"#,
    );
    let first_chunk = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"pong\"}}\n\n"
        )
        .to_owned();
    let streamed_text = format!("pong\\n{tagged}")
        .replace('\n', "\\n")
        .replace('"', "\\\"");
    let remaining_chunks = format!(
        "event: content_block_start\n\
data: {{\"type\":\"content_block_start\",\"index\":1,\"content_block\":{{\"type\":\"text\",\"text\":\"\"}}}}\n\n\
event: content_block_delta\n\
data: {{\"type\":\"content_block_delta\",\"index\":1,\"delta\":{{\"type\":\"text_delta\",\"text\":\"{streamed_text}\"}}}}\n\n\
event: content_block_stop\n\
data: {{\"type\":\"content_block_stop\",\"index\":1}}\n\n\
event: message_delta\n\
data: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"end_turn\"}},\"usage\":{{\"input_tokens\":14,\"output_tokens\":82}}}}\n\n\
event: message_stop\n\
data: {{\"type\":\"message_stop\"}}\n\n"
    );
    let (base_url, rx, released_rx, continue_tx, handle) =
        spawn_incremental_stream_server(first_chunk, remaining_chunks);

    let mut seen_reasoning_before_release = false;
    let outcome = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        live_request(true),
        |event| {
            if matches!(
                event,
                ReasonBroadcastEvent::Semantic(semantic)
                    if semantic.kind == SemanticEventKind::Reasoning
            ) {
                seen_reasoning_before_release = true;
                let _ = continue_tx.send(());
            }
        },
        |_| {},
        |_| {},
    )
    .expect("live bridge");
    let raw_request = rx.recv().expect("request");
    let released = released_rx.recv().expect("release");
    handle.join().expect("join");

    assert!(raw_request.contains("\"stream\":true"));
    assert!(
        released,
        "bridge did not apply reasoning output before stream end"
    );
    assert!(seen_reasoning_before_release);
    assert_eq!(
        strip_completion_submission_block(&collect_turn_text(&outcome.turn)),
        "pong"
    );
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|e| e.status.clone()),
        Some(TerminalStatus::Success)
    );
}

#[test]
fn live_bridge_cancel_token_stops_before_tool_execution() {
    let cancel_token = Arc::new(AtomicBool::new(true));
    let mut request = live_request(false);
    request.cancel_token = Some(cancel_token);

    let err = run_live_reason_turn(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        request,
    )
    .expect_err("cancelled live bridge");

    assert_eq!(err, RuntimeLiveBridgeError::Cancelled);
}

#[test]
fn live_bridge_cancel_token_stops_after_provider_output_before_tool_execution() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cancel_token = Arc::new(AtomicBool::new(false));
    let mut request = live_request(false);
    request.cancel_token = Some(Arc::clone(&cancel_token));
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();
    let (base_url, _rx, handle) =
        spawn_mock_server(200, "application/json", tool_use_single_response());

    let err = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
        |event| {
            if matches!(event, ReasonBroadcastEvent::Tool(_)) {
                cancel_token.store(true, Ordering::SeqCst);
            }
        },
        |_| {},
        |_| {},
    )
    .expect_err("cancelled before tool execution");
    handle.join().expect("join");

    assert_eq!(err, RuntimeLiveBridgeError::Cancelled);

    let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
        .restore(&session_id)
        .expect("restore live session");
    assert!(
        restored
            .closed_turns
            .iter()
            .all(|turn| turn.terminal_event.is_none()),
        "tool-call cancellation should not materialize terminal truth"
    );
    let latest = restored
        .active_turn
        .as_ref()
        .expect("active turn should remain");
    assert!(latest.turn.tool_results.is_empty());
    assert!(latest.turn.terminal_event.is_none());
}

#[test]
fn live_bridge_cancel_token_stops_before_terminal_persistence() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cancel_token = Arc::new(AtomicBool::new(false));
    let mut request = live_request(false);
    request.cancel_token = Some(Arc::clone(&cancel_token));
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();
    let (base_url, _rx, handle) =
        spawn_mock_server(200, "application/json", complete_single_response("pong"));

    let err = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
        |event| {
            if matches!(event, ReasonBroadcastEvent::Terminal(_)) {
                cancel_token.store(true, Ordering::SeqCst);
            }
        },
        |_| {},
        |_| {},
    )
    .expect_err("cancelled before terminal persistence");
    handle.join().expect("join");

    assert_eq!(err, RuntimeLiveBridgeError::Cancelled);

    let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
        .restore(&session_id)
        .expect("restore live session");
    assert!(
        restored.closed_turns.is_empty(),
        "terminal cancellation should not materialize closed-turn truth"
    );
    let latest = restored
        .active_turn
        .as_ref()
        .expect("active turn should remain");
    assert!(
        latest.turn.terminal_event.is_none(),
        "terminal cancellation should not persist terminal truth into the active snapshot"
    );
}

#[test]
fn live_bridge_records_error_center_metadata_for_schema_repair() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            invalid_complete_response(),
            complete_single_response("pong"),
        ],
    );

    let request = live_request(false);
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("live bridge");
    let first_request = rx.recv().expect("first request");
    let second_request = rx.recv().expect("second request");
    handle.join().expect("join");

    assert!(first_request.contains("reply exactly pong"));
    assert!(second_request.contains("Fix these schema entries"));
    assert!(second_request.contains("`completion_reason`: is required"));
    assert!(second_request.contains("`evidence`: is required"));
    assert!(second_request.contains("`learned`: is required"));
    assert!(second_request.contains("Use plain string values for required text fields"));
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.schema_rejections.len(), 1);
    assert!(outcome.broadcasts.iter().any(|event| {
        matches!(
            event,
            ReasonBroadcastEvent::CompletionSchemaRejected(rejection)
                if rejection.feedback.contains("`evidence`: is required")
                    && rejection.feedback.contains("`completion_reason`: is required")
        )
    }));
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
    let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "error.center"
            && record.write_node.pipeline_node == "ReasonResp04CompletionSchemaRejected"
            && record.entries.iter().any(|entry| {
                entry.key == "error.recovery_action" && entry.value == json!("repair_schema")
            })
            && record
                .entries
                .iter()
                .any(|entry| entry.key == "error.domain" && entry.value == json!("schema"))
    }));
    assert!(!metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "error.center"
            && record
                .entries
                .iter()
                .any(|entry| entry.key == "error.domain" && entry.value == json!("provider"))
    }));
    assert!(!metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "error.center"
            && record.entries.iter().any(|entry| {
                entry.key == "error.recovery_action" && entry.value == json!("fail_turn")
            })
    }));
}

#[test]
fn live_bridge_retries_missing_completion_schema_then_completes() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            missing_completion_schema_response(),
            complete_single_response("pong"),
        ],
    );

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        live_request(false),
    )
    .expect("live bridge");
    let _first_request = rx.recv().expect("first request");
    let second_request = rx.recv().expect("second request");
    handle.join().expect("join");

    assert!(second_request.contains("Fix these schema entries"));
    assert!(second_request.contains("`freehand_completion`: missing"));
    assert!(second_request.contains("<freehand_completion>"));
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.schema_rejections.len(), 1);
    assert!(outcome.broadcasts.iter().any(|event| {
        matches!(
            event,
            ReasonBroadcastEvent::CompletionSchemaRejected(rejection)
                if rejection.feedback.contains("`freehand_completion`: missing")
        )
    }));
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
}

#[test]
fn live_bridge_uses_continue_next_step_for_next_round() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            continue_single_response("open the file and confirm pong"),
            complete_single_response("pong"),
        ],
    );

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        live_request(false),
    )
    .expect("live bridge");
    let _first_request = rx.recv().expect("first request");
    let second_request = rx.recv().expect("second request");
    handle.join().expect("join");

    assert!(second_request.contains("open the file and confirm pong"));
    assert_eq!(outcome.rounds, 2);
    assert!(outcome.schema_rejections.is_empty());
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
}

#[test]
fn live_bridge_executes_real_registry_tool_reenters_result_and_persists_terminal_turn() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_single_response(),
            complete_single_response("tool done"),
        ],
    );
    let mut request = live_request(false);
    request.cwd = Some(std::env::current_dir().expect("current repo cwd"));
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();
    let mut debug_events = Vec::<DebugEvent>::new();
    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    selected.mode = AgentMode::Slave;
    set_single_master_peer(&mut selected, "master-live");

    let outcome = run_live_reason_turn_with_policy(
        &selected,
        request,
        LiveReasonExecutionRole::Worker,
        None,
        |_| {},
        |event| debug_events.push(event.clone()),
        |_| {},
    )
    .expect("live bridge");
    let first_request = rx.recv().expect("first request");
    let second_request = rx.recv().expect("second request");
    handle.join().expect("join");

    assert!(first_request.contains("\"tools\""));
    assert!(first_request.contains("\"name\":\"read_file\""));
    assert!(!first_request.contains("\"tool_choice\""));
    assert!(second_request.contains("\"type\":\"tool_result\""));
    assert!(second_request.contains("toolu_read_1"));
    assert!(second_request.contains("Cargo.toml"));
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.tool_executions, 1);
    assert_eq!(outcome.restore_status, LiveReasonRestoreStatus::CreatedNew);
    assert!(
        outcome
            .turns
            .iter()
            .any(|turn| !turn.tool_results.is_empty())
    );
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );

    let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
        .restore(&session_id)
        .expect("restore persisted live session");
    assert_eq!(
        restored
            .closed_turns
            .last()
            .and_then(|turn| turn.terminal_event.as_ref())
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
    assert!(restored.cursor.last_applied_reason_seq >= 4);

    let metadata = metadata_ledger_records(&runtime_home, "agent-live", &session_id);
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "provider.reason-live-bridge"
            && record.write_node.pipeline_node == "RuntimeLive03ToolExecuted"
            && record
                .entries
                .iter()
                .any(|entry| entry.key == "tool.name" && entry.value == json!("read_file"))
    }));
    let tool_debug = runtime_debug_events(&debug_events, "RuntimeLive03ToolExecuted");
    assert_eq!(tool_debug.len(), 1);
    let tool_snapshot = tool_debug[0].snapshot.as_ref().expect("tool snapshot");
    assert!(
        tool_snapshot
            .detail_lines
            .iter()
            .any(|line| line == "tool_name=read_file")
    );
    assert!(
        tool_snapshot
            .detail_lines
            .iter()
            .any(|line| line == "tool_call_id=toolu_read_1")
    );
}

#[test]
fn live_bridge_sends_image_payload_once_and_persists_metadata_only() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_single_response(),
            complete_single_response("image inspected after one tool round"),
        ],
    );
    let mut request = live_request(false);
    request.cwd = Some(std::env::current_dir().expect("current repo cwd"));
    request.prompt = "Describe the attached image without copying metadata into text".to_owned();
    request.attachments = vec![ProviderInputAttachment {
        attachment_id: "att-image-1".to_owned(),
        kind: ProviderInputAttachmentKind::Image,
        media_type: "image/png".to_owned(),
        name: "screen.png".to_owned(),
        size_bytes: Some(5),
        data_base64: "aW1hZ2U=".to_owned(),
    }];
    request.attachment_metadata = vec![InputAttachmentMetadata {
        attachment_id: "att-image-1".to_owned(),
        kind: InputAttachmentKind::Image,
        media_type: "image/png".to_owned(),
        name: "screen.png".to_owned(),
        size_bytes: Some(5),
    }];
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();
    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    selected.mode = AgentMode::Slave;
    set_single_master_peer(&mut selected, "master-live");

    let outcome = run_live_reason_turn_with_policy(
        &selected,
        request,
        LiveReasonExecutionRole::Worker,
        None,
        |_| {},
        |_| {},
        |_| {},
    )
    .expect("live bridge");
    let first_request = rx.recv().expect("first provider request");
    let second_request = rx.recv().expect("second provider request");
    handle.join().expect("join provider");

    let first_body = http_request_body_json(&first_request);
    let first_content = first_body["messages"][0]["content"]
        .as_array()
        .expect("first user content");
    assert_eq!(first_content[1]["type"], json!("image"));
    assert_eq!(first_content[1]["source"]["type"], json!("base64"));
    assert_eq!(first_content[1]["source"]["media_type"], json!("image/png"));
    assert_eq!(first_content[1]["source"]["data"], json!("aW1hZ2U="));
    assert!(!first_request.contains("att-image-1"));
    assert!(!first_request.contains("screen.png"));

    let second_body = http_request_body_json(&second_request);
    let second_user_content = second_body["messages"]
        .as_array()
        .expect("second messages")
        .iter()
        .rfind(|message| message["role"] == json!("user"))
        .expect("tool-result user message")["content"]
        .as_array()
        .expect("tool result content");
    assert!(
        second_user_content
            .iter()
            .all(|block| block["type"] != json!("image"))
    );
    assert!(!second_request.contains("aW1hZ2U="));
    assert!(!second_request.contains("data:image/png;base64"));

    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.turns[0].attachments.len(), 1);
    assert!(outcome.turns[1].attachments.is_empty());
    assert_eq!(
        outcome.turns[0].request.user_text,
        "Describe the attached image without copying metadata into text"
    );
    assert!(!outcome.turns[0].request.user_text.contains("att-image-1"));
    assert!(!outcome.turns[0].request.user_text.contains("aW1hZ2U="));

    let persistence = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"));
    let restored_turns = persistence
        .restore_turn_snapshots_for_ui(&session_id)
        .expect("restore persisted live turn snapshots");
    assert_eq!(restored_turns.len(), 2);
    assert_eq!(restored_turns[0].attachments.len(), 1);
    assert!(restored_turns[1].attachments.is_empty());
    let projection = project_runtime_turn_history(
        &AgentId::new("agent-live"),
        "agent-live-node",
        std::slice::from_ref(&restored_turns[0]),
        None,
    );
    assert_eq!(projection.attachments.len(), 1);
    assert_eq!(projection.attachments[0].attachment_id, "att-image-1");
    assert_eq!(projection.attachments[0].name, "screen.png");
    let projection_json = serde_json::to_string(&projection).expect("projection json");
    assert!(projection_json.contains("att-image-1"));
    assert!(!projection_json.contains("aW1hZ2U="));
    assert!(!projection_json.contains("data_base64"));

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn live_bridge_returns_incomplete_tool_use_as_failed_tool_result_without_schema_retry() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "text/event-stream",
        vec![
            incomplete_tool_use_stream_response(),
            complete_stream_response("tool recovered"),
        ],
    );

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        live_request(true),
    )
    .expect("live bridge");
    let first_request = rx.recv().expect("first request");
    let second_request = rx.recv().expect("second request");
    handle.join().expect("join");

    assert!(first_request.contains("\"stream\":true"));
    assert!(
        second_request.contains("\"type\":\"tool_result\""),
        "incomplete tool_use must be paired back to the model"
    );
    assert!(second_request.contains("toolu_incomplete_1"));
    assert!(second_request.contains("is_error"));
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.schema_rejections.len(), 0);
    assert_eq!(outcome.tool_executions, 1);
    assert!(outcome.turns.iter().any(|turn| {
        turn.tool_results.iter().any(|result| {
            result.tool_result.status == ToolResultStatus::Failed
                && result
                    .tool_result
                    .output
                    .contains("cannot execute incomplete tool arguments")
        })
    }));
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
}

#[test]
fn live_bridge_returns_tool_execution_failure_to_model_for_next_round() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_missing_read_response(),
            complete_single_response("recovered after tool failure"),
        ],
    );
    let request = live_request(false);
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("tool execution failure should be model-visible result");
    let _first_request = rx.recv().expect("first provider request");
    let second_request = rx.recv().expect("second provider request");
    handle.join().expect("join");

    assert!(second_request.contains("\"type\":\"tool_result\""));
    assert!(second_request.contains("\"tool_use_id\":\"toolu_missing_read_1\""));
    assert!(second_request.contains("\"is_error\":true"));
    assert!(second_request.contains("Tool execution failed"));
    assert!(second_request.contains("cannot resolve"));
    assert!(second_request.contains("path_diagnostic"));
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.tool_executions, 1);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );

    let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
        .restore(&session_id)
        .expect("restore live session");
    let latest = restored
        .closed_turns
        .last()
        .expect("turn should be materialized after model continuation");
    assert!(restored.active_turn.is_none());
    assert!(outcome.turns.iter().any(|turn| {
        turn.tool_calls
            .iter()
            .any(|call| call.tool_call.tool_name == "read_file")
            && turn.tool_results.iter().any(|result| {
                result.tool_result.tool_call_id.as_str() == "toolu_missing_read_1"
                    && result.tool_result.status == ToolResultStatus::Failed
            })
    }));
    assert!(outcome.broadcasts.iter().any(|event| {
        matches!(
            event,
            ReasonBroadcastEvent::ModelContinuationWaiting(waiting)
                if waiting.detail.contains("1 failed / 1 total")
        )
    }));
    assert_eq!(
        latest
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
    assert!(latest.error_events.is_empty());
}

#[test]
fn live_bridge_rejects_cross_cwd_master_read_then_accepts_worker_dispatch() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let external_workspace = runtime_home.with_file_name(format!(
        "{}-external-repo",
        runtime_home
            .file_name()
            .expect("runtime home file name")
            .to_string_lossy()
    ));
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    fs::create_dir_all(&external_workspace).expect("create external workspace");
    assert!(!external_workspace.starts_with(&runtime_home));
    fs::write(external_workspace.join("secret.txt"), "must-not-be-read")
        .expect("write external fixture");
    let task_id = "task-cross-workspace-boundary";
    let agent_id = "worker-cross-workspace-boundary";
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_named_response(
                "toolu_external_read",
                "read_file",
                json!({"path": external_workspace.join("secret.txt")}),
            ),
            task_tool_use_response(
                "toolu_create_worker",
                json!({
                    "op": "create_agent",
                    "agent_id": agent_id,
                    "capabilities": ["repository"]
                }),
            ),
            task_tool_use_response(
                "toolu_create_task",
                json!({
                    "op": "create",
                    "task_id": task_id,
                    "title": "Inspect external repository",
                    "content": "Inspect the target repository without master-side access",
                    "goal": "Delegate external workspace work",
                    "deliverables": ["worker report"],
                    "acceptance": ["worker owns external access"],
                    "priority": 90,
                    "target_cwd": external_workspace,
                    "dispatch": {"mode": "none"}
                }),
            ),
            task_tool_use_response(
                "toolu_assign_task",
                json!({
                    "op": "assign",
                    "task_id": task_id,
                    "agent_id": agent_id
                }),
            ),
            waiting_single_response(
                "worker task assigned for external workspace; inspect TaskHistory and Worker review before final answer",
            ),
        ],
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.cwd = Some(runtime_home.clone());

    let mut selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    set_single_worker_peer(&mut selected, agent_id);
    let outcome = run_live_reason_turn(&selected, request).expect(
        "cross-cwd master read failure must return to model and still permit task dispatch",
    );
    let requests = (0..5)
        .map(|_| rx.recv().expect("provider request"))
        .collect::<Vec<_>>();
    handle.join().expect("join");

    assert!(!requests[0].contains("\"name\":\"bash\""));
    assert!(requests[0].contains("\"name\":\"task\""));
    assert!(requests[0].contains("\"name\":\"read_file\""));
    assert!(requests[0].contains("\"name\":\"ls\""));
    assert!(requests[0].contains("\"name\":\"grep\""));
    assert!(requests[0].contains("\"name\":\"glob\""));
    assert!(requests[1].contains("\"type\":\"tool_result\""));
    assert!(requests[1].contains("\"tool_use_id\":\"toolu_external_read\""));
    assert!(requests[1].contains("\"is_error\":true"));
    assert!(requests[1].contains("Workspace boundary denied"));
    assert!(requests[1].contains("task({\\\"op\\\":\\\"create\\\""));
    assert!(
        !requests[1].contains("must-not-be-read"),
        "forbidden master read must not leak external file content"
    );
    assert!(requests[2].contains("Agent created"));
    assert!(requests[3].contains("Task created"));
    assert!(requests[4].contains("Task assigned"));
    assert_eq!(outcome.rounds, 5);
    assert_eq!(outcome.tool_executions, 4);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::ToolPending)
    );

    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime");
    let task = task_runtime
        .query_task(&TaskId::new(task_id))
        .expect("delegated task");
    assert_eq!(
        task.assignee.as_ref().map(|assignee| &assignee.agent_id),
        Some(&AgentId::new(agent_id))
    );
    assert_eq!(task.target_cwd.as_deref(), external_workspace.to_str());
    let _ = fs::remove_dir_all(runtime_home);
    let _ = fs::remove_dir_all(
        external_workspace
            .parent()
            .expect("external workspace parent"),
    );
}

#[test]
fn live_bridge_returns_external_write_boundary_as_tool_result_guidance() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let external_workspace = runtime_home.with_file_name(format!(
        "{}-external-write-repo",
        runtime_home
            .file_name()
            .expect("runtime home file name")
            .to_string_lossy()
    ));
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    fs::create_dir_all(&external_workspace).expect("create external workspace");
    assert!(!external_workspace.starts_with(&runtime_home));
    let outside_file = external_workspace.join("notes.txt");
    fs::write(&outside_file, "original").expect("write external fixture");
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_named_response(
                "toolu_external_write",
                "write_file",
                json!({"path": outside_file.to_string_lossy().to_string(), "content": "mutated"}),
            ),
            complete_single_response("external write boundary observed"),
        ],
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.cwd = Some(runtime_home.clone());

    let outcome = run_worker_live_reason_turn(
        &live_selected_worker_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("external write boundary should return to model");
    let _first_request = rx.recv().expect("first provider request");
    let second_request = rx.recv().expect("second provider request");
    handle.join().expect("join");

    assert!(second_request.contains("\"type\":\"tool_result\""));
    assert!(second_request.contains("\"tool_use_id\":\"toolu_external_write\""));
    assert!(second_request.contains("\"is_error\":true"));
    assert!(second_request.contains("Write boundary denied"));
    assert!(second_request.contains("write_file"));
    assert_eq!(
        fs::read_to_string(&outside_file).expect("read external file"),
        "original"
    );
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.tool_executions, 1);

    let _ = fs::remove_dir_all(runtime_home);
    let _ = fs::remove_dir_all(
        external_workspace
            .parent()
            .expect("external workspace parent"),
    );
}

#[test]
fn live_bridge_returns_worker_external_read_boundary_with_locked_workspace_guidance() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let external_workspace = runtime_home.with_file_name(format!(
        "{}-external-read-repo",
        runtime_home
            .file_name()
            .expect("runtime home file name")
            .to_string_lossy()
    ));
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    fs::create_dir_all(&external_workspace).expect("create external workspace");
    assert!(!external_workspace.starts_with(&runtime_home));
    fs::write(external_workspace.join("visible.txt"), "must-not-be-read")
        .expect("write external fixture");
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_named_response(
                "toolu_external_ls",
                "ls",
                json!({"path": external_workspace.to_string_lossy().to_string()}),
            ),
            complete_single_response("external read boundary observed"),
        ],
    );
    let mut request = live_request(false);
    request.runtime_home = runtime_home.clone();
    request.cwd = Some(runtime_home.clone());

    let outcome = run_worker_live_reason_turn(
        &live_selected_worker_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("external read boundary should return to model");
    let _first_request = rx.recv().expect("first provider request");
    let second_request = rx.recv().expect("second provider request");
    handle.join().expect("join");

    assert!(second_request.contains("\"type\":\"tool_result\""));
    assert!(second_request.contains("\"tool_use_id\":\"toolu_external_ls\""));
    assert!(second_request.contains("\"is_error\":true"));
    assert!(second_request.contains("Workspace boundary denied"));
    assert!(second_request.contains("ls.path"));
    assert!(second_request.contains("Worker path tools are locked"));
    assert!(second_request.contains("Use relative paths inside the task cwd"));
    assert!(second_request.contains("path_diagnostic"));
    assert!(
        !second_request.contains("Read/query operations may inspect external paths"),
        "failed read/list tool result must not invite external probing"
    );
    assert!(
        !second_request.contains("must-not-be-read"),
        "external read boundary must not leak external file content"
    );
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.tool_executions, 1);

    let _ = fs::remove_dir_all(runtime_home);
    let _ = fs::remove_dir_all(
        external_workspace
            .parent()
            .expect("external workspace parent"),
    );
}

#[test]
fn live_bridge_returns_unknown_tool_as_failed_tool_result_without_terminalizing() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_unknown_response(),
            complete_single_response("recovered after unknown tool"),
        ],
    );
    let mut request = live_request(false);
    fs::create_dir_all(&request.runtime_home).expect("create runtime home");
    request.cwd = Some(request.runtime_home.clone());
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();

    let outcome = run_worker_live_reason_turn(
        &live_selected_worker_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("unknown tool should be returned to model as failed tool result");
    let _first_request = rx.recv().expect("first provider request");
    let second_request = rx.recv().expect("second provider request");
    handle.join().expect("join");

    assert!(second_request.contains("\"type\":\"tool_result\""));
    assert!(second_request.contains("\"tool_use_id\":\"toolu_unknown_1\""));
    assert!(second_request.contains("\"is_error\":true"));
    assert!(second_request.contains("unknown tool `totally_unknown_tool`"));
    assert!(second_request.contains("Available Worker tools are exactly"));
    assert!(second_request.contains("read_file"));
    assert!(second_request.contains("write_file"));
    assert!(second_request.contains("complete_step"));
    assert!(second_request.contains("Do not call shell, bash, readlink"));
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.tool_executions, 1);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Success)
    );
    let metadata = metadata_ledger_records(&runtime_home, "worker-live", &session_id);
    assert!(metadata.iter().any(|record| {
        record.owner.feature_id.as_str() == "error.center"
            && record.write_node.pipeline_node == "RuntimeLive03ToolExecuted"
            && record
                .entries
                .iter()
                .any(|entry| entry.key == "error.domain" && entry.value == json!("tool"))
            && record.entries.iter().any(|entry| {
                entry.key == "error.recovery_action" && entry.value == json!("repair_schema")
            })
    }));
    assert!(metadata.iter().all(|record| {
        let encoded = serde_json::to_string(record).expect("metadata json");
        !encoded.contains("unknown tool `totally_unknown_tool`")
    }));
}

#[test]
fn live_dispatch_projects_failed_tool_result_without_command_failure() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_unknown_response(),
            complete_single_response("dispatch recovered after first failure"),
            tool_use_unknown_response(),
            complete_single_response("dispatch recovered after second failure"),
        ],
    );
    let runtime_home = temp_runtime_home();
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        runtime_home,
        false,
    )
    .expect("runtime");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "trigger tool failure".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("envelope"),
        )
        .expect("submit should continue after tool execution failure");
    let _ = rx.recv().expect("first provider request");
    let first_reentry = rx.recv().expect("first reentry provider request");
    assert!(
        receipt
            .dispatch_status
            .contains("reason_live_turn_completed")
    );
    assert!(first_reentry.contains("\"is_error\":true"));
    assert!(
        first_reentry
            .contains("`totally_unknown_tool` is not available to the Master live tool surface"),
        "first reentry missing Master capability boundary text: {first_reentry}"
    );
    let second_receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "trigger tool failure again".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("envelope"),
        )
        .expect("second submit should also continue after tool execution failure");
    let _ = rx.recv().expect("second provider request");
    let second_reentry = rx.recv().expect("second reentry provider request");
    handle.join().expect("join");
    assert!(
        second_receipt
            .dispatch_status
            .contains("reason_live_turn_completed")
    );
    assert!(second_reentry.contains("\"is_error\":true"));

    let latest = runtime
        .ui_state()
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QueryLatestActiveTurn)
        .expect("query");
    match latest {
        UiQueryResult::Turn(Some(turn)) => {
            assert_eq!(turn.turn_id, TurnId::new("runtime-turn-2-r2"));
            assert!(
                turn.tool_activities.is_empty(),
                "final round must not aggregate failed tool activity from the previous round"
            );
            assert_eq!(turn.terminal_status, Some(TerminalStatus::Success));
            assert!(
                turn.terminal_text
                    .as_deref()
                    .is_some_and(|text| text.contains("dispatch recovered after second failure"))
            );
            assert!(turn.errors.is_empty());
        }
        other => panic!("unexpected failed latest turn: {other:?}"),
    }
    let transcript = runtime
        .ui_state()
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QuerySessionTurns {
            session_id: SessionId::new("runtime-session-agent-live"),
        })
        .expect("query transcript");
    match transcript {
        UiQueryResult::SessionTurns(transcript) => {
            let failed_tool_round = transcript
                .turns
                .iter()
                .find(|turn| turn.turn_id == TurnId::new("runtime-turn-2"))
                .expect("second request first round");
            assert_eq!(failed_tool_round.tool_activities.len(), 1);
            assert_eq!(
                failed_tool_round.tool_activities[0].status.as_str(),
                "failed"
            );
            assert!(failed_tool_round.terminal_status.is_none());
        }
        other => panic!("unexpected transcript query: {other:?}"),
    }
}

#[test]
fn live_bridge_fails_explicitly_when_runtime_metadata_ledger_is_not_writable() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let request = live_request(false);
    let metadata_path = metadata_ledger_path(
        &request.runtime_home,
        &AgentId::new("agent-live"),
        &request.session_id,
    );
    fs::create_dir_all(&metadata_path).expect("poison metadata path as directory");

    let err = run_live_reason_turn(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        request,
    )
    .expect_err("must fail when metadata ledger is unwritable");

    assert!(matches!(err, RuntimeLiveBridgeError::MetadataFailed(_)));
}

#[test]
fn live_bridge_fails_explicitly_when_provider_raw_ledger_is_not_writable() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, _rx, handle) =
        spawn_mock_server(200, "application/json", complete_single_response("pong"));
    let request = live_request(false);
    let raw_path = request
        .runtime_home
        .join("ledgers")
        .join("providers")
        .join("anthropic")
        .join("agent-live")
        .join(request.session_id.as_str())
        .join("turn-live.jsonl");
    fs::create_dir_all(&raw_path).expect("poison provider raw ledger path as directory");

    let err = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect_err("must fail when provider raw ledger is unwritable");
    handle.join().expect("join");

    assert!(matches!(
        err,
        RuntimeLiveBridgeError::ReasonPersistenceFailed(_)
    ));
}

#[test]
fn live_bridge_blocks_after_three_invalid_schema_retries_without_failed_status() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, _rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            invalid_complete_response(),
            invalid_complete_response(),
            invalid_complete_response(),
        ],
    );

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        live_request(false),
    )
    .expect("live bridge");
    handle.join().expect("join");

    assert_eq!(outcome.rounds, 3);
    assert_eq!(outcome.schema_rejections.len(), 3);
    assert_eq!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(TerminalStatus::Blocked)
    );
}

#[test]
fn live_bridge_interrupts_non_candidate_max_tokens_without_failed_status() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, _rx, handle) =
        spawn_mock_server(200, "application/json", max_tokens_text_response());

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        live_request(false),
    )
    .expect("live bridge should materialize interrupted turn");
    handle.join().expect("join");

    assert_eq!(outcome.rounds, 1);
    assert_eq!(outcome.schema_rejections.len(), 0);
    let terminal = outcome
        .turn
        .terminal_event
        .as_ref()
        .expect("terminal event");
    assert_eq!(terminal.status, TerminalStatus::Interrupted);
    assert!(
        terminal
            .summary
            .contains("Provider ended before completion schema was available: max_tokens")
    );
}

#[test]
fn live_bridge_maps_openai_protocols_to_provider_descriptor() {
    let responses_agent = live_selected_agent_with_protocol(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::OpenAi,
        ConfigProviderProtocol::Responses,
    );
    let responses = provider_descriptor(&responses_agent.provider).expect("responses descriptor");
    assert_eq!(responses.family, ProviderFamily::OpenAiCompatible);
    assert_eq!(responses.protocol, ProviderProtocol::OpenAiResponses);

    let chat_agent = live_selected_agent_with_protocol(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::OpenAi,
        ConfigProviderProtocol::ChatCompletions,
    );
    let chat = provider_descriptor(&chat_agent.provider).expect("chat completions descriptor");
    assert_eq!(chat.family, ProviderFamily::OpenAiCompatible);
    assert_eq!(chat.protocol, ProviderProtocol::OpenAiChatCompletions);
}

#[test]
fn live_bridge_derives_hosted_web_search_for_configured_provider_native_protocols() {
    let mut responses_agent = live_selected_agent_with_protocol(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::OpenAi,
        ConfigProviderProtocol::Responses,
    );
    responses_agent.provider.default_model = "custom-responses-model".to_owned();
    let responses = provider_descriptor(&responses_agent.provider).expect("responses descriptor");
    assert_eq!(
        responses.capabilities.web_search,
        ProviderWebSearchCapability::hosted_live_with_functions()
    );

    let mut preview_responses_agent = live_selected_agent_with_protocol(
        "https://example.invalid/v1".to_owned(),
        freehand_config::ProviderType::OpenAi,
        ConfigProviderProtocol::Responses,
    );
    preview_responses_agent.provider.default_model = "gpt-5.6-sol".to_owned();
    preview_responses_agent.provider.web_search_wire =
        freehand_config::ProviderWebSearchWire::WebSearchPreview;
    let preview_responses = provider_descriptor(&preview_responses_agent.provider)
        .expect("preview responses descriptor");
    assert_eq!(
        preview_responses.capabilities.web_search,
        ProviderWebSearchCapability::hosted_live_with_functions()
            .with_wire_tool_type(ProviderWebSearchToolType::WebSearchPreview)
    );
    assert_eq!(
        LiveReasonExecutionRole::Master
            .hosted_tool_definitions(&responses, LiveReasonExecutionProfile::Workspace)
            .len(),
        1
    );
    assert!(
        LiveReasonExecutionRole::Worker
            .hosted_tool_definitions(&responses, LiveReasonExecutionProfile::Workspace)
            .is_empty()
    );
    assert_eq!(
        LiveReasonExecutionRole::Worker
            .hosted_tool_definitions(&responses, LiveReasonExecutionProfile::CleanSearch)
            .len(),
        1
    );

    responses_agent.provider.web_search = freehand_config::ProviderWebSearchMode::Disabled;
    let disabled = provider_descriptor(&responses_agent.provider).expect("disabled descriptor");
    assert_eq!(
        disabled.capabilities.web_search,
        ProviderWebSearchCapability::Unsupported
    );
    assert!(
        LiveReasonExecutionRole::Master
            .hosted_tool_definitions(&disabled, LiveReasonExecutionProfile::Workspace)
            .is_empty()
    );

    let mut chat_agent = live_selected_agent_with_protocol(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::OpenAi,
        ConfigProviderProtocol::ChatCompletions,
    );
    chat_agent.provider.default_model = "gpt-5.5".to_owned();
    let chat = provider_descriptor(&chat_agent.provider).expect("chat descriptor");
    assert_eq!(
        chat.capabilities.web_search,
        ProviderWebSearchCapability::Unsupported
    );

    let mut messages_agent = live_selected_agent_with_protocol(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::Anthropic,
        ConfigProviderProtocol::Messages,
    );
    messages_agent.provider.default_model = "MiniMax-M3".to_owned();
    let messages = provider_descriptor(&messages_agent.provider).expect("messages descriptor");
    assert_eq!(
        messages.capabilities.web_search,
        ProviderWebSearchCapability::hosted_live_with_functions()
    );
    assert_eq!(
        LiveReasonExecutionRole::Master
            .hosted_tool_definitions(&messages, LiveReasonExecutionProfile::Workspace)
            .len(),
        1
    );
}

#[test]
fn live_bridge_does_not_mix_search_only_hosted_tool_with_master_functions() {
    let mut descriptor = provider_descriptor(
        &live_selected_agent_with_protocol(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::OpenAi,
            ConfigProviderProtocol::Responses,
        )
        .provider,
    )
    .expect("descriptor");
    descriptor.capabilities.web_search = ProviderWebSearchCapability::hosted_live_search_only();

    assert!(
        LiveReasonExecutionRole::Master
            .hosted_tool_definitions(&descriptor, LiveReasonExecutionProfile::Workspace)
            .is_empty()
    );
    assert_eq!(
        LiveReasonExecutionRole::Worker
            .hosted_tool_definitions(&descriptor, LiveReasonExecutionProfile::CleanSearch)
            .len(),
        1
    );
}

#[test]
fn provider_web_search_test_declares_hosted_tool_and_requires_observation() {
    let response = json!({
        "id": "resp-search-test",
        "object": "response",
        "status": "completed",
        "output": [
            {
                "type": "web_search_call",
                "id": "ws-test",
                "status": "completed",
                "action": {
                    "type": "search",
                    "query": "Freehand provider web_search live capability test"
                }
            },
            {
                "type": "message",
                "id": "msg-search-test",
                "role": "assistant",
                "status": "completed",
                "content": [{
                    "type": "output_text",
                    "text": "web search completed",
                    "annotations": []
                }]
            }
        ]
    });
    let (base_url, rx, handle) = spawn_mock_server(200, "application/json", response.to_string());
    let mut selected = live_selected_agent_with_protocol(
        base_url,
        freehand_config::ProviderType::OpenAi,
        ConfigProviderProtocol::Responses,
    );
    selected.provider.default_model = "custom-responses-model".to_owned();

    let status = execute_provider_web_search_test(
        &selected,
        selected.provider.clone(),
        Some("Freehand provider web_search live capability test"),
    )
    .expect("provider web_search test");
    let raw_request = rx.recv().expect("provider request");
    handle.join().expect("join provider");
    let body = http_request_body_json(&raw_request);
    let tools = body["tools"].as_array().expect("tools");

    assert!(status.starts_with("provider_web_search_test_passed:provider=provider-live"));
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], json!("web_search"));
    assert!(body.to_string().contains("provider-hosted web_search now"));
    assert!(!body.to_string().contains("\"name\":\"web_search\""));
    assert!(!body.to_string().contains("\"type\":\"function\""));
}

#[test]
fn provider_web_search_test_forces_anthropic_hosted_search_tool_choice() {
    let response = json!({
        "content": [
            {
                "type": "server_tool_use",
                "id": "srv-search-test",
                "name": "web_search",
                "input": {
                    "query": "Freehand Anthropic hosted search capability test"
                }
            },
            {
                "type": "web_search_tool_result",
                "tool_use_id": "srv-search-test",
                "content": [
                    {
                        "title": "Freehand hosted search",
                        "url": "https://example.test/freehand"
                    }
                ]
            },
            {
                "type": "text",
                "text": "web search completed"
            }
        ],
        "stop_reason": "end_turn"
    });
    let (base_url, rx, handle) = spawn_mock_server(200, "application/json", response.to_string());
    let mut selected = live_selected_agent_with_protocol(
        base_url,
        freehand_config::ProviderType::Anthropic,
        ConfigProviderProtocol::Messages,
    );
    selected.provider.default_model = "MiniMax-M3".to_owned();

    let status = execute_provider_web_search_test(
        &selected,
        selected.provider.clone(),
        Some("Freehand Anthropic hosted search capability test"),
    )
    .expect("provider web_search test");
    let raw_request = rx.recv().expect("provider request");
    handle.join().expect("join provider");
    let body = http_request_body_json(&raw_request);
    let tools = body["tools"].as_array().expect("tools");

    assert!(status.starts_with("provider_web_search_test_passed:provider=provider-live"));
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], json!("web_search_20250305"));
    assert_eq!(tools[0]["name"], json!("web_search"));
    assert_eq!(
        body["tool_choice"],
        json!({"type":"tool","name":"web_search"})
    );
    assert!(body.to_string().contains("provider-hosted web_search now"));
    assert!(!body.to_string().contains("\"input_schema\""));
}

#[test]
fn provider_web_search_test_fails_when_provider_does_not_observe_hosted_search() {
    let (base_url, _rx, handle) = spawn_mock_server(
        200,
        "application/json",
        openai_responses_complete_response("plain response"),
    );
    let mut selected = live_selected_agent_with_protocol(
        base_url,
        freehand_config::ProviderType::OpenAi,
        ConfigProviderProtocol::Responses,
    );
    selected.provider.default_model = "custom-responses-model".to_owned();

    let err = execute_provider_web_search_test(&selected, selected.provider.clone(), None)
        .expect_err("missing hosted observation fails");
    handle.join().expect("join provider");

    assert!(
        err.to_string()
            .contains("did not observe provider-hosted web_search")
    );
    assert!(err.to_string().contains("observed_outputs=semantic:"));
    assert!(err.to_string().contains("plain response"));
}

#[test]
fn clean_search_worker_profile_exposes_hosted_search_without_function_tools() {
    let mut agent = live_selected_agent_with_protocol(
        "http://127.0.0.1:1".to_owned(),
        freehand_config::ProviderType::OpenAi,
        ConfigProviderProtocol::Responses,
    );
    agent.provider.default_model = "gpt-5.5".to_owned();
    let descriptor = provider_descriptor(&agent.provider).expect("descriptor");
    let registry = BuiltinToolRegistry::reasonix_aligned();

    assert!(
        LiveReasonExecutionRole::Worker
            .tool_definitions(&registry, LiveReasonExecutionProfile::CleanSearch)
            .is_empty()
    );
    assert_eq!(
        LiveReasonExecutionRole::Worker
            .tool_schema_fingerprint(&registry, LiveReasonExecutionProfile::CleanSearch),
        "clean-search:no-function-tools"
    );
    assert_eq!(
        LiveReasonExecutionRole::Worker
            .hosted_tool_definitions(&descriptor, LiveReasonExecutionProfile::CleanSearch)
            .len(),
        1
    );

    assert!(
        LiveReasonExecutionRole::Worker
            .tool_definitions(&registry, LiveReasonExecutionProfile::Workspace)
            .iter()
            .any(|tool| tool.name == "read_file")
    );
    assert!(
        LiveReasonExecutionRole::Worker
            .hosted_tool_definitions(&descriptor, LiveReasonExecutionProfile::Workspace)
            .is_empty()
    );
}

#[test]
fn clean_search_worker_request_uses_hosted_search_without_local_instruction_scan() {
    with_locked_cwd(|| {
        let original_cwd = std::env::current_dir().expect("current cwd");
        let runtime_home = temp_runtime_home();
        let local_workspace = temp_runtime_home();
        fs::create_dir_all(&runtime_home).expect("runtime home");
        fs::create_dir_all(local_workspace.join(".agents/skills/local-search-skill"))
            .expect("local skill dir");
        fs::write(
            runtime_home.join("AGENTS.md"),
            "FH-CLEAN-SEARCH-RUNTIME-AGENTS must not enter hosted search request",
        )
        .expect("runtime agents");
        fs::write(
            local_workspace.join("Cargo.toml"),
            "[package]\nname=\"clean-search-local\"\nversion=\"0.0.0\"\nedition=\"2021\"\n",
        )
        .expect("workspace marker");
        fs::write(
            local_workspace.join("AGENTS.md"),
            "FH-CLEAN-SEARCH-LOCAL-AGENTS must not enter hosted search request",
        )
        .expect("local agents");
        fs::write(
            local_workspace.join(".agents/skills/local-search-skill/SKILL.md"),
            "---\nname: local-search-skill\ndescription: FH-CLEAN-SEARCH-LOCAL-SKILL sentinel\n---\n# Local Search Skill\n",
        )
        .expect("local skill");
        std::env::set_current_dir(&local_workspace).expect("set local cwd");
        let restore_cwd = RestoreCwd {
            original: original_cwd,
        };

        let (base_url, rx, handle) = spawn_mock_server(
            200,
            "application/json",
            openai_responses_complete_response("clean search complete"),
        );
        let mut selected =
            live_selected_worker_agent(base_url, freehand_config::ProviderType::OpenAi);
        selected.provider.protocol = ConfigProviderProtocol::Responses;
        selected.provider.default_model = "gpt-5.5".to_owned();
        let request = LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            session_id: SessionId::new("clean-search-worker-session"),
            turn_id: TurnId::new("clean-search-worker-turn"),
            trace_id: TraceId::new("clean-search-worker-trace"),
            prompt: "Find current source evidence for a broad web question.".to_owned(),
            attachments: Vec::new(),
            attachment_metadata: Vec::new(),
            cwd: None,
            execution_profile: LiveReasonExecutionProfile::CleanSearch,
            stream: false,
            cancel_token: None,
        };

        let outcome = run_worker_live_reason_turn(&selected, request)
            .expect("clean_search worker live request");
        let raw_request = rx.recv().expect("provider request");
        handle.join().expect("join provider");
        let body = http_request_body_json(&raw_request);
        let tools = body["tools"].as_array().expect("tools array");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["type"], json!("web_search"));
        assert_eq!(tools[0]["external_web_access"], json!(true));
        assert!(tools[0].get("name").is_none());
        assert!(tools[0].get("parameters").is_none());
        let body_text = body.to_string();
        assert!(body_text.contains("execution_profile=clean_search"));
        assert!(body_text.contains("No local workspace instruction capability was loaded"));
        assert!(!body_text.contains("FH-CLEAN-SEARCH-RUNTIME-AGENTS"));
        assert!(!body_text.contains("FH-CLEAN-SEARCH-LOCAL-AGENTS"));
        assert!(!body_text.contains("FH-CLEAN-SEARCH-LOCAL-SKILL"));
        assert_eq!(
            outcome
                .turn
                .terminal_event
                .as_ref()
                .map(|event| event.status.clone()),
            Some(TerminalStatus::Success)
        );

        drop(restore_cwd);
        fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
        fs::remove_dir_all(local_workspace).expect("cleanup local workspace");
    });
}

#[test]
fn live_bridge_rejects_unsupported_provider_selection() {
    let err = run_live_reason_turn(
        &live_selected_agent_with_protocol(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
            ConfigProviderProtocol::Responses,
        ),
        live_request(false),
    )
    .expect_err("must fail");

    assert!(matches!(
        err,
        RuntimeLiveBridgeError::UnsupportedLiveProvider { provider, protocol }
            if provider == "anthropic" && protocol == "responses"
    ));
}

#[test]
fn live_bridge_writes_provider_error_metadata_on_executor_failure() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // Return HTTP 500 so the executor returns HttpStatus, which maps to
    // RuntimeLiveBridgeError::ProviderExecutorFailed and triggers
    // RuntimeLive05ProviderError metadata + debug emission.
    let (base_url, _rx, handle) = spawn_status_sequence_server(
        (0..PROVIDER_EXECUTOR_RETRY_CAP)
            .map(|_| {
                (
                    500,
                    "application/json",
                    r#"{"error":{"type":"internal_error","message":"server exploded"}}"#
                        .to_string(),
                )
            })
            .collect(),
    );
    let request = live_request(false);
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();
    let metadata_path =
        metadata_ledger_path(&runtime_home, &AgentId::new("agent-live"), &session_id);

    let err = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect_err("must fail on HTTP 500");

    assert!(matches!(
        err,
        RuntimeLiveBridgeError::ProviderExecutorFailed(ref msg)
            if msg.contains("500")
    ));

    // Verify provider error metadata was written to the durable ledger.
    let raw = fs::read_to_string(&metadata_path).expect("read metadata ledger");
    let records: Vec<MetadataEnvelope> = raw
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("decode metadata"))
        .collect();

    assert!(records.iter().any(|record| {
        record.kind == MetadataKind::Provider
            && record.write_node.pipeline_node == "RuntimeLive05ProviderError"
            && record
                .entries
                .iter()
                .any(|e| e.key == "error.kind" && e.value == json!("executor_failure"))
    }));
    assert!(records.iter().any(|record| {
        record.owner.feature_id.as_str() == "error.center"
            && record.write_node.pipeline_node == "RuntimeLive05ProviderError"
            && record
                .entries
                .iter()
                .any(|entry| entry.key == "error.domain" && entry.value == json!("provider"))
            && record.entries.iter().any(|entry| {
                entry.key == "error.recovery_action" && entry.value == json!("fail_turn")
            })
            && record.entries.iter().any(|entry| {
                entry.key == "error.retry_index"
                    && entry.value == json!(PROVIDER_EXECUTOR_RETRY_CAP)
            })
    }));
    assert!(records.iter().any(|record| {
        record.owner.feature_id.as_str() == "error.center"
            && record.entries.iter().any(|entry| {
                entry.key == "error.public_message"
                    && entry.value == json!("internal_error: server exploded")
            })
    }));
    assert!(
        records
            .iter()
            .filter(|record| record.owner.feature_id.as_str() == "error.center")
            .all(|record| {
                let encoded = serde_json::to_string(record).expect("metadata json");
                !encoded.contains(r#""error":{"type":"internal_error""#)
            }),
        "error center may expose a sanitized public message, not the raw provider JSON body"
    );

    let restored = ReasonPersistence::new(&runtime_home, AgentId::new("agent-live"))
        .restore(&session_id)
        .expect("restore failed provider turn");
    assert!(restored.active_turn.is_none());
    let failed_turn = restored
        .closed_turns
        .last()
        .expect("provider failure must close the turn");
    assert_eq!(
        failed_turn
            .terminal_event
            .as_ref()
            .map(|event| event.status.clone()),
        Some(freehand_contracts::TerminalStatus::Failed)
    );
    assert!(failed_turn.error_events.iter().any(|event| {
        event.error.code == "anthropic_http_status_500"
            && event
                .error
                .message
                .contains("provider live executor failed")
    }));

    let _ = handle.join();
    let _ = fs::remove_dir_all(&runtime_home);
}

#[test]
fn live_bridge_creates_checkpoint_for_write_file_and_rewinds_created_file() {
    with_temp_workspace(|root| {
        fs::create_dir_all(root.join("scratch")).expect("create parent directory");
        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_write_file_response("scratch/note.txt", "pong\n"),
                complete_single_response("write done"),
            ],
        );
        let mut request = live_request(false);
        request.runtime_home = root.to_path_buf();
        request.cwd = Some(root.to_path_buf());
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();

        let outcome = run_worker_live_reason_turn(
            &live_selected_worker_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("live bridge");
        let _ = rx.recv().expect("first provider request");
        let _ = rx.recv().expect("second provider request");
        handle.join().expect("join");

        assert_eq!(outcome.tool_executions, 1);
        let file_path = root.join("scratch/note.txt");
        assert_eq!(
            fs::read_to_string(&file_path).expect("written file"),
            "pong\n"
        );

        let rows = checkpoint_ledger_rows(&runtime_home, "worker-live", &session_id);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].event, RuntimeCheckpointLedgerEvent::Created);
        assert_eq!(rows[1].event, RuntimeCheckpointLedgerEvent::Applied);
        let checkpoint_id = rows[0].checkpoint_id.clone();

        let store =
            RuntimeCheckpointStore::new(&runtime_home, &AgentId::new("worker-live"), &session_id)
                .expect("checkpoint store");
        let manifest = store.load_manifest(&checkpoint_id).expect("manifest");
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].kind, ToolPreviewChangeKind::Create);
        assert_eq!(manifest.entries[0].blob_file, None);

        rewind_checkpoint(
            &runtime_home,
            &AgentId::new("worker-live"),
            &session_id,
            &checkpoint_id,
        )
        .expect("rewind");
        assert!(!file_path.exists());

        let rows = checkpoint_ledger_rows(&runtime_home, "worker-live", &session_id);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[2].event, RuntimeCheckpointLedgerEvent::Restored);
    });
}

#[test]
fn live_bridge_rewinds_modify_checkpoint_back_to_original_text() {
    with_temp_workspace(|root| {
        let file_path = root.join("edit-target.txt");
        fs::write(&file_path, "before\n").expect("seed file");

        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_edit_file_response("edit-target.txt", "before", "after"),
                complete_single_response("edit done"),
            ],
        );
        let mut request = live_request(false);
        request.runtime_home = root.to_path_buf();
        request.cwd = Some(root.to_path_buf());
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();

        let outcome = run_worker_live_reason_turn(
            &live_selected_worker_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("live bridge");
        let _ = rx.recv().expect("first provider request");
        let _ = rx.recv().expect("second provider request");
        handle.join().expect("join");

        assert_eq!(outcome.tool_executions, 1);
        assert_eq!(
            fs::read_to_string(&file_path).expect("edited file"),
            "after\n"
        );

        let rows = checkpoint_ledger_rows(&runtime_home, "worker-live", &session_id);
        assert_eq!(rows[0].event, RuntimeCheckpointLedgerEvent::Created);
        assert_eq!(rows[1].event, RuntimeCheckpointLedgerEvent::Applied);
        let checkpoint_id = rows[0].checkpoint_id.clone();

        let store =
            RuntimeCheckpointStore::new(&runtime_home, &AgentId::new("worker-live"), &session_id)
                .expect("checkpoint store");
        let manifest = store.load_manifest(&checkpoint_id).expect("manifest");
        assert_eq!(manifest.entries[0].kind, ToolPreviewChangeKind::Modify);
        assert_eq!(manifest.entries[0].blob_file.as_deref(), Some("blob-0.txt"));

        rewind_checkpoint(
            &runtime_home,
            &AgentId::new("worker-live"),
            &session_id,
            &checkpoint_id,
        )
        .expect("rewind");
        assert_eq!(
            fs::read_to_string(&file_path).expect("rewound file"),
            "before\n"
        );
    });
}

#[test]
fn rewind_checkpoint_rejects_missing_manifest_explicitly() {
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let err = rewind_checkpoint(
        &runtime_home,
        &AgentId::new("agent-live"),
        &SessionId::new("session-live"),
        "checkpoint-missing",
    )
    .expect_err("missing manifest must fail");

    assert_eq!(
        err,
        RuntimeCheckpointError::MissingManifest("checkpoint-missing".to_owned())
    );
    let _ = fs::remove_dir_all(runtime_home);
}

#[test]
fn rewind_checkpoint_rejects_missing_blob_file_explicitly() {
    with_temp_workspace(|root| {
        let file_path = root.join("edit-target.txt");
        fs::write(&file_path, "before\n").expect("seed file");

        let (base_url, rx, handle) = spawn_sequence_server(
            "application/json",
            vec![
                tool_use_edit_file_response("edit-target.txt", "before", "after"),
                complete_single_response("edit done"),
            ],
        );
        let mut request = live_request(false);
        request.runtime_home = root.to_path_buf();
        request.cwd = Some(root.to_path_buf());
        let runtime_home = request.runtime_home.clone();
        let session_id = request.session_id.clone();

        let outcome = run_worker_live_reason_turn(
            &live_selected_worker_agent(base_url, freehand_config::ProviderType::Anthropic),
            request,
        )
        .expect("live bridge");
        let _ = rx.recv().expect("first provider request");
        let _ = rx.recv().expect("second provider request");
        handle.join().expect("join");

        assert_eq!(outcome.tool_executions, 1);
        let rows = checkpoint_ledger_rows(&runtime_home, "worker-live", &session_id);
        let checkpoint_id = rows[0].checkpoint_id.clone();

        let store =
            RuntimeCheckpointStore::new(&runtime_home, &AgentId::new("worker-live"), &session_id)
                .expect("checkpoint store");
        let manifest = store.load_manifest(&checkpoint_id).expect("manifest");
        let blob = manifest.entries[0]
            .blob_file
            .clone()
            .expect("modify checkpoint blob");
        fs::remove_file(
            runtime_home
                .join("state")
                .join("checkpoints")
                .join("worker-live")
                .join(session_id.as_str())
                .join(&checkpoint_id)
                .join(&blob),
        )
        .expect("remove blob");

        let err = rewind_checkpoint(
            &runtime_home,
            &AgentId::new("worker-live"),
            &session_id,
            &checkpoint_id,
        )
        .expect_err("missing blob must fail");
        assert_eq!(
            err,
            RuntimeCheckpointError::MissingBlob {
                checkpoint_id: checkpoint_id.clone(),
                blob: blob.clone(),
            }
        );
        assert_eq!(
            fs::read_to_string(&file_path).expect("post-failure file still modified"),
            "after\n"
        );
    });
}

#[test]
fn list_checkpoints_rejects_corrupt_ledger_line_explicitly() {
    let runtime_home = temp_runtime_home();
    let session_id = SessionId::new("session-live");
    let ledger_dir = runtime_home
        .join("ledgers")
        .join("checkpoints")
        .join("agent-live");
    fs::create_dir_all(&ledger_dir).expect("create ledger dir");
    fs::write(
        ledger_dir.join(format!("{}.jsonl", session_id.as_str())),
        "{not-json}\n",
    )
    .expect("write corrupt ledger");

    let err = list_checkpoints(&runtime_home, &AgentId::new("agent-live"), &session_id)
        .expect_err("corrupt ledger must fail");
    match err {
        RuntimeCheckpointError::PersistenceFailed(message) => {
            assert!(message.contains("checkpoint ledger line 1 failed to parse"));
        }
        other => panic!("unexpected corrupt-ledger error: {other:?}"),
    }
}

#[test]
fn live_bridge_executes_bash_without_checkpoint_preview() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_bash_response("printf 'pong'"),
            complete_single_response("bash done"),
        ],
    );
    let request = live_request(false);
    let runtime_home = request.runtime_home.clone();
    let session_id = request.session_id.clone();

    let outcome = run_live_reason_turn(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
    )
    .expect("live bridge");
    let _ = rx.recv().expect("first provider request");
    let second_request = rx.recv().expect("second provider request");
    handle.join().expect("join");

    assert!(second_request.contains("\"type\":\"tool_result\""));
    assert!(second_request.contains("pong"));
    assert_eq!(outcome.tool_executions, 1);
    assert_eq!(outcome.rounds, 2);
    let checkpoint_path = runtime_home
        .join("ledgers")
        .join("checkpoints")
        .join("agent-live")
        .join(format!("{}.jsonl", session_id.as_str()));
    assert!(!checkpoint_path.exists());
}

#[test]
fn bootstrap_with_live_restore_recovers_ui_projection_and_next_turn_ordinal() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let session_id = SessionId::new("runtime-session-agent-live");
    let (first_url, first_rx, first_handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_single_response(),
            complete_single_response("first done"),
        ],
    );
    let selected = live_selected_agent(first_url, freehand_config::ProviderType::Anthropic);
    let first_outcome = run_live_reason_turn(
        &selected,
        LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            session_id: session_id.clone(),
            turn_id: TurnId::new("runtime-turn-1"),
            trace_id: TraceId::new("runtime-trace-1"),
            prompt: "first request".to_owned(),
            attachments: Vec::new(),
            attachment_metadata: Vec::new(),
            cwd: None,
            execution_profile: LiveReasonExecutionProfile::Workspace,
            stream: false,
            cancel_token: None,
        },
    )
    .expect("first live turn");
    let _ = first_rx.recv().expect("first provider request");
    let _ = first_rx.recv().expect("second provider request");
    first_handle.join().expect("join first provider");
    assert_eq!(
        first_outcome.turn.request.turn_id,
        TurnId::new("runtime-turn-1-r2")
    );

    let (second_url, second_rx, second_handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_single_response(),
            complete_single_response("second done"),
        ],
    );
    let mut restored_selected = selected.clone();
    restored_selected.provider.base_url = second_url;
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &restored_selected,
        runtime_home.clone(),
        false,
    )
    .expect("restored runtime");

    let latest = runtime
        .ui_state()
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QueryLatestActiveTurn)
        .expect("query");
    match latest {
        UiQueryResult::Turn(Some(turn)) => {
            assert_eq!(turn.turn_id, TurnId::new("runtime-turn-1-r2"));
            assert!(
                turn.terminal_text
                    .as_deref()
                    .is_some_and(|text| text.contains("Summary: first done"))
            );
        }
        other => panic!("unexpected restored latest turn: {other:?}"),
    }

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "second request".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("envelope"),
        )
        .expect("second receipt");
    assert_eq!(
        receipt.dispatch_status,
        "reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 restored_closed_turns=1"
    );
    let _ = second_rx.recv().expect("restart provider request");
    let _ = second_rx.recv().expect("restart tool-result request");
    second_handle.join().expect("join second provider");

    let latest = runtime
        .ui_state()
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QueryLatestActiveTurn)
        .expect("query");
    match latest {
        UiQueryResult::Turn(Some(turn)) => {
            assert_eq!(turn.turn_id, TurnId::new("runtime-turn-2-r2"));
            assert!(
                turn.terminal_text
                    .as_deref()
                    .is_some_and(|text| text.contains("Summary: second done"))
            );
        }
        other => panic!("unexpected latest turn after restart submit: {other:?}"),
    }
}

#[test]
fn live_restore_resumes_turn_ordinal_from_selected_non_default_session() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let selected_session_id = SessionId::new("webui-session-selected-ordinal");
    let (first_url, first_rx, first_handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_single_response(),
            complete_single_response("selected first done"),
        ],
    );
    let selected = live_selected_agent(first_url, freehand_config::ProviderType::Anthropic);
    let first_outcome = run_live_reason_turn(
        &selected,
        LiveReasonTurnRequest {
            runtime_home: runtime_home.clone(),
            session_id: selected_session_id.clone(),
            turn_id: TurnId::new("runtime-turn-1"),
            trace_id: TraceId::new("runtime-trace-1"),
            prompt: "selected first request".to_owned(),
            attachments: Vec::new(),
            attachment_metadata: Vec::new(),
            cwd: None,
            execution_profile: LiveReasonExecutionProfile::Workspace,
            stream: false,
            cancel_token: None,
        },
    )
    .expect("first selected live turn");
    let _ = first_rx.recv().expect("first selected provider request");
    let _ = first_rx.recv().expect("first selected tool-result request");
    first_handle.join().expect("join first selected provider");
    assert_eq!(
        first_outcome.turn.request.turn_id,
        TurnId::new("runtime-turn-1-r2")
    );

    let (second_url, second_rx, second_handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_single_response(),
            complete_single_response("selected second done"),
        ],
    );
    let mut restored_selected = selected.clone();
    restored_selected.provider.base_url = second_url;
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &restored_selected,
        runtime_home.clone(),
        false,
    )
    .expect("restored runtime");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "selected second request".to_owned(),
                session_id: Some(selected_session_id.clone()),
                cwd: None,
                metadata: None,
            })
            .expect("envelope"),
        )
        .expect("second selected receipt");
    assert_eq!(
        receipt.dispatch_status,
        "reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 restored_closed_turns=1"
    );
    let _ = second_rx.recv().expect("second selected provider request");
    let _ = second_rx
        .recv()
        .expect("second selected tool-result request");
    second_handle.join().expect("join second selected provider");

    let latest = runtime
        .ui_state()
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QuerySessionTurns {
            session_id: selected_session_id,
        })
        .expect("query selected session");
    match latest {
        UiQueryResult::SessionTurns(transcript) => {
            let turn_ids = transcript
                .turns
                .iter()
                .map(|turn| turn.turn_id.as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                turn_ids,
                vec![
                    "runtime-turn-1",
                    "runtime-turn-1-r2",
                    "runtime-turn-2",
                    "runtime-turn-2-r2"
                ]
            );
        }
        other => panic!("unexpected selected session turns after restart submit: {other:?}"),
    }
}

#[test]
fn submit_input_dispatches_to_reason_and_updates_ui_state() {
    let runtime = runtime();
    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "hello runtime".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("envelope"),
        )
        .expect("receipt");
    assert_eq!(receipt.target_feature_id, "reason.turn");
    assert_eq!(receipt.dispatch_status, "reason_turn_started");

    let ui_state = runtime.ui_state();
    let latest = ui_state
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QueryLatestActiveTurn)
        .expect("query");
    match latest {
        UiQueryResult::Turn(Some(turn)) => {
            assert_eq!(turn.source.source_node_id, "master-node");
            assert_eq!(turn.turn_id, TurnId::new("runtime-turn-1"));
            assert_eq!(turn.user_text.as_deref(), Some("hello runtime"));
            let public = freehand_ui_protocol::public_turn_projection(turn);
            assert_eq!(public.public_conversation[0].body, "hello runtime");
        }
        other => panic!("unexpected latest turn query: {other:?}"),
    }
}

#[test]
fn cancel_turn_dispatches_to_reason_owner() {
    let runtime = runtime();
    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "cancel me".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("envelope"),
        )
        .expect("submit");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::CancelTurn {
                turn_id: TurnId::new("runtime-turn-1"),
            })
            .expect("envelope"),
        )
        .expect("cancel receipt");
    assert_eq!(receipt.dispatch_status, "reason_turn_cancelled");

    let ui_state = runtime.ui_state();
    let latest = ui_state
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QueryLatestActiveTurn)
        .expect("query");
    match latest {
        UiQueryResult::Turn(Some(turn)) => {
            assert_eq!(
                turn.terminal_text.as_deref(),
                Some("cancelled by ui command")
            );
        }
        other => panic!("unexpected latest turn query: {other:?}"),
    }
}

#[test]
fn cancel_latest_active_turn_dispatches_to_latest_reason_turn() {
    let runtime = runtime();
    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "cancel latest".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("submit envelope"),
        )
        .expect("submit");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::CancelLatestActiveTurn {})
                .expect("cancel latest envelope"),
        )
        .expect("cancel latest receipt");
    assert_eq!(receipt.ingress.command_kind, "cancel_latest_active_turn");
    assert_eq!(receipt.dispatch_status, "reason_turn_cancelled");

    let latest = runtime
        .ui_state()
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QueryLatestActiveTurn)
        .expect("query");
    match latest {
        UiQueryResult::Turn(Some(turn)) => {
            assert_eq!(turn.terminal_status, Some(TerminalStatus::Cancelled));
        }
        other => panic!("unexpected latest turn query: {other:?}"),
    }
}

#[test]
fn cancel_turn_missing_target_returns_target_not_found() {
    let runtime = runtime();
    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::CancelTurn {
                turn_id: TurnId::new("runtime-turn-missing"),
            })
            .expect("cancel envelope"),
        )
        .expect_err("missing turn must fail");
    assert_eq!(
        err,
        UiCommandDispatchPortError::TargetNotFound("runtime-turn-missing".to_owned())
    );
}

#[test]
fn cancel_latest_active_turn_without_any_turn_returns_target_not_found() {
    let runtime = runtime();
    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::CancelLatestActiveTurn {})
                .expect("cancel latest envelope"),
        )
        .expect_err("empty runtime must fail");
    assert_eq!(
        err,
        UiCommandDispatchPortError::TargetNotFound("latest-active-turn".to_owned())
    );
}

#[test]
fn active_live_cancel_returns_before_provider_finishes_and_blocks_success_projection() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let first_chunk = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"working\"}}\n\n"
        )
        .to_owned();
    let remaining_chunks = complete_stream_response("late success");
    let (base_url, request_rx, released_rx, continue_tx, handle) =
        spawn_incremental_stream_server(first_chunk, remaining_chunks);
    let runtime = Arc::new(
        RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            temp_runtime_home(),
            true,
        )
        .expect("runtime"),
    );
    let submit_runtime = Arc::clone(&runtime);
    let submit_handle = thread::spawn(move || {
        submit_runtime.dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "start long stream".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("submit envelope"),
        )
    });

    loop {
        let latest = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryLatestActiveTurn)
            .expect("query");
        if matches!(latest, UiQueryResult::Turn(Some(_))) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    request_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("provider request before stream cancel");

    let cancel_receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::CancelTurn {
                turn_id: TurnId::new("runtime-turn-1"),
            })
            .expect("cancel envelope"),
        )
        .expect("cancel receipt");
    assert_eq!(
        cancel_receipt.dispatch_status,
        "reason_live_turn_cancel_requested"
    );

    let latest = runtime
        .ui_state()
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QueryLatestActiveTurn)
        .expect("query");
    match latest {
        UiQueryResult::Turn(Some(turn)) => {
            assert_eq!(turn.terminal_status, Some(TerminalStatus::Cancelled));
            let public = freehand_ui_protocol::public_turn_projection(turn);
            assert_eq!(
                public
                    .public_conversation
                    .last()
                    .map(|item| item.status.as_str()),
                Some("cancelled")
            );
        }
        other => panic!("unexpected cancelled latest turn: {other:?}"),
    }

    continue_tx.send(()).expect("release provider");
    let released = released_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("release status");
    assert!(released);
    let submit_err = submit_handle
        .join()
        .expect("submit thread")
        .expect_err("submit should observe cancellation");
    assert_eq!(
        submit_err,
        UiCommandDispatchPortError::DispatchFailed("live turn cancelled".to_owned())
    );
    handle.join().expect("join provider");

    let latest = runtime
        .ui_state()
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QueryLatestActiveTurn)
        .expect("query");
    match latest {
        UiQueryResult::Turn(Some(turn)) => {
            assert_eq!(turn.terminal_status, Some(TerminalStatus::Cancelled));
            assert!(
                turn.terminal_text
                    .as_deref()
                    .is_some_and(|text| text.contains("cancelled"))
            );
        }
        other => panic!("unexpected final cancelled latest turn: {other:?}"),
    }
}

#[test]
fn runtime_live_submit_registers_and_clears_master_active_work() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let first_chunk = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"working\"}}\n\n"
        )
        .to_owned();
    let remaining_chunks = complete_stream_response("active work done");
    let (base_url, _rx, released_rx, continue_tx, handle) =
        spawn_incremental_stream_server(first_chunk, remaining_chunks);
    let runtime_home = temp_runtime_home();
    let runtime = Arc::new(
        RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            runtime_home.clone(),
            true,
        )
        .expect("runtime"),
    );
    let submit_runtime = Arc::clone(&runtime);
    let submit_handle = thread::spawn(move || {
        submit_runtime.dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "hold active work".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("submit envelope"),
        )
    });

    loop {
        let active =
            master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
                .expect("load active work");
        if active.as_ref().is_some_and(|checkpoint| {
            checkpoint.safe_point == master_runner::MasterWorkSafePoint::ProviderInFlight
        }) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let active = master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
        .expect("load active work")
        .expect("active work");
    assert_eq!(
        active.session_id,
        SessionId::new("runtime-session-agent-live")
    );
    assert_eq!(active.logical_turn_id, TurnId::new("runtime-turn-1"));
    assert_eq!(active.trace_id, TraceId::new("runtime-trace-1"));
    assert_eq!(active.state, master_runner::MasterActiveWorkState::Running);
    assert_eq!(
        active.safe_point,
        master_runner::MasterWorkSafePoint::ProviderInFlight
    );

    continue_tx.send(()).expect("release provider");
    assert!(released_rx.recv().expect("released"));
    handle.join().expect("join provider");
    submit_handle
        .join()
        .expect("submit join")
        .expect("submit success");
    assert!(
        master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
            .expect("load active work")
            .is_none(),
        "terminal live submit must clear its active-work checkpoint"
    );
}

#[test]
fn runtime_live_submit_rejects_concurrent_master_active_work_without_ordinal_gap() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let first_chunk = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"working\"}}\n\n"
        )
        .to_owned();
    let remaining_chunks = complete_stream_response("first done");
    let (base_url, _rx, released_rx, continue_tx, handle) =
        spawn_incremental_stream_server(first_chunk, remaining_chunks);
    let runtime_home = temp_runtime_home();
    let runtime = Arc::new(
        RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
            runtime_home.clone(),
            true,
        )
        .expect("runtime"),
    );
    let submit_runtime = Arc::clone(&runtime);
    let submit_handle = thread::spawn(move || {
        submit_runtime.dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "first active work".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("first submit envelope"),
        )
    });

    loop {
        let active =
            master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
                .expect("load active work");
        if active.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    let second = runtime.dispatch(
        build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
            text: "second active work".to_owned(),
            session_id: None,
            cwd: None,
            metadata: None,
        })
        .expect("second submit envelope"),
    );
    let error = second.expect_err("concurrent Master work must fail");
    match error {
        UiCommandDispatchPortError::DispatchFailed(message) => {
            assert!(message.contains("Master active work"));
            assert!(message.contains("runtime-turn-2"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
    {
        let state = runtime.state.lock().expect("lock runtime state");
        assert_eq!(state.next_turn_ordinal, 1);
        assert_eq!(state.active_turns.len(), 1);
    }
    let active = master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
        .expect("load active work")
        .expect("active work");
    assert_eq!(active.logical_turn_id, TurnId::new("runtime-turn-1"));

    continue_tx.send(()).expect("release provider");
    assert!(released_rx.recv().expect("released"));
    handle.join().expect("join provider");
    submit_handle
        .join()
        .expect("submit join")
        .expect("first submit success");
}

#[test]
fn runtime_live_submit_persists_pre_provider_active_turn_for_refresh() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let canonical_runtime_home = fs::canonicalize(&runtime_home).expect("canonical runtime home");
    let session_id = SessionId::new("pre-provider-refresh");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:9".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");
    let prepared = {
        let mut state = runtime.state.lock().expect("lock runtime state");
        runtime
            .prepare_live_submit_user_input(
                &mut state,
                "accepted turn must survive refresh before provider".to_owned(),
                Some(session_id.clone()),
                Some(runtime_home.to_string_lossy().into_owned()),
                None,
            )
            .expect("prepare")
            .expect("prepared live submit")
    };

    let turns = runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("query")
        .expect("session turns");
    match turns {
        UiQueryResult::SessionTurns(projection) => {
            assert_eq!(projection.turns.len(), 1);
            assert_eq!(projection.turns[0].turn_id, TurnId::new("runtime-turn-1"));
            assert_eq!(projection.turns[0].terminal_status, None);
            assert_eq!(
                projection.turns[0].user_text.as_deref(),
                Some("accepted turn must survive refresh before provider")
            );
            assert_eq!(
                projection.turns[0].cwd.as_deref(),
                Some(canonical_runtime_home.to_string_lossy().as_ref())
            );
        }
        other => panic!("unexpected query result: {other:?}"),
    }

    let ledger_path = runtime_home
        .join("ledgers")
        .join("reason")
        .join("agent-live")
        .join(format!("{}.jsonl", session_id.as_str()));
    let ledger = fs::read_to_string(&ledger_path).expect("read reason ledger");
    assert!(ledger.contains("RewriteStateUpdated"));
    assert!(
        !ledger.contains("TurnStarted"),
        "prepared active snapshot must not duplicate canonical provider TurnStarted"
    );

    let recovered = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:9".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("recovered runtime");
    let recovered_turns = recovered
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("query recovered")
        .expect("recovered session turns");
    match recovered_turns {
        UiQueryResult::SessionTurns(projection) => {
            assert_eq!(projection.turns.len(), 1);
            assert_eq!(projection.turns[0].turn_id, prepared.turn_id);
            assert_eq!(
                projection.turns[0].user_text.as_deref(),
                Some("accepted turn must survive refresh before provider")
            );
            assert_eq!(projection.turns[0].terminal_status, None);
        }
        other => panic!("unexpected recovered query result: {other:?}"),
    }

    prepared.cancel_token.store(true, Ordering::SeqCst);
    runtime
        .finish_live_submit(&prepared, Err(RuntimeLiveBridgeError::Cancelled))
        .expect_err("cleanup cancellation returns dispatch failure");
}

#[test]
fn runtime_live_submit_materializes_cancelled_turn_before_provider_request() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:9".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");
    let prepared = {
        let mut state = runtime.state.lock().expect("lock runtime state");
        runtime
            .prepare_live_submit_user_input(
                &mut state,
                "cancel before provider request".to_owned(),
                Some(SessionId::new("pre-provider-cancel")),
                Some(runtime_home.to_string_lossy().into_owned()),
                None,
            )
            .expect("prepare")
            .expect("prepared live submit")
    };
    let active_turns = runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: SessionId::new("pre-provider-cancel"),
        })
        .expect("query active prepared")
        .expect("active session turns");
    match active_turns {
        UiQueryResult::SessionTurns(projection) => {
            assert_eq!(projection.turns.len(), 1);
            assert_eq!(projection.turns[0].turn_id, TurnId::new("runtime-turn-1"));
            assert_eq!(projection.turns[0].terminal_status, None);
            assert_eq!(
                projection.turns[0].user_text.as_deref(),
                Some("cancel before provider request")
            );
        }
        other => panic!("unexpected active query result: {other:?}"),
    }
    prepared.cancel_token.store(true, Ordering::SeqCst);

    let error = runtime
        .finish_live_submit(&prepared, Err(RuntimeLiveBridgeError::Cancelled))
        .expect_err("cancelled submit returns dispatch failure");
    assert_eq!(
        error,
        UiCommandDispatchPortError::DispatchFailed("live turn cancelled".to_owned())
    );
    assert!(
        master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
            .expect("load active work")
            .is_none(),
        "cancelled pre-provider submit must clear active-work checkpoint"
    );
    let turns = runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: SessionId::new("pre-provider-cancel"),
        })
        .expect("query")
        .expect("session turns");
    match turns {
        UiQueryResult::SessionTurns(projection) => {
            assert_eq!(projection.turns.len(), 1);
            assert_eq!(projection.turns[0].turn_id, TurnId::new("runtime-turn-1"));
            assert_eq!(
                projection.turns[0].terminal_status,
                Some(TerminalStatus::Cancelled)
            );
            assert_eq!(
                projection.turns[0].user_text.as_deref(),
                Some("cancel before provider request")
            );
        }
        other => panic!("unexpected query result: {other:?}"),
    }
    let failed_prepared = {
        let mut state = runtime.state.lock().expect("lock runtime state");
        runtime
            .prepare_live_submit_user_input(
                &mut state,
                "fail before provider request in existing session".to_owned(),
                Some(SessionId::new("pre-provider-cancel")),
                None,
                None,
            )
            .expect("prepare failed turn")
            .expect("prepared failed live submit")
    };
    assert_eq!(failed_prepared.turn_id, TurnId::new("runtime-turn-2"));
    let active_turns = runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: SessionId::new("pre-provider-cancel"),
        })
        .expect("query active failed prepared")
        .expect("active failed session turns");
    match active_turns {
        UiQueryResult::SessionTurns(projection) => {
            assert_eq!(projection.turns.len(), 2);
            assert_eq!(projection.turns[1].turn_id, TurnId::new("runtime-turn-2"));
            assert_eq!(projection.turns[1].terminal_status, None);
            assert_eq!(
                projection.turns[1].user_text.as_deref(),
                Some("fail before provider request in existing session")
            );
        }
        other => panic!("unexpected active failed query result: {other:?}"),
    }
    let error = runtime
        .finish_live_submit(
            &failed_prepared,
            Err(RuntimeLiveBridgeError::InstructionCapabilityFailed(
                "instruction capability segment timed out after 5ms".to_owned(),
            )),
        )
        .expect_err("pre-provider failure returns dispatch failure");
    assert_eq!(
        error,
        UiCommandDispatchPortError::DispatchFailed(
            "instruction capability admission failed: instruction capability segment timed out after 5ms"
                .to_owned()
        )
    );
    let turns = runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: SessionId::new("pre-provider-cancel"),
        })
        .expect("query")
        .expect("session turns");
    match turns {
        UiQueryResult::SessionTurns(projection) => {
            assert_eq!(projection.turns.len(), 2);
            assert_eq!(projection.turns[1].turn_id, TurnId::new("runtime-turn-2"));
            assert_eq!(
                projection.turns[1].terminal_status,
                Some(TerminalStatus::Failed)
            );
            assert_eq!(
                projection.turns[1].user_text.as_deref(),
                Some("fail before provider request in existing session")
            );
        }
        other => panic!("unexpected query result after failed turn: {other:?}"),
    }
    let recovered = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:9".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home,
        false,
    )
    .expect("recovered runtime");
    let next_prepared = {
        let mut state = recovered
            .state
            .lock()
            .expect("lock recovered runtime state");
        recovered
            .prepare_live_submit_user_input(
                &mut state,
                "next turn after cancelled pre-provider submit".to_owned(),
                Some(SessionId::new("pre-provider-cancel")),
                None,
                None,
            )
            .expect("prepare next")
            .expect("prepared next")
    };
    assert_eq!(next_prepared.turn_id, TurnId::new("runtime-turn-3"));
}

#[test]
fn cancel_latest_active_live_turn_materializes_pre_provider_terminal_truth() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let session_id = SessionId::new("pre-provider-dispatch-cancel");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:9".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");
    {
        let mut state = runtime.state.lock().expect("lock runtime state");
        runtime
            .prepare_live_submit_user_input(
                &mut state,
                "cancel while request context is still preparing".to_owned(),
                Some(session_id.clone()),
                Some(runtime_home.to_string_lossy().into_owned()),
                None,
            )
            .expect("prepare")
            .expect("prepared live submit");
        assert_eq!(state.active_turns.len(), 1);
    }

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::CancelLatestActiveTurn {})
                .expect("cancel latest envelope"),
        )
        .expect("cancel latest receipt");
    assert_eq!(receipt.dispatch_status, "reason_live_turn_cancel_requested");
    {
        let state = runtime.state.lock().expect("lock runtime state");
        assert!(
            state.active_turns.is_empty(),
            "cancel command must not wait for a stuck pre-provider context builder to clear active_turns"
        );
    }
    assert!(
        master_runner::load_master_active_work(&runtime_home, &AgentId::new("agent-live"))
            .expect("load active work")
            .is_none(),
        "cancel command must immediately release the Master active-work checkpoint"
    );

    let turns = runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("query")
        .expect("session turns");
    match turns {
        UiQueryResult::SessionTurns(projection) => {
            assert_eq!(projection.turns.len(), 1);
            assert_eq!(projection.turns[0].turn_id, TurnId::new("runtime-turn-1"));
            assert_eq!(
                projection.turns[0].terminal_status,
                Some(TerminalStatus::Cancelled)
            );
            assert_eq!(
                projection.turns[0].user_text.as_deref(),
                Some("cancel while request context is still preparing")
            );
        }
        other => panic!("unexpected query result: {other:?}"),
    }

    let recovered = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:9".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home,
        false,
    )
    .expect("recovered runtime");
    let recovered_turns = recovered
        .query_runtime(&UiCommand::QuerySessionTurns { session_id })
        .expect("query recovered")
        .expect("recovered session turns");
    match recovered_turns {
        UiQueryResult::SessionTurns(projection) => {
            assert_eq!(projection.turns.len(), 1);
            assert_eq!(
                projection.turns[0].terminal_status,
                Some(TerminalStatus::Cancelled)
            );
        }
        other => panic!("unexpected recovered query result: {other:?}"),
    }
}

#[test]
fn runtime_live_submit_success_does_not_duplicate_prepared_turn_started() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![complete_single_response("prepared active success")],
    );
    let runtime_home = temp_runtime_home();
    fs::create_dir_all(&runtime_home).expect("create runtime home");
    let session_id = SessionId::new("pre-provider-success");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "prepared active should not become historical context".to_owned(),
                session_id: Some(session_id.clone()),
                cwd: Some(runtime_home.to_string_lossy().into_owned()),
                metadata: None,
            })
            .expect("submit envelope"),
        )
        .expect("submit success");
    assert!(
        receipt
            .dispatch_status
            .contains("reason_live_turn_completed")
    );
    let provider_request = rx.recv().expect("provider request");
    handle.join().expect("join provider");
    assert!(
        !provider_request.contains("Historical turn 1"),
        "prepared active snapshot must not be replayed as historical context for the same request"
    );

    let turns = runtime
        .query_runtime(&UiCommand::QuerySessionTurns {
            session_id: session_id.clone(),
        })
        .expect("query")
        .expect("session turns");
    match turns {
        UiQueryResult::SessionTurns(projection) => {
            assert_eq!(projection.turns.len(), 1);
            assert_eq!(projection.turns[0].turn_id, TurnId::new("runtime-turn-1"));
            assert_eq!(
                projection.turns[0].terminal_status,
                Some(TerminalStatus::Success)
            );
            assert_eq!(
                projection.turns[0].user_text.as_deref(),
                Some("prepared active should not become historical context")
            );
        }
        other => panic!("unexpected success query result: {other:?}"),
    }

    let ledger_path = runtime_home
        .join("ledgers")
        .join("reason")
        .join("agent-live")
        .join(format!("{}.jsonl", session_id.as_str()));
    let ledger = fs::read_to_string(&ledger_path).expect("read reason ledger");
    assert_eq!(ledger.matches("RewriteStateUpdated").count(), 1);
    assert_eq!(ledger.matches("TurnStarted").count(), 1);
    assert_eq!(ledger.matches("TurnClosed").count(), 1);
}

#[test]
fn live_master_attention_invalidates_stale_tool_without_side_effect() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let stale_task_id = "task-stale-tool-side-effect";
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            task_tool_use_response(
                "toolu_stale_create",
                json!({
                    "op": "create",
                    "task_id": stale_task_id,
                    "title": "stale task",
                    "content": "must not be created after attention",
                    "goal": "prove stale side effect is blocked",
                    "deliverables": ["none"],
                    "acceptance": ["no task is created"],
                    "priority": 90,
                    "target_cwd": runtime_home,
                    "dispatch": {"mode": "none"}
                }),
            ),
            complete_single_response("continued after attention"),
        ],
    );
    let selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    let request = live_request_for(&runtime_home, "session-stale-tool", 1);
    master_runner::register_master_active_work(
        &runtime_home,
        &AgentId::new("agent-live"),
        &request.session_id,
        &request.turn_id,
        &request.trace_id,
    )
    .expect("register active work");
    let injected = Arc::new(AtomicBool::new(false));
    let injected_flag = Arc::clone(&injected);
    let request_ids = (
        request.session_id.clone(),
        request.turn_id.clone(),
        request.trace_id.clone(),
    );
    let callback_runtime_home = runtime_home.clone();
    let outcome = run_live_reason_turn_with_hooks(
        &selected,
        request.clone(),
        move |event| {
            if matches!(event, ReasonBroadcastEvent::Tool(_))
                && !injected_flag.swap(true, Ordering::SeqCst)
            {
                inject_live_master_attention_resolution(
                    &callback_runtime_home,
                    &request_ids.0,
                    &request_ids.1,
                    &request_ids.2,
                    "attention-stale-tool",
                );
            }
        },
        |_| {},
        |_| {},
    )
    .expect("stale tool is paired and re-reasoned");
    let first_request = rx.recv().expect("first provider request");
    let second_request = rx.recv().expect("second provider request");
    handle.join().expect("join provider");

    assert!(!first_request.contains("toolu_stale_create"));
    assert!(!first_request.contains("invalidated_before_execution_by_master_attention"));
    assert!(!first_request.contains("kind=\\\"attention_resolution\\\""));
    assert!(second_request.contains("\"type\":\"tool_result\""));
    assert!(second_request.contains("toolu_stale_create"));
    assert!(second_request.contains("invalidated_before_execution_by_master_attention"));
    assert!(second_request.contains("kind=\\\"attention_resolution\\\""));
    assert_eq!(outcome.rounds, 2);
    assert_eq!(outcome.tool_executions, 1);
    assert!(outcome.turns.iter().any(|turn| {
        turn.tool_results.iter().any(|result| {
            result.tool_result.tool_call_id.as_str() == "toolu_stale_create"
                && result.tool_result.status == ToolResultStatus::Failed
                && result
                    .tool_result
                    .output
                    .contains("invalidated_before_execution_by_master_attention")
        })
    }));
    let task_runtime =
        TaskRuntime::boot(&request.runtime_home, AgentId::new("agent-live")).expect("runtime");
    assert!(
        task_runtime
            .query_task(&TaskId::new(stale_task_id))
            .is_err(),
        "stale task tool call must not mutate Task Center truth"
    );
}

#[test]
fn live_master_attention_rejects_stale_terminal_persistence() {
    let _cwd_lock = cwd_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let runtime_home = temp_runtime_home();
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            complete_single_response("stale terminal candidate"),
            complete_single_response("fresh terminal after attention"),
        ],
    );
    let selected = live_selected_agent(base_url, freehand_config::ProviderType::Anthropic);
    let request = live_request_for(&runtime_home, "session-stale-terminal", 1);
    master_runner::register_master_active_work(
        &runtime_home,
        &AgentId::new("agent-live"),
        &request.session_id,
        &request.turn_id,
        &request.trace_id,
    )
    .expect("register active work");
    let injected = Arc::new(AtomicBool::new(false));
    let injected_flag = Arc::clone(&injected);
    let request_ids = (
        request.session_id.clone(),
        request.turn_id.clone(),
        request.trace_id.clone(),
    );
    let callback_runtime_home = runtime_home.clone();
    let outcome = run_live_reason_turn_with_hooks(
        &selected,
        request.clone(),
        move |event| {
            if matches!(event, ReasonBroadcastEvent::Semantic(_))
                && !injected_flag.swap(true, Ordering::SeqCst)
            {
                inject_live_master_attention_resolution(
                    &callback_runtime_home,
                    &request_ids.0,
                    &request_ids.1,
                    &request_ids.2,
                    "attention-stale-terminal",
                );
            }
        },
        |_| {},
        |_| {},
    )
    .expect("stale terminal is discarded and re-reasoned");
    assert!(
        injected.load(Ordering::SeqCst),
        "provider semantic output must trigger the attention fixture"
    );
    let _first_request = rx.recv().expect("first provider request");
    let second_request = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("second provider request");
    handle.join().expect("join provider");

    assert!(second_request.contains("kind=\\\"attention_resolution\\\""));
    assert_eq!(outcome.rounds, 2);
    assert_eq!(
        outcome
            .turns
            .first()
            .and_then(|turn| turn.terminal_event.as_ref()),
        None,
        "stale terminal candidate must not be persisted"
    );
    assert!(
        outcome
            .turn
            .terminal_event
            .as_ref()
            .is_some_and(|terminal| terminal.summary.contains("fresh terminal after attention"))
    );
    let restored = ReasonPersistence::new(&request.runtime_home, AgentId::new("agent-live"))
        .restore(&request.session_id)
        .expect("restore session");
    assert!(
        restored.closed_turns.iter().all(|turn| {
            !turn
                .terminal_event
                .as_ref()
                .is_some_and(|terminal| terminal.summary.contains("stale terminal candidate"))
        }),
        "stale terminal must not enter durable closed-turn truth"
    );
}

#[test]
fn direct_message_dispatches_to_node_owner() {
    let runtime = runtime();
    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SendDirectMessageToSlave {
                node_id: "slave-node".to_owned(),
                text: "ping".to_owned(),
            })
            .expect("envelope"),
        )
        .expect("receipt");
    assert_eq!(receipt.target_feature_id, "node.master-slave");
    assert_eq!(receipt.dispatch_status, "node_direct_message_dispatched");
}

#[test]
fn direct_message_wrong_slave_target_returns_target_not_found() {
    let runtime = runtime();
    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SendDirectMessageToSlave {
                node_id: "wrong-slave".to_owned(),
                text: "ping".to_owned(),
            })
            .expect("envelope"),
        )
        .expect_err("wrong node must fail");
    assert_eq!(
        err,
        UiCommandDispatchPortError::TargetNotFound("wrong-slave".to_owned())
    );
}

#[test]
fn rewind_checkpoint_dispatch_rejects_non_live_runtime() {
    let runtime = runtime();
    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::RewindCheckpoint {
                checkpoint_id: "checkpoint-1".to_owned(),
            })
            .expect("envelope"),
        )
        .expect_err("rewind should fail");
    assert_eq!(
        err,
        UiCommandDispatchPortError::Unsupported(
            "rewind dispatch requires a live runtime home".to_owned()
        )
    );
}

#[test]
fn rewind_checkpoint_dispatch_restores_workspace_file_state() {
    with_temp_workspace(|root| {
        fs::create_dir_all(root.join("scratch")).expect("create parent directory");
        let runtime_home = fs::canonicalize(root).expect("canonical runtime home");
        let session_id = SessionId::new("runtime-session-agent-live");
        let mut history = SessionHistory::new(session_id.clone(), Vec::new()).expect("history");
        let turn = closed_turn_for_context(
            &mut history,
            &session_id,
            "runtime-turn-checkpoint-dispatch",
            "runtime-trace-checkpoint-dispatch",
            "create checkpoint",
            TerminalStatus::Success,
            "checkpoint source",
        );
        let file_path = runtime_home.join("scratch/rewind.txt");
        let preview = ToolPreviewContract {
            tool_call_id: ToolCallId::new("tool-call-checkpoint-dispatch"),
            changes: vec![ToolPreviewFileChange {
                locked_path: file_path.to_string_lossy().into_owned(),
                kind: ToolPreviewChangeKind::Create,
                before_text: None,
                after_text: Some("rewind me\n".to_owned()),
            }],
        };
        let store =
            RuntimeCheckpointStore::new(&runtime_home, &AgentId::new("agent-live"), &session_id)
                .expect("checkpoint store");
        let manifest = store
            .create_from_preview(&turn, &preview, "write_file")
            .expect("create checkpoint");
        fs::write(&file_path, "rewind me\n").expect("simulate applied write");
        store.mark_applied(&manifest).expect("mark applied");

        let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
            &live_selected_agent(
                "http://127.0.0.1:1".to_owned(),
                freehand_config::ProviderType::Anthropic,
            ),
            runtime_home.clone(),
            false,
        )
        .expect("runtime");

        assert_eq!(
            fs::read_to_string(&file_path).expect("written file"),
            "rewind me\n"
        );
        let rows = checkpoint_ledger_rows(
            &runtime_home,
            "agent-live",
            &SessionId::new("runtime-session-agent-live"),
        );
        let checkpoint_id = rows.first().expect("created row").checkpoint_id.clone();
        let checkpoint_query = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryCheckpoints)
            .expect("checkpoint query");
        match checkpoint_query {
            UiQueryResult::Checkpoints(snapshot) => {
                assert_eq!(snapshot.checkpoints.len(), 1);
                assert_eq!(snapshot.checkpoints[0].checkpoint_id, checkpoint_id);
                assert_eq!(snapshot.checkpoints[0].latest_status, "applied");
            }
            other => panic!("unexpected checkpoint query: {other:?}"),
        }

        let rewind = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::RewindCheckpoint {
                    checkpoint_id: checkpoint_id.clone(),
                })
                .expect("envelope"),
            )
            .expect("rewind receipt");
        assert_eq!(
            rewind.dispatch_status,
            format!("runtime_checkpoint_rewound checkpoint_id={checkpoint_id}")
        );
        assert!(!file_path.exists());
        let checkpoint_query = runtime
            .ui_state()
            .lock()
            .expect("lock ui state")
            .query(&UiCommand::QueryCheckpoints)
            .expect("checkpoint query");
        match checkpoint_query {
            UiQueryResult::Checkpoints(snapshot) => {
                assert_eq!(snapshot.checkpoints[0].latest_status, "restored");
            }
            other => panic!("unexpected checkpoint query after rewind: {other:?}"),
        }
    });
}

#[test]
fn rewind_checkpoint_dispatch_maps_missing_manifest_to_target_not_found() {
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        temp_runtime_home(),
        false,
    )
    .expect("runtime");

    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::RewindCheckpoint {
                checkpoint_id: "checkpoint-missing".to_owned(),
            })
            .expect("envelope"),
        )
        .expect_err("missing checkpoint must fail");
    assert_eq!(
        err,
        UiCommandDispatchPortError::TargetNotFound("checkpoint-missing".to_owned())
    );
}

#[test]
fn runtime_query_reads_task_truth_from_task_runtime() {
    let runtime_home = temp_runtime_home();
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime boot");
    task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(TaskId::new("runtime-query-task-1")),
            title: "Runtime query task".to_owned(),
            content: "Task query bridge content".to_owned(),
            goal: "Expose persisted task truth".to_owned(),
            deliverables: vec!["task list".to_owned()],
            acceptance: vec!["task history".to_owned()],
            priority: 90,
            target_cwd: Some("/tmp".to_owned()),
            execution_profile: TaskExecutionProfile::Workspace,
            dispatch: TaskDispatchRequest::None,
            parent: TaskParentRef {
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            actor: TaskActor {
                agent_id: AgentId::new("agent-live"),
                source: "runtime_query_test".to_owned(),
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            watermark: TaskWatermark {
                metadata_id: None,
                hook: Some("runtime_query_test".to_owned()),
                action_tool_call_id: None,
            },
        })
        .expect("create task");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");

    let list = runtime
        .query_runtime(&UiCommand::QueryTaskList {
            status: Some("waiting_agent".to_owned()),
            agent_id: None,
        })
        .expect("task list query")
        .expect("runtime-backed task list");
    match list {
        UiQueryResult::TaskList(list) => {
            assert_eq!(list.source_agent_id.as_str(), "agent-live");
            assert_eq!(list.tasks.len(), 1);
            assert_eq!(list.tasks[0].task_id, "runtime-query-task-1");
            assert_eq!(list.tasks[0].status, "waiting_agent");
            assert_eq!(list.tasks[0].priority, 90);
            assert_eq!(
                list.tasks[0]
                    .worker_session_id
                    .as_ref()
                    .map(SessionId::as_str),
                Some("worker-task-runtime-query-task-1")
            );
        }
        other => panic!("unexpected task list result: {other:?}"),
    }

    let history = runtime
        .query_runtime(&UiCommand::QueryTaskHistory {
            task_id: "runtime-query-task-1".to_owned(),
        })
        .expect("task history query")
        .expect("runtime-backed task history");
    match history {
        UiQueryResult::TaskHistory(history) => {
            assert_eq!(history.task_id, "runtime-query-task-1");
            assert_eq!(history.events.len(), 2);
            assert_eq!(history.events[0].event_type, "TaskCreated");
            assert_eq!(history.events[1].event_type, "TaskWaitingAgent");
        }
        other => panic!("unexpected task history result: {other:?}"),
    }

    let err = runtime
        .query_runtime(&UiCommand::QueryTaskHistory {
            task_id: "missing-runtime-task".to_owned(),
        })
        .expect_err("missing task history must fail");
    assert_eq!(
        err,
        UiCommandDispatchPortError::TargetNotFound("missing-runtime-task".to_owned())
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_query_reads_phase1_task_and_agent_boards() {
    let runtime_home = temp_runtime_home();
    let task_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("agent-live")).expect("task runtime boot");
    task_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(TaskId::new("runtime-phase1-board-task")),
            title: "Runtime phase1 board task".to_owned(),
            content: "TaskBoard and AgentBoard query bridge content".to_owned(),
            goal: "Expose phase1 board truth".to_owned(),
            deliverables: vec!["task board".to_owned()],
            acceptance: vec!["agent board".to_owned()],
            priority: 91,
            target_cwd: Some("/tmp".to_owned()),
            execution_profile: TaskExecutionProfile::Workspace,
            dispatch: TaskDispatchRequest::SelfAgent,
            parent: TaskParentRef {
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            actor: TaskActor {
                agent_id: AgentId::new("agent-live"),
                source: "runtime_phase1_board_test".to_owned(),
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            watermark: TaskWatermark {
                metadata_id: None,
                hook: Some("runtime_phase1_board_test".to_owned()),
                action_tool_call_id: None,
            },
        })
        .expect("create task");
    let process_started_at = now_unix_seconds();
    task_runtime
        .apply_agent_lifecycle_event(freehand_task::AgentLifecycleEvent::ProcessStarted {
            agent_id: AgentId::new("agent-live"),
            process_id: 501,
            process_instance_id: "agent-live-process-1".to_owned(),
            started_at: process_started_at,
        })
        .expect("process lifecycle");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");

    let task_board = runtime
        .query_runtime(&UiCommand::QueryTaskBoard {
            status: None,
            agent_id: None,
            include_terminal: false,
        })
        .expect("task board query")
        .expect("runtime-backed task board");
    match task_board {
        UiQueryResult::TaskBoard(board) => {
            assert_eq!(board.source_agent_id.as_str(), "agent-live");
            assert_eq!(board.tasks.len(), 1);
            assert_eq!(board.tasks[0].task_id, "runtime-phase1-board-task");
            assert_eq!(board.tasks[0].status, "assigned");
            assert_eq!(board.agents.len(), 1);
            assert_eq!(board.agents[0].agent_id.as_str(), "agent-live");
            assert_eq!(
                board.agents[0].current_task_id.as_deref(),
                Some("runtime-phase1-board-task")
            );
        }
        other => panic!("unexpected task board result: {other:?}"),
    }

    let agent_board = runtime
        .query_runtime(&UiCommand::QueryAgentBoard)
        .expect("agent board query")
        .expect("runtime-backed agent board");
    match agent_board {
        UiQueryResult::AgentBoard(board) => {
            assert_eq!(board.source_agent_id.as_str(), "agent-live");
            assert_eq!(board.agents.len(), 1);
            assert_eq!(board.agents[0].agent_id.as_str(), "agent-live");
            assert_eq!(board.agents[0].state, "assigned");
            assert!(board.agents[0].alive);
            let process = board.agents[0]
                .process
                .as_ref()
                .expect("process projection");
            assert_eq!(process.process_id, Some(501));
            assert_eq!(
                process.process_instance_id.as_deref(),
                Some("agent-live-process-1")
            );
            assert_eq!(process.restart_count, 0);
            assert_eq!(
                board.agents[0].current_task_id.as_deref(),
                Some("runtime-phase1-board-task")
            );
        }
        other => panic!("unexpected agent board result: {other:?}"),
    }

    let lifecycle = runtime
        .query_runtime(&UiCommand::QueryAgentLifecycle {
            agent_id: AgentId::new("agent-live"),
        })
        .expect("agent lifecycle query")
        .expect("runtime-backed agent lifecycle");
    match lifecycle {
        UiQueryResult::AgentLifecycle(lifecycle) => {
            assert_eq!(lifecycle.agent_id.as_str(), "agent-live");
            assert_eq!(lifecycle.state, "assigned");
            assert!(lifecycle.alive);
            let process = lifecycle.process.as_ref().expect("process projection");
            assert_eq!(process.started_at, Some(process_started_at));
            assert_eq!(process.heartbeat_at, Some(process_started_at));
            assert!(process.next_check_at.is_some());
            assert_eq!(
                lifecycle.current_task_id.as_deref(),
                Some("runtime-phase1-board-task")
            );
        }
        other => panic!("unexpected lifecycle result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatch_execution_fact_and_scheduler_tick_update_task_truth() {
    let runtime_home = temp_runtime_home();
    let owner_id = AgentId::new("agent-live");
    let worker_id = AgentId::new("runtime-phase1-worker");
    let task_runtime =
        TaskRuntime::boot(&runtime_home, owner_id.clone()).expect("task runtime boot");
    task_runtime
        .create_agent(AgentCreateRequest {
            agent_id: worker_id.clone(),
            capabilities: vec!["phase1".to_owned()],
            actor: TaskActor {
                agent_id: owner_id.clone(),
                source: "runtime_phase1_fact_test".to_owned(),
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            watermark: TaskWatermark {
                metadata_id: None,
                hook: Some("runtime_phase1_fact_test".to_owned()),
                action_tool_call_id: None,
            },
        })
        .expect("create worker agent");
    for (task_id, title) in [
        ("runtime-phase1-review-task", "Runtime phase1 review task"),
        ("runtime-phase1-blocked-task", "Runtime phase1 blocked task"),
    ] {
        task_runtime
            .create_task(TaskCreateRequest {
                task_id: Some(TaskId::new(task_id)),
                title: title.to_owned(),
                content: format!("{title} content"),
                goal: "prove phase1 execution fact dispatch".to_owned(),
                deliverables: vec!["execution fact".to_owned()],
                acceptance: vec!["TaskBoard projection updates".to_owned()],
                priority: 80,
                target_cwd: None,
                execution_profile: TaskExecutionProfile::Workspace,
                dispatch: TaskDispatchRequest::None,
                parent: TaskParentRef {
                    session_id: Some(SessionId::new("runtime-phase1-fact-session")),
                    turn_id: Some(TurnId::new("runtime-phase1-fact-turn")),
                    trace_id: None,
                },
                actor: TaskActor {
                    agent_id: owner_id.clone(),
                    source: "runtime_phase1_fact_test".to_owned(),
                    session_id: None,
                    turn_id: None,
                    trace_id: None,
                },
                watermark: TaskWatermark {
                    metadata_id: None,
                    hook: Some("runtime_phase1_fact_test".to_owned()),
                    action_tool_call_id: None,
                },
            })
            .expect("create waiting task");
        task_runtime
            .assign_task(TaskAssignRequest {
                task_id: TaskId::new(task_id),
                agent_id: worker_id.clone(),
                actor: TaskActor {
                    agent_id: owner_id.clone(),
                    source: "runtime_phase1_fact_test".to_owned(),
                    session_id: None,
                    turn_id: None,
                    trace_id: None,
                },
                watermark: TaskWatermark {
                    metadata_id: None,
                    hook: Some("runtime_phase1_fact_test".to_owned()),
                    action_tool_call_id: None,
                },
            })
            .expect("assign waiting task");
    }
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");
    let turn_id = TurnId::new("runtime-phase1-fact-turn");
    let agent_id = worker_id;

    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: "runtime-phase1-exec-blocked".to_owned(),
                    task_id: "runtime-phase1-blocked-task".to_owned(),
                    agent_id: agent_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Running {
                        phase: "implementation".to_owned(),
                        summary: "worker started".to_owned(),
                        evidence: vec!["running evidence".to_owned()],
                    },
                },
            })
            .expect("running fact envelope"),
        )
        .expect("running fact dispatch");
    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: "runtime-phase1-exec-blocked".to_owned(),
                    task_id: "runtime-phase1-blocked-task".to_owned(),
                    agent_id: agent_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::Recovering {
                        summary: "worker retrying".to_owned(),
                        evidence: vec!["recovering evidence".to_owned()],
                        retry_count: 1,
                    },
                },
            })
            .expect("recovering fact envelope"),
        )
        .expect("recovering fact dispatch");
    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: "runtime-phase1-exec-review".to_owned(),
                    task_id: "runtime-phase1-review-task".to_owned(),
                    agent_id: agent_id.clone(),
                    turn_id: Some(turn_id.clone()),
                    kind: UiExecutionFactKind::ReviewReady {
                        summary: "ready for review".to_owned(),
                        deliverables: vec!["review deliverable".to_owned()],
                        evidence: vec!["review evidence".to_owned()],
                    },
                },
            })
            .expect("review fact envelope"),
        )
        .expect("review fact dispatch");

    thread::sleep(Duration::from_secs(2));
    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::RunSchedulerTick {
                tick: UiSchedulerTickCommand {
                    stale_after_seconds: 1,
                    soft_timeout_seconds: 1,
                    hard_timeout_seconds: 30,
                },
            })
            .expect("scheduler tick envelope"),
        )
        .expect("scheduler tick dispatch");
    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::ApplyExecutionFact {
                fact: UiExecutionFactCommand {
                    execution_id: "runtime-phase1-exec-blocked".to_owned(),
                    task_id: "runtime-phase1-blocked-task".to_owned(),
                    agent_id,
                    turn_id: Some(turn_id),
                    kind: UiExecutionFactKind::Blocked {
                        reason: "waiting on dependency".to_owned(),
                        evidence: vec!["blocked evidence".to_owned()],
                    },
                },
            })
            .expect("blocked fact envelope"),
        )
        .expect("blocked fact dispatch");

    let board = runtime
        .query_runtime(&UiCommand::QueryTaskBoard {
            status: None,
            agent_id: None,
            include_terminal: false,
        })
        .expect("final board query")
        .expect("final board result");
    match board {
        UiQueryResult::TaskBoard(board) => {
            assert!(
                board
                    .blocked
                    .iter()
                    .any(|task| task.task_id == "runtime-phase1-blocked-task"),
                "blocked view must include execution-blocked task: {:?}",
                board.blocked
            );
            assert!(
                board
                    .review_ready
                    .iter()
                    .any(|task| task.task_id == "runtime-phase1-review-task"),
                "review view must include review-ready task: {:?}",
                board.review_ready
            );
            assert!(
                board
                    .stale
                    .iter()
                    .any(|task| task.task_id == "runtime-phase1-blocked-task"),
                "stale view must include scheduler-observed task: {:?}",
                board.stale
            );
        }
        other => panic!("unexpected final board result: {other:?}"),
    }

    let history = runtime
        .query_runtime(&UiCommand::QueryTaskHistory {
            task_id: "runtime-phase1-blocked-task".to_owned(),
        })
        .expect("history query")
        .expect("history result");
    match history {
        UiQueryResult::TaskHistory(history) => {
            let event_types = history
                .events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>();
            assert!(event_types.contains(&"TaskExecutionRecorded"));
            assert!(event_types.contains(&"TaskExecutionRecovering"));
            assert!(event_types.contains(&"TaskSchedulerTick"));
            assert!(event_types.contains(&"TaskBlocked"));
        }
        other => panic!("unexpected history result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatches_phase2a_master_worker_loop_into_task_truth() {
    let runtime_home = temp_runtime_home();
    let owner_id = AgentId::new("agent-live");
    let worker_id = AgentId::new("runtime-phase2a-worker");
    let task_id = "runtime-phase2a-task".to_owned();
    let execution_id = "runtime-phase2a-exec".to_owned();
    let turn_id = TurnId::new("runtime-phase2a-turn");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");

    for command in [
        UiCommand::CreateTaskAgent {
            agent: UiTaskAgentCreateCommand {
                agent_id: worker_id.clone(),
                capabilities: vec!["code_edit".to_owned()],
            },
        },
        UiCommand::CreateTask {
            task: UiTaskCreateCommand {
                task_id: Some(task_id.clone()),
                title: "Runtime phase2a task".to_owned(),
                content: "Runtime phase2a content".to_owned(),
                goal: "prove runtime master worker loop".to_owned(),
                deliverables: vec!["worker loop".to_owned()],
                acceptance: vec!["approved before close".to_owned()],
                priority: 90,
                target_cwd: None,
                execution_profile: "workspace".to_owned(),
                session_id: Some(SessionId::new("runtime-phase2a-session")),
                turn_id: Some(turn_id.clone()),
                dispatch: Some(UiTaskDispatchCommand::None),
            },
        },
        UiCommand::AssignTask {
            assignment: UiTaskAssignCommand {
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
            },
        },
        UiCommand::ClaimNextTask {
            claim: UiTaskClaimCommand {
                agent_id: worker_id.clone(),
                execution_id: execution_id.clone(),
                ttl_seconds: Some(300),
            },
        },
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id.clone()),
                kind: UiExecutionFactKind::Running {
                    phase: "progress".to_owned(),
                    summary: "worker progress".to_owned(),
                    evidence: vec!["progress evidence".to_owned()],
                },
            },
        },
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id.clone()),
                kind: UiExecutionFactKind::Blocked {
                    reason: "needs input".to_owned(),
                    evidence: vec!["blocked evidence".to_owned()],
                },
            },
        },
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id.clone()),
                kind: UiExecutionFactKind::Recovering {
                    summary: "recovering".to_owned(),
                    evidence: vec!["recovery evidence".to_owned()],
                    retry_count: 1,
                },
            },
        },
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id.clone()),
                kind: UiExecutionFactKind::ReviewReady {
                    summary: "first review".to_owned(),
                    deliverables: vec!["draft".to_owned()],
                    evidence: vec!["draft evidence".to_owned()],
                },
            },
        },
        UiCommand::RejectTaskReview {
            rejection: UiTaskReviewRejectionCommand {
                task_id: task_id.clone(),
                reject_reason: "needs retry".to_owned(),
                next_requirements: vec!["retry".to_owned()],
            },
        },
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id.clone()),
                kind: UiExecutionFactKind::Running {
                    phase: "retry".to_owned(),
                    summary: "retry progress".to_owned(),
                    evidence: vec!["retry evidence".to_owned()],
                },
            },
        },
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id),
                kind: UiExecutionFactKind::ReviewReady {
                    summary: "second review".to_owned(),
                    deliverables: vec!["accepted".to_owned()],
                    evidence: vec!["accepted evidence".to_owned()],
                },
            },
        },
        UiCommand::ApproveTaskReview {
            task_id: task_id.clone(),
        },
        UiCommand::CloseTask {
            task_id: task_id.clone(),
        },
    ] {
        runtime
            .dispatch(build_command_dispatch_envelope(&command).expect("phase2a envelope"))
            .expect("phase2a dispatch");
    }

    let board = runtime
        .query_runtime(&UiCommand::QueryTaskBoard {
            status: None,
            agent_id: None,
            include_terminal: true,
        })
        .expect("phase2a board query")
        .expect("phase2a board result");
    match board {
        UiQueryResult::TaskBoard(board) => {
            let task = board
                .tasks
                .iter()
                .find(|task| task.task_id == task_id)
                .expect("closed task");
            assert_eq!(task.status, "closed");
            assert_eq!(task.assignee_agent_id.as_ref(), Some(&worker_id));
            assert_eq!(
                task.active_execution_id.as_deref(),
                Some(execution_id.as_str())
            );
        }
        other => panic!("unexpected phase2a board result: {other:?}"),
    }

    let lifecycle = runtime
        .query_runtime(&UiCommand::QueryAgentLifecycle {
            agent_id: worker_id.clone(),
        })
        .expect("phase2a lifecycle query")
        .expect("phase2a lifecycle result");
    match lifecycle {
        UiQueryResult::AgentLifecycle(lifecycle) => {
            assert_eq!(lifecycle.agent_id, worker_id);
            assert_eq!(lifecycle.state, "idle");
            assert_eq!(lifecycle.current_task_id, None);
            assert_eq!(lifecycle.current_execution_id, None);
            assert_eq!(
                lifecycle
                    .last_activity
                    .as_ref()
                    .map(|activity| activity.kind.as_str()),
                Some("closed")
            );
        }
        other => panic!("unexpected phase2a lifecycle result: {other:?}"),
    }

    let history = runtime
        .query_runtime(&UiCommand::QueryTaskHistory {
            task_id: task_id.clone(),
        })
        .expect("phase2a history query")
        .expect("phase2a history result");
    match history {
        UiQueryResult::TaskHistory(history) => {
            let event_types = history
                .events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>();
            for required in [
                "TaskCreated",
                "TaskAssigned",
                "TaskResumed",
                "TaskExecutionRecorded",
                "TaskBlocked",
                "TaskExecutionRecovering",
                "TaskReviewSubmitted",
                "TaskReviewRejected",
                "TaskReviewApproved",
                "TaskClosed",
            ] {
                assert!(
                    event_types.contains(&required),
                    "missing {required}: {event_types:?}"
                );
            }
        }
        other => panic!("unexpected phase2a history result: {other:?}"),
    }

    assert_eq!(owner_id.as_str(), "agent-live");
    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatches_phase2b_master_poll_and_event_inbox() {
    let runtime_home = temp_runtime_home();
    let worker_id = AgentId::new("runtime-phase2b-worker");
    let task_id = "runtime-phase2b-task".to_owned();
    let execution_id = "runtime-phase2b-exec".to_owned();
    let turn_id = TurnId::new("runtime-phase2b-turn");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");

    for index in 0..55 {
        let backlog_task_id = format!("runtime-phase2b-backlog-{index:03}");
        runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::CreateTask {
                    task: UiTaskCreateCommand {
                        task_id: Some(backlog_task_id),
                        title: format!("Runtime phase2b backlog {index:03}"),
                        content: "Runtime phase2b backlog content".to_owned(),
                        goal: "prove default master poll drains backlog".to_owned(),
                        deliverables: vec!["backlog event".to_owned()],
                        acceptance: vec!["backlog remains visible to EventInbox".to_owned()],
                        priority: 1,
                        target_cwd: None,
                        execution_profile: "workspace".to_owned(),
                        session_id: Some(SessionId::new(format!(
                            "runtime-phase2b-backlog-session-{index:03}"
                        ))),
                        turn_id: None,
                        dispatch: Some(UiTaskDispatchCommand::None),
                    },
                })
                .expect("phase2b backlog envelope"),
            )
            .expect("phase2b backlog dispatch");
    }

    for command in [
        UiCommand::CreateTaskAgent {
            agent: UiTaskAgentCreateCommand {
                agent_id: worker_id.clone(),
                capabilities: vec!["code_edit".to_owned()],
            },
        },
        UiCommand::CreateTask {
            task: UiTaskCreateCommand {
                task_id: Some(task_id.clone()),
                title: "Runtime phase2b task".to_owned(),
                content: "Runtime phase2b content".to_owned(),
                goal: "prove runtime master poll loop".to_owned(),
                deliverables: vec!["event inbox".to_owned()],
                acceptance: vec!["master poll reads state without mutating".to_owned()],
                priority: 95,
                target_cwd: None,
                execution_profile: "workspace".to_owned(),
                session_id: Some(SessionId::new("runtime-phase2b-session")),
                turn_id: Some(turn_id.clone()),
                dispatch: Some(UiTaskDispatchCommand::None),
            },
        },
        UiCommand::AssignTask {
            assignment: UiTaskAssignCommand {
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
            },
        },
        UiCommand::ClaimNextTask {
            claim: UiTaskClaimCommand {
                agent_id: worker_id.clone(),
                execution_id: execution_id.clone(),
                ttl_seconds: Some(300),
            },
        },
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id.clone()),
                kind: UiExecutionFactKind::Running {
                    phase: "phase2b_running".to_owned(),
                    summary: "worker running".to_owned(),
                    evidence: vec!["running evidence".to_owned()],
                },
            },
        },
    ] {
        runtime
            .dispatch(build_command_dispatch_envelope(&command).expect("phase2b envelope"))
            .expect("phase2b dispatch");
    }

    thread::sleep(Duration::from_secs(2));
    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::RunSchedulerTick {
                tick: UiSchedulerTickCommand {
                    stale_after_seconds: 1,
                    soft_timeout_seconds: 10,
                    hard_timeout_seconds: 30,
                },
            })
            .expect("scheduler tick envelope"),
        )
        .expect("scheduler tick dispatch");
    for command in [
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id.clone()),
                kind: UiExecutionFactKind::Blocked {
                    reason: "needs master unblock".to_owned(),
                    evidence: vec!["blocked evidence".to_owned()],
                },
            },
        },
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id.clone()),
                kind: UiExecutionFactKind::Recovering {
                    summary: "worker recovered".to_owned(),
                    evidence: vec!["recovery evidence".to_owned()],
                    retry_count: 1,
                },
            },
        },
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id),
                kind: UiExecutionFactKind::ReviewReady {
                    summary: "ready for master review".to_owned(),
                    deliverables: vec!["phase2b deliverable".to_owned()],
                    evidence: vec!["review evidence".to_owned()],
                },
            },
        },
    ] {
        runtime
            .dispatch(build_command_dispatch_envelope(&command).expect("phase2b envelope"))
            .expect("phase2b dispatch");
    }

    let before_poll = runtime
        .query_runtime(&UiCommand::QueryTaskBoard {
            status: None,
            agent_id: None,
            include_terminal: true,
        })
        .expect("before poll board query")
        .expect("before poll board");
    let before_status = match before_poll {
        UiQueryResult::TaskBoard(board) => board
            .tasks
            .iter()
            .find(|task| task.task_id == task_id)
            .expect("task before poll")
            .status
            .clone(),
        other => panic!("unexpected before poll result: {other:?}"),
    };

    let inbox = runtime
        .query_runtime(&UiCommand::QueryEventInbox {
            after_cursor: None,
            limit: None,
        })
        .expect("event inbox query")
        .expect("event inbox result");
    let inbox_cursor = match inbox {
        UiQueryResult::EventInbox(inbox) => {
            assert!(
                inbox.events.len() > 100,
                "backlog regression must exceed old default page size"
            );
            let kinds = inbox
                .events
                .iter()
                .map(|event| event.kind.as_str())
                .collect::<Vec<_>>();
            assert!(
                kinds.contains(&"execution_blocked"),
                "missing blocked event: {kinds:?}"
            );
            assert!(
                kinds.contains(&"review_ready"),
                "missing review event: {kinds:?}"
            );
            assert!(
                kinds.contains(&"scheduler_tick"),
                "missing scheduler event: {kinds:?}"
            );
            inbox.next_cursor.expect("event inbox cursor")
        }
        other => panic!("unexpected event inbox result: {other:?}"),
    };

    let poll = runtime
        .query_runtime(&UiCommand::QueryMasterPoll {
            after_cursor: None,
            limit: None,
            include_terminal: true,
            replay_from_start: true,
        })
        .expect("master poll query")
        .expect("master poll result");
    let persisted_cursor = match poll {
        UiQueryResult::MasterPoll(poll) => {
            assert!(poll.task_board.include_terminal);
            assert_eq!(poll.next_cursor.as_deref(), Some(inbox_cursor.as_str()));
            assert_eq!(
                poll.persisted_cursor, None,
                "query route must not advance the master cursor"
            );
            let kinds = poll
                .classifications
                .iter()
                .map(|classification| classification.kind.as_str())
                .collect::<Vec<_>>();
            assert!(kinds.contains(&"blocked"), "missing blocked: {kinds:?}");
            assert!(
                kinds.contains(&"review_ready"),
                "missing review_ready: {kinds:?}"
            );
            assert!(kinds.contains(&"stale"), "missing stale: {kinds:?}");
            poll.next_cursor.expect("next cursor")
        }
        other => panic!("unexpected master poll result: {other:?}"),
    };

    let after_poll = runtime
        .query_runtime(&UiCommand::QueryTaskBoard {
            status: None,
            agent_id: None,
            include_terminal: true,
        })
        .expect("after poll board query")
        .expect("after poll board");
    match after_poll {
        UiQueryResult::TaskBoard(board) => {
            let task = board
                .tasks
                .iter()
                .find(|task| task.task_id == task_id)
                .expect("task after poll");
            assert_eq!(task.status, before_status);
            assert_eq!(task.status, "review_submitted");
            assert_eq!(
                task.active_execution_id.as_deref(),
                Some(execution_id.as_str())
            );
        }
        other => panic!("unexpected after poll result: {other:?}"),
    }

    let receipt = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::RunMasterPoll {
                after_cursor: None,
                limit: None,
                include_terminal: true,
                replay_from_start: true,
            })
            .expect("master poll envelope"),
        )
        .expect("master poll receipt");
    assert!(receipt.dispatch_status.starts_with("master_poll_recorded:"));

    let recovered = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("recovered runtime");
    let recovered_poll = recovered
        .query_runtime(&UiCommand::QueryMasterPoll {
            after_cursor: None,
            limit: None,
            include_terminal: true,
            replay_from_start: false,
        })
        .expect("recovered master poll query")
        .expect("recovered master poll");
    match recovered_poll {
        UiQueryResult::MasterPoll(poll) => {
            assert_eq!(
                poll.source_cursor.as_deref(),
                Some(persisted_cursor.as_str())
            );
            assert_eq!(
                poll.persisted_cursor.as_deref(),
                Some(persisted_cursor.as_str())
            );
            assert!(poll.event_inbox.events.is_empty());
        }
        other => panic!("unexpected recovered master poll result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_dispatches_worker_control_to_task_owner() {
    let runtime_home = temp_runtime_home();
    let worker_id = AgentId::new("runtime-phase2c-worker");
    let task_id = "runtime-phase2c-task".to_owned();
    let execution_id = "runtime-phase2c-exec".to_owned();
    let turn_id = TurnId::new("runtime-phase2c-turn");
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");

    for command in [
        UiCommand::CreateTaskAgent {
            agent: UiTaskAgentCreateCommand {
                agent_id: worker_id.clone(),
                capabilities: vec!["code_edit".to_owned(), "test_run".to_owned()],
            },
        },
        UiCommand::CreateTask {
            task: UiTaskCreateCommand {
                task_id: Some(task_id.clone()),
                title: "Runtime phase2c task".to_owned(),
                content: "Runtime phase2c content".to_owned(),
                goal: "prove worker control bridge".to_owned(),
                deliverables: vec!["worker control".to_owned()],
                acceptance: vec!["control events persist".to_owned()],
                priority: 97,
                target_cwd: None,
                execution_profile: "workspace".to_owned(),
                session_id: Some(SessionId::new("runtime-phase2c-session")),
                turn_id: Some(turn_id.clone()),
                dispatch: Some(UiTaskDispatchCommand::None),
            },
        },
        UiCommand::AssignTask {
            assignment: UiTaskAssignCommand {
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
            },
        },
        UiCommand::ClaimNextTask {
            claim: UiTaskClaimCommand {
                agent_id: worker_id.clone(),
                execution_id: execution_id.clone(),
                ttl_seconds: Some(300),
            },
        },
        UiCommand::ApplyExecutionFact {
            fact: UiExecutionFactCommand {
                execution_id: execution_id.clone(),
                task_id: task_id.clone(),
                agent_id: worker_id.clone(),
                turn_id: Some(turn_id),
                kind: UiExecutionFactKind::Running {
                    phase: "phase2c_running".to_owned(),
                    summary: "worker running before safe-point control".to_owned(),
                    evidence: vec!["running evidence".to_owned()],
                },
            },
        },
    ] {
        runtime
            .dispatch(build_command_dispatch_envelope(&command).expect("phase2c setup envelope"))
            .expect("phase2c setup dispatch");
    }

    let controls = [
        (
            "cli-phase2c-query",
            "wctl-phase2c-query",
            "query_status",
            None,
            None,
        ),
        (
            "cli-phase2c-ask",
            "wctl-phase2c-ask",
            "ask_at_safe_point",
            Some("what is blocking the execution?".to_owned()),
            None,
        ),
        (
            "cli-phase2c-constraint",
            "wctl-phase2c-constraint",
            "add_constraint",
            None,
            Some("do not leave the task without a checkpoint".to_owned()),
        ),
        (
            "cli-phase2c-pause",
            "wctl-phase2c-pause",
            "pause",
            None,
            None,
        ),
        (
            "cli-phase2c-resume",
            "wctl-phase2c-resume",
            "resume",
            None,
            None,
        ),
        (
            "cli-phase2c-cancel",
            "wctl-phase2c-cancel",
            "cancel",
            None,
            None,
        ),
    ];
    for (_request_id, control_id, op, question, constraint) in controls {
        let receipt = runtime
            .dispatch(
                build_command_dispatch_envelope(&UiCommand::WorkerControl {
                    control: UiWorkerControlCommand {
                        control_id: Some(control_id.to_owned()),
                        task_id: task_id.clone(),
                        execution_id: execution_id.clone(),
                        agent_id: worker_id.clone(),
                        op: op.to_owned(),
                        question,
                        constraint,
                        note: Some("runtime phase2c proof".to_owned()),
                    },
                })
                .expect("phase2c worker control envelope"),
            )
            .expect("phase2c worker control dispatch");
        assert!(
            receipt
                .dispatch_status
                .starts_with(&format!("worker_control_applied:{op}:{control_id}:")),
            "unexpected receipt {}",
            receipt.dispatch_status
        );
    }

    let control_query = runtime
        .query_runtime(&UiCommand::QueryWorkerControl {
            task_id: task_id.clone(),
            execution_id: execution_id.clone(),
        })
        .expect("worker control query")
        .expect("worker control result");
    match control_query {
        UiQueryResult::WorkerControl(projection) => {
            assert_eq!(projection.source_agent_id, AgentId::new("agent-live"));
            assert_eq!(projection.events.len(), 6);
            let ids = projection
                .events
                .iter()
                .map(|event| event.control_id.as_str())
                .collect::<Vec<_>>();
            for required in [
                "wctl-phase2c-query",
                "wctl-phase2c-ask",
                "wctl-phase2c-constraint",
                "wctl-phase2c-pause",
                "wctl-phase2c-resume",
                "wctl-phase2c-cancel",
            ] {
                assert!(ids.contains(&required), "missing {required}: {ids:?}");
            }
            assert_eq!(
                projection.event.as_ref().map(|event| event.op.as_str()),
                Some("cancel")
            );
        }
        other => panic!("unexpected worker control result: {other:?}"),
    }

    let board = runtime
        .query_runtime(&UiCommand::QueryTaskBoard {
            status: None,
            agent_id: None,
            include_terminal: true,
        })
        .expect("phase2c board query")
        .expect("phase2c board result");
    match board {
        UiQueryResult::TaskBoard(board) => {
            let task = board
                .tasks
                .iter()
                .find(|task| task.task_id == task_id)
                .expect("phase2c task");
            assert_eq!(task.status, "cancelled");
            assert_eq!(task.active_execution_id.as_deref(), None);
        }
        other => panic!("unexpected phase2c board result: {other:?}"),
    }

    let history = runtime
        .query_runtime(&UiCommand::QueryTaskHistory {
            task_id: task_id.clone(),
        })
        .expect("phase2c history query")
        .expect("phase2c history result");
    match history {
        UiQueryResult::TaskHistory(history) => {
            let event_types = history
                .events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>();
            for required in ["TaskPaused", "TaskResumed", "TaskCancelled"] {
                assert!(
                    event_types.contains(&required),
                    "missing {required}: {event_types:?}"
                );
            }
        }
        other => panic!("unexpected phase2c history result: {other:?}"),
    }

    let recovered = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("recovered runtime");
    let recovered_control = recovered
        .query_runtime(&UiCommand::QueryWorkerControl {
            task_id: task_id.clone(),
            execution_id: execution_id.clone(),
        })
        .expect("recovered worker control query")
        .expect("recovered worker control result");
    match recovered_control {
        UiQueryResult::WorkerControl(projection) => {
            assert_eq!(projection.events.len(), 6);
            assert!(projection.events.iter().any(|event| {
                event.control_id == "wctl-phase2c-cancel" && event.op == "cancel"
            }));
        }
        other => panic!("unexpected recovered phase2c result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_worker_control_invalid_target_returns_explicit_failure() {
    let runtime_home = temp_runtime_home();
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");

    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::WorkerControl {
                control: UiWorkerControlCommand {
                    control_id: Some("wctl-phase2c-missing".to_owned()),
                    task_id: "missing-phase2c-task".to_owned(),
                    execution_id: "missing-phase2c-exec".to_owned(),
                    agent_id: AgentId::new("missing-phase2c-worker"),
                    op: "query_status".to_owned(),
                    question: None,
                    constraint: None,
                    note: None,
                },
            })
            .expect("worker control envelope"),
        )
        .expect_err("missing target must fail");
    assert_eq!(
        err,
        UiCommandDispatchPortError::TargetNotFound("missing-phase2c-task".to_owned())
    );

    let query_err = runtime
        .query_runtime(&UiCommand::QueryWorkerControl {
            task_id: "missing-phase2c-task".to_owned(),
            execution_id: "missing-phase2c-exec".to_owned(),
        })
        .expect_err("missing worker-control query must fail");
    assert_eq!(
        query_err,
        UiCommandDispatchPortError::TargetNotFound("missing-phase2c-task".to_owned())
    );

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_query_reads_error_center_metadata_without_raw_text() {
    let runtime_home = temp_runtime_home();
    let session_id = SessionId::new("runtime-session-agent-live");
    let trace_id = TraceId::new("runtime-trace-error-query");
    let turn_id = TurnId::new("runtime-turn-error-query");
    let ledger_path = metadata_ledger_path(&runtime_home, &AgentId::new("agent-live"), &session_id);
    let mut center = MetadataCenter::with_ledger_path(&ledger_path).expect("metadata center");
    center
        .write(
            MetadataEnvelope::new(
                MetadataId::new("error.center:runtime-trace-error-query:schema"),
                MetadataKind::RuntimeState,
                MetadataWriteOwner {
                    feature_id: FeatureId::new("error.center"),
                    crate_name: "freehand-control".to_owned(),
                    module_path: "freehand_control".to_owned(),
                    symbol_path: "classify_error_center_failure".to_owned(),
                },
                MetadataWriteNode {
                    pipeline_node: "ReasonResp04CompletionSchemaRejected".to_owned(),
                    runtime_node_id: None,
                },
                MetadataSubject {
                    agent_id: Some(AgentId::new("agent-live")),
                    session_id: Some(session_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    trace_id: trace_id.clone(),
                },
                vec![
                    MetadataEntry {
                        key: "error.domain".to_owned(),
                        value: json!("schema"),
                    },
                    MetadataEntry {
                        key: "error.class".to_owned(),
                        value: json!("validation"),
                    },
                    MetadataEntry {
                        key: "error.code".to_owned(),
                        value: json!("completion_schema_rejected"),
                    },
                    MetadataEntry {
                        key: "error.source_owner".to_owned(),
                        value: json!("provider.reason-live-bridge"),
                    },
                    MetadataEntry {
                        key: "error.source_pipeline_node".to_owned(),
                        value: json!("ReasonResp04CompletionSchemaRejected"),
                    },
                    MetadataEntry {
                        key: "error.recovery_action".to_owned(),
                        value: json!("repair_schema"),
                    },
                    MetadataEntry {
                        key: "error.retry_index".to_owned(),
                        value: json!(1),
                    },
                    MetadataEntry {
                        key: "error.retry_cap".to_owned(),
                        value: json!(2),
                    },
                    MetadataEntry {
                        key: "error.public_visibility".to_owned(),
                        value: json!("internal"),
                    },
                    MetadataEntry {
                        key: "error.owner_target".to_owned(),
                        value: json!("reason.turn"),
                    },
                    MetadataEntry {
                        key: "error.repair_fields".to_owned(),
                        value: json!(["summary"]),
                    },
                    MetadataEntry {
                        key: "error.raw_hash".to_owned(),
                        value: json!("hash-only"),
                    },
                ],
            )
            .expect("error center envelope"),
        )
        .expect("write error center metadata");
    center
        .write(
            MetadataEnvelope::new(
                MetadataId::new("control.center:runtime-trace-error-query:ignored"),
                MetadataKind::RuntimeState,
                MetadataWriteOwner {
                    feature_id: FeatureId::new("control.center"),
                    crate_name: "freehand-control".to_owned(),
                    module_path: "freehand_control".to_owned(),
                    symbol_path: "control_status_rhythm_decision".to_owned(),
                },
                MetadataWriteNode {
                    pipeline_node: "ControlHook03AfterModelResponse".to_owned(),
                    runtime_node_id: None,
                },
                MetadataSubject {
                    agent_id: Some(AgentId::new("agent-live")),
                    session_id: Some(session_id.clone()),
                    turn_id: Some(turn_id.clone()),
                    trace_id: trace_id.clone(),
                },
                vec![MetadataEntry {
                    key: "control.hook".to_owned(),
                    value: json!("ControlHook03AfterModelResponse"),
                }],
            )
            .expect("control envelope"),
        )
        .expect("write control metadata");

    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");

    let result = runtime
        .query_runtime(&UiCommand::QueryErrorCenterEvents {
            session_id: session_id.clone(),
            trace_id: Some(trace_id.as_str().to_owned()),
            turn_id: Some(turn_id.clone()),
            domain: Some("schema".to_owned()),
        })
        .expect("error center query")
        .expect("runtime-backed error center result");
    match result {
        UiQueryResult::ErrorCenterEvents(list) => {
            assert_eq!(list.session_id, session_id);
            assert_eq!(list.events.len(), 1);
            let event = &list.events[0];
            assert_eq!(event.domain, "schema");
            assert_eq!(event.class, "validation");
            assert_eq!(event.recovery_action, "repair_schema");
            assert_eq!(event.raw_hash, "hash-only");
            assert_eq!(event.repair_fields, vec!["summary".to_owned()]);
            assert!(
                !serde_json::to_string(event)
                    .expect("json")
                    .contains("raw provider body")
            );
        }
        other => panic!("unexpected error center result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_task_tool_mutation_publishes_task_list_projection() {
    let runtime_home = temp_runtime_home();
    let (base_url, rx, handle) = spawn_sequence_server(
        "application/json",
        vec![
            tool_use_named_response(
                "toolu_task_create_1",
                "task",
                json!({
                    "op":"create",
                    "task_id":"runtime-push-task-1",
                    "title":"Runtime push task",
                    "content":"Task list push content",
                    "goal":"Publish task projection",
                    "deliverables":["task projection"],
                    "acceptance":["subscriber sees task"],
                    "dispatch":{"mode":"none"},
                    "priority":77
                }),
            ),
            waiting_single_response("await Worker pickup for runtime-push-task-1"),
        ],
    );
    let request = LiveReasonTurnRequest {
        runtime_home: runtime_home.clone(),
        session_id: SessionId::new("runtime-task-push-session"),
        turn_id: TurnId::new("runtime-turn-task-push-1"),
        trace_id: TraceId::new("runtime-trace-task-push-1"),
        prompt: "create a task".to_owned(),
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: None,
        execution_profile: LiveReasonExecutionProfile::Workspace,
        stream: false,
        cancel_token: None,
    };
    let mut task_projections = Vec::<UiTaskListProjection>::new();

    let outcome = run_live_reason_turn_with_hooks(
        &live_selected_agent(base_url, freehand_config::ProviderType::Anthropic),
        request,
        |_| {},
        |_| {},
        |projection| task_projections.push(projection.clone()),
    )
    .expect("live bridge");
    let _ = rx.recv().expect("first provider request");
    let _ = rx.recv().expect("second provider request");
    handle.join().expect("join provider");

    assert_eq!(outcome.tool_executions, 1);
    assert_eq!(task_projections.len(), 1);
    assert_eq!(task_projections[0].tasks.len(), 1);
    assert_eq!(task_projections[0].tasks[0].task_id, "runtime-push-task-1");
    assert_eq!(task_projections[0].tasks[0].status, "waiting_agent");
    assert_eq!(task_projections[0].tasks[0].priority, 77);

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn bootstrap_with_corrupt_checkpoint_ledger_fails_explicitly() {
    let runtime_home = temp_runtime_home();
    let session_id = SessionId::new("runtime-session-agent-live");
    let ledger_dir = runtime_home
        .join("ledgers")
        .join("checkpoints")
        .join("agent-live");
    fs::create_dir_all(&ledger_dir).expect("create ledger dir");
    fs::write(
        ledger_dir.join(format!("{}.jsonl", session_id.as_str())),
        "{not-json}\n",
    )
    .expect("write corrupt ledger");

    let err = match RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home,
        false,
    ) {
        Ok(_) => panic!("bootstrap must fail"),
        Err(err) => err,
    };
    match err {
        RuntimeCommandDispatcherError::CheckpointProjectionBootstrap(message) => {
            assert!(message.contains("checkpoint ledger line 1 failed to parse"));
        }
        other => panic!("unexpected bootstrap error: {other:?}"),
    }
}

#[test]
fn resume_turn_is_explicitly_unsupported() {
    let runtime = runtime();
    let err = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::ResumeTurn {
                turn_id: TurnId::new("runtime-turn-1"),
            })
            .expect("envelope"),
        )
        .expect_err("resume should fail");
    assert_eq!(
        err,
        UiCommandDispatchPortError::Unsupported(
            "resume dispatch for `runtime-turn-1` is not implemented".to_owned()
        )
    );
}

#[test]
fn bootstrap_from_selected_master_agent_uses_selected_runtime_truth() {
    let runtime =
        RuntimeCommandDispatcher::from_selected_agent(&selected_master_agent()).expect("runtime");

    let ui_state = runtime.ui_state();
    let node_status = ui_state
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QueryNodeStatus {
            node_id: "worker-node".to_owned(),
        })
        .expect("query");
    match node_status {
        UiQueryResult::NodeStatus(Some(snapshot)) => {
            assert_eq!(snapshot.node_id, "worker-node");
            assert_eq!(snapshot.pairing_state, "paired");
        }
        other => panic!("unexpected node status query: {other:?}"),
    }
}

#[test]
fn bootstrap_from_selected_live_agent_wires_node_metadata_into_shared_ledger() {
    let runtime_home = temp_runtime_home();
    let _runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    )
    .expect("runtime");

    let metadata_path = metadata_ledger_path(
        &runtime_home,
        &AgentId::new("agent-live"),
        &SessionId::new("runtime-session-agent-live"),
    );
    let raw = fs::read_to_string(&metadata_path).expect("read metadata ledger");
    let records: Vec<MetadataEnvelope> = raw
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("decode metadata"))
        .collect();

    assert!(records.iter().any(|record| {
        record.owner.feature_id == FeatureId::new("node.master-slave")
            && record.write_node.pipeline_node == "NodeReq01BootstrapListening"
    }));
    assert!(records.iter().any(|record| {
        record.owner.feature_id == FeatureId::new("node.master-slave")
            && record.write_node.pipeline_node == "NodeReq02PairingAccepted"
    }));
    assert!(!raw.contains("pair-token"));

    let _ = fs::remove_dir_all(&runtime_home);
}

#[test]
fn bootstrap_rejects_unwritable_node_metadata_ledger_explicitly() {
    let runtime_home = temp_runtime_home();
    let metadata_path = metadata_ledger_path(
        &runtime_home,
        &AgentId::new("agent-live"),
        &SessionId::new("runtime-session-agent-live"),
    );
    fs::create_dir_all(&metadata_path).expect("poison metadata path as directory");

    let err = match RuntimeCommandDispatcher::from_selected_agent_with_live(
        &live_selected_agent(
            "http://127.0.0.1:1".to_owned(),
            freehand_config::ProviderType::Anthropic,
        ),
        runtime_home.clone(),
        false,
    ) {
        Ok(_) => panic!("bootstrap must fail"),
        Err(err) => err,
    };

    match err {
        RuntimeCommandDispatcherError::NodeRuntimeInit(message) => {
            assert!(message.contains("metadata write failed"));
            assert!(message.contains("metadata ledger io failed"));
        }
        other => panic!("unexpected bootstrap error: {other:?}"),
    }

    let _ = fs::remove_dir_all(&runtime_home);
}

#[test]
fn bootstrap_rejects_slave_mode_agent_without_master_peer() {
    let mut selected = selected_master_agent();
    selected.mode = AgentMode::Slave;
    let err = match RuntimeCommandDispatcher::from_selected_agent(&selected) {
        Ok(_) => panic!("slave-mode agent without a master peer must be rejected"),
        Err(err) => err,
    };
    assert_eq!(
        err,
        RuntimeCommandDispatcherError::HostRequiresMasterPeer {
            agent_name: "master".to_owned(),
        }
    );
}

#[test]
fn worker_selected_dispatcher_uses_worker_identity_and_rejects_master_only_command() {
    let mut selected = selected_master_agent();
    selected.name = "worker".to_owned();
    selected.mode = AgentMode::Slave;
    selected.node_id = "worker-node".to_owned();
    selected.paired_agents = vec![SelectedPeerAgentConfig {
        name: "master".to_owned(),
        mode: AgentMode::Master,
        node_id: "master-node".to_owned(),
        allowed_pair_ip: None,
        pair_token_env: "FREEHAND_PAIR_TOKEN_MASTER".to_owned(),
        provider_id: "provider-master".to_owned(),
        fallback_provider_id: None,
        model_group_id: None,
    }];
    let runtime = RuntimeCommandDispatcher::from_selected_agent(&selected)
        .expect("worker-selected dispatcher");
    runtime
        .state
        .lock()
        .expect("lock runtime state")
        .active_turns
        .push(ActiveRuntimeTurn {
            turn_id: TurnId::new("runtime-turn-owner-status"),
            session_id: SessionId::new("runtime-session-worker"),
            cwd: std::env::temp_dir(),
            trace_id: TraceId::new("runtime-trace-owner-status"),
            user_text: "owner status probe".to_owned(),
            cancel_token: Arc::new(AtomicBool::new(false)),
        });
    assert_eq!(
        runtime.current_agent_activity(),
        RuntimeAgentActivityProjection {
            status: RuntimeAgentActivityStatus::Running,
            active_session_count: 1,
        }
    );

    runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SubmitUserInput {
                text: "worker-owned input".to_owned(),
                session_id: None,
                cwd: None,
                metadata: None,
            })
            .expect("submit envelope"),
        )
        .expect("worker local turn");
    let latest = runtime
        .ui_state()
        .lock()
        .expect("lock ui state")
        .query(&UiCommand::QueryLatestActiveTurn)
        .expect("latest worker turn");
    match latest {
        UiQueryResult::Turn(Some(turn)) => {
            assert_eq!(turn.source.source_agent_id, AgentId::new("worker"));
            assert_eq!(turn.source.source_node_id, "worker-node");
        }
        other => panic!("unexpected worker projection: {other:?}"),
    }

    let error = runtime
        .dispatch(
            build_command_dispatch_envelope(&UiCommand::SendDirectMessageToSlave {
                node_id: "worker-node".to_owned(),
                text: "must not route as Master".to_owned(),
            })
            .expect("direct-message envelope"),
        )
        .expect_err("Worker host must reject Master-only direct messaging");
    assert!(
        matches!(error, UiCommandDispatchPortError::Unsupported(message) if message.contains("Worker host"))
    );
}

#[test]
fn worker_selected_task_queries_route_to_master_owner_truth() {
    let runtime_home = temp_runtime_home();
    let master_runtime =
        TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("master task runtime");
    let task_id = TaskId::new("worker-query-task");
    master_runtime
        .create_task(TaskCreateRequest {
            task_id: Some(task_id.clone()),
            title: "worker query task".to_owned(),
            content: "Worker-selected TaskBoard must read paired Master truth".to_owned(),
            goal: "Route Worker task queries to the Task Center owner".to_owned(),
            deliverables: vec!["master-owned task projection".to_owned()],
            acceptance: vec!["done".to_owned()],
            priority: 10,
            target_cwd: Some(runtime_home.to_string_lossy().into_owned()),
            execution_profile: TaskExecutionProfile::Workspace,
            dispatch: TaskDispatchRequest::SelfAgent,
            parent: TaskParentRef {
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            actor: TaskActor {
                agent_id: AgentId::new("master"),
                source: "worker_selected_task_queries_route_to_master_owner_truth".to_owned(),
                session_id: None,
                turn_id: None,
                trace_id: None,
            },
            watermark: TaskWatermark {
                metadata_id: None,
                hook: Some("worker_selected_task_queries_route_to_master_owner_truth".to_owned()),
                action_tool_call_id: None,
            },
        })
        .expect("seed master task truth");

    let mut selected = selected_master_agent();
    selected.name = "worker".to_owned();
    selected.mode = AgentMode::Slave;
    selected.node_id = "worker-node".to_owned();
    selected.paired_agents = vec![SelectedPeerAgentConfig {
        name: "master".to_owned(),
        mode: AgentMode::Master,
        node_id: "master-node".to_owned(),
        allowed_pair_ip: None,
        pair_token_env: "FREEHAND_PAIR_TOKEN_MASTER".to_owned(),
        provider_id: "provider-master".to_owned(),
        fallback_provider_id: None,
        model_group_id: None,
    }];
    let runtime = RuntimeCommandDispatcher::from_selected_agent_with_live(
        &selected,
        runtime_home.clone(),
        false,
    )
    .expect("worker-selected runtime");

    let task_board = runtime
        .query_runtime(&UiCommand::QueryTaskBoard {
            status: None,
            agent_id: None,
            include_terminal: false,
        })
        .expect("task board query")
        .expect("task board projection");
    match task_board {
        UiQueryResult::TaskBoard(board) => {
            assert_eq!(board.source_agent_id.as_str(), "master");
            assert!(
                board
                    .tasks
                    .iter()
                    .any(|task| task.task_id == "worker-query-task")
            );
        }
        other => panic!("unexpected task board result: {other:?}"),
    }

    let task_list = runtime
        .query_runtime(&UiCommand::QueryTaskList {
            status: None,
            agent_id: None,
        })
        .expect("task list query")
        .expect("task list projection");
    match task_list {
        UiQueryResult::TaskList(list) => {
            assert_eq!(list.source_agent_id.as_str(), "master");
            assert!(
                list.tasks
                    .iter()
                    .any(|task| task.task_id == "worker-query-task")
            );
        }
        other => panic!("unexpected task list result: {other:?}"),
    }

    fs::remove_dir_all(runtime_home).expect("cleanup runtime home");
}

#[test]
fn runtime_agent_activity_merge_preserves_active_truth_and_saturates_count() {
    let merged = RuntimeAgentActivityProjection {
        status: RuntimeAgentActivityStatus::Error,
        active_session_count: u32::MAX,
    }
    .merge(RuntimeAgentActivityProjection {
        status: RuntimeAgentActivityStatus::Running,
        active_session_count: 1,
    });

    assert_eq!(
        merged,
        RuntimeAgentActivityProjection {
            status: RuntimeAgentActivityStatus::Running,
            active_session_count: u32::MAX,
        }
    );
    assert_eq!(
        RuntimeAgentActivityProjection {
            status: RuntimeAgentActivityStatus::Idle,
            active_session_count: 0,
        }
        .merge(RuntimeAgentActivityProjection {
            status: RuntimeAgentActivityStatus::Waiting,
            active_session_count: 0,
        })
        .status,
        RuntimeAgentActivityStatus::Waiting
    );
}
