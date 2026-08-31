export const WebUiSurface = Object.freeze({
  Root: "root",
  HomeDashboard: "home_dashboard",
  SessionDetail: "session_detail",
  ToolsRegistry: "tools_registry",
  TimerDashboard: "timer_dashboard",
  Settings: "settings",
  SessionSearch: "session_search",
  Memory: "memory",
  NewSession: "new_session",
});

export const webuiEdges = Object.freeze([
  {
    id: "root.open_home",
    from: WebUiSurface.Root,
    event: "home.open",
    to: WebUiSurface.HomeDashboard,
    requires: [],
    allowedEffects: ["set_route", "clear_primary_surface_selection"],
    forbiddenEffects: ["mutate_session_truth", "synthesize_latest_active_turn"],
  },
  {
    id: "home.open_session",
    from: WebUiSurface.HomeDashboard,
    event: "session.open",
    to: WebUiSurface.SessionDetail,
    requires: ["session_id"],
    allowedEffects: ["set_route", "pin_selected_session", "query_session_turns"],
    forbiddenEffects: ["mutate_session_truth", "clear_unrelated_surface_state", "synthesize_worker_session_id"],
  },
  {
    id: "session.rename_session",
    from: WebUiSurface.SessionDetail,
    event: "session.rename",
    to: WebUiSurface.SessionDetail,
    requires: ["session_id", "title"],
    allowedEffects: ["command_rename_session", "query_session_list", "refresh_selected_session"],
    forbiddenEffects: ["browser_local_crud_truth", "mutate_transcript_truth", "rename_unselected_session"],
  },
  {
    id: "session.compact_context",
    from: WebUiSurface.SessionDetail,
    event: "session.compact",
    to: WebUiSurface.SessionDetail,
    requires: ["session_id"],
    allowedEffects: ["command_compact_session_context", "query_session_turns"],
    forbiddenEffects: ["mutate_transcript_truth", "browser_local_crud_truth", "drop_recent_tail"],
  },
  {
    id: "home.delete_session",
    from: WebUiSurface.HomeDashboard,
    event: "session.delete",
    to: WebUiSurface.HomeDashboard,
    requires: ["session_id"],
    allowedEffects: ["confirm_user_intent", "command_delete_session", "query_session_list"],
    forbiddenEffects: ["physical_delete_turn_truth", "browser_local_crud_truth"],
  },
  {
    id: "home.open_search",
    from: WebUiSurface.HomeDashboard,
    event: "search.open",
    to: WebUiSurface.SessionSearch,
    requires: [],
    allowedEffects: ["set_route", "focus_search_input"],
    forbiddenEffects: ["browser_local_session_search", "create_session"],
  },
  {
    id: "home.open_memory",
    from: WebUiSurface.HomeDashboard,
    event: "memory.open",
    to: WebUiSurface.Memory,
    requires: [],
    allowedEffects: ["set_route", "focus_memory_query", "query_memory"],
    forbiddenEffects: ["browser_local_memory_search", "direct_filesystem_write", "mutate_session_truth"],
  },
  {
    id: "memory.close",
    from: WebUiSurface.Memory,
    event: "home.open",
    to: WebUiSurface.HomeDashboard,
    requires: [],
    allowedEffects: ["set_route"],
    forbiddenEffects: ["mutate_session_truth"],
  },
  {
    id: "home.open_new",
    from: WebUiSurface.HomeDashboard,
    event: "new.open",
    to: WebUiSurface.NewSession,
    requires: [],
    allowedEffects: ["set_route", "open_create_session_surface"],
    forbiddenEffects: ["random_verifier_spam", "create_without_owner_receipt"],
  },
  {
    id: "home.open_agent_directory",
    from: WebUiSurface.HomeDashboard,
    event: "agent_directory.open",
    to: "configured_agent_directory_sheet",
    requires: [],
    allowedEffects: ["open_configured_agent_directory"],
    forbiddenEffects: ["show_session_worker_tasks", "mutate_worker_capacity"],
  },
  {
    id: "root.open_tools",
    from: WebUiSurface.Root,
    event: "tools.open",
    to: WebUiSurface.ToolsRegistry,
    requires: [],
    allowedEffects: ["set_route", "query_tool_registry"],
    forbiddenEffects: ["execute_tool", "browser_local_tool_registry"],
  },
  {
    id: "root.open_timer",
    from: WebUiSurface.Root,
    event: "timer.open",
    to: WebUiSurface.TimerDashboard,
    requires: [],
    allowedEffects: ["set_route", "query_timer_list"],
    forbiddenEffects: ["browser_local_timer_truth", "task_center_timer_fallback"],
  },
  {
    id: "root.open_settings",
    from: WebUiSurface.Root,
    event: "settings.open",
    to: WebUiSurface.Settings,
    requires: [],
    allowedEffects: ["set_route", "query_config_status"],
    forbiddenEffects: ["read_config_file", "write_config_file", "expose_secret"],
  },
  {
    id: "session.back_home",
    from: WebUiSurface.SessionDetail,
    event: "home.back",
    to: WebUiSurface.HomeDashboard,
    requires: [],
    allowedEffects: ["set_route", "preserve_selected_session"],
    forbiddenEffects: ["clear_transcript_truth", "mutate_session_truth"],
  },
  {
    id: "session.submit",
    from: WebUiSurface.SessionDetail,
    event: "composer.submit",
    to: WebUiSurface.SessionDetail,
    requires: ["session_id"],
    allowedEffects: ["command_submit_user_input", "render_pending_receipt"],
    forbiddenEffects: ["browser_local_turn_truth", "drop_attachments_on_unknown_dispatch"],
  },
  {
    id: "session.open_agent_sheet",
    from: WebUiSurface.SessionDetail,
    event: "agent_sheet.open",
    to: "current_session_agent_sheet",
    requires: ["session_id"],
    allowedEffects: ["open_scoped_agent_sheet"],
    forbiddenEffects: ["show_global_task_history", "mutate_worker_capacity"],
  },
  {
    id: "session.expand_worker_status",
    from: WebUiSurface.SessionDetail,
    event: "worker_status.expand",
    to: WebUiSurface.SessionDetail,
    requires: ["session_id", "task_id"],
    allowedEffects: ["toggle_header_worker_detail"],
    forbiddenEffects: ["show_global_task_history", "mutate_worker_capacity", "synthesize_worker_session_id"],
  },
  {
    id: "session.open_worker_session",
    from: WebUiSurface.SessionDetail,
    event: "worker_session.open",
    to: WebUiSurface.SessionDetail,
    requires: ["worker_session_id"],
    allowedEffects: ["pin_selected_session", "query_session_turns"],
    forbiddenEffects: ["synthesize_worker_session_id", "show_stale_parent_transcript"],
  },
  {
    id: "session.open_parent_session",
    from: WebUiSurface.SessionDetail,
    event: "parent_session.open",
    to: WebUiSurface.SessionDetail,
    requires: ["session_id"],
    allowedEffects: ["pin_selected_session", "query_session_turns"],
    forbiddenEffects: ["clear_parent_lifecycle_truth", "show_global_home_lists"],
  },
  {
    id: "search.open_result",
    from: WebUiSurface.SessionSearch,
    event: "search_result.open",
    to: WebUiSurface.SessionDetail,
    requires: ["session_id"],
    allowedEffects: ["pin_selected_session", "query_session_turns"],
    forbiddenEffects: ["promote_worker_to_top_level", "browser_local_search_truth"],
  },
  {
    id: "new.created",
    from: WebUiSurface.NewSession,
    event: "session.created",
    to: WebUiSurface.SessionDetail,
    requires: ["session_id"],
    allowedEffects: ["command_create_session", "query_session_list", "pin_selected_session"],
    forbiddenEffects: ["create_without_owner_receipt", "random_verifier_spam"],
  },
  {
    id: "tools.refresh",
    from: WebUiSurface.ToolsRegistry,
    event: "tools.refresh",
    to: WebUiSurface.ToolsRegistry,
    requires: [],
    allowedEffects: ["query_tool_registry"],
    forbiddenEffects: ["execute_tool", "create_session"],
  },
  {
    id: "timer.refresh",
    from: WebUiSurface.TimerDashboard,
    event: "timer.refresh",
    to: WebUiSurface.TimerDashboard,
    requires: [],
    allowedEffects: ["query_timer_list"],
    forbiddenEffects: ["browser_local_timer_truth"],
  },
  {
    id: "settings.navigate",
    from: WebUiSurface.Settings,
    event: "settings.navigate",
    to: WebUiSurface.Settings,
    requires: ["page_id"],
    allowedEffects: ["set_settings_page"],
    forbiddenEffects: ["cross_surface_private_state_mutation", "write_config_file"],
  },
]);

const edgeById = new Map(webuiEdges.map((edge) => [edge.id, edge]));

export function edgeForId(edgeId) {
  return edgeById.get(edgeId) || null;
}

export function requireEdge(edgeId, payload = {}) {
  const edge = edgeForId(edgeId);
  if (!edge) {
    throw new Error(`unknown WebUI edge: ${edgeId}`);
  }
  const missing = (edge.requires || []).filter((key) => {
    const value = payload[key];
    return value === undefined || value === null || `${value}`.trim() === "";
  });
  if (missing.length > 0) {
    throw new Error(`edge ${edgeId} missing payload: ${missing.join(", ")}`);
  }
  return edge;
}
