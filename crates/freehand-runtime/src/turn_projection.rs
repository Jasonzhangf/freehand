//! Runtime turn/session projection and recovery helpers.

use super::*;
use freehand_reason::ActiveTurnSnapshot;

#[cfg(test)]
pub(crate) fn publish_live_pending_user_projection(
    ui_state: &Arc<Mutex<UiProtocolState>>,
    reason_agent_id: &AgentId,
    master_node_id: &str,
    session_id: &SessionId,
    cwd: &Path,
    base_turn_id: &TurnId,
    user_text: &str,
) {
    ui_state
        .lock()
        .expect("lock ui state")
        .apply_turn_projection(turn_projection_for_client(
            turn_projection_from_events(TurnProjectionInput {
                source_agent_id: reason_agent_id.clone(),
                source_node_id: master_node_id.to_owned(),
                session_id: session_id.clone(),
                turn_id: derived_turn_id(base_turn_id, 1),
                created_at: Some(now_unix_seconds()),
                timing: None,
                cwd: Some(cwd.to_string_lossy().into_owned()),
                user_text: ui_user_text_projection_for_session_user_text(session_id, user_text),
                semantic_events: Vec::new(),
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                usage_events: Vec::new(),
                terminal_event: None,
                error_events: Vec::new(),
                slave_substream_card: false,
            }),
            UiClientKind::WebUi,
        ));
}

pub(crate) fn publish_live_cancelled_projection(
    ui_state: &Arc<Mutex<UiProtocolState>>,
    reason_agent_id: &AgentId,
    master_node_id: &str,
    active: &ActiveRuntimeTurn,
) {
    ui_state
        .lock()
        .expect("lock ui state")
        .apply_turn_projection(turn_projection_for_client(
            turn_projection_from_events(TurnProjectionInput {
                source_agent_id: reason_agent_id.clone(),
                source_node_id: master_node_id.to_owned(),
                session_id: active.session_id.clone(),
                turn_id: active.turn_id.clone(),
                created_at: Some(now_unix_seconds()),
                timing: None,
                cwd: Some(active.cwd.to_string_lossy().into_owned()),
                user_text: ui_user_text_projection_for_session_user_text(
                    &active.session_id,
                    &active.user_text,
                ),
                semantic_events: Vec::new(),
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                usage_events: Vec::new(),
                terminal_event: Some(freehand_contracts::ReasonResp03TerminalEvent {
                    session_id: active.session_id.clone(),
                    turn_id: active.turn_id.clone(),
                    trace_id: active.trace_id.clone(),
                    feature_id: FeatureId::new("runtime.ui-command-dispatch"),
                    agent_id: reason_agent_id.clone(),
                    status: freehand_contracts::TerminalStatus::Cancelled,
                    summary: "cancelled by ui command".to_owned(),
                    user_options: None,
                }),
                error_events: Vec::new(),
                slave_substream_card: false,
            }),
            UiClientKind::WebUi,
        ));
}

pub(crate) fn project_runtime_turn_history(
    reason_agent_id: &AgentId,
    master_node_id: &str,
    turns: &[TurnRecord],
    cwd: Option<String>,
) -> UiTurnProjection {
    let turn = turns
        .last()
        .expect("runtime turn history projection requires at least one turn");
    let mut projection = turn_projection_from_events(TurnProjectionInput {
        source_agent_id: reason_agent_id.clone(),
        source_node_id: master_node_id.to_owned(),
        session_id: turn.request.session_id.clone(),
        turn_id: turn.request.turn_id.clone(),
        created_at: (turn.created_at != 0).then_some(turn.created_at),
        timing: ui_turn_timing_from_turn(turn),
        cwd: cwd.or_else(|| turn.cwd.clone()),
        user_text: ui_user_text_projection_for_turn(turn),
        semantic_events: turn.semantic_events.clone(),
        tool_calls: turn.tool_calls.clone(),
        tool_results: turn.tool_results.clone(),
        usage_events: turn.usage_events.clone(),
        terminal_event: turn.terminal_event.clone(),
        error_events: turn.error_events.clone(),
        slave_substream_card: false,
    });
    projection.attachments = turn
        .attachments
        .iter()
        .map(|attachment| UiAttachmentMetadataProjection {
            attachment_id: attachment.attachment_id.clone(),
            kind: match attachment.kind {
                InputAttachmentKind::Image => UiInputAttachmentKind::Image,
            },
            media_type: attachment.media_type.clone(),
            name: attachment.name.clone(),
            size_bytes: attachment.size_bytes,
        })
        .collect();
    turn_projection_for_client(projection, UiClientKind::WebUi)
}

fn ui_turn_timing_from_turn(turn: &TurnRecord) -> Option<UiTurnTimingProjection> {
    if turn.timing.is_empty() {
        return None;
    }
    Some(UiTurnTimingProjection {
        turn_started_at_ms: turn.timing.turn_started_at_ms,
        first_response_at_ms: turn.timing.first_response_at_ms,
        completed_at_ms: turn.timing.completed_at_ms,
        time_to_first_response_ms: turn.timing.time_to_first_response_ms,
        total_elapsed_ms: turn.timing.total_elapsed_ms,
    })
}

pub(crate) fn current_runtime_turn_for_projection(
    turns: &[TurnRecord],
    base_turn_id: &TurnId,
) -> Result<TurnRecord, UiCommandDispatchPortError> {
    let target_ordinal = runtime_turn_position(base_turn_id).0;
    turns
        .iter()
        .filter(|turn| runtime_turn_position(&turn.request.turn_id).0 == target_ordinal)
        .max_by_key(|turn| runtime_turn_position(&turn.request.turn_id))
        .cloned()
        .ok_or_else(|| {
            UiCommandDispatchPortError::DispatchFailed(format!(
                "failed to project live error turn `{}` from persistence",
                base_turn_id.as_str()
            ))
        })
}

pub(crate) fn restore_or_materialize_failed_live_submit(
    state: &mut RuntimeCommandDispatcherState,
    prepared: &PreparedLiveSubmit,
    failure_message: &str,
) -> Result<TurnRecord, UiCommandDispatchPortError> {
    let persistence = ReasonPersistence::new(
        prepared.live.runtime_home.clone(),
        state.config.reason_agent_id.clone(),
    );
    match restore_live_submit_turns_for_projection(&persistence, &prepared.session_id) {
        Ok(mut restored_turns) => {
            let needs_terminal_materialization = restored_turns
                .iter()
                .find(|turn| turn.request.turn_id == prepared.turn_id)
                .is_none_or(|turn| {
                    !turn.terminal_event.as_ref().is_some_and(|terminal| {
                        terminal.status == freehand_contracts::TerminalStatus::Failed
                    })
                });
            if needs_terminal_materialization {
                let failed_turn = materialize_dispatch_failed_turn(
                    &persistence,
                    prepared,
                    state.config.model.clone(),
                    failure_message,
                )?;
                upsert_restored_turn(&mut restored_turns, failed_turn);
            }
            state
                .turns
                .retain(|turn| turn.request.session_id != prepared.session_id);
            state.turns.extend(restored_turns);
        }
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => {
            let failed_turn = materialize_dispatch_failed_turn(
                &persistence,
                prepared,
                state.config.model.clone(),
                failure_message,
            )?;
            state.turns.push(failed_turn);
        }
        Err(err) => {
            return Err(UiCommandDispatchPortError::DispatchFailed(format!(
                "failed to project live error turn from persistence: {err}"
            )));
        }
    }
    state
        .turns
        .sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
    state.session_cwds = session_cwds_from_turns(&state.turns);
    current_runtime_turn_for_projection(&state.turns, &prepared.turn_id)
}

pub(crate) fn restore_or_materialize_cancelled_live_submit(
    state: &mut RuntimeCommandDispatcherState,
    prepared: &PreparedLiveSubmit,
    summary: &str,
) -> Result<TurnRecord, UiCommandDispatchPortError> {
    let persistence = ReasonPersistence::new(
        prepared.live.runtime_home.clone(),
        state.config.reason_agent_id.clone(),
    );
    match restore_live_submit_turns_for_projection(&persistence, &prepared.session_id) {
        Ok(mut restored_turns) => {
            let needs_terminal_materialization = restored_turns
                .iter()
                .find(|turn| turn.request.turn_id == prepared.turn_id)
                .is_none_or(|turn| {
                    !turn.terminal_event.as_ref().is_some_and(|terminal| {
                        terminal.status == freehand_contracts::TerminalStatus::Cancelled
                    })
                });
            if needs_terminal_materialization {
                let cancelled_turn = materialize_dispatch_cancelled_turn(
                    &persistence,
                    prepared,
                    state.config.model.clone(),
                    summary,
                )?;
                upsert_restored_turn(&mut restored_turns, cancelled_turn);
            }
            state
                .turns
                .retain(|turn| turn.request.session_id != prepared.session_id);
            state.turns.extend(restored_turns);
        }
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => {
            let cancelled_turn = materialize_dispatch_cancelled_turn(
                &persistence,
                prepared,
                state.config.model.clone(),
                summary,
            )?;
            state.turns.push(cancelled_turn);
        }
        Err(err) => {
            return Err(UiCommandDispatchPortError::DispatchFailed(format!(
                "failed to project cancelled live turn from persistence: {err}"
            )));
        }
    }
    state
        .turns
        .sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
    state.session_cwds = session_cwds_from_turns(&state.turns);
    current_runtime_turn_for_projection(&state.turns, &prepared.turn_id)
}

pub(crate) fn persist_prepared_live_submit_active_turn(
    state: &mut RuntimeCommandDispatcherState,
    prepared: &PreparedLiveSubmit,
) -> Result<TurnRecord, UiCommandDispatchPortError> {
    let persistence = ReasonPersistence::new(
        prepared.live.runtime_home.clone(),
        state.config.reason_agent_id.clone(),
    );
    let mut history = restore_or_create_session_history(&persistence, &prepared.session_id)?;
    let engine = ReasonTurnEngine::new();
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: prepared.session_id.clone(),
                turn_id: prepared.turn_id.clone(),
                trace_id: prepared.trace_id.clone(),
                feature_id: FeatureId::new("runtime.ui-command-dispatch"),
                agent_id: prepared.reason_agent_id.clone(),
                user_text: prepared.prompt.clone(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model: state.config.model.clone(),
            },
        )
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    turn.cwd = Some(prepared.cwd.to_string_lossy().into_owned());
    turn.attachments = prepared.attachment_metadata.clone();
    persistence
        .record_rewrite_state_updated(
            &history,
            Some(prepared.turn_id.clone()),
            Some(ActiveTurnSnapshot {
                turn: turn.clone(),
                schema_rejections: 0,
            }),
        )
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;

    let mut restored_turns =
        restore_live_submit_turns_for_projection(&persistence, &prepared.session_id).map_err(
            |err| {
                UiCommandDispatchPortError::DispatchFailed(format!(
                    "failed to restore prepared live turn from persistence: {err}"
                ))
            },
        )?;
    upsert_restored_turn(&mut restored_turns, turn);
    state
        .turns
        .retain(|existing| existing.request.session_id != prepared.session_id);
    state.turns.extend(restored_turns);
    state
        .turns
        .sort_by_key(|existing| runtime_turn_position(&existing.request.turn_id));
    state.session_cwds = session_cwds_from_turns(&state.turns);
    state
        .session_cwds
        .insert(prepared.session_id.clone(), prepared.cwd.clone());
    current_runtime_turn_for_projection(&state.turns, &prepared.turn_id)
}

fn restore_live_submit_turns_for_projection(
    persistence: &ReasonPersistence,
    session_id: &SessionId,
) -> Result<Vec<TurnRecord>, ReasonPersistenceError> {
    let restored = persistence.restore(session_id)?;
    let mut restored_turns = persistence.restore_turn_snapshots_for_ui(session_id)?;
    if let Some(active_turn) = restored.active_turn {
        restored_turns.push(active_turn.turn);
    }
    Ok(restored_turns)
}

fn restore_or_create_session_history(
    persistence: &ReasonPersistence,
    session_id: &SessionId,
) -> Result<SessionHistory, UiCommandDispatchPortError> {
    match persistence.restore(session_id) {
        Ok(restored) => Ok(restored.history),
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => {
            SessionHistory::new(session_id.clone(), Vec::new())
                .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))
        }
        Err(err) => Err(UiCommandDispatchPortError::DispatchFailed(format!(
            "failed to restore session history before live submit persistence: {err}"
        ))),
    }
}

fn upsert_restored_turn(turns: &mut Vec<TurnRecord>, turn: TurnRecord) {
    turns.retain(|existing| existing.request.turn_id != turn.request.turn_id);
    turns.push(turn);
}

fn materialize_dispatch_failed_turn(
    persistence: &ReasonPersistence,
    prepared: &PreparedLiveSubmit,
    model: String,
    failure_message: &str,
) -> Result<TurnRecord, UiCommandDispatchPortError> {
    let engine = ReasonTurnEngine::new();
    let (history, mut turn, schema_rejections, needs_start_persist) =
        restore_or_start_dispatch_terminal_turn(&engine, persistence, prepared, model)?;
    if needs_start_persist {
        persistence
            .record_turn_started(&history, &turn, schema_rejections)
            .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    }
    engine.fail_turn(&mut turn, failure_message.to_owned());
    persistence
        .record_turn_closed(&history, &turn, schema_rejections)
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    Ok(turn)
}

fn materialize_dispatch_cancelled_turn(
    persistence: &ReasonPersistence,
    prepared: &PreparedLiveSubmit,
    model: String,
    summary: &str,
) -> Result<TurnRecord, UiCommandDispatchPortError> {
    let engine = ReasonTurnEngine::new();
    let (history, mut turn, schema_rejections, needs_start_persist) =
        restore_or_start_dispatch_terminal_turn(&engine, persistence, prepared, model)?;
    if needs_start_persist {
        persistence
            .record_turn_started(&history, &turn, schema_rejections)
            .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    }
    engine.cancel_turn(&mut turn, summary.to_owned());
    persistence
        .record_turn_closed(&history, &turn, schema_rejections)
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    Ok(turn)
}

fn restore_or_start_dispatch_terminal_turn(
    engine: &ReasonTurnEngine,
    persistence: &ReasonPersistence,
    prepared: &PreparedLiveSubmit,
    model: String,
) -> Result<(SessionHistory, TurnRecord, u32, bool), UiCommandDispatchPortError> {
    match persistence.restore(&prepared.session_id) {
        Ok(restored) => {
            if let Some(snapshot) = restored
                .active_turn
                .filter(|snapshot| snapshot.turn.request.turn_id == prepared.turn_id)
            {
                return Ok((
                    restored.history,
                    snapshot.turn,
                    snapshot.schema_rejections,
                    false,
                ));
            }
            start_dispatch_terminal_turn(engine, restored.history, prepared, model)
        }
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => {
            let history = SessionHistory::new(prepared.session_id.clone(), Vec::new())
                .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
            start_dispatch_terminal_turn(engine, history, prepared, model)
        }
        Err(err) => Err(UiCommandDispatchPortError::DispatchFailed(format!(
            "failed to restore live submit before terminal materialization: {err}"
        ))),
    }
}

fn start_dispatch_terminal_turn(
    engine: &ReasonTurnEngine,
    mut history: SessionHistory,
    prepared: &PreparedLiveSubmit,
    model: String,
) -> Result<(SessionHistory, TurnRecord, u32, bool), UiCommandDispatchPortError> {
    let mut turn = engine
        .start_turn(
            &mut history,
            TurnStartInput {
                session_id: prepared.session_id.clone(),
                turn_id: prepared.turn_id.clone(),
                trace_id: prepared.trace_id.clone(),
                feature_id: FeatureId::new("runtime.ui-command-dispatch"),
                agent_id: prepared.reason_agent_id.clone(),
                user_text: prepared.prompt.clone(),
                planned_context_segments: Vec::new(),
                tool_schema_fingerprint: None,
                model,
            },
        )
        .map_err(|err| UiCommandDispatchPortError::DispatchFailed(err.to_string()))?;
    turn.cwd = Some(prepared.cwd.to_string_lossy().into_owned());
    Ok((history, turn, 0, true))
}

pub(crate) fn project_runtime_turn(
    reason_agent_id: &AgentId,
    master_node_id: &str,
    turn: &TurnRecord,
    cwd: Option<String>,
) -> UiTurnProjection {
    project_runtime_turn_history(
        reason_agent_id,
        master_node_id,
        std::slice::from_ref(turn),
        cwd,
    )
}

pub(crate) fn restore_all_persisted_sessions_into_ui(
    persistence: &ReasonPersistence,
    ui_state: &Arc<Mutex<UiProtocolState>>,
    reason_agent_id: &AgentId,
    master_node_id: &str,
) -> Result<u64, ReasonPersistenceError> {
    let sessions = persistence.list_persisted_sessions()?;
    let mut ui = ui_state.lock().expect("lock ui state");
    let mut max_turn_ordinal = 0_u64;
    for session in sessions {
        // Prefer UI restore so incomplete multi-round sessions backfill earlier
        // rounds from the reason ledger when available. Poisoned/incomplete
        // ledgers fall back to authoritative snapshots inside owner restore.
        // One historical session must not poison whole-daemon bootstrap.
        let mut turns = match persistence.restore_turn_snapshots_for_ui(&session.session_id) {
            Ok(turns) => turns,
            Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => continue,
            Err(
                ReasonPersistenceError::JsonParseFailed(_)
                | ReasonPersistenceError::InvalidCursorCoherence(_)
                | ReasonPersistenceError::InvalidLedgerCoherence(_)
                | ReasonPersistenceError::LedgerSequenceGap { .. },
            ) => continue,
            Err(error) => return Err(error),
        };
        turns.sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
        for turn in turns {
            max_turn_ordinal = max_turn_ordinal.max(runtime_turn_position(&turn.request.turn_id).0);
            ui.apply_turn_projection(project_runtime_turn_history(
                reason_agent_id,
                master_node_id,
                std::slice::from_ref(&turn),
                None,
            ));
        }
    }
    Ok(max_turn_ordinal)
}

pub(crate) fn ui_user_text_for_turn(turn: &TurnRecord) -> String {
    turn.request
        .context_segments
        .iter()
        .find(|segment| {
            segment.provenance.source == "freehand_runtime"
                && segment.provenance.reference.as_deref() == Some("original_task")
        })
        .and_then(|segment| {
            segment
                .content
                .strip_prefix("Original operator task:\n")
                .map(str::to_owned)
        })
        .unwrap_or_else(|| turn.request.user_text.clone())
}

fn ui_user_text_projection_for_turn(turn: &TurnRecord) -> Option<String> {
    if is_runtime_continuation_round(&turn.request.turn_id) {
        return None;
    }
    let user_text = ui_user_text_for_turn(turn);
    if ui_should_hide_user_text(&turn.request.session_id, &turn.request.user_text)
        || ui_should_hide_user_text(&turn.request.session_id, &user_text)
    {
        None
    } else {
        Some(user_text)
    }
}

fn ui_user_text_projection_for_session_user_text(
    session_id: &SessionId,
    user_text: &str,
) -> Option<String> {
    if ui_should_hide_user_text(session_id, user_text) {
        None
    } else {
        Some(user_text.to_owned())
    }
}

fn ui_should_hide_user_text(session_id: &SessionId, user_text: &str) -> bool {
    user_text.contains("<freehand_parent_")
        || user_text.starts_with(
            "You are the production Master starting a new follow-up turn injected by a due timer.",
        )
        || is_framework_worker_task_session(session_id)
}

fn is_runtime_continuation_round(turn_id: &TurnId) -> bool {
    let (ordinal, round, _) = runtime_turn_position(turn_id);
    ordinal > 0 && round > 1
}

fn is_framework_worker_task_session(session_id: &SessionId) -> bool {
    session_id.as_str().starts_with("worker-task-")
}

pub(crate) fn rebuild_session_history_from_effective_turns(
    history: &mut SessionHistory,
    session_id: &SessionId,
    turns: &[TurnRecord],
) -> Result<(), RuntimeLiveBridgeError> {
    let rebuilt_segments = effective_turn_context_segments(turns);
    if rebuilt_segments == history.base_context_segments() {
        return Ok(());
    }
    if rebuilt_segments.is_empty() {
        return Ok(());
    }
    history
        .stage_resume_rebuild(
            rebuilt_segments,
            "rebuild session transcript context from effective persisted turns",
            format!("runtime_restore:{}", session_id.as_str()),
        )
        .map_err(|err| RuntimeLiveBridgeError::RewriteRuntimeFailed(err.to_string()))?;
    Ok(())
}

pub(crate) fn effective_turn_context_segments(turns: &[TurnRecord]) -> Vec<ContextSegment> {
    let mut latest_by_logical_turn: BTreeMap<String, &TurnRecord> = BTreeMap::new();
    for turn in turns {
        let (ordinal, round, raw_turn_id) = runtime_turn_position(&turn.request.turn_id);
        let logical_key = if ordinal == 0 {
            raw_turn_id
        } else {
            ordinal.to_string()
        };
        let replace = latest_by_logical_turn
            .get(&logical_key)
            .map(|existing| {
                let (_, existing_round, _) = runtime_turn_position(&existing.request.turn_id);
                round >= existing_round
            })
            .unwrap_or(true);
        if replace {
            latest_by_logical_turn.insert(logical_key, turn);
        }
    }
    let mut latest_turns = latest_by_logical_turn
        .into_values()
        .collect::<Vec<&TurnRecord>>();
    latest_turns.sort_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
    latest_turns
        .into_iter()
        .filter_map(turn_context_segment)
        .collect()
}

fn turn_context_segment(turn: &TurnRecord) -> Option<ContextSegment> {
    let user_text = model_history_user_text_for_turn(turn);
    let assistant_text = history_visible_assistant_text(turn);
    if user_text.trim().is_empty() && assistant_text.trim().is_empty() {
        return None;
    }
    let (ordinal, round, raw_turn_id) = runtime_turn_position(&turn.request.turn_id);
    let content = if assistant_text.trim().is_empty() {
        format!(
            "Historical turn {} (round {}):\nUser: {}",
            ordinal,
            round,
            user_text.trim()
        )
    } else {
        format!(
            "Historical turn {} (round {}):\nUser: {}\nAssistant: {}",
            ordinal,
            round,
            user_text.trim(),
            assistant_text.trim()
        )
    };
    Some(ContextSegment {
        segment_id: ContextSegmentId::new(format!("session-memory-{raw_turn_id}")),
        kind: ContextSegmentKind::SessionMemory,
        stability: ContextStability::SessionStable,
        cache_policy: ContextCachePolicy::Cacheable,
        role: ContextRole::Developer,
        token_budget: history_segment_token_budget(&content),
        content,
        provenance: ContextProvenance {
            source: "freehand_runtime".to_owned(),
            reference: Some(format!("historical_turn:{raw_turn_id}")),
        },
    })
}

fn model_history_user_text_for_turn(turn: &TurnRecord) -> String {
    let user_text = ui_user_text_for_turn(turn);
    if ui_should_hide_user_text(&turn.request.session_id, &turn.request.user_text)
        || ui_should_hide_user_text(&turn.request.session_id, &user_text)
    {
        String::new()
    } else {
        user_text
    }
}

fn history_visible_assistant_text(turn: &TurnRecord) -> String {
    let visible_text = strip_completion_submission_block(&collect_turn_text(turn));
    if !visible_text.trim().is_empty() {
        return visible_text;
    }
    if let Some(terminal) = turn.terminal_event.as_ref()
        && !terminal.summary.trim().is_empty()
    {
        return terminal.summary.clone();
    }
    if let Some(error) = turn.error_events.last() {
        return format!("{}: {}", error.error.code, error.error.message);
    }
    String::new()
}

fn history_segment_token_budget(content: &str) -> u32 {
    let estimated = ((content.chars().count() as u32) / 4).max(32);
    estimated.saturating_add(64)
}

pub(crate) fn runtime_turn_position(turn_id: &TurnId) -> (u64, u64, String) {
    let raw = turn_id.as_str();
    let Some(rest) = raw.strip_prefix("runtime-turn-") else {
        return (0, 0, raw.to_owned());
    };
    let (ordinal_part, round) = match rest.split_once("-r") {
        Some((ordinal, round)) => (ordinal, round.parse::<u64>().ok().unwrap_or(1)),
        None => (rest, 1),
    };
    let ordinal = ordinal_part.parse::<u64>().ok().unwrap_or(0);
    (ordinal, round, raw.to_owned())
}
