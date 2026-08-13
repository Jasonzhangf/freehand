import { createHomeDashboardModel } from "./surfaces/home-dashboard/model.js?v=__WEBUI_ASSET_VERSION__";
import { createHomeSessionRow } from "./surfaces/home-dashboard/controls.js?v=__WEBUI_ASSET_VERSION__";
import { renderSurface as renderHomeDashboardSurface } from "./surfaces/home-dashboard/index.js?v=__WEBUI_ASSET_VERSION__";
import { renderRunningList as renderHomeRunningList, renderHistoryList as renderHomeHistoryList } from "./surfaces/home-dashboard/view.js?v=__WEBUI_ASSET_VERSION__";
import { openToolsRegistrySurface, refreshToolsRegistrySurface, renderSurface as renderToolsRegistrySurface } from "./surfaces/tools-registry/index.js?v=__WEBUI_ASSET_VERSION__";
import { cancelTimerFromSurface, openTimerDashboardSurface, refreshTimerDashboardSurface, renderSurface as renderTimerDashboardSurface, scheduleTimerFromSurface } from "./surfaces/timer-dashboard/index.js?v=__WEBUI_ASSET_VERSION__";
import { renderSurface as renderSessionSearchSurface, renderSessionSearchResult as renderSessionSearchResultSurface } from "./surfaces/session-search/index.js?v=__WEBUI_ASSET_VERSION__";
import { renderDiagnosticLogRow as renderDiagnosticLogRowSurface, renderSurface as renderSettingsShellSurface, renderNavigation as renderSettingsNavigationSurface, renderDiagnostics as renderSettingsDiagnosticsSurface } from "./surfaces/settings/index.js?v=__WEBUI_ASSET_VERSION__";
import { chooseNewTaskDirectory as chooseNewTaskDirectoryFromSurface, openNewSessionSurface, closeNewSessionSurface, selectedNewSessionKind as selectedNewSessionKindFromSurface, submitNewSessionSurface, syncNewSessionDialogMode as syncNewSessionDialogModeFromSurface } from "./surfaces/new-session/index.js?v=__WEBUI_ASSET_VERSION__";
import { setSelectedSessionId as setSelectedSessionIdInSurface, clearConversationForSessionSwitch as clearConversationForSessionSwitchInSurface, switchConversationSession as switchConversationSessionInSurface } from "./surfaces/session-detail/index.js?v=__WEBUI_ASSET_VERSION__";
import { HISTORICAL_FAILURE_RECOVERED, historicalFailureRecoveredLifecycle, historicalFailureRecoveredRows, historicalRecoveryProjectionChanged, recoveredHistoricalWorkerFailureTurnIds } from "./surfaces/session-detail/recovery.js?v=__WEBUI_ASSET_VERSION__";
import { createAdpClient, settleAdpResponseFrame } from "./app-shell/adp-client.js?v=__WEBUI_ASSET_VERSION__";
import { adpCommandOf, adpQueryOf, adpSubscribeOf } from "./generated/adp-protocol.js?v=__WEBUI_ASSET_VERSION__";

export const classifyLayoutShape = window.__freehandLayout.classifyLayoutShape;
export const classifyLayoutShapeForClient = window.__freehandLayout.classifyLayoutShapeForClient;

function viewportDimensionsForLayout() {
  return window.__freehandLayout.viewportDimensionsForLayout(window);
}

function layoutClient() {
  return new URLSearchParams(window.location.search).get("client") || "";
}

function isMobileDrawerLayout(shape) {
  return ["phone_portrait", "tall_phone", "tablet_portrait"].includes(shape);
}

const shell = document.querySelector("[data-webui-shell]");
const messageList = document.getElementById("message-list");
const sessionList = document.getElementById("session-list");
const mobileDrawerScrim = document.getElementById("mobile-drawer-scrim");
const openSessionDrawerButton = document.getElementById("open-session-drawer-button");
const closeSessionDrawerButton = document.getElementById("close-session-drawer-button");
const openDetailDrawerButton = document.getElementById("open-detail-drawer-button");
const openSettingsDrawerButton = document.getElementById("open-settings-drawer-button");
const openTimerDashboardButton = document.getElementById("open-timer-dashboard-button");
const openToolsDashboardButton = document.getElementById("open-tools-dashboard-button");
const mobileNewEntryButton = document.getElementById("mobile-new-entry-button");
const closeDetailDrawerButton = document.getElementById("close-detail-drawer-button");
const mobileHomeDashboard = document.getElementById("mobile-home-dashboard");
const mobileHomeActiveMarker = document.getElementById("mobile-home-active-marker");
const mobileHomeActiveList = document.getElementById("mobile-home-active-list");
const mobileHomeSessionList = document.getElementById("mobile-home-session-list");
const mobileAgentSummaryStrip = document.getElementById("mobile-agent-summary-strip");
const openMobileAgentSheetButton = document.getElementById("open-mobile-agent-sheet-button");
const closeMobileAgentSheetButton = document.getElementById("close-mobile-agent-sheet-button");
const mobileAgentSheet = document.getElementById("mobile-agent-sheet");
const mobileAgentTaskList = document.getElementById("mobile-agent-task-list");
const sessionRelationHeader = document.getElementById("session-relation-header");
const sessionRelationToggleButton = document.getElementById("session-relation-toggle-button");
const sessionWorkerRail = document.getElementById("session-worker-rail");
const sessionTreeDropdown = document.getElementById("session-tree-dropdown");
const sessionTree = document.getElementById("session-tree");
const workerSessionNav = document.getElementById("worker-session-nav");
const workerSessionBackButton = document.getElementById("worker-session-back-button");
const settingsAgentResourceDecrement = document.getElementById("settings-agent-resource-decrement");
const settingsAgentResourceIncrement = document.getElementById("settings-agent-resource-increment");
const settingsAgentResourceSave = document.getElementById("settings-agent-resource-save");
const settingsShellToggle = document.getElementById("settings-shell-toggle");
const inspectorEyebrow = document.getElementById("inspector-eyebrow");
const inspectorTitle = document.getElementById("inspector-title");
const inspectorCopy = document.getElementById("inspector-copy");
const inspectorDebugPanel = document.getElementById("inspector-debug-panel");
const taskBoardStatus = document.getElementById("task-board-status");
const taskBoardList = document.getElementById("task-board-list");
const agentBoardStatus = document.getElementById("agent-board-status");
const agentBoardList = document.getElementById("agent-board-list");
const eventInboxStatus = document.getElementById("event-inbox-status");
const eventInboxList = document.getElementById("event-inbox-list");
const taskHistoryStatus = document.getElementById("task-history-status");
const taskHistoryList = document.getElementById("task-history-list");
const workerControlStatus = document.getElementById("worker-control-status");
const workerControlList = document.getElementById("worker-control-list");
const settingsShell = document.getElementById("settings-shell");
const settingsProviderForm = document.getElementById("settings-provider-form");
const settingsProviderRegistryList = document.getElementById("settings-provider-registry-list");
const settingsProviderCurrentSelect = document.getElementById("settings-provider-current-select");
const settingsProviderFallbackSelect = document.getElementById("settings-provider-fallback-select");
const settingsProviderSwitchButton = document.getElementById("settings-provider-switch-button");
const settingsProviderIdInput = document.getElementById("settings-provider-id-input");
const settingsProviderTypeInput = document.getElementById("settings-provider-type-input");
const settingsProviderProtocolInput = document.getElementById("settings-provider-protocol-input");
const settingsProviderUrlInput = document.getElementById("settings-provider-url-input");
const settingsProviderModelInput = document.getElementById("settings-provider-model-input");
const settingsProviderWebSearchInput = document.getElementById("settings-provider-web-search-input");
const settingsProviderEnvInput = document.getElementById("settings-provider-env-input");
const settingsProviderSaveButton = document.getElementById("settings-provider-save-button");
const settingsProviderWebSearchTestButton = document.getElementById("settings-provider-web-search-test-button");
const settingsModelGroupSummary = document.getElementById("settings-model-group-summary");
const settingsModelGroupCurrentSelect = document.getElementById("settings-model-group-current-select");
const settingsModelGroupSwitchButton = document.getElementById("settings-model-group-switch-button");
const settingsModelGroupRegistryList = document.getElementById("settings-model-group-registry-list");
const settingsModelGroupForm = document.getElementById("settings-model-group-form");
const settingsModelGroupIdInput = document.getElementById("settings-model-group-id-input");
const settingsModelGroupLabelInput = document.getElementById("settings-model-group-label-input");
const settingsModelGroupEnabledInput = document.getElementById("settings-model-group-enabled-input");
const settingsModelGroupPrimaryProviderInput = document.getElementById("settings-model-group-primary-provider-input");
const settingsModelGroupPrimaryModelInput = document.getElementById("settings-model-group-primary-model-input");
const settingsModelGroupSubProviderInput = document.getElementById("settings-model-group-sub-provider-input");
const settingsModelGroupSubModelInput = document.getElementById("settings-model-group-sub-model-input");
const settingsModelGroupSearchProviderInput = document.getElementById("settings-model-group-search-provider-input");
const settingsModelGroupSearchModelInput = document.getElementById("settings-model-group-search-model-input");
const settingsModelGroupTitleProviderInput = document.getElementById("settings-model-group-title-provider-input");
const settingsModelGroupTitleModelInput = document.getElementById("settings-model-group-title-model-input");
const settingsModelGroupFallbackProviderInput = document.getElementById("settings-model-group-fallback-provider-input");
const settingsModelGroupFallbackModelInput = document.getElementById("settings-model-group-fallback-model-input");
const settingsModelGroupLoadBalanceInput = document.getElementById("settings-model-group-load-balance-input");
const settingsModelGroupContextWindowInput = document.getElementById("settings-model-group-context-window-input");
const settingsModelGroupCompactionThresholdInput = document.getElementById("settings-model-group-compaction-threshold-input");
const settingsModelGroupSaveButton = document.getElementById("settings-model-group-save-button");
const settingsApkUpdateSummary = document.getElementById("settings-apk-update-summary");
const settingsApkUpdateSource = document.getElementById("settings-apk-update-source");
const settingsApkUpdateStatus = document.getElementById("settings-apk-update-status");
const settingsApkUpdateCheckButton = document.getElementById("settings-apk-update-check-button");
const settingsAccountConfigSyncMarker = document.getElementById("settings-account-config-sync-marker");
const settingsAccountConfigSyncSummary = document.getElementById("settings-account-config-sync-summary");
const settingsAccountConfigSyncAccount = document.getElementById("settings-account-config-sync-account");
const settingsAccountConfigSyncRevision = document.getElementById("settings-account-config-sync-revision");
const settingsAccountConfigSyncContent = document.getElementById("settings-account-config-sync-content");
const settingsAccountConfigSyncStatus = document.getElementById("settings-account-config-sync-status");
const settingsAccountConfigPullButton = document.getElementById("settings-account-config-pull-button");
const settingsAccountConfigPushButton = document.getElementById("settings-account-config-push-button");
const settingsDiagnosticsSummary = document.getElementById("settings-diagnostics-summary");
const settingsDiagnosticsRuntimeHome = document.getElementById("settings-diagnostics-runtime-home");
const settingsDiagnosticsStatus = document.getElementById("settings-diagnostics-status");
const settingsDiagnosticsRefreshButton = document.getElementById("settings-diagnostics-refresh-button");
const settingsDiagnosticsList = document.getElementById("settings-diagnostics-list");
const newConversationButton = document.getElementById("new-conversation-button");
const newTaskButton = document.getElementById("new-task-button");
const taskCwdInput = document.getElementById("task-cwd-input");
const sessionBulkCount = document.getElementById("session-bulk-count");
const sessionSelectAllButton = document.getElementById("session-select-all-button");
const sessionClearSelectionButton = document.getElementById("session-clear-selection-button");
const sessionDeleteSelectedButton = document.getElementById("session-delete-selected-button");
const selectedSessionRenameButton = document.getElementById("selected-session-rename-button");
const newSessionDialog = document.getElementById("new-session-dialog");
const newSessionForm = document.getElementById("new-session-form");
const newSessionCwdInput = document.getElementById("new-session-cwd-input");
const newSessionBrowseButton = document.getElementById("new-session-browse-button");
const newTaskPathPresets = document.getElementById("new-task-path-presets");
const newSessionCancelButton = document.getElementById("new-session-cancel-button");
const newSessionCloseButton = document.getElementById("new-session-close-button");
const newSessionConfirmButton = document.getElementById("new-session-confirm-button");
const timerDashboardDialog = document.getElementById("timer-dashboard-dialog");
const timerDashboardForm = document.getElementById("timer-dashboard-form");
const timerDashboardCloseButton = document.getElementById("timer-dashboard-close-button");
const timerDashboardRefreshButton = document.getElementById("timer-dashboard-refresh-button");
const timerDashboardStatus = document.getElementById("timer-dashboard-status");
const timerDashboardList = document.getElementById("timer-dashboard-list");
const timerDashboardHistory = document.getElementById("timer-dashboard-history");
const toolsDashboardDialog = document.getElementById("tools-dashboard-dialog");
const toolsDashboardCloseButton = document.getElementById("tools-dashboard-close-button");
const toolsDashboardRefreshButton = document.getElementById("tools-dashboard-refresh-button");
const toolsDashboardStatus = document.getElementById("tools-dashboard-status");
const toolsDashboardGuidance = document.getElementById("tools-dashboard-guidance");
const toolsDashboardList = document.getElementById("tools-dashboard-list");
const sessionSearchDialog = document.getElementById("session-search-dialog");
const sessionSearchForm = document.getElementById("session-search-form");
const sessionSearchCloseButton = document.getElementById("session-search-close-button");
const sessionSearchInput = document.getElementById("session-search-input");
const sessionSearchSubmitButton = document.getElementById("session-search-submit-button");
const sessionSearchStatus = document.getElementById("session-search-status");
const sessionSearchResults = document.getElementById("session-search-results");
const timerModeInput = document.getElementById("timer-mode-input");
const timerDelayInput = document.getElementById("timer-delay-input");
const timerRunAtInput = document.getElementById("timer-run-at-input");
const timerRepeatKindInput = document.getElementById("timer-repeat-kind-input");
const timerIntervalInput = document.getElementById("timer-interval-input");
const timerTimeOfDayInput = document.getElementById("timer-time-of-day-input");
const timerWeekdaysInput = document.getElementById("timer-weekdays-input");
const timerCronInput = document.getElementById("timer-cron-input");
const timerMaxRunsInput = document.getElementById("timer-max-runs-input");
const timerSourceSessionInput = document.getElementById("timer-source-session-input");
const timerReasonInput = document.getElementById("timer-reason-input");
const timerPromptInput = document.getElementById("timer-prompt-input");
const composerForm = document.getElementById("composer-form");
const composerInput = document.getElementById("composer-input");
const cancelButton = document.getElementById("cancel-button");
const debugDetailsToggle = document.getElementById("debug-details-toggle");
const attachFileButton = document.getElementById("attach-file-button");
const attachImageButton = document.getElementById("attach-image-button");
const attachVideoButton = document.getElementById("attach-video-button");
const previewAttachmentsButton = document.getElementById("preview-attachments-button");
const refreshSessionButton = document.getElementById("refresh-session-button");
const modelSelector = document.getElementById("model-selector");
const cwdInput = document.getElementById("cwd-input");
const attachmentFileInput = document.getElementById("attachment-file-input");
const attachmentImageInput = document.getElementById("attachment-image-input");
const attachmentVideoInput = document.getElementById("attachment-video-input");
const attachmentTray = document.getElementById("attachment-tray");
const composerContextStrip = document.getElementById("composer-context-strip");
const compactContextButton = document.getElementById("compact-context-button");
const contextStatCacheHit = document.getElementById("context-stat-cache-hit");
const contextStatCacheAvg = document.getElementById("context-stat-cache-avg");
const contextStatThinking = document.getElementById("context-stat-thinking");
const contextStatContext = document.getElementById("context-stat-context");
const contextStatCompacted = document.getElementById("context-stat-compacted");

const samplePrompts = {
  success:
    "Answer with one short sentence and a valid Freehand completion schema. Do not call tools.",
  failure:
    'Call the task tool exactly once with {"op":"query","task_id":"definitely-missing-freehand-task"}, then use the failed tool result to continue and report success through the required Freehand completion schema.',
};

const selectedSessionStorageKey = "freehand-webui-selected-session";
const selectedCwdStorageKey = "freehand-webui-selected-cwd";
const attachmentDraftStorageKey = "freehand-webui-attachment-drafts-v1";
const androidNotificationStorageKey = "freehand-android-notified-turns-v1";
const layoutWidthsStorageKey = "freehand-webui-layout-widths-v1";
const adpRequestTimeoutMs = 45000;
const foregroundRefreshMinIntervalMs = 1500;
const adpReconnectBaseDelayMs = 1000;
const adpReconnectMaxDelayMs = 10000;
const liveTruthWatchdogIntervalMs = 10000;
const headerWorkerRailStatusRefreshMs = 3000;
const workerTranscriptRefreshRetryDelayMs = 3000;
const shortcutHelp =
  "快捷键：Cmd/Ctrl+Enter 发送 · Esc 停止/清空 · Cmd/Ctrl+R 刷新 · Cmd/Ctrl+K 聚焦输入框 · Cmd/Ctrl+1 成功样例 · Cmd/Ctrl+2 失败样例。Slash：/help /new /task /设置 /cwd /sessions /reload /success /failure /cancel /clear /附件 /model";
const initialSelectedSessionId = window.localStorage.getItem(selectedSessionStorageKey) || null;
const initialSelectedCwd = window.localStorage.getItem(selectedCwdStorageKey) || "";

function applyLayoutShape() {
  const { width, height } = viewportDimensionsForLayout();
  const shape = classifyLayoutShapeForClient(width, height, layoutClient());
  document.body.dataset.layoutShape = shape;
  if (shell) {
    shell.dataset.layoutShape = shape;
  }
  return shape;
}

window.__freehandLayout = {
  ...(window.__freehandLayout || {}),
  classifyLayoutShape,
  classifyLayoutShapeForClient,
  applyLayoutShape,
};

function markWebUiJavascriptReady() {
  document.body.dataset.webuiJsReady = "true";
  if (shell) {
    shell.dataset.webuiJsReady = "true";
  }
}

const state = {
  turn: null,
  sessions: [],
  sessionListLoaded: false,
  selectedSessionIds: new Set(),
  selectedSessionId: initialSelectedSessionId,
  selectedCwd: initialSelectedCwd,
  draftSessionId: null,
  sessionTurns: [],
  publicConversation: [],
  debug: null,
  checkpoints: [],
  taskBoard: null,
  agentBoard: null,
  eventInbox: null,
  taskHistory: null,
  workerControl: null,
  timerList: null,
  timerStatusError: null,
  timerCommandInFlight: false,
  toolRegistry: null,
  toolRegistryError: null,
  toolRegistryInFlight: false,
  sessionSearch: null,
  sessionSearchError: null,
  sessionSearchInFlight: false,
  phase2StatusError: null,
  phase2LastRefreshAt: null,
  phase2LiveRefreshInFlight: false,
  workerControlInFlight: false,
  configStatus: null,
  configStatusError: null,
  accountConfigSyncInFlight: false,
  configSaveInFlight: false,
  providerSelectionInFlight: false,
  providerWebSearchTestInFlight: "",
  providerSelectionDraft: null,
  modelGroupSelectionDraft: null,
  modelGroupSaveInFlight: false,
  modelGroupSelectionInFlight: false,
  agentResourceDraftCount: null,
  agentResourceSaveInFlight: false,
  agentResourceSaveMessage: null,
  agentResourceSaveError: null,
  androidApkUpdateStatus: null,
  androidApkUpdateInFlight: false,
  diagnostics: null,
  diagnosticsError: null,
  diagnosticsInFlight: false,
  settingsPage: "root",
  sessionTreeOpen: false,
  workerRailExpandedTaskId: null,
  toolTimings: new Map(),
  lifecycleClocks: new Map(),
  pendingUserInput: null,
  pendingSubmitId: null,
  pendingSubmitSessionId: null,
  pendingSubmitError: null,
  pendingCancelTurnId: null,
  locallyCancelledTurnIds: new Set(),
  acceptedSubmitReceipt: null,
  ambiguousSubmitRecoveryTimer: null,
  ambiguousSubmitRecoveryStartedAt: null,
  sessionRefreshInFlight: null,
  sessionRefreshError: null,
  sessionRefreshRetryTimer: null,
  pendingAttachments: [],
  inputHistory: [],
  inputHistoryIndex: null,
  mobileDrawer: null,
  mobileAgentSheetOpen: false,
  inspectorPanel: "debug",
  submitStartedAt: null,
  submitInFlight: false,
  commandStatusMessage: "正在连接服务...",
  commandStatusStickyUntil: 0,
  adpFailure: null,
  adpStatus: "connecting",
  adpSocket: null,
  adpOpened: null,
  adpRequests: new Map(),
  adpSubscriptions: new Set(),
  adpReconnectTimer: null,
  adpReconnectAttempt: 0,
  sseTurnStream: null,
  requestSequence: 0,
  attachmentDrafts: loadAttachmentDrafts(),
  attachmentsPreviewOpen: true,
  androidNotifiedTurns: loadAndroidNotifiedTurns(),
  androidObservedNonTerminalTurns: new Set(),
  debugDetailsVisible: false,
  forceScrollToBottom: false,
  userScrollLocked: false,
  rollbackArmedAt: 0,
  composerFocused: false,
  newSessionKind: "conversation",
  layoutResize: null,
  renderedCycleSessionId: null,
  foregroundRefreshInFlight: false,
  foregroundRefreshLastAt: 0,
};

const webuiRouteController = window.__freehandCreateRouteController({ state, document });
webuiRouteController.dispatch("root.open_home");
const adpClient = createAdpClient({
  state,
  windowRef: window,
  WebSocketCtor: WebSocket,
  url: adpUrl,
  nextRequestId,
  formatDuration,
  setCommandStatus,
  renderAll,
  scheduleReconnect: scheduleAdpReconnect,
  clearReconnectTimer: clearAdpReconnectTimer,
  handleFrame: handleAdpFrame,
  requestTimeoutMs: adpRequestTimeoutMs,
});

function dispatchWebUiEdge(edgeId, payload = {}) {
  const edge = window.__freehandRequireWebUiEdge(edgeId, payload);
  webuiRouteController.dispatch(edgeId, payload);
  return edge;
}

function selectedSessionDetailRouteActive() {
  return state.route === "session_detail" && !!state.selectedSessionId;
}

window.__freehandAndroidApkUpdateStatus = (payload) => {
  receiveAndroidApkUpdateStatus(payload);
};

function shellConfig() {
  return {
    adpEndpoint: shell.dataset.adpEndpoint,
    turnQuery: shell.dataset.turnQuery,
    turnSubscribe: shell.dataset.turnSubscribe,
    debugQueryBase: shell.dataset.debugQueryBase,
    debugSubscribeBase: shell.dataset.debugSubscribeBase,
    checkpointQuery: shell.dataset.checkpointQuery,
    commandEndpoint: shell.dataset.commandEndpoint,
  };
}

function applyMobileDrawerState() {
  const shape = document.body.dataset.layoutShape || applyLayoutShape();
  const drawer = isMobileDrawerLayout(shape) ? state.mobileDrawer : null;
  if (drawer) {
    document.body.dataset.mobileDrawer = drawer;
    if (shell) {
      shell.dataset.mobileDrawer = drawer;
    }
  } else {
    delete document.body.dataset.mobileDrawer;
    if (shell) {
      delete shell.dataset.mobileDrawer;
    }
  }
  if (openSessionDrawerButton) {
    openSessionDrawerButton.setAttribute("aria-expanded", drawer === "sessions" ? "true" : "false");
  }
  if (openDetailDrawerButton) {
    openDetailDrawerButton.setAttribute("aria-expanded", state.mobileAgentSheetOpen ? "true" : "false");
  }
  if (openSettingsDrawerButton) {
    openSettingsDrawerButton.setAttribute("aria-expanded", drawer === "settings" ? "true" : "false");
  }
  if (mobileDrawerScrim) {
    mobileDrawerScrim.setAttribute("aria-hidden", drawer || state.mobileAgentSheetOpen ? "false" : "true");
  }
}

function applyMobileAgentSheetState() {
  const shape = document.body.dataset.layoutShape || applyLayoutShape();
  const open = isMobileDrawerLayout(shape) && state.mobileAgentSheetOpen;
  if (open) {
    document.body.dataset.mobileAgentSheet = "open";
    if (shell) {
      shell.dataset.mobileAgentSheet = "open";
    }
  } else {
    delete document.body.dataset.mobileAgentSheet;
    if (shell) {
      delete shell.dataset.mobileAgentSheet;
    }
  }
  if (mobileAgentSheet) {
    mobileAgentSheet.setAttribute("aria-hidden", open ? "false" : "true");
  }
  if (openMobileAgentSheetButton) {
    openMobileAgentSheetButton.setAttribute("aria-expanded", open ? "true" : "false");
  }
  if (openDetailDrawerButton) {
    openDetailDrawerButton.setAttribute("aria-expanded", open ? "true" : "false");
  }
  if (mobileDrawerScrim) {
    mobileDrawerScrim.setAttribute("aria-hidden", open || state.mobileDrawer ? "false" : "true");
  }
}

function setComposerFocused(focused) {
  state.composerFocused = !!focused;
  if (state.composerFocused) {
    document.body.dataset.composerFocused = "true";
    if (shell) {
      shell.dataset.composerFocused = "true";
    }
  } else {
    delete document.body.dataset.composerFocused;
    if (shell) {
      delete shell.dataset.composerFocused;
    }
  }
}

function setMobileDrawer(drawer) {
  const shape = document.body.dataset.layoutShape || applyLayoutShape();
  state.mobileDrawer = drawer && isMobileDrawerLayout(shape) ? drawer : null;
  if (state.mobileDrawer) {
    state.mobileAgentSheetOpen = false;
  }
  applyMobileDrawerState();
  applyMobileAgentSheetState();
}

function closeMobileDrawer() {
  const wasSettingsRoute = state.mobileDrawer === "settings" && state.route === "settings";
  setMobileDrawer(null);
  if (wasSettingsRoute) {
    dispatchWebUiEdge("root.open_home");
  }
}

function setMobileAgentSheetOpen(open) {
  const shape = document.body.dataset.layoutShape || applyLayoutShape();
  const homeDirectoryRoute = state.route === "home_dashboard" && !state.selectedSessionId;
  const sessionWorkerRoute = selectedSessionDetailRouteActive();
  if (open && homeDirectoryRoute) {
    dispatchWebUiEdge("home.open_agent_directory");
  } else if (open && sessionWorkerRoute) {
    dispatchWebUiEdge("session.open_agent_sheet", { session_id: state.selectedSessionId });
  }
  state.mobileAgentSheetOpen = !!open && isMobileDrawerLayout(shape) && (homeDirectoryRoute || sessionWorkerRoute);
  if (state.mobileAgentSheetOpen) {
    state.mobileDrawer = null;
  }
  applyMobileDrawerState();
  applyMobileAgentSheetState();
}

function closeMobileOverlays() {
  state.mobileDrawer = null;
  state.mobileAgentSheetOpen = false;
  applyMobileDrawerState();
  applyMobileAgentSheetState();
}

function focusedEditableElement() {
  const active = document.activeElement;
  if (!active || active === document.body || active === document.documentElement) {
    return null;
  }
  if (active.isContentEditable) {
    return active;
  }
  const tag = `${active.tagName || ""}`.toLowerCase();
  return ["input", "select", "textarea"].includes(tag) ? active : null;
}

function closeVisibleNavigationSurface() {
  if (newSessionDialog && newSessionDialog.open) {
    closeNewSessionDialog();
    return true;
  }
  if (sessionSearchDialog && sessionSearchDialog.open) {
    sessionSearchDialog.close();
    dispatchWebUiEdge("root.open_home");
    renderAll();
    return true;
  }
  if (toolsDashboardDialog && toolsDashboardDialog.open) {
    toolsDashboardDialog.close();
    dispatchWebUiEdge("root.open_home");
    renderAll();
    return true;
  }
  if (timerDashboardDialog && timerDashboardDialog.open) {
    timerDashboardDialog.close();
    dispatchWebUiEdge("root.open_home");
    renderAll();
    return true;
  }
  if (state.sessionTreeOpen) {
    state.sessionTreeOpen = false;
    renderSessionRelationHeader();
    return true;
  }
  if (state.workerRailExpandedTaskId) {
    state.workerRailExpandedTaskId = null;
    renderSessionRelationHeader();
    return true;
  }
  if (state.mobileAgentSheetOpen) {
    setMobileAgentSheetOpen(false);
    return true;
  }
  if (state.mobileDrawer) {
    closeMobileDrawer();
    return true;
  }
  return false;
}

function handleBackNavigationIntent() {
  const focused = focusedEditableElement();
  if (focused) {
    focused.blur();
    setCommandStatus("输入框焦点已清除", { stickyMs: 2000 });
    return true;
  }
  if (selectedSessionRefreshErrorForRender() && !selectedWorkerTranscriptRefreshRetryable()) {
    returnToSessionListFromRefreshError();
    return true;
  }
  if (closeVisibleNavigationSurface()) {
    return true;
  }
  if (selectedSessionDetailRouteActive()) {
    dispatchWebUiEdge("session.back_home");
    renderAll();
    return true;
  }
  return false;
}

window.__freehandHandleAndroidBack = handleBackNavigationIntent;

function syncMobileDrawerForLayout() {
  if (!isMobileDrawerLayout(document.body.dataset.layoutShape || applyLayoutShape())) {
    state.mobileDrawer = null;
    state.mobileAgentSheetOpen = false;
  }
  applyMobileDrawerState();
  applyMobileAgentSheetState();
}

function desktopResizableLayoutActive() {
  return (document.body.dataset.layoutShape || applyLayoutShape()) === "desktop_large";
}

function clampNumber(value, min, max) {
  const number = Number(value);
  if (!Number.isFinite(number)) {
    return min;
  }
  return Math.min(max, Math.max(min, number));
}

function loadLayoutWidths() {
  try {
    const parsed = JSON.parse(window.localStorage.getItem(layoutWidthsStorageKey) || "{}");
    return {
      session: Number.isFinite(Number(parsed.session)) ? clampNumber(parsed.session, 180, 520) : 254,
      detail: Number.isFinite(Number(parsed.detail)) ? clampNumber(parsed.detail, 180, 560) : 244,
    };
  } catch (_error) {
    return { session: 254, detail: 244 };
  }
}

function saveLayoutWidths(widths) {
  window.localStorage.setItem(layoutWidthsStorageKey, JSON.stringify(widths));
}

function setLayoutWidths(widths, options = {}) {
  if (!shell) {
    return;
  }
  const viewportWidth = Math.max(1, window.innerWidth || document.documentElement.clientWidth || 1);
  const railWidth = 56;
  const resizerTotal = 20;
  const minWorkspace = Math.min(760, Math.max(360, viewportWidth * 0.36));
  const available = Math.max(360, viewportWidth - railWidth - resizerTotal - minWorkspace);
  const maxSide = Math.max(180, Math.floor(available * 0.72));
  let session = clampNumber(widths.session, 180, Math.min(520, maxSide));
  let detail = clampNumber(widths.detail, 180, Math.min(560, maxSide));
  if (session + detail > available) {
    const scale = available / (session + detail);
    session = Math.max(180, Math.floor(session * scale));
    detail = Math.max(180, Math.floor(detail * scale));
  }
  const next = { session, detail };
  shell.style.setProperty("--session-panel-width", `${next.session}px`);
  shell.style.setProperty("--detail-panel-width", `${next.detail}px`);
  if (!options.skipSave) {
    saveLayoutWidths(next);
  }
}

function applySavedLayoutWidths() {
  if (!desktopResizableLayoutActive()) {
    return;
  }
  setLayoutWidths(loadLayoutWidths(), { skipSave: true });
}

function installDesktopLayoutResizers() {
  const resizers = Array.from(document.querySelectorAll("[data-layout-resizer]"));
  if (!shell || resizers.length === 0) {
    return;
  }
  applySavedLayoutWidths();

  const beginResize = (event, side) => {
    if (!desktopResizableLayoutActive()) {
      return;
    }
    event.preventDefault();
    const widths = loadLayoutWidths();
    state.layoutResize = {
      side,
      startX: event.clientX,
      session: widths.session,
      detail: widths.detail,
    };
    document.body.dataset.layoutResizing = side;
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };

  const moveResize = (event) => {
    if (!state.layoutResize) {
      return;
    }
    event.preventDefault();
    const delta = event.clientX - state.layoutResize.startX;
    const next = {
      session:
        state.layoutResize.side === "left"
          ? state.layoutResize.session + delta
          : state.layoutResize.session,
      detail:
        state.layoutResize.side === "right"
          ? state.layoutResize.detail - delta
          : state.layoutResize.detail,
    };
    setLayoutWidths(next);
  };

  const endResize = () => {
    state.layoutResize = null;
    delete document.body.dataset.layoutResizing;
  };

  resizers.forEach((resizer) => {
    const side = resizer.dataset.layoutResizer;
    resizer.addEventListener("pointerdown", (event) => beginResize(event, side));
    resizer.addEventListener("keydown", (event) => {
      if (!["ArrowLeft", "ArrowRight", "Home"].includes(event.key) || !desktopResizableLayoutActive()) {
        return;
      }
      event.preventDefault();
      if (event.key === "Home") {
        setLayoutWidths({ session: 254, detail: 244 });
        return;
      }
      const step = event.shiftKey ? 40 : 16;
      const direction = event.key === "ArrowRight" ? 1 : -1;
      const widths = loadLayoutWidths();
      if (side === "left") {
        widths.session += direction * step;
      } else {
        widths.detail -= direction * step;
      }
      setLayoutWidths(widths);
    });
  });
  document.addEventListener("pointermove", moveResize, { passive: false });
  document.addEventListener("pointerup", endResize, { passive: true });
  document.addEventListener("pointercancel", endResize, { passive: true });
}

function shouldIgnoreSessionSwipeTarget(target) {
  if (!target || !target.closest) {
    return false;
  }
  return !!target.closest("input, textarea, select, button, a, dialog, .composer-card, .sidebar, .inspector");
}

function installMobileSessionSwipeGesture() {
  const openThreshold = 68;
  const verticalTolerance = 58;
  let gesture = null;

  const begin = (event) => {
    if (!isMobileDrawerLayout(document.body.dataset.layoutShape || applyLayoutShape())) {
      gesture = null;
      return;
    }
    if (state.mobileDrawer || shouldIgnoreSessionSwipeTarget(event.target)) {
      gesture = null;
      return;
    }
    const point = event.touches ? event.touches[0] : event;
    if (!point) {
      gesture = null;
      return;
    }
    gesture = {
      id: event.pointerId,
      startX: point.clientX,
      startY: point.clientY,
      tracking: true,
    };
  };

  const move = (event) => {
    if (!gesture || !gesture.tracking) {
      return;
    }
    const point = event.touches ? event.touches[0] : event;
    if (!point) {
      return;
    }
    const deltaX = point.clientX - gesture.startX;
    const deltaY = point.clientY - gesture.startY;
    if (deltaX < 0 || Math.abs(deltaY) > verticalTolerance) {
      gesture = null;
      return;
    }
    if (deltaX > 12 && deltaX > Math.abs(deltaY) * 1.4 && event.cancelable) {
      event.preventDefault();
    }
    if (deltaX >= openThreshold && deltaX > Math.abs(deltaY) * 1.4) {
      setMobileDrawer("sessions");
      gesture = null;
    }
  };

  const end = () => {
    gesture = null;
  };

  if (window.PointerEvent) {
    document.addEventListener("pointerdown", begin, { passive: true });
    document.addEventListener("pointermove", move, { passive: false });
    document.addEventListener("pointerup", end, { passive: true });
    document.addEventListener("pointercancel", end, { passive: true });
  }
  document.addEventListener("touchstart", begin, { passive: true });
  document.addEventListener("touchmove", move, { passive: false });
  document.addEventListener("touchend", end, { passive: true });
  document.addEventListener("touchcancel", end, { passive: true });
}

function adpUrl() {
  const endpoint = shellConfig().adpEndpoint || "adp";
  const url = new URL(endpoint, window.location.href);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return url.toString();
}

function nextRequestId(prefix) {
  state.requestSequence += 1;
  return `webui-${prefix}-${state.requestSequence}`;
}

function adpClientKind() {
  return "WebUi";
}

function clearAdpReconnectTimer() {
  if (state.adpReconnectTimer) {
    window.clearTimeout(state.adpReconnectTimer);
  }
  state.adpReconnectTimer = null;
}

function hasRecoverableProtocolState() {
  return !!(
    state.selectedSessionId ||
    state.pendingUserInput ||
    state.turn ||
    state.sessionTurns.length > 0 ||
    state.sessionListLoaded
  );
}

function protocolConnectionCanRenderLive() {
  return state.adpStatus === "connected";
}

function scheduleAdpReconnect(reason) {
  if (document.visibilityState === "hidden" || state.adpReconnectTimer) {
    return;
  }
  if (!hasRecoverableProtocolState() && state.adpStatus !== "closed" && state.adpStatus !== "error") {
    return;
  }
  const delay = Math.min(
    adpReconnectMaxDelayMs,
    adpReconnectBaseDelayMs * (2 ** Math.min(state.adpReconnectAttempt, 4)),
  );
  state.adpReconnectAttempt += 1;
  setBackgroundCommandStatus(`连接已关闭，${reason} 后重连...`);
  state.adpReconnectTimer = window.setTimeout(() => {
    state.adpReconnectTimer = null;
    refreshAllProtocolStateAfterReconnect(reason).catch((error) => {
      setCommandStatus(`服务重连失败：${error.message}`, { stickyMs: 5000 });
      scheduleAdpReconnect("重试失败");
    });
  }, delay);
}

async function refreshAllProtocolStateAfterReconnect(reason) {
  await ensureAdpSocket();
  ensureTurnSubscription();
  await refreshAllProtocolState();
  state.adpReconnectAttempt = 0;
  clearPendingUserInputIfMaterialized();
  renderAll();
  setBackgroundCommandStatus(`${reason} 后已刷新服务真源`);
}

function ensureAdpSocket() {
  return adpClient.ensureSocket();
}

async function sendAdpFrame(frame) {
  return adpClient.send(frame);
}

function requestAdp(kind, payloadKey, payload, prefix) {
  return adpClient.request(kind, payloadKey, payload, prefix);
}

function adpQuery(query) {
  return adpClient.query(query);
}

let adpCommandForTest = null;

function adpCommand(command) {
  if (adpCommandForTest) {
    return adpCommandForTest(command);
  }
  return adpClient.command(command);
}

function adpSubscribe(subscription, prefix) {
  return adpClient.subscribe(subscription, prefix);
}

function setCommandStatus(message, options = {}) {
  state.commandStatusMessage = message;
  state.commandStatusStickyUntil = options.stickyMs ? Date.now() + options.stickyMs : 0;
  renderCommandStatus();
}

function providerConfigReceiptStatus(receipt) {
  if (receipt && receipt.dispatch_status === "provider_config_saved_restart_required") {
    return "模型服务配置已保存，需要重启。";
  }
  throw new Error("配置保存返回了未预期的服务状态。");
}

function providerConfigUpsertReceiptStatus(receipt) {
  if (receipt && receipt.dispatch_status === "provider_config_upserted_restart_required") {
    return "模型服务定义已保存，需要重启。";
  }
  throw new Error("模型服务定义保存返回了未预期的服务状态。");
}

function providerWebSearchTestReceiptStatus(receipt) {
  const status = receipt?.dispatch_status || "";
  if (status.startsWith("provider_web_search_test_passed:")) {
    return `模型服务联网搜索测试通过：${status}`;
  }
  throw new Error("模型服务联网搜索测试返回了未预期的服务状态。");
}

function providerSelectionReceiptStatus(receipt) {
  if (receipt && receipt.dispatch_status === "agent_provider_selection_saved_restart_required") {
    return "模型服务选择已保存，需要重启。";
  }
  throw new Error("模型服务选择保存返回了未预期的服务状态。");
}

function modelGroupUpsertReceiptStatus(receipt) {
  if (receipt && receipt.dispatch_status === "model_group_config_upserted_restart_required") {
    return "模型组已保存，需要重启。";
  }
  throw new Error("模型组保存返回了未预期的服务状态。");
}

function modelGroupSelectionReceiptStatus(receipt) {
  if (receipt && receipt.dispatch_status === "model_group_selection_saved_restart_required") {
    return "模型组选择已保存，需要重启。";
  }
  throw new Error("模型组选择保存返回了未预期的服务状态。");
}

function agentResourceConfigReceiptStatus(receipt, expectedCount) {
  const expected = `agent_resource_config_saved_restart_required:count=${expectedCount}`;
  if (receipt && receipt.dispatch_status === expected) {
    return `工作器上限已保存：${expectedCount}。需要重启。`;
  }
  throw new Error("Agent 资源保存返回了未预期的服务状态。");
}

function timerScheduleReceiptStatus(receipt) {
  const status = receipt?.dispatch_status || "";
  if (status.startsWith("timer_scheduled:timer_id=")) {
    return `定时器已创建：${status}`;
  }
  throw new Error("定时器创建返回了未预期的服务状态。");
}

function timerCancelReceiptStatus(receipt) {
  const status = receipt?.dispatch_status || "";
  if (status.startsWith("timer_cancelled:timer_id=")) {
    return `定时器已取消：${status}`;
  }
  throw new Error("定时器取消返回了未预期的服务状态。");
}

function setBackgroundCommandStatus(message) {
  if (state.commandStatusStickyUntil > Date.now()) {
    return;
  }
  setCommandStatus(message);
}

function handleAdpFrame(frame) {
  const settlement = settleAdpResponseFrame({ state, windowRef: window, frame });
  switch (settlement.kind) {
    case "query_result":
      state.adpFailure = null;
      return;
    case "command_receipt":
      state.adpFailure = null;
      setBackgroundCommandStatus(commandReceiptStatus(settlement.receipt));
      return;
    case "subscription_accepted":
      state.adpFailure = null;
      setBackgroundCommandStatus(`更新流已连接：${settlement.selector.stream_kind}`);
      return;
    case "subscription_event":
      state.adpFailure = null;
      applyAdpSubscriptionEvent(settlement.event);
      return;
    case "failure":
      state.adpFailure = settlement.failure.message;
      setCommandStatus(`请求失败：${settlement.failure.code}`);
      return;
    default:
      setCommandStatus(`不支持的服务消息：${settlement.kind}`);
  }
}

function setText(id, value) {
  const element = document.getElementById(id);
  if (element) {
    element.textContent = value;
  }
}

function webSearchStatusLabel(provider) {
  if (!provider) {
    return "加载中";
  }
  const configured = provider.provider_web_search || "auto";
  const effective = provider.provider_web_search_effective || "unknown";
  return `${configured} -> ${effective}`;
}

function setTitle(id, value) {
  const element = document.getElementById(id);
  if (element) {
    element.title = value || "";
  }
}

function setShellDataset(name, value) {
  if (!shell) {
    return;
  }
  if (value === null || value === undefined || value === "") {
    delete shell.dataset[name];
    return;
  }
  shell.dataset[name] = value;
}

function loadAttachmentDrafts() {
  try {
    const raw = window.localStorage.getItem(attachmentDraftStorageKey);
    if (!raw) {
      return new Map();
    }
    const parsed = JSON.parse(raw);
    const entries = Array.isArray(parsed) ? parsed : [];
    return new Map(
      entries
        .filter((entry) => entry && typeof entry.session_id === "string" && Array.isArray(entry.attachments))
        .map((entry) => [
          entry.session_id,
          entry.attachments.map((attachment) => ({
            ...attachment,
            file: null,
            available: false,
            status: "metadata-only",
          })),
        ]),
    );
  } catch (error) {
    return new Map();
  }
}

function loadAndroidNotifiedTurns() {
  try {
    const raw = window.localStorage.getItem(androidNotificationStorageKey);
    const parsed = raw ? JSON.parse(raw) : [];
    return new Set(Array.isArray(parsed) ? parsed.filter((value) => typeof value === "string") : []);
  } catch (error) {
    console.warn("Freehand Android notification de-duplication state could not be loaded", error);
    return new Set();
  }
}

function persistAndroidNotifiedTurns() {
  try {
    window.localStorage.setItem(
      androidNotificationStorageKey,
      JSON.stringify(Array.from(state.androidNotifiedTurns).slice(-500)),
    );
  } catch (error) {
    console.warn("Freehand Android notification de-duplication state could not be saved", error);
  }
}

function persistAttachmentDrafts() {
  const entries = Array.from(state.attachmentDrafts.entries()).map(([sessionId, attachments]) => ({
    session_id: sessionId,
    attachments: attachments.map(({ file, previewUrl, dataBase64, ...metadata }) => ({
      ...metadata,
      available: false,
      status: metadata.status === "ready" ? "metadata-only" : metadata.status,
    })),
  }));
  window.localStorage.setItem(attachmentDraftStorageKey, JSON.stringify(entries));
}

function attachmentSessionId() {
  if (state.selectedSessionId) {
    return state.selectedSessionId;
  }
  const sessionId = newDraftSessionId();
  state.draftSessionId = sessionId;
  setSelectedSessionId(sessionId);
  return sessionId;
}

function currentAttachmentSessionId() {
  return state.selectedSessionId || state.draftSessionId || null;
}

function currentAttachments() {
  const sessionId = currentAttachmentSessionId();
  if (!sessionId) {
    return [];
  }
  return state.attachmentDrafts.get(sessionId) || [];
}

function setCurrentAttachments(attachments) {
  const sessionId = attachments.length > 0 ? attachmentSessionId() : currentAttachmentSessionId();
  if (!sessionId) {
    return;
  }
  if (attachments.length === 0) {
    state.attachmentDrafts.delete(sessionId);
  } else {
    state.attachmentDrafts.set(sessionId, attachments);
  }
  persistAttachmentDrafts();
  renderAttachmentTray();
  renderMessages();
}

function attachmentKind(file, forcedKind = null) {
  if (forcedKind) {
    return forcedKind;
  }
  if (file.type.startsWith("image/")) {
    return "image";
  }
  if (file.type.startsWith("video/")) {
    return "video";
  }
  return "file";
}

function formatBytes(size) {
  if (!Number.isFinite(size)) {
    return "未知大小";
  }
  if (size < 1024) {
    return `${size} B`;
  }
  if (size < 1024 * 1024) {
    return `${(size / 1024).toFixed(1)} KB`;
  }
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

function addAttachmentFiles(files, forcedKind = null) {
  const next = [...currentAttachments()];
  Array.from(files || []).forEach((file) => {
    next.push({
      id: browserRandomId(),
      name: file.name,
      size: file.size,
      type: file.type || "application/octet-stream",
      kind: attachmentKind(file, forcedKind),
      added_at: new Date().toISOString(),
      status: "ready",
      available: true,
      previewUrl: file.type.startsWith("image/") ? URL.createObjectURL(file) : null,
      file,
    });
  });
  state.attachmentsPreviewOpen = true;
  setCurrentAttachments(next);
  setCommandStatus(`当前会话已有 ${next.length} 个附件草稿`, { stickyMs: 4000 });
}

function addAndroidAttachmentDrafts(kind, files) {
  const next = [...currentAttachments()];
  Array.from(files || []).forEach((file) => {
    next.push({
      id: browserRandomId(),
      name: file.name || "附件",
      size: Number.isFinite(file.size) ? file.size : -1,
      type: file.type || "application/octet-stream",
      kind: attachmentKind({ type: file.type || "" }, kind),
      added_at: new Date().toISOString(),
      status: "ready",
      available: true,
      uri: file.uri || "",
      dataBase64: file.data_base64 || file.dataBase64 || null,
      previewUrl: file.data_base64 || file.dataBase64
        ? `data:${file.type || "application/octet-stream"};base64,${file.data_base64 || file.dataBase64}`
        : null,
      file: null,
    });
  });
  state.attachmentsPreviewOpen = true;
  setCurrentAttachments(next);
  setCommandStatus(`当前会话已有 ${next.length} 个附件草稿`, { stickyMs: 4000 });
}

window.__freehandAndroidAttachmentSelected = (kind, files) => {
  addAndroidAttachmentDrafts(kind, files);
};

function removeAttachment(id) {
  const next = currentAttachments().filter((attachment) => attachment.id !== id);
  setCurrentAttachments(next);
  setCommandStatus("附件已移除", { stickyMs: 3000 });
}

function clearCurrentAttachments() {
  setCurrentAttachments([]);
}

function attachmentDisplayLines(attachments = currentAttachments(), options = {}) {
  if (attachments.length === 0) {
    return [];
  }
  const lines = [options.heading || "附件"];
  attachments.forEach((attachment) => {
    const mediaType = attachment.type || attachment.media_type || "unknown";
    const sizeBytes = Number.isFinite(attachment.size) ? attachment.size : attachment.size_bytes;
    const availability = attachment.available ? "可发送" : options.defaultAvailability || "仅元数据";
    lines.push(
      `- ${attachment.kind}: ${attachment.name} (${formatBytes(sizeBytes)}, ${mediaType}, ${availability})`,
    );
  });
  return lines;
}

function textWithAttachmentDisplay(text, attachments = currentAttachments()) {
  const lines = attachmentDisplayLines(attachments);
  if (lines.length === 0) {
    return text;
  }
  return `${text}\n\n${lines.join("\n")}`;
}

function textWithSubmittedAttachmentDisplay(text, attachments = []) {
  const lines = attachmentDisplayLines(attachments, {
    heading: "已提交附件",
    defaultAvailability: "仅元数据",
  });
  if (lines.length === 0) {
    return text;
  }
  return `${text}\n\n${lines.join("\n")}`;
}

function attachmentSummary(attachments = currentAttachments()) {
  if (attachments.length === 0) {
    return "没有附件草稿";
  }
  const ready = attachments.filter((attachment) => attachment.available).length;
  return `${attachments.length} 个附件草稿，本页可发送 ${ready} 个`;
}

function renderAttachmentTray() {
  if (!attachmentTray) {
    return;
  }
  const attachments = currentAttachments();
  if (attachments.length > 0) {
    document.body.dataset.hasAttachments = "true";
  } else {
    delete document.body.dataset.hasAttachments;
  }
  attachmentTray.replaceChildren();
  const summary = document.createElement("div");
  summary.className = "attachment-summary";
  summary.textContent = attachmentSummary(attachments);
  attachmentTray.appendChild(summary);

  if (!state.attachmentsPreviewOpen || attachments.length === 0) {
    return;
  }

  const list = document.createElement("div");
  list.className = "attachment-list";
  attachments.forEach((attachment) => {
    const chip = document.createElement("div");
    chip.className = `attachment-chip ${attachment.available ? "ready" : "metadata-only"} ${attachment.kind === "image" ? "image-attachment" : ""}`;

    if (attachment.kind === "image" && attachment.previewUrl) {
      const imageButton = document.createElement("button");
      imageButton.className = "attachment-thumb-button";
      imageButton.type = "button";
      imageButton.title = "预览选中的图片";
      const img = document.createElement("img");
      img.className = "attachment-thumb";
      img.alt = attachment.name || "已选图片";
      img.src = attachment.previewUrl;
      imageButton.appendChild(img);
      imageButton.addEventListener("click", () => showAttachmentPreview(attachment));
      chip.appendChild(imageButton);
    }

    const text = document.createElement("span");
    text.className = "attachment-chip-text";
    text.textContent = `${attachment.kind} · ${attachment.name} · ${formatBytes(attachment.size)}`;
    text.title = attachment.available
      ? "本页仍持有 File handle，可用于重试。"
      : "已从会话恢复元数据；发送二进制内容前需要重新选择文件。";

    const remove = document.createElement("button");
    remove.className = "attachment-remove";
    remove.type = "button";
    remove.textContent = "×";
    remove.setAttribute("aria-label", `移除 ${attachment.name}`);
    remove.addEventListener("click", (event) => {
      event.stopPropagation();
      removeAttachment(attachment.id);
    });

    chip.append(text, remove);
    list.appendChild(chip);
  });
  attachmentTray.appendChild(list);
}

function showAttachmentPreview(attachment) {
  if (attachment.kind !== "image" || !attachment.previewUrl) {
    setCommandStatus("图片预览不可用；如果它来自元数据恢复，请重新选择图片", { stickyMs: 5000 });
    return;
  }
  const overlay = document.createElement("div");
  overlay.className = "attachment-preview-overlay";
  const panel = document.createElement("div");
  panel.className = "attachment-preview-panel";
  const close = document.createElement("button");
  close.className = "attachment-preview-close";
  close.type = "button";
  close.textContent = "×";
  close.setAttribute("aria-label", "关闭图片预览");
  const img = document.createElement("img");
  img.className = "attachment-preview-image";
  img.src = attachment.previewUrl;
  img.alt = attachment.name || "已选图片";
  const caption = document.createElement("div");
  caption.className = "attachment-preview-caption";
  caption.textContent = `${attachment.name || "图片"} · ${attachment.type || "未知类型"} · ${formatBytes(attachment.size)}`;
  panel.append(close, img, caption);
  overlay.appendChild(panel);
  const dismiss = () => overlay.remove();
  close.addEventListener("click", dismiss);
  overlay.addEventListener("click", (event) => {
    if (event.target === overlay) dismiss();
  });
  document.body.appendChild(overlay);
}

function fileToBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error || new Error("读取图片失败"));
    reader.onload = () => {
      const value = String(reader.result || "");
      const comma = value.indexOf(",");
      resolve(comma >= 0 ? value.slice(comma + 1) : value);
    };
    reader.readAsDataURL(file);
  });
}

async function attachmentsForSubmit(attachments) {
  const imageAttachments = attachments.filter((attachment) => attachment.kind === "image");
  const payloads = [];
  for (const attachment of imageAttachments) {
    let dataBase64 = attachment.dataBase64 || attachment.data_base64 || null;
    if (!dataBase64 && attachment.file) {
      dataBase64 = await fileToBase64(attachment.file);
    }
    if (!dataBase64) {
      throw new Error(`图片 ${attachment.name || attachment.id} 只有元数据；发送前请重新选择。`);
    }
    payloads.push({
      attachment_id: attachment.id,
      kind: "image",
      media_type: attachment.type || "application/octet-stream",
      name: attachment.name || "image",
      size_bytes: Number.isFinite(attachment.size) && attachment.size >= 0 ? attachment.size : null,
      data_base64: dataBase64,
    });
  }
  return payloads;
}

function card(role, status, title, body, variant = "assistant", identity = null) {
  const article = document.createElement("article");
  article.className = `dialog-block ${variant}-block ${status.className}-state`;
  if (identity) {
    article.dataset.identity = identity;
  }
  if (variant === "tool" && status.className === "running") {
    article.dataset.live = "true";
  }

  const head = document.createElement("div");
  head.className = "block-head";

  const roleBadge = document.createElement("span");
  roleBadge.className = `role-badge ${variant}-badge`;
  roleBadge.textContent = role;

  const stateBadge = document.createElement("span");
  stateBadge.className = `block-state ${status.className}`;
  if (variant === "tool" && (status.className === "success" || status.className === "failed")) {
    stateBadge.classList.add("compact-tool-state");
    stateBadge.textContent = "";
    stateBadge.title = status.label;
    stateBadge.setAttribute("aria-label", status.label);
  } else if (variant === "tool" && status.className === "running") {
    stateBadge.classList.add("running-tool-state");
    stateBadge.textContent = status.label;
  } else {
    stateBadge.textContent = status.label;
  }

  head.append(roleBadge, stateBadge);

  const content = document.createElement("div");
  content.className = `${variant === "tool" || variant === "failure" ? "tool-kind" : "text-kind"} block-body`;

  const titleNode = document.createElement("div");
  titleNode.className = "block-title";
  titleNode.textContent = title;
  content.appendChild(titleNode);

  const bodyNode = document.createElement("div");
  if (variant === "tool") {
    renderToolBody(bodyNode, body);
  } else {
    bodyNode.textContent = body;
  }
  content.appendChild(bodyNode);

  article.append(head, content);
  return article;
}

function turnLifecycleForRender(turn) {
  if (!turn) {
    return { phase: "neutral", className: "pending", label: "空闲", isLive: false, elapsed: "" };
  }
  if (turn.terminal_text || isTerminalStatus(turn.terminal_status) || isToolPendingStatus(turn.terminal_status)) {
    const terminal = `${turn.terminal_status || "success"}`.toLowerCase();
    const phase = terminal === "success" ? "completed" : terminal;
    if (terminal === "running" || isToolPendingStatus(terminal)) {
      return toolPendingLifecycleForRender(turn);
    }
    if (isAwaitingUserOptionsStatus(terminal)) {
      return {
        phase: "waiting_user_options",
        className: "pending",
        label: "等待用户选择",
        isLive: false,
        elapsed: "",
      };
    }
    const label = terminalTurnStatusLabelForTurn(turn, terminal);
    return {
      phase,
      className: terminal === "success" ? "success" : "failed",
      label,
      isLive: false,
      elapsed: "",
    };
  }
  const isCurrentLiveTurn = turnIsCurrentLiveTurn(turn);
  const waitingTools = (turn.tool_activities || []).filter(
    (tool) => tool.status === "Waiting" || tool.status === "waiting",
  );
  if (isCurrentLiveTurn && waitingTools.length > 0) {
    return { phase: "tool_waiting", className: "running", label: waitingToolStatus(waitingTools, turn), isLive: true, elapsed: "" };
  }
  if (isCurrentLiveTurn && turnIsWaitingForModelResponse(turn)) {
    const elapsed = elapsedSince(lifecycleClockStartedAt(modelRequestTimingKey(turn)));
    const label = modelRequestLabel(turn);
    return {
      phase: modelRequestPhase(turn),
      className: "running",
      label: elapsed ? `${label}... ${elapsed}` : `${label}...`,
      isLive: true,
      elapsed,
    };
  }
  if (isCurrentLiveTurn && state.submitInFlight) {
    const elapsed = elapsedSince(state.submitStartedAt);
    return {
      phase: "dispatching",
      className: "running",
      label: elapsed ? `派发中... ${elapsed}` : "派发中...",
      isLive: true,
      elapsed,
    };
  }
  const inactiveToolLifecycle = inactiveToolLifecycleForRender(turn);
  if (inactiveToolLifecycle) {
    return inactiveToolLifecycle;
  }
  return { phase: "neutral", className: "pending", label: "等待中", isLive: false, neutral: true, elapsed: "" };
}

function inactiveToolLifecycleForRender(turn) {
  const tools = (turn && turn.tool_activities) || [];
  if (tools.length === 0) {
    return null;
  }
  const statuses = tools.map((tool) => `${tool.status || ""}`.toLowerCase());
  if (statuses.some((status) => status === "failed")) {
    return {
      phase: "tool_failed",
      className: "failed",
      label: "失败",
      isLive: false,
      elapsed: "",
    };
  }
  if (statuses.every((status) => status === "completed" || status === "success")) {
    return {
      phase: "tool_completed",
      className: "success",
      label: "已完成",
      isLive: false,
      elapsed: "",
    };
  }
  return null;
}

function pendingExecutionCard(renderPending) {
  const elapsed = renderPending.elapsed;
  const article = executionShell({
    status: {
      className: renderPending.isLive ? "running" : "pending",
      label: elapsed ? `派发中 · ${elapsed}` : "派发中",
    },
    live: renderPending.isLive,
  });
  const body = article.querySelector(".execution-body");
  body.appendChild(executionRow({
    kind: "user",
    title: "用户",
    body: [textWithAttachmentDisplay(renderPending.text, renderPending.attachments)],
    status: "已提交",
  }));
  body.appendChild(executionRow({
    kind: "system",
    title: "客户端",
    body: ["请求已接收，等待服务派发。"],
    status: elapsed || "0s",
  }));
  return article;
}

function pendingChatCards(renderPending) {
  const elapsed = renderPending.elapsed;
  const retainedAttachmentCount = Array.isArray(renderPending.attachments)
    ? renderPending.attachments.length
    : 0;
  const userRow = {
    kind: "user",
    title: "用户",
    body: [textWithAttachmentDisplay(renderPending.text, renderPending.attachments)],
    status: "已提交",
  };
  const assistantRows = [{
    kind: "system",
    title: "客户端",
    body: renderPending.error
      ? [
          "正在根据服务真源验证提交收据。",
          "服务刷新完成前不要重复发送。",
          retainedAttachmentCount > 0
            ? `已保留 ${retainedAttachmentCount} 个附件草稿用于重试。`
            : "已保留附件草稿用于重试：无。",
        ]
      : ["请求已接收，等待服务派发。"],
    status: renderPending.error ? "检查服务真源" : elapsed || "0s",
  }];
  const renderTurn = {
    turnId: "pending-submit",
    createdAt: renderPending.startedAt || Date.now(),
    lifecycle: {
      className: renderPending.error ? "running" : renderPending.isLive ? "running" : "pending",
      label: renderPending.error
        ? "检查服务真源"
        : elapsed
          ? `派发中... ${elapsed}`
          : "派发中",
      isLive: renderPending.isLive || !!renderPending.error,
    },
  };
  return [userChatBubble(renderTurn, userRow), assistantChatBubble(renderTurn, assistantRows)];
}

function acceptedSubmitReceiptChatCards(receipt) {
  const text = receipt && receipt.text ? receipt.text : "";
  const taskLine = [receipt.taskId, receipt.status, receipt.title]
    .filter(Boolean)
    .join(" · ");
  const userRow = {
    kind: "user",
    title: "用户",
    body: [text],
    status: "已提交",
  };
  const assistantRows = [{
    kind: "system",
    title: "服务",
    body: [
      "服务已通过 任务面板 真源接收该请求。",
      taskLine ? `工作器任务：${taskLine}` : "工作器生命周期可在 智能体任务列表中观察。",
    ],
    status: "已接收",
  }];
  const renderTurn = {
    turnId: "accepted-submit",
    createdAt: receipt.createdAt || receipt.created_at || Date.now(),
    lifecycle: {
      className: "running",
      label: "服务已接收",
      isLive: true,
    },
  };
  return [userChatBubble(renderTurn, userRow), assistantChatBubble(renderTurn, assistantRows)];
}

function failureChatBubble(message) {
  const failure = typeof message === "object" && message !== null ? message : { message };
  if (failure.sessionRefresh) {
    return sessionRefreshFailureBubble(failure);
  }
  return assistantChatBubble(
    {
      turnId: "adp-failure",
      lifecycle: { className: "failed", label: "失败", isLive: false },
    },
    [{
      kind: "error",
      title: "连接",
      body: [failure.message || message],
      status: "失败",
    }],
  );
}

function sessionRefreshFailureBubble(failure) {
  const body = [
    failure.message || "选中会话刷新失败",
    "这是选中 transcript 的刷新错误，不是全局连接失败。可以新建会话、返回会话列表，或先关闭这条错误提示。",
  ];
  const article = assistantChatBubble(
    {
      turnId: "session-refresh-failure",
      lifecycle: { className: "failed", label: "会话刷新失败", isLive: false },
    },
    [{
      kind: "error",
      title: "会话刷新",
      body,
      status: "失败",
    }],
  );
  const bar = document.createElement("div");
  bar.className = "turn-action-bar session-refresh-action-bar";
  const newButton = turnActionButton("新建会话");
  newButton.addEventListener("click", () => {
    exitSessionRefreshErrorToNewConversation().catch((error) => {
      setCommandStatus(`新建会话失败：${error.message}`, { stickyMs: 9000 });
    });
  });
  const listButton = turnActionButton("返回会话列表");
  listButton.addEventListener("click", returnToSessionListFromRefreshError);
  const dismissButton = turnActionButton("忽略错误");
  dismissButton.addEventListener("click", dismissSessionRefreshError);
  bar.append(newButton, listButton, dismissButton);
  article.appendChild(bar);
  return article;
}

function workerTranscriptWaitingBubble(error) {
  const detail = error || {};
  const body = [
    "工作器记录 尚未持久化；任务面板 仍显示该 工作器任务处于活动状态。",
  ];
  const taskLine = [detail.task_id, detail.task_status, detail.assignee_agent_id]
    .filter(Boolean)
    .join(" · ");
  if (taskLine) {
    body.push(`任务面板: ${taskLine}`);
  }
  if (detail.session_id) {
    body.push(`工作器会话：${detail.session_id}`);
  }
  if (detail.message) {
    body.push(`最近刷新：${compactSentence(detail.message, 180)}`);
  }
  body.push("正在刷新同一个 owner 投影的 工作器会话；这不是任务派发失败。");
  return assistantChatBubble(
    {
      turnId: "worker-transcript-waiting",
      lifecycle: { className: "running", label: "等待 工作器记录", isLive: true },
    },
    [{
      kind: "system",
      title: "工作器记录",
      body,
      status: "等待中",
    }],
  );
}

function loadingConversationBubble() {
  return assistantChatBubble(
    {
      turnId: "session-refresh-loading",
      lifecycle: { className: "running", label: "加载会话中", isLive: true },
    },
    [{
      kind: "system",
      title: "会话",
      body: ["正在从运行时真源加载选中会话 transcript。"],
      status: "加载中",
    }],
  );
}

function turnExecutionCard(renderTurn) {
  const lifecycle = renderTurn.lifecycle;
  const status = { className: lifecycle.className, label: lifecycle.label };
  const article = executionShell({ status, live: lifecycle.isLive });
  article.dataset.turnId = renderTurn.turnId || "";
  if (lifecycle.neutral) {
    article.classList.add("pending-state");
  }
  const body = article.querySelector(".execution-body");
  if (renderTurn.rows.length === 0) {
    body.appendChild(executionRow({ kind: "system", title: "Turn", body: ["等待投影。"], status: lifecycle.label }));
    return article;
  }
  renderTurn.rows.forEach((row) => {
    body.appendChild(executionRow(row));
  });
  return article;
}

function turnChatCards(renderTurn) {
  const cards = [];
  let assistantRows = [];
  const flushAssistant = () => {
    if (assistantRows.length === 0) {
      return;
    }
    cards.push(assistantChatBubble(renderTurn, assistantRows));
    assistantRows = [];
  };

  renderTurn.rows.forEach((row) => {
    if (row.kind === "user") {
      flushAssistant();
      cards.push(userChatBubble(renderTurn, row));
      return;
    }
    assistantRows.push(row);
  });
  flushAssistant();

  if (cards.length === 0) {
    cards.push(assistantChatBubble(renderTurn, [{
      kind: "system",
      title: "Turn",
      body: ["等待投影。"],
      status: renderTurn.lifecycle.label,
      identity: { turnId: renderTurn.turnId },
    }]));
  }
  return cards;
}

function userChatBubble(renderTurn, row) {
  const article = document.createElement("article");
  article.className = `chat-message chat-message-user ${renderTurn.lifecycle.className}-state`;
  article.dataset.turnId = renderTurn.turnId || "";

  const meta = document.createElement("div");
  meta.className = "chat-message-meta";
  const label = document.createElement("span");
  label.className = "chat-role-label";
  label.textContent = "用户";
  const status = document.createElement("span");
  status.className = "chat-row-status";
  status.textContent = row.status || "";
  meta.append(label, status);
  appendChatMessageTime(meta, renderTurn);

  const body = document.createElement("div");
  body.className = "chat-message-body";
  renderTextLines(body, row.body || []);

  article.append(meta, body);
  appendTurnActionBar(article, renderTurn, row);
  return article;
}

function assistantChatBubble(renderTurn, rows) {
  const lifecycle = renderTurn.lifecycle;
  const className = chatAssistantStateClass(lifecycle, rows);
  const article = document.createElement("article");
  article.className = `chat-message chat-message-assistant ${className}-state`;
  article.dataset.turnId = renderTurn.turnId || "";
  if (lifecycle.isLive) {
    article.dataset.live = "true";
  }

  const meta = document.createElement("div");
  meta.className = "chat-message-meta";
  const label = document.createElement("span");
  label.className = "chat-role-label";
  label.textContent = "助手";
  const status = document.createElement("span");
  status.className = `chat-state-pill ${className}`;
  status.textContent = chatAssistantStatusLabel(lifecycle, rows);
  meta.append(label, status);
  appendChatMessageTime(meta, renderTurn);
  article.appendChild(meta);

  rows.forEach((row) => {
    article.appendChild(chatAssistantSection(row));
  });
  // Assistant output has no editable/forkable/rerunnable input: the edit,
  // fork and rerun actions belong to the user message that started the turn,
  // so the turn action bar is intentionally omitted here.
  return article;
}

function appendTurnActionBar(article, renderTurn, rows) {
  if (renderTurn && renderTurn.turnId === "session-refresh-failure") {
    return;
  }
  const bar = document.createElement("div");
  bar.className = "turn-action-bar";
  bar.dataset.turnId = renderTurn.turnId || "";
  const copyButton = turnActionButton("复制");
  copyButton.addEventListener("click", () => {
    copyTurnActionText(renderTurn, rows).catch((error) => {
      setCommandStatus(`复制失败：${error.message}`, { stickyMs: 6000 });
    });
  });
  const editButton = turnActionButton("从这里编辑重跑");
  editButton.addEventListener("click", () => {
    editAndRerunFromTurn(renderTurn).catch((error) => {
      setCommandStatus(`从这里编辑失败：${error.message}`, { stickyMs: 9000 });
    });
  });
  const newSessionButton = turnActionButton("从这里新建会话");
  newSessionButton.addEventListener("click", () => {
    newSessionFromTurn(renderTurn, rows).catch((error) => {
      setCommandStatus(`从这里新建会话失败：${error.message}`, { stickyMs: 9000 });
    });
  });
  bar.append(copyButton, editButton, newSessionButton);
  article.appendChild(bar);
}

function turnActionButton(label) {
  const button = document.createElement("button");
  button.className = "turn-action-button";
  button.type = "button";
  button.textContent = label;
  return button;
}

async function copyTurnActionText(renderTurn, rows) {
  const text = turnActionText(renderTurn, rows);
  if (!text) {
    setCommandStatus("没有可复制内容", { stickyMs: 4000 });
    return;
  }
  await copyTextToClipboard(text);
  setCommandStatus("已复制 turn 文本", { stickyMs: 3000 });
}

async function editAndRerunFromTurn(renderTurn) {
  if (!renderTurn || !renderTurn.turnId) {
    setCommandStatus("选中的 turn 没有持久化 ID", { stickyMs: 6000 });
    return;
  }
  if (!state.selectedSessionId || isDraftSessionId(state.selectedSessionId)) {
    setCommandStatus("从这里编辑需要选中持久化会话", { stickyMs: 6000 });
    return;
  }
  const userText = turnUserTextForAction(renderTurn.turnId);
  if (!userText) {
    setCommandStatus("选中的 turn 没有可编辑用户提示词", { stickyMs: 6000 });
    return;
  }
  await rollbackEffectiveTranscriptThroughTurn(renderTurn.turnId);
  composerInput.value = userText;
  composerInput.focus();
  setCommandStatus("已回滚到该 turn；编辑后发送替换内容", { stickyMs: 8000 });
}

async function newSessionFromTurn(renderTurn, rows) {
  const text = turnUserTextForAction(renderTurn && renderTurn.turnId) || turnActionText(renderTurn, rows);
  await startNewConversationFromText(text);
}

function turnActionText(renderTurn, rows) {
  const userText = turnUserTextForAction(renderTurn && renderTurn.turnId);
  if (userText) {
    return userText;
  }
  const rowList = Array.isArray(rows) ? rows : [rows];
  return rowList
    .flatMap((row) => (row && Array.isArray(row.body) ? row.body : []))
    .map((line) => `${line || ""}`.trim())
    .filter(Boolean)
    .join("\n\n");
}

function turnUserTextForAction(turnId) {
  const targetBase = baseTurnId(turnId);
  const turn = conversationTurnsForRender().find((candidate) => baseTurnId(candidate && candidate.turn_id) === targetBase);
  return `${(turn && turn.user_text) || ""}`.trim();
}

async function rollbackEffectiveTranscriptThroughTurn(turnId) {
  const targetBase = baseTurnId(turnId);
  for (let guard = 0; guard < 50; guard += 1) {
    const turns = conversationTurnsForRender();
    const hasTarget = turns.some((turn) => baseTurnId(turn && turn.turn_id) === targetBase);
    if (!hasTarget) {
      return;
    }
    await adpCommand(adpCommandOf("RollbackLatestSessionTurn", { session_id: state.selectedSessionId }));
    await refreshSessions();
    await refreshSelectedSession();
  }
  throw new Error("选中 turn 移除前触发了回滚保护");
}

async function startNewConversationFromText(text) {
  const draftText = `${text || ""}`.trim();
  if (!draftText) {
    setCommandStatus("选中的 turn 没有可用于新会话的文本", { stickyMs: 6000 });
    return;
  }
  await startNewConversation();
  composerInput.value = draftText;
  composerInput.focus();
  setCommandStatus("新会话已就绪；可基于复制的 turn 编辑发送", { stickyMs: 7000 });
}

async function copyTextToClipboard(text) {
  if (navigator.clipboard && typeof navigator.clipboard.writeText === "function") {
    await navigator.clipboard.writeText(text);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand("copy");
  textarea.remove();
  if (!copied) {
    throw new Error("剪贴板不可用");
  }
}

function appendChatMessageTime(meta, renderTurn) {
  const ms = timestampToMilliseconds(renderTurn && renderTurn.createdAt);
  if (!ms) {
    return;
  }
  const time = document.createElement("time");
  time.className = "chat-message-time";
  time.dateTime = new Date(ms).toISOString();
  time.textContent = localChatTimeLabel(ms);
  meta.appendChild(time);
}

function timestampToMilliseconds(timestamp) {
  const value = Number(timestamp || 0);
  if (!Number.isFinite(value) || value <= 0) {
    return null;
  }
  return value > 10_000_000_000 ? value : value * 1000;
}

function localChatTimeLabel(ms) {
  const date = new Date(ms);
  if (!Number.isFinite(date.getTime())) {
    return "";
  }
  const now = new Date();
  const sameDay =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();
  const options = sameDay
    ? { hour: "2-digit", minute: "2-digit" }
    : { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" };
  return new Intl.DateTimeFormat(undefined, options).format(date);
}

function chatAssistantStateClass(lifecycle, rows) {
  if (rows.some((row) => row.kind === "error")) {
    return "failed";
  }
  if (rows.some((row) => row.kind === "tool" && `${row.status || ""}`.toLowerCase().includes("失败"))) {
    return "failed";
  }
  if (lifecycle.className === "failed") {
    return "failed";
  }
  if (lifecycle.className === "running") {
    return "running";
  }
  if (lifecycle.className === "success") {
    return "success";
  }
  return "pending";
}

function chatAssistantStatusLabel(lifecycle, rows) {
  const failedTool = rows.find((row) => row.kind === "tool" && `${row.status || ""}`.toLowerCase().includes("失败"));
  if (failedTool) {
    return "工具失败";
  }
  if (lifecycle.isLive) {
    return lifecycle.label || "运行中";
  }
  const waitingFinal = rows.find((row) =>
    row.kind === "final" && ["running", "pending", "waiting_user", "waiting_for_user_options"].includes(`${row.status || ""}`.toLowerCase())
  );
  if (waitingFinal) {
    return waitingFinal.statusText || waitingFinal.label || waitingFinal.status || lifecycle.label || "等待中";
  }
  const finalRow = rows.find((row) => row.kind === "final");
  if (finalRow) {
    return finalRow.status || lifecycle.label || "已完成";
  }
  return lifecycle.label || "已接收";
}

function chatAssistantSection(row) {
  const section = document.createElement("section");
  section.className = `chat-section chat-section-${row.kind}`;
  if (row.identity && row.identity.turnId) {
    section.dataset.turnId = row.identity.turnId;
  }
  if (row.identity && row.identity.toolCallId) {
    section.dataset.toolCallId = row.identity.toolCallId;
  }

  if (row.kind === "tool") {
    renderToolSection(section, row);
    return section;
  }

  if (row.kind === "system") {
    section.classList.add("chat-section-reasoning");
  }

  const body = document.createElement("div");
  body.className = row.kind === "system" ? "chat-reasoning-body" : "chat-message-body";
  const headingLabel = assistantSectionHeadingLabel(row);
  const showSectionStatus = row.kind !== "assistant" && row.status;
  if (headingLabel || showSectionStatus) {
    const heading = document.createElement("div");
    heading.className = "chat-section-heading";
    if (headingLabel) {
      heading.textContent = headingLabel;
    }
    if (showSectionStatus) {
      const status = document.createElement("span");
      status.className = "chat-row-status";
      status.textContent = row.status;
      heading.appendChild(status);
    }
    section.appendChild(heading);
  }
  if (row.kind === "final") {
    renderFinalSummary(body, row.body || []);
  } else {
    renderTextLines(body, row.body || []);
  }
  section.appendChild(body);
  renderUserOptionButtons(section, row);
  return section;
}

function assistantSectionHeadingLabel(row) {
  if (!row) {
    return "";
  }
  if (row.kind === "final") {
    if (row.title) {
      return row.title;
    }
    const status = `${row.status || ""}`.toLowerCase();
    if (status === "running") {
      return "生命周期";
    }
    if (status === "waiting_user" || status === "waiting_for_user_options") {
      return "等待用户选择";
    }
    return "最终结果";
  }
  if (row.kind === "system") {
    return row.title || "Model";
  }
  if (row.kind === "error") {
    return row.title || "Error";
  }
  // Plain assistant text already lives inside an assistant bubble with a meta header.
  // Repeating an inner "Assistant" heading adds no new semantics and renders as duplication.
  return "";
}

function renderTextLines(container, lines) {
  const text = Array.isArray(lines) ? lines.join("\n") : `${lines || ""}`;
  const chunks = text.split(/\n{2,}/).map((chunk) => chunk.trim()).filter(Boolean);
  if (chunks.length === 0) {
    container.textContent = "";
    return;
  }
  chunks.forEach((chunk) => {
    const paragraph = document.createElement("p");
    paragraph.textContent = chunk;
    container.appendChild(paragraph);
  });
}

function renderFinalSummary(container, lines) {
  container.classList.add("final-summary");
  const text = Array.isArray(lines) ? lines.join("\n") : `${lines || ""}`;
  const blocks = finalSummaryBlocks(text);
  if (blocks.length === 0) {
    container.textContent = "";
    return;
  }
  blocks.forEach((block, index) => {
    const item = document.createElement("div");
    item.className = [
      "final-summary-item",
      index === 0 ? "final-summary-lead" : "",
      block.label ? "" : "final-summary-plain",
    ].filter(Boolean).join(" ");
    if (block.label) {
      const label = document.createElement("span");
      label.className = "final-summary-label";
      label.textContent = block.label;
      const value = document.createElement("span");
      value.className = "final-summary-value";
      value.textContent = block.text;
      item.append(label, value);
    } else {
      item.textContent = block.text;
    }
    container.appendChild(item);
  });
}

function finalSummaryBlocks(text) {
  return `${text || ""}`
    .split(/\n+/)
    .flatMap((line) => normalizeFinalSummaryLine(line))
    .map((line) => parseFinalSummaryLine(line))
    .filter((block) => block.text || block.label);
}

function normalizeFinalSummaryLine(line) {
  const normalized = `${line || ""}`.replace(/\s+/g, " ").trim();
  return normalized ? [normalized] : [];
}

function parseFinalSummaryLine(line) {
  const labelMatch = `${line || ""}`.match(/^([^:：]{2,18})[:：]\s*(.*)$/);
  if (labelMatch) {
    return {
      label: labelMatch[1].trim(),
      text: labelMatch[2].trim(),
    };
  }
  const severityMatch = `${line || ""}`.match(/^[（(]([^）)]+)[）)]\s*(.+)$/);
  if (severityMatch) {
    return {
      label: severityMatch[1].trim(),
      text: severityMatch[2].trim(),
    };
  }
  const numberMatch = `${line || ""}`.match(/^((?:\d+|[一二三四五六七八九十]+)[.、])\s*(.+)$/);
  if (numberMatch) {
    return {
      label: numberMatch[1].trim(),
      text: numberMatch[2].trim(),
    };
  }
  return {
    label: "",
    text: `${line || ""}`.trim(),
  };
}

function renderUserOptionButtons(section, row) {
  const options = (row && Array.isArray(row.userOptions) && row.userOptions) || [];
  if (row.kind !== "final" || options.length === 0) {
    return;
  }
  const wrap = document.createElement("div");
  wrap.className = "user-options";
  const heading = document.createElement("div");
  heading.className = "user-options-heading";
  heading.textContent = "选择一项继续";
  wrap.appendChild(heading);
  const list = document.createElement("div");
  list.className = "user-options-list";
  options.forEach((option) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "user-option-button";
    button.textContent = option;
    button.addEventListener("click", async () => {
      if (button.disabled) {
        return;
      }
      list.querySelectorAll(".user-option-button").forEach((btn) => {
        btn.disabled = true;
      });
      try {
        await submitUserInput(option);
      } catch (error) {
        list.querySelectorAll(".user-option-button").forEach((btn) => {
          btn.disabled = false;
        });
        setCommandStatus(`选项提交失败：${error && error.message ? error.message : error}`, { stickyMs: 5000 });
      }
    });
    list.appendChild(button);
  });
  wrap.appendChild(list);
  section.appendChild(wrap);
}

function renderToolSection(section, row) {
  const stateClass = toolStateClass(row.status);
  section.classList.add(stateClass);
  const display = row.display || null;
  if (display && display.kind) {
    section.dataset.toolKind = toolKindLabel(display.kind);
  }
  const head = document.createElement("div");
  head.className = "tool-chat-head";
  const titleWrap = document.createElement("span");
  titleWrap.className = "tool-chat-title-wrap";
  const title = document.createElement("span");
  title.className = "tool-chat-title";
  title.textContent = (display && display.action) || row.title || "Tool";
  const kind = document.createElement("span");
  kind.className = "tool-chat-kind";
  kind.textContent = toolKindLabel(display && display.kind);
  titleWrap.append(title, kind);
  const state = document.createElement("span");
  state.className = `tool-chat-state ${stateClass}`;
  state.textContent = row.status || "";
  head.append(titleWrap, state);

  const body = document.createElement("div");
  body.className = "tool-chat-body";
  const primary = toolPrimaryLine(row);
  if (primary) {
    const primaryNode = document.createElement("div");
    primaryNode.className = "tool-chat-line tool-chat-line-primary";
    primaryNode.textContent = primary;
    primaryNode.title = primary;
    body.appendChild(primaryNode);
  }
  toolSemanticLines(row).forEach((line) => {
    body.appendChild(toolSemanticLineNode(line));
  });
  section.append(head, body);
}

function toolStateClass(status) {
  const normalized = `${status || ""}`.toLowerCase();
  if (normalized.includes("失败") || normalized === "failed") {
    return "failed";
  }
  if (normalized.includes("等待") || normalized === "等待中") {
    return "running";
  }
  return "success";
}

function toolSemanticLines(row) {
  const lines = Array.isArray(row.body) ? row.body : [];
  const rendered = lines
    .map((line) => `${line || ""}`.trim())
    .filter(Boolean)
    .map((line) => ({
      text: line,
      tone: /^(result|failure):/i.test(line) ? toolStateClass(row.status) : "",
    }));
  return rendered.length > 0 ? rendered : [{ text: row.status || "tool activity" }];
}

function toolPrimaryLine(row) {
  const display = row.display || null;
  const action = `${(display && display.action) || row.title || "Tool"}`.trim();
  const target = display && display.target ? `${display.target}`.trim() : "";
  const parameterSummary = display && display.parameter_summary ? `${display.parameter_summary}`.trim() : "";
  if (target) {
    return `${action} -> ${target}`;
  }
  if (parameterSummary) {
    return `${action} -> ${parameterSummary}`;
  }
  return action;
}

function toolSemanticLineNode(line) {
  const item = document.createElement("div");
  item.className = [
    "tool-chat-line",
    "tool-chat-line-secondary",
    line.tone ? `tool-chat-line-${line.tone}` : "",
  ].filter(Boolean).join(" ");
  item.textContent = line.text;
  item.title = line.fullText || line.text;
  return item;
}

function toolKindLabel(kind) {
  return `${kind || "tool"}`
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/_/g, " ")
    .toLowerCase();
}

function truncateForChat(value, maxLength) {
  const text = `${value || ""}`.replace(/\s+/g, " ").trim();
  if (text.length <= maxLength) {
    return text;
  }
  return `${text.slice(0, Math.max(0, maxLength - 1)).trimEnd()}…`;
}

function commandReceiptStatus(receipt) {
  const rawStatus = `${(receipt && receipt.dispatch_status) || ""}`.trim();
  const statusCode = commandReceiptCode(rawStatus);
  if (!statusCode) {
    return "不支持的命令回执：缺少派发状态";
  }
  switch (statusCode) {
    case "reason_live_turn_cancel_requested":
      return "已请求停止";
    case "reason_turn_cancelled":
      return "请求已取消";
    case "runtime_checkpoint_rewound":
      return "检查点已恢复";
    case "session_metadata_updated":
    case "session_turn_rolled_back":
      return "会话已更新";
    case "reason_turn_started":
      return "请求已接收";
    case "reason_live_turn_completed":
      return "请求已完成";
    case "provider_config_saved_restart_required":
    case "provider_config_upserted_restart_required":
    case "agent_provider_selection_saved_restart_required":
    case "model_group_config_upserted_restart_required":
    case "model_group_selection_saved_restart_required":
    case "agent_resource_config_saved_restart_required":
    case "account_config_pulled":
    case "account_config_pushed":
      return "设置已保存";
    case "node_direct_message_dispatched":
      return "Worker 消息已发送";
    case "worker_control_applied":
      return "工作器控制已接收";
    case "task_agent_created":
      return "Worker 已更新";
    case "task_created":
    case "task_assigned":
    case "task_claimed":
    case "task_review_submitted":
    case "task_review_rejected":
    case "task_review_approved":
    case "task_closed":
    case "execution_fact_applied":
    case "scheduler_tick_recorded":
    case "master_poll_recorded":
      return "任务已更新";
    case "queued_by_static_dispatch_port":
      return "命令已排队";
    default:
      return `不支持的命令回执：${truncateForChat(statusCode, 80)}`;
  }
}

function commandReceiptCode(dispatchStatus) {
  return `${dispatchStatus || ""}`
    .trim()
    .toLowerCase()
    .split(/[:\s]/, 1)[0];
}

function executionShell({ status, live }) {
  const article = document.createElement("article");
  article.className = `dialog-block execution-block ${status.className}-state`;
  if (live) {
    article.dataset.live = "true";
  }

  const head = document.createElement("div");
  head.className = "block-head execution-head";

  const title = document.createElement("span");
  title.className = "role-badge assistant-badge";
  title.textContent = "Turn";

  const stateBadge = document.createElement("span");
  stateBadge.className = `block-state ${status.className}`;
  stateBadge.textContent = status.label;

  head.append(title, stateBadge);

  const body = document.createElement("div");
  body.className = "execution-body";

  article.append(head, body);
  return article;
}

function executionRow(renderRow) {
  const row = document.createElement("section");
  row.className = `execution-row execution-row-${renderRow.kind}`;
  if (renderRow.identity && renderRow.identity.turnId) {
    row.dataset.turnId = renderRow.identity.turnId;
  }
  if (renderRow.identity && renderRow.identity.toolCallId) {
    row.dataset.toolCallId = renderRow.identity.toolCallId;
  }

  const meta = document.createElement("div");
  meta.className = "execution-row-meta";

  const label = document.createElement("span");
  label.className = "execution-row-label";
  label.textContent = renderRow.title;
  meta.appendChild(label);

  if (renderRow.status) {
    const state = document.createElement("span");
    state.className = "execution-row-status";
    state.textContent = renderRow.status;
    meta.appendChild(state);
  }

  const content = document.createElement("div");
  content.className = "execution-row-body";
  if (renderRow.kind === "tool") {
    renderToolBody(content, renderRow.body);
  } else {
    content.textContent = (renderRow.body || []).join("\n");
  }

  row.append(meta, content);
  return row;
}

function renderToolBody(container, body) {
  const lines = Array.isArray(body)
    ? body.filter((line) => `${line || ""}`.length > 0)
    : `${body || ""}`.split("\n").filter((line) => line.length > 0);
  if (lines.length === 0) {
    container.textContent = "";
    return;
  }
  lines.forEach((line, index) => {
    const lineNode = document.createElement("div");
    lineNode.className = index === 0 ? "tool-primary-line" : "tool-secondary-line";
    lineNode.textContent = line;
    container.appendChild(lineNode);
  });
}

function normalizePublicConversation(items) {
  return Array.isArray(items) ? items.filter(Boolean) : [];
}

function buildConversationRenderModel() {
  const conversationTurns = conversationTurnsForRender();
  const turnsForRender =
    conversationTurns.length === 0 && state.turn
      ? [state.turn]
      : conversationTurns;
  const workerTask = taskForWorkerSessionId(state.selectedSessionId);
  const recoveredFailureTurnIds = recoveredHistoricalWorkerFailureTurnIds(turnsForRender, {
    isWorkerSession: !!workerTask,
    taskStatus: workerTask && workerTask.status,
  });
  const pendingStartedAt =
    state.submitStartedAt || state.ambiguousSubmitRecoveryStartedAt || Date.now();
  return {
    selectedSessionId: state.selectedSessionId,
    turns: turnsForRender.map((turn) => buildRenderTurn(turn, {
      historicalFailureRecovered: recoveredFailureTurnIds.has(turn.turn_id),
    })),
    pendingSubmit: state.pendingUserInput
      ? {
          text: state.pendingUserInput,
          attachments: state.pendingAttachments,
          isLive: state.submitInFlight,
          elapsed: elapsedSince(state.submitStartedAt),
          startedAt: pendingStartedAt,
          sessionId: state.pendingSubmitSessionId || state.selectedSessionId || "",
          submitId: state.pendingSubmitId || "",
          error: state.pendingSubmitError,
        }
      : null,
    acceptedSubmitReceipt: acceptedSubmitReceiptForRender(),
    sessionLoading: selectedSessionIsLoading(),
    sessionRefreshError: selectedSessionRefreshErrorForRender(),
    adpFailure: state.adpFailure
      ? { message: state.adpFailure }
      : selectedSessionRefreshFailureForRender(),
  };
}

function acceptedSubmitReceiptForRender() {
  const receipt = state.acceptedSubmitReceipt;
  if (!receipt || !receipt.sessionId || receipt.sessionId !== state.selectedSessionId) {
    return null;
  }
  const expected = normalizeVisibleText(receipt.text);
  if (
    expected &&
    conversationTurnsForRender().some((turn) => turnContainsVisibleUserText(turn, expected))
  ) {
    return null;
  }
  return receipt;
}

function selectedSessionRefreshErrorForRender() {
  if (
    state.sessionRefreshError &&
    state.selectedSessionId &&
    state.sessionRefreshError.session_id === state.selectedSessionId
  ) {
    return state.sessionRefreshError;
  }
  return null;
}

function selectedSessionRefreshFailureForRender() {
  const error = selectedSessionRefreshErrorForRender();
  if (!error || selectedWorkerTranscriptRefreshRetryable(error.session_id)) {
    return null;
  }
  return {
    sessionRefresh: true,
    sessionId: error.session_id || state.selectedSessionId || "",
    message: error.message || "选中会话刷新失败",
  };
}

function selectedSessionIsLoading() {
  return Boolean(
    state.selectedSessionId &&
      ((state.sessionRefreshInFlight === state.selectedSessionId &&
        (!state.sessionRefreshError || state.sessionRefreshError.session_id !== state.selectedSessionId)) ||
        selectedWorkerTranscriptRefreshRetryable(state.selectedSessionId)),
  );
}

function baseTurnId(turnId) {
  const parsed = turnOrderKey(turnId);
  if (parsed.round <= 1) {
    return parsed.raw;
  }
  return `${parsed.prefix}${parsed.ordinal}`;
}

function buildRenderTurn(turn, options = {}) {
  const historicalFailureRecovered = options.historicalFailureRecovered === true;
  const lifecycle = historicalFailureRecovered
    ? historicalFailureRecoveredLifecycle()
    : turnLifecycleForRender(turn);
  const rows = buildRenderRows(turn, lifecycle);
  return {
    turnId: turn.turn_id || "",
    sessionId: turn.session_id || "",
    submitId: turn.submit_id || "",
    createdAt: turn.created_at || null,
    timing: turnTimingProjection(turn),
    sourceTurn: turn,
    orderKey: turnOrderKey(turn.turn_id),
    lifecycle,
    recoveryState: historicalFailureRecovered ? HISTORICAL_FAILURE_RECOVERED : "",
    recoveryDebugDetails: historicalFailureRecovered && state.debugDetailsVisible,
    rows: historicalFailureRecovered && !state.debugDetailsVisible
      ? historicalFailureRecoveredRows(rows)
      : rows,
  };
}

function buildRenderRows(turn, lifecycle) {
  const rows = conversationItemsForTurn(turn).map((item) =>
    buildRenderRowFromConversationItem(turn, item),
  );
  const modelRow = buildModelRequestRenderRow(turn, lifecycle);
  if (modelRow) {
    rows.push(modelRow);
  }
  if (rows.length === 0 && turnIsCurrentLiveTurn(turn)) {
    rows.push(buildObservableLiveTurnRenderRow(turn, lifecycle));
  }
  return rows;
}

function buildRenderRowFromConversationItem(turn, item) {
  if (item.kind === "ToolSummary") {
    return buildToolActivityRenderRow(turn, item);
  }
  if (item.kind === "UserText") {
    return {
      kind: "user",
      title: item.title,
      body: [item.body],
      status: item.status,
      identity: { turnId: turn.turn_id },
    };
  }
  if (item.kind === "Terminal") {
    return buildTerminalRenderRow(turn, item);
  }
  if (item.kind === "Error") {
    return {
      kind: "error",
      title: item.title,
      body: [item.body],
      status: item.status,
      identity: { turnId: turn.turn_id },
    };
  }
  return {
    kind: "assistant",
    title: item.title,
    body: [item.body],
    status: assistantRowStatus(turn, item.status),
    identity: { turnId: turn.turn_id },
  };
}

function buildToolActivityRenderRow(turn, item) {
  const status = `${item.status || ""}`.toLowerCase();
  const canShowTiming =
    status === "completed" || status === "failed" || (status === "waiting" && turnIsCurrentLiveTurn(turn));
  const timing = item.tool_call_id && canShowTiming
    ? state.toolTimings.get(toolTimingKey(turn, item.tool_call_id))
    : null;
  return {
    kind: "tool",
    title: item.title,
    body: toolSummaryBodyLines(item),
    status: toolTimelineLine(item, timing) || item.status,
    display: item.display || null,
    identity: { turnId: turn.turn_id, toolCallId: item.tool_call_id },
  };
}

function buildModelRequestRenderRow(turn, lifecycle) {
  if (!turnIsWaitingForModelResponse(turn) || !turn.model_request) {
    return null;
  }
  return {
    kind: "system",
    title: modelRequestTitle(turn),
    body: modelRequestDisplayLines(turn),
    status: lifecycle.isLive ? lifecycle.elapsed || "0s" : modelRequestStaticStatus(turn),
    identity: { turnId: turn.turn_id },
  };
}

function modelRequestTitle(turn) {
  return modelRequestPhase(turn) === "schema_retry" ? "Schema 修复" : "模型";
}

function modelRequestDisplayLines(turn) {
  const request = (turn && turn.model_request) || {};
  const lines = [];
  const mainDetail = request.detail || "等待模型响应。";
  if (mainDetail) {
    lines.push(mainDetail);
  }
  const timingLine = turnTimingLine(turn, { includeLiveWait: true });
  if (timingLine) {
    lines.push(`耗时：${timingLine}`);
  }
  const transport = modelRequestTransport(turn);
  if (transport && transport.detail) {
    lines.push(`${modelRequestTransportLabel(transport)}：${transport.detail}`);
  }
  return lines;
}

function modelRequestStaticStatus(turn) {
  const transportPhase = modelRequestTransportPhase(turn);
  if (transportPhase === "provider_retry") {
    return "传输重试中";
  }
  if (transportPhase === "provider_failover") {
    return "传输切换中";
  }
  const timing = turnTimingProjection(turn);
  if (timing && Number.isFinite(timing.timeToFirstResponseMs)) {
    return `等待 ${formatDuration(timing.timeToFirstResponseMs)}`;
  }
  return "等待中";
}

function buildObservableLiveTurnRenderRow(turn, lifecycle) {
  const status = lifecycle.elapsed ? `${lifecycle.label || "处理中"}... ${lifecycle.elapsed}` : lifecycle.label || "处理中";
  return {
    kind: "system",
    title: "Turn",
    body: ["请求已接收；等待协议可见的 turn 详情"],
    status,
    identity: { turnId: turn.turn_id },
  };
}

function buildTerminalRenderRow(turn, item) {
  return {
    kind: "final",
    title: item.title,
    body: [item.body],
    status: item.status,
    statusText: isAwaitingUserOptionsStatus(turn.terminal_status)
      ? "等待用户选择"
      : item.status,
    userOptions: (item.userOptions || []).slice(),
    identity: { turnId: turn.turn_id },
  };
}

function assistantRowStatus(turn, status) {
  if (isToolPendingStatus(turn.terminal_status)) {
    return toolPendingStatusLabelForTurn(turn);
  }
  if (turn.terminal_text || isTerminalStatus(turn.terminal_status)) {
    return terminalTurnStatusLabelForTurn(turn, turn.terminal_status);
  }
  if (turnIsCurrentLiveTurn(turn)) {
    return status || "流式响应中";
  }
  return status === "streaming" ? "已接收" : status;
}

function toolStatusLabel(status) {
  switch ((status || "").toLowerCase()) {
    case "waiting":
      return "等待中";
    case "completed":
      return "已完成";
    case "failed":
      return "执行失败";
    default:
      return status || "未知状态";
  }
}

function isTerminalStatus(status) {
  const normalized = `${status || ""}`.toLowerCase();
  return [
    "success",
    "failed",
    "blocked",
    "interrupted",
    "cancelled",
    "awaitinguseroptions",
  ].includes(normalized);
}

function isToolPendingStatus(status) {
  const normalized = `${status || ""}`.toLowerCase().replace(/[_-]/g, "");
  return normalized === "toolpending";
}

function isAwaitingUserOptionsStatus(status) {
  const normalized = `${status || ""}`.toLowerCase().replace(/[_-]/g, "");
  return normalized === "awaitinguseroptions";
}

function lifecycleOwnerTaskProjectionLoaded() {
  return state.taskBoard !== null;
}

function lifecycleOwnerTimerProjectionLoaded() {
  return state.timerList !== null;
}

function lifecycleOwnerProjectionLoaded() {
  return lifecycleOwnerTaskProjectionLoaded() && lifecycleOwnerTimerProjectionLoaded();
}

function turnHasWaitingToolActivity(turn) {
  return ((turn && turn.tool_activities) || []).some(
    (tool) => `${tool.status || ""}`.toLowerCase() === "waiting",
  );
}

function taskKeepsSessionLifecycleRunning(task) {
  const normalized = `${(task && task.status) || ""}`.toLowerCase();
  return [
    "created",
    "waiting_agent",
    "assigned",
    "running",
    "interrupted",
    "paused",
    "review_submitted",
    "rejected",
  ].includes(normalized);
}

function sessionHasOpenTaskLifecycle(sessionId) {
  const id = `${sessionId || ""}`.trim();
  if (!id || !state.taskBoard) {
    return false;
  }
  return ((state.taskBoard && state.taskBoard.tasks) || []).some((task) =>
    taskVisibleInSession(task, id) && taskKeepsSessionLifecycleRunning(task)
  );
}

function sessionHasOpenTimerLifecycle(sessionId) {
  const id = `${sessionId || ""}`.trim();
  if (!id || !state.timerList) {
    return false;
  }
  return ((state.timerList && state.timerList.timers) || []).some((timer) =>
    timer &&
      timer.source_session_id === id &&
      ["active", "running"].includes(`${timer.status || ""}`.toLowerCase())
  );
}

function sessionHasOpenLifecycleOwner(sessionId) {
  return sessionHasOpenTaskLifecycle(sessionId) || sessionHasOpenTimerLifecycle(sessionId);
}

function toolPendingRepresentsLifecycle(turn) {
  if (!turn || !isToolPendingStatus(turn.terminal_status)) {
    return false;
  }
  if (turnHasWaitingToolActivity(turn) || turn.model_request) {
    return true;
  }
  const sessionId = turn.session_id || state.selectedSessionId || "";
  if (sessionHasOpenTaskLifecycle(sessionId) || sessionHasOpenTimerLifecycle(sessionId)) {
    return true;
  }
  if (lifecycleOwnerTaskProjectionLoaded() && !sessionHasOpenTaskLifecycle(sessionId)) {
    return false;
  }
  return false;
}

function toolPendingStatusLabelForTurn(turn) {
  return toolPendingRepresentsLifecycle(turn) ? "等待生命周期" : "等待用户选择";
}

function toolPendingLifecycleForRender(turn) {
  if (toolPendingRepresentsLifecycle(turn)) {
    return {
      phase: "waiting_lifecycle",
      className: "running",
      label: "等待生命周期",
      isLive: false,
      elapsed: "",
    };
  }
  return {
    phase: "waiting_user",
    className: "pending",
    label: "等待用户选择",
    isLive: false,
    elapsed: "",
  };
}

function terminalTurnStatusLabelForTurn(turn, status) {
  if (isToolPendingStatus(status)) {
    return toolPendingStatusLabelForTurn(turn);
  }
  return terminalTurnStatusLabel(status);
}

function terminalTurnStatusLabel(status) {
  const normalized = `${status || ""}`.toLowerCase().replace(/[_-]/g, "");
  if (normalized === "toolpending" || normalized === "running") {
    return "等待生命周期";
  }
  if (normalized === "failed") {
    return "失败";
  }
  if (normalized === "blocked") {
    return "已阻塞";
  }
  if (normalized === "interrupted") {
    return "已中断";
  }
  if (normalized === "cancelled") {
    return "已取消";
  }
  if (normalized === "awaitinguseroptions") {
    return "等待用户选择";
  }
  return "已完成";
}

function maybeNotifyAndroidTurnFinished(previousTurn, nextTurn) {
  if (!nextTurn || document.body.dataset.layoutClient !== "android-webview") {
    return;
  }
  const turnKey = `${nextTurn.session_id || ""}:${nextTurn.turn_id || ""}`;
  if (!turnHasTerminalOutcome(nextTurn)) {
    state.androidObservedNonTerminalTurns.add(turnKey);
    return;
  }
  const sameTurnWasNonTerminal = previousTurn &&
    previousTurn.session_id === nextTurn.session_id &&
    previousTurn.turn_id === nextTurn.turn_id &&
    !turnHasTerminalOutcome(previousTurn);
  const observedLiveTurn = sameTurnWasNonTerminal || state.androidObservedNonTerminalTurns.has(turnKey);
  if (!observedLiveTurn) return;
  const bridge = window.FreehandAndroidNotifications;
  if (!bridge || typeof bridge.turnFinished !== "function") {
    return;
  }
  const key = `${nextTurn.session_id || ""}:${nextTurn.turn_id || ""}:${nextTurn.terminal_status || ""}`;
  if (state.androidNotifiedTurns.has(key)) {
    return;
  }
  state.androidNotifiedTurns.add(key);
  state.androidObservedNonTerminalTurns.delete(turnKey);
  persistAndroidNotifiedTurns();
  const label = terminalTurnStatusLabelForTurn(nextTurn, nextTurn.terminal_status);
  const summary = nextTurn.terminal_text || nextTurn.text?.slice(-1)?.[0] || label;
  bridge.turnFinished(JSON.stringify({
    sessionId: nextTurn.session_id || "",
    turnId: nextTurn.turn_id || "",
    status: `${nextTurn.terminal_status || ""}`,
    title: "任务已经完成",
    text: `${label}: ${String(summary).slice(0, 180)}`,
  }));
}

function formatDuration(ms) {
  if (!Number.isFinite(ms) || ms < 0) {
    return "";
  }
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  if (totalSeconds < 60) {
    return `${totalSeconds}s`;
  }
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes < 60) {
    return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
  }
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h ${String(remainingMinutes).padStart(2, "0")}m`;
}

function elapsedSince(startedAt) {
  if (!startedAt) {
    return "";
  }
  return formatDuration(Date.now() - startedAt);
}

function turnIsWaitingForModelResponse(turn) {
  return !!(
    turn &&
    turn.model_request &&
    !turn.terminal_text &&
    !isTerminalStatus(turn.terminal_status) &&
    !isToolPendingStatus(turn.terminal_status)
  );
}

function turnIsCurrentLiveTurn(turn) {
  return !!(
    turn &&
    protocolConnectionCanRenderLive() &&
    state.turn &&
    turn.turn_id === state.turn.turn_id &&
    turn.session_id === state.turn.session_id &&
    !turn.terminal_text &&
    !isTerminalStatus(turn.terminal_status) &&
    !isToolPendingStatus(turn.terminal_status)
  );
}

function modelRequestKind(turn) {
  return `${turn && turn.model_request && turn.model_request.kind ? turn.model_request.kind : "Thinking"}`.toLowerCase();
}

function modelRequestPhase(turn) {
  const kind = modelRequestKind(turn);
  if (kind === "schemaretry" || kind === "schema_retry") {
    return "schema_retry";
  }
  if (kind === "toolresultcontinuation" || kind === "tool_result_continuation") {
    return "tool_result_continuation";
  }
  return "thinking";
}

function modelRequestTimingKey(turn) {
  if (!turnIsWaitingForModelResponse(turn)) {
    return null;
  }
  return [turn.session_id || "", turn.turn_id || "", modelRequestPhase(turn)].join("|");
}

function modelRequestLabel(turn) {
  const kind = modelRequestKind(turn);
  if (kind === "schemaretry" || kind === "schema_retry") {
    return "Schema 修复中";
  }
  if (kind === "toolresultcontinuation" || kind === "tool_result_continuation") {
    return "工具结果后继续推理";
  }
  return "推理中";
}

function modelRequestTransport(turn) {
  const request = (turn && turn.model_request) || {};
  if (request.transport && request.transport.kind) {
    return request.transport;
  }
  return null;
}

function modelRequestTransportPhase(turn) {
  const transport = modelRequestTransport(turn);
  const kind = `${(transport && transport.kind) || ""}`.toLowerCase();
  if (kind === "providerretry" || kind === "provider_retry") {
    return "provider_retry";
  }
  if (kind === "providerfailover" || kind === "provider_failover") {
    return "provider_failover";
  }
  return "";
}

function modelRequestTransportLabel(transport) {
  const kind = `${(transport && transport.kind) || ""}`.toLowerCase();
  if (kind === "providerretry" || kind === "provider_retry") {
    return "传输重试";
  }
  if (kind === "providerfailover" || kind === "provider_failover") {
    return "传输切换";
  }
  return "传输";
}

function turnTimingProjection(turn) {
  const raw = (turn && turn.timing) || null;
  if (!raw || typeof raw !== "object") {
    return null;
  }
  const timing = {
    turnStartedAtMs: numberOrNull(raw.turn_started_at_ms ?? raw.turnStartedAtMs),
    firstResponseAtMs: numberOrNull(raw.first_response_at_ms ?? raw.firstResponseAtMs),
    completedAtMs: numberOrNull(raw.completed_at_ms ?? raw.completedAtMs),
    timeToFirstResponseMs: numberOrNull(raw.time_to_first_response_ms ?? raw.timeToFirstResponseMs),
    totalElapsedMs: numberOrNull(raw.total_elapsed_ms ?? raw.totalElapsedMs),
  };
  return Object.values(timing).some((value) => Number.isFinite(value)) ? timing : null;
}

function numberOrNull(value) {
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? number : null;
}

function turnTimingLine(turn, options = {}) {
  const timing = turnTimingProjection(turn);
  if (!timing) {
    return "";
  }
  const parts = [];
  const waitMs = turnWaitDurationMs(turn, timing, options);
  if (Number.isFinite(waitMs)) {
    parts.push(`等待 ${formatDuration(waitMs)}`);
  }
  if (Number.isFinite(timing.timeToFirstResponseMs)) {
    parts.push(`首字 ${formatDuration(timing.timeToFirstResponseMs)}`);
  }
  if (Number.isFinite(timing.totalElapsedMs)) {
    parts.push(`总耗时 ${formatDuration(timing.totalElapsedMs)}`);
  }
  return parts.join(" · ");
}

function turnWaitDurationMs(turn, timing, options = {}) {
  if (Number.isFinite(timing.timeToFirstResponseMs)) {
    return timing.timeToFirstResponseMs;
  }
  if (
    options.includeLiveWait &&
    turnIsWaitingForModelResponse(turn) &&
    Number.isFinite(timing.turnStartedAtMs)
  ) {
    return Math.max(0, Date.now() - timing.turnStartedAtMs);
  }
  return null;
}

function lifecycleClockStartedAt(key) {
  const clock = key ? state.lifecycleClocks.get(key) : null;
  return clock ? clock.startedAt : null;
}

function pruneLifecycleClocks(activeKeys) {
  const keep = new Set(activeKeys.filter(Boolean));
  for (const key of Array.from(state.lifecycleClocks.keys())) {
    if (!keep.has(key)) {
      state.lifecycleClocks.delete(key);
    }
  }
}

function syncRenderLifecycleClocks() {
  const activeKeys = [];
  conversationTurnsForRender().forEach((turn) => {
    if (turnIsCurrentLiveTurn(turn) && turnIsWaitingForModelResponse(turn)) {
      const key = modelRequestTimingKey(turn);
      activeKeys.push(key);
      if (!state.lifecycleClocks.has(key)) {
        state.lifecycleClocks.set(key, { startedAt: Date.now() });
      }
    }
  });
  pruneLifecycleClocks(activeKeys);
}

function syncToolTimings(turns) {
  const now = Date.now();
  const seen = new Set();

  turns.filter(Boolean).forEach((turn) => {
    const items = normalizePublicConversation(derivePublicConversation(turn));
    items.forEach((item) => {
      if (item.kind !== "ToolSummary" || !item.tool_call_id) {
        return;
      }
      const key = toolTimingKey(turn, item.tool_call_id);
      seen.add(key);
      const previous = state.toolTimings.get(key);
      const status = `${item.status || ""}`.toLowerCase();
      const isTerminalTool = status === "completed" || status === "failed";
      if (!previous) {
        state.toolTimings.set(key, {
          startedAt: now,
          finishedAt: isTerminalTool ? now : null,
          status: item.status,
        });
        return;
      }
      const next = { ...previous, status: item.status };
      if ((previous.status !== item.status || !previous.finishedAt) && isTerminalTool) {
        next.finishedAt = now;
      }
      state.toolTimings.set(key, next);
    });
  });

  for (const toolKey of Array.from(state.toolTimings.keys())) {
    if (!seen.has(toolKey)) {
      state.toolTimings.delete(toolKey);
    }
  }
}

function toolSummaryBody(item) {
  return toolSummaryBodyLines(item).join("\n");
}

function toolSummaryBodyLines(item) {
  const display = item.display || null;
  const lines = [];
  if (display && display.diff) {
    pushToolLine(lines, `diff target: ${display.diff.target || ""}`);
    pushToolLine(lines, `diff before: ${display.diff.before || ""}`);
    pushToolLine(lines, `diff after: ${display.diff.after || ""}`);
  }
  if (display && display.target) {
    pushToolLine(lines, `target: ${display.target}`);
  }
  if (display && display.parameter_summary) {
    pushToolLine(lines, `parameters: ${display.parameter_summary}`);
  }
  if (display && display.summary) {
    pushToolLine(lines, `summary: ${display.summary}`);
  }
  if (display && display.result_summary) {
    pushToolLine(lines, `result: ${display.result_summary}`);
  }
  if (display && Array.isArray(display.fields)) {
    display.fields.forEach((field) => {
      const label = `${field && field.label ? field.label : "field"}`.trim();
      const value = `${field && field.value ? field.value : ""}`.trim();
      if (label && value) {
        pushToolLine(lines, `${label}: ${value}`);
      }
    });
  }
  splitToolDetailLines(item.body).forEach((line) => pushToolLine(lines, line));
  return lines.filter(Boolean);
}

function toolTimelineLine(item, timing) {
  const status = `${item.status || ""}`.toLowerCase();
  if (!timing && status !== "waiting") {
    return "";
  }
  const label = toolStatusLabel(item.status);
  const endAt = timing && timing.finishedAt ? timing.finishedAt : Date.now();
  const elapsed = timing ? formatDuration(endAt - timing.startedAt) : "";
  return elapsed ? `${label} · ${elapsed}` : label;
}

function pushToolLine(lines, value) {
  const line = `${value || ""}`.trim();
  if (line) {
    lines.push(line);
  }
}

function splitToolDetailLines(value) {
  return `${value || ""}`
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function waitingToolStatus(tools, turn = state.turn) {
  const names = tools.map((tool) => tool.tool_name).join(", ");
  const elapsedValues = tools
    .map((tool) => {
      const timing = tool.tool_call_id ? state.toolTimings.get(toolTimingKey(turn, tool.tool_call_id)) : null;
      return timing ? Date.now() - timing.startedAt : null;
    })
    .filter((elapsed) => Number.isFinite(elapsed));
  const longestElapsed = elapsedValues.length > 0 ? Math.max(...elapsedValues) : null;
  const elapsed = longestElapsed === null ? "" : formatDuration(longestElapsed);
  return elapsed ? `工具执行中：${names} · ${elapsed}` : `工具执行中：${names}`;
}

function toolTimingKey(turn, toolCallId) {
  return `${turn && turn.turn_id ? turn.turn_id : "unknown"}|${toolCallId || "unknown"}`;
}

function variantPayload(value, variant) {
  if (!value || typeof value !== "object" || !(variant in value)) {
    return undefined;
  }
  return value[variant];
}

function derivePublicConversation(turn) {
  if (!turn) {
    return [];
  }
  const items = [];
  if (turn.user_text && !isInternalRuntimePrompt(turn)) {
    items.push({
      kind: "UserText",
      title: "用户",
      body: textWithSubmittedAttachmentDisplay(turn.user_text, turn.attachments || []),
      status: "已提交",
    });
  }
  const assistantBodies = [];
  (turn.text || []).forEach((text) => {
    const visibleText = stripFreehandCompletionBlock(text);
    if (visibleText) {
      assistantBodies.push(visibleText);
    }
  });
  if (assistantBodies.length > 0) {
    items.push({
      kind: "AssistantText",
      title: "助手",
      body: assistantBodies.join("\n"),
      status: "流式响应中",
    });
  }
  (turn.tool_activities || []).forEach((tool) => {
    const status = `${tool.status || "waiting"}`.toLowerCase();
    items.push({
      kind: "ToolSummary",
      title: tool.display && tool.display.action ? tool.display.action : tool.tool_name || "工具",
      body: tool.detail || status,
      status,
      tool_call_id: tool.tool_call_id,
      display: tool.display || null,
    });
  });
  if (turn.terminal_text) {
    const terminalStatus = `${turn.terminal_status || "Success"}`.toLowerCase();
    const isToolPending = isToolPendingStatus(terminalStatus);
    const toolPendingIsLifecycle = isToolPending && toolPendingRepresentsLifecycle(turn);
    const awaitingUserOptions = isAwaitingUserOptionsStatus(terminalStatus);
    const status =
      isToolPending
        ? toolPendingIsLifecycle ? "running" : "waiting_user"
        : awaitingUserOptions
          ? "waiting_for_user_options"
          : terminalStatus === "failed"
          ? "failed"
          : terminalStatus === "cancelled"
            ? "cancelled"
            : terminalStatus === "blocked"
              ? "blocked"
              : terminalStatus === "interrupted"
                ? "interrupted"
                : "completed";
    items.push({
      kind: "Terminal",
      title: isToolPending
        ? toolPendingIsLifecycle ? "生命周期" : "等待用户"
        : awaitingUserOptions ? "等待用户选择" : "最终结果",
      body: terminalBodyForDisplay(turn.terminal_text, {
        waitingUserToolPending: isToolPending && !toolPendingIsLifecycle,
      }),
      status: isToolPending ? toolPendingStatusLabelForTurn(turn) : status,
      userOptions: awaitingUserOptions ? turn.user_options || [] : [],
    });
  }
  (turn.errors || []).forEach((error) => {
    items.push({
      kind: "Error",
      title: "Error",
      body: error,
      status: "failed",
    });
  });
  return items;
}

function terminalBodyForDisplay(text, options = {}) {
  const stripped = stripFreehandCompletionBlock(text);
  if (state.debugDetailsVisible) {
    return stripped;
  }
  const summary = terminalSummaryBlock(stripped);
  const body = summary || stripDebugTerminalLines(stripped);
  return options.waitingUserToolPending ? normalizeWaitingUserToolPendingTerminalBody(body) : body;
}

function normalizeWaitingUserToolPendingTerminalBody(text) {
  return `${text || ""}`
    .replace(/^\s*Waiting for lifecycle\s*[:：]\s*/i, "等待用户选择：")
    .replace(/^\s*等待生命周期\s*[:：]\s*/, "等待用户选择：")
    .trim();
}

function terminalSummaryBlock(text) {
  const lines = `${text || ""}`.split(/\r?\n/);
  const summaryIndex = lines.findIndex((line) => /^summary\s*:/i.test(line.trim()));
  if (summaryIndex < 0) {
    return "";
  }
  const summaryLines = [lines[summaryIndex].replace(/^summary\s*:\s*/i, "")];
  for (let index = summaryIndex + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (/^\s*(evidence|learned|completion reason)\s*:/i.test(line)) {
      break;
    }
    summaryLines.push(line);
  }
  return summaryLines.join("\n").trim();
}

function stripDebugTerminalLines(text) {
  return `${text || ""}`
    .split(/\r?\n/)
    .filter((line) => !/^\s*(evidence|learned|completion reason)\s*:/i.test(line))
    .join("\n")
    .trim();
}

function stripFreehandCompletionBlock(text) {
  const stripped = `${text || ""}`
    .replace(/<freehand_completion>[\s\S]*?<\/freehand_completion>/g, "")
    .trim();
  if (stripped.includes("</freehand_completion>")) {
    return "";
  }
  return stripped;
}

function conversationItemsForTurn(turn) {
  const items = normalizePublicConversation(derivePublicConversation(turn));
  return items;
}

function isInternalRuntimePrompt(turn) {
  const text = `${turn && turn.user_text ? turn.user_text : ""}`.trim();
  if (!text) {
    return false;
  }
  return (
    turnOrderKey(turn.turn_id).round > 1 &&
    (text.startsWith("The tool result has been returned.") ||
      text.startsWith("Your Freehand completion schema was rejected."))
  );
}

function logicalSessionTurns(turns) {
  const merged = [];
  turns.filter(Boolean).forEach((turn) => {
    const index = merged.findIndex((existing) => sameRenderableTurn(existing, turn));
    if (index >= 0) {
      merged[index] = turn;
    } else {
      merged.push(turn);
    }
  });
  return merged;
}

function normalizeVisibleText(text) {
  return `${text || ""}`.replace(/\s+/g, " ").trim();
}

function turnContainsVisibleUserText(turn, expectedText) {
  const expected = normalizeVisibleText(expectedText);
  if (!turn || !expected || isInternalRuntimePrompt(turn)) {
    return false;
  }
  const userText = normalizeVisibleText(turn.user_text);
  if (userText && (userText === expected || userText.includes(expected))) {
    return true;
  }
  return conversationItemsForTurn(turn).some((item) => {
    if (item.kind !== "UserText") {
      return false;
    }
    const body = normalizeVisibleText(item.body);
    return body === expected || body.includes(expected);
  });
}

function pendingUserInputIsMaterialized() {
  if (!state.pendingUserInput) {
    return false;
  }
  const submitId = state.pendingSubmitId;
  const submitSessionId = state.pendingSubmitSessionId;
  return conversationTurnsForRender().some((turn) => {
    if (submitSessionId && turn.session_id !== submitSessionId) {
      return false;
    }
    if (submitId && turn.submit_id !== submitId) {
      return false;
    }
    return turnContainsVisibleUserText(turn, state.pendingUserInput);
  });
}

function pendingSubmitAcceptedTaskByTaskTruth(startedAtMs = state.ambiguousSubmitRecoveryStartedAt || state.submitStartedAt) {
  if (!state.pendingUserInput || !state.pendingSubmitSessionId || !state.taskBoard) {
    return null;
  }
  const startedAtUnix = startedAtMs ? Math.floor(startedAtMs / 1000) - 5 : 0;
  return ((state.taskBoard && state.taskBoard.tasks) || []).find((task) => {
    if (!taskVisibleInSession(task, state.pendingSubmitSessionId)) {
      return false;
    }
    const taskCreatedAt = Number(task.created_at);
    if (!Number.isFinite(taskCreatedAt) || taskCreatedAt <= 0) {
      return false;
    }
    if (startedAtUnix > 0 && taskCreatedAt < startedAtUnix) {
      return false;
    }
    return true;
  }) || null;
}

function pendingSubmitAcceptedByTaskTruth(startedAtMs = state.ambiguousSubmitRecoveryStartedAt || state.submitStartedAt) {
  return !!pendingSubmitAcceptedTaskByTaskTruth(startedAtMs);
}

function clearPendingUserInputIfMaterialized() {
  const materializedInTranscript = pendingUserInputIsMaterialized();
  const acceptedTask = pendingSubmitAcceptedTaskByTaskTruth();
  if (!materializedInTranscript && !acceptedTask) {
    return;
  }
  state.acceptedSubmitReceipt = materializedInTranscript
    ? null
    : {
        text: state.pendingUserInput,
        sessionId: state.pendingSubmitSessionId,
        taskId: acceptedTask.task_id || "",
        status: acceptedTask.status || "",
        title: acceptedTask.title || acceptedTask.goal || "",
        createdAt: acceptedTask.created_at || Math.floor(Date.now() / 1000),
      };
  state.pendingUserInput = null;
  state.pendingSubmitId = null;
  state.pendingSubmitSessionId = null;
  state.pendingSubmitError = null;
  state.pendingCancelTurnId = null;
  state.locallyCancelledTurnIds.clear();
  state.pendingAttachments = [];
  state.ambiguousSubmitRecoveryStartedAt = null;
  stopAmbiguousSubmitRecoveryPolling();
}

function sameRenderableTurn(left, right) {
  if (!left || !right || left.turn_id !== right.turn_id) {
    return false;
  }
  const leftSessionId = `${left.session_id || ""}`.trim();
  const rightSessionId = `${right.session_id || ""}`.trim();
  return !!leftSessionId && leftSessionId === rightSessionId;
}

function conversationTurnsForRender() {
  const transcriptTurns = logicalSessionTurns(state.sessionTurns);
  const latestTurn = state.turn;
  if (!latestTurn) {
    return transcriptTurns;
  }
  if (!state.selectedSessionId) {
    const merged = transcriptTurns.length > 0 ? transcriptTurns.slice() : [latestTurn];
    const index = merged.findIndex((turn) => sameRenderableTurn(turn, latestTurn));
    if (index >= 0) {
      merged[index] = latestTurn;
    } else if (merged.length === 0 || !sameRenderableTurn(merged[merged.length - 1], latestTurn)) {
      merged.push(latestTurn);
    }
    return logicalSessionTurns(merged);
  }
  if (latestTurn.session_id !== state.selectedSessionId) {
    return transcriptTurns;
  }
  const merged = transcriptTurns.slice();
  const index = merged.findIndex((turn) => sameRenderableTurn(turn, latestTurn));
  if (index >= 0) {
    merged[index] = latestTurn;
  } else {
    merged.push(latestTurn);
  }
  return logicalSessionTurns(merged);
}

function setSelectedSessionId(sessionId) {
  setSelectedSessionIdInSurface(sessionDetailSurfaceContext(), sessionId);
}

function clearConversationForSessionSwitch(sessionId) {
  clearConversationForSessionSwitchInSurface(sessionDetailSurfaceContext(), sessionId);
}

function switchConversationSession(sessionId, options = {}) {
  switchConversationSessionInSurface(sessionDetailSurfaceContext(), sessionId, options);
}

function sessionDetailSurfaceContext() {
  return {
    state,
    selectedSessionStorageKey,
    dispatchEdge: dispatchWebUiEdge,
    clearSessionRefreshRetryTimer,
    refreshSelectedSession,
    renderSessionRefreshFailure,
    renderAll,
    setCommandStatus,
  };
}

function clearSessionRefreshRetryTimer() {
  if (state.sessionRefreshRetryTimer) {
    window.clearTimeout(state.sessionRefreshRetryTimer);
  }
  state.sessionRefreshRetryTimer = null;
}

function workerTranscriptMissingRefreshMessage(message) {
  return /Worker session [`'"]?[^`'"]+[`'"]? has no persisted transcript/.test(`${message || ""}`);
}

function taskForWorkerSessionId(sessionId) {
  const id = `${sessionId || ""}`.trim();
  if (!id || !state.taskBoard) {
    return null;
  }
  return ((state.taskBoard && state.taskBoard.tasks) || []).find((task) =>
    workerSessionIdForTask(task) === id
  ) || null;
}

function taskStatusAllowsTranscriptRetry(task) {
  if (!task) {
    return false;
  }
  return taskKeepsSessionLifecycleRunning(task);
}

function workerTranscriptRetryContext(sessionId, message) {
  if (!workerTranscriptMissingRefreshMessage(message)) {
    return null;
  }
  const task = taskForWorkerSessionId(sessionId);
  if (!taskStatusAllowsTranscriptRetry(task)) {
    return null;
  }
  return {
    task,
    session_id: sessionId || "",
    task_id: task.task_id || "",
    task_status: task.status || "",
    assignee_agent_id: task.assignee_agent_id || "",
    message,
  };
}

function selectedWorkerTranscriptRefreshRetryable(sessionId = state.selectedSessionId) {
  const error = state.sessionRefreshError;
  if (!error || !error.retryable || error.kind !== "worker_transcript_pending") {
    return false;
  }
  const id = `${sessionId || ""}`.trim();
  if (!id || error.session_id !== id) {
    return false;
  }
  return Boolean(workerTranscriptRetryContext(id, error.message));
}

function scheduleSessionRefreshRetry(delayMs = workerTranscriptRefreshRetryDelayMs) {
  if (state.sessionRefreshRetryTimer || !selectedWorkerTranscriptRefreshRetryable()) {
    return;
  }
  const sessionId = state.selectedSessionId;
  state.sessionRefreshRetryTimer = window.setTimeout(() => {
    state.sessionRefreshRetryTimer = null;
    if (!sessionId || state.selectedSessionId !== sessionId || !selectedWorkerTranscriptRefreshRetryable(sessionId)) {
      return;
    }
    state.sessionRefreshInFlight = sessionId;
    renderAll();
    refreshSelectedSession().catch((error) => {
      renderSessionRefreshFailure(error, sessionId);
    });
  }, delayMs);
}

function renderSessionRefreshFailure(error, requestedSessionId = state.selectedSessionId) {
  if (requestedSessionId && state.selectedSessionId !== requestedSessionId) {
    return;
  }
  const message = `会话刷新失败：${error && error.message ? error.message : error}`;
  const retryContext = workerTranscriptRetryContext(requestedSessionId || state.selectedSessionId, message);
  state.sessionRefreshInFlight = null;
  if (retryContext) {
    state.sessionRefreshError = {
      kind: "worker_transcript_pending",
      retryable: true,
      session_id: retryContext.session_id,
      task_id: retryContext.task_id,
      task_status: retryContext.task_status,
      assignee_agent_id: retryContext.assignee_agent_id,
      message,
    };
    if (`${state.adpFailure || ""}`.startsWith("会话刷新失败：")) {
      state.adpFailure = null;
    }
    setCommandStatus(
      `工作器记录 未就绪 · 任务 ${retryContext.task_id || "unknown"} 状态 ${statusLabel(retryContext.task_status || "active")}；正在重试`,
      { stickyMs: 6000 },
    );
    scheduleSessionRefreshRetry();
    renderAll();
    return;
  }
  clearSessionRefreshRetryTimer();
  state.sessionRefreshError = {
    session_id: requestedSessionId || state.selectedSessionId || "",
    message,
  };
  if (`${state.adpFailure || ""}`.startsWith("会话刷新失败：")) {
    state.adpFailure = null;
  }
  setCommandStatus(message, { stickyMs: 8000 });
  renderAll();
}

async function exitSessionRefreshErrorToNewConversation() {
  const failedSessionId = state.selectedSessionId;
  clearSessionRefreshState(failedSessionId);
  await startNewConversation();
}

function returnToSessionListFromRefreshError() {
  const failedSessionId = state.selectedSessionId;
  clearSessionRefreshState(failedSessionId);
  dispatchWebUiEdge("session.back_home");
  setSelectedSessionId(null);
  state.mobileDrawer = "sessions";
  state.sessionTurns = [];
  state.turn = null;
  state.publicConversation = [];
  state.debug = null;
  setCommandStatus("已退出当前错误状态；可在首页选择会话或新建会话。", { stickyMs: 6000 });
  renderAll();
}

function dismissSessionRefreshError() {
  clearSessionRefreshState(state.selectedSessionId);
  setCommandStatus("会话刷新错误已关闭；当前 transcript 未重新加载。", { stickyMs: 5000 });
  renderAll();
}

function clearSessionRefreshState(sessionId) {
  if (!sessionId || state.sessionRefreshInFlight === sessionId) {
    state.sessionRefreshInFlight = null;
  }
  if (!sessionId || (state.sessionRefreshError && state.sessionRefreshError.session_id === sessionId)) {
    state.sessionRefreshError = null;
    clearSessionRefreshRetryTimer();
    if (`${state.adpFailure || ""}`.startsWith("会话刷新失败：")) {
      state.adpFailure = null;
    }
  }
}

function normalizeCwd(cwd) {
  return `${cwd || ""}`.trim();
}

function setSelectedCwd(cwd) {
  state.selectedCwd = normalizeCwd(cwd);
  if (cwdInput && cwdInput.value !== state.selectedCwd) {
    cwdInput.value = state.selectedCwd;
  }
  if (taskCwdInput && taskCwdInput.value !== state.selectedCwd) {
    taskCwdInput.value = state.selectedCwd;
  }
  if (state.selectedCwd) {
    window.localStorage.setItem(selectedCwdStorageKey, state.selectedCwd);
  } else {
    window.localStorage.removeItem(selectedCwdStorageKey);
  }
}

function selectedWorkspaceCwd() {
  const sidebarCwd = taskCwdInput ? normalizeCwd(taskCwdInput.value) : "";
  const composerCwd = cwdInput ? normalizeCwd(cwdInput.value) : "";
  return sidebarCwd || composerCwd || normalizeCwd(state.selectedCwd);
}

function requireTaskCwd(action) {
  const cwd = selectedWorkspaceCwd();
  if (cwd) {
    setSelectedCwd(cwd);
    return cwd;
  }
  setCommandStatus(`${action} 需要任务目标目录`, { stickyMs: 6000 });
  (taskCwdInput || cwdInput || composerInput).focus();
  return "";
}

function selectedNewSessionKind() {
  return selectedNewSessionKindFromSurface(newSessionSurfaceContext());
}

function syncNewSessionDialogMode() {
  syncNewSessionDialogModeFromSurface(newSessionSurfaceContext());
}

function openNewSessionDialog(kind = "conversation") {
  openNewSessionSurface(kind, newSessionSurfaceContext());
}

function closeNewSessionDialog() {
  closeNewSessionSurface(newSessionSurfaceContext());
}

async function chooseNewTaskDirectory() {
  await chooseNewTaskDirectoryFromSurface(newSessionSurfaceContext());
}

async function submitNewSessionDialog() {
  await submitNewSessionSurface(newSessionSurfaceContext());
}

function newSessionSurfaceContext() {
  return {
    state,
    dom: {
      dialog: newSessionDialog,
      form: newSessionForm,
      cwdInput: newSessionCwdInput,
      browseButton: newSessionBrowseButton,
      confirmButton: newSessionConfirmButton,
      pathPresets: newTaskPathPresets,
    },
    dispatchEdge: dispatchWebUiEdge,
    selectedWorkspaceCwd,
    normalizeCwd,
    setSelectedCwd,
    startNewTask,
    startNewConversation,
    setCommandStatus,
    renderAll,
  };
}

function sessionSummaryForSelected() {
  if (!state.selectedSessionId) {
    return null;
  }
  return (
    state.sessions.find((session) => session.session_id === state.selectedSessionId) ||
    workerChildSessionForSessionId(state.selectedSessionId) ||
    null
  );
}

function syncSelectedCwdFromProjection(projection) {
  const cwd = normalizeCwd(projection && projection.cwd);
  if (cwd) {
    setSelectedCwd(cwd);
  }
}

function browserRandomId() {
  const cryptoApi = globalThis.crypto;
  if (cryptoApi && typeof cryptoApi.randomUUID === "function") {
    return cryptoApi.randomUUID();
  }
  if (cryptoApi && typeof cryptoApi.getRandomValues === "function") {
    const bytes = new Uint8Array(16);
    cryptoApi.getRandomValues(bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"));
    return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex
      .slice(6, 8)
      .join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10).join("")}`;
  }
  return `local-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 12)}`;
}

function newDraftSessionId() {
  if (
    globalThis.__freehandEnableTestHooks &&
    Array.isArray(globalThis.__freehandDraftSessionIdsForTest) &&
    globalThis.__freehandDraftSessionIdsForTest.length > 0
  ) {
    const fixedDraftSessionId = `${globalThis.__freehandDraftSessionIdsForTest.shift() || ""}`.trim();
    if (fixedDraftSessionId) return fixedDraftSessionId;
  }
  const stamp = new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
  return `webui-session-${stamp}-${browserRandomId().slice(0, 8)}`;
}

function resetLocalConversationState(sessionId) {
  clearSessionRefreshRetryTimer();
  state.draftSessionId = sessionId;
  state.sessionTurns = [];
  state.turn = null;
  state.publicConversation = [];
  state.debug = null;
  state.adpFailure = null;
  state.sessionRefreshInFlight = null;
  state.sessionRefreshError = null;
  state.pendingUserInput = null;
  state.pendingSubmitId = null;
  state.pendingSubmitSessionId = null;
  state.pendingSubmitError = null;
  state.pendingAttachments = [];
  state.acceptedSubmitReceipt = null;
  state.lifecycleClocks.clear();
  state.toolTimings.clear();
  state.submitStartedAt = null;
  state.submitInFlight = false;
  setSelectedSessionId(sessionId);
  composerInput.value = "";
  composerInput.focus();
  renderAll();
}

async function startNewConversation() {
  const sessionId = newDraftSessionId();
  resetLocalConversationState(sessionId);
  setSelectedCwd("");
  setCommandStatus("正在创建会话...", { stickyMs: 5000 });
  try {
    await adpCommand(adpCommandOf("CreateSession", { session_id: sessionId, title: "新会话" }));
    dispatchWebUiEdge("new.created", { session_id: sessionId });
    state.draftSessionId = null;
    await refreshSessions();
    await refreshSelectedSession();
    closeMobileDrawer();
    setCommandStatus("新会话已就绪", { stickyMs: 5000 });
  } catch (error) {
    setCommandStatus(`新建会话失败：${error.message}`, { stickyMs: 8000 });
    throw error;
  }
}

async function startNewTask(options = {}) {
  const cwd = normalizeCwd(options.cwd) || requireTaskCwd("新建任务");
  if (!cwd) {
    return;
  }
  const sessionId = newDraftSessionId();
  resetLocalConversationState(sessionId);
  setSelectedCwd(cwd);
  setCommandStatus(`正在创建任务会话 · cwd ${cwd}`, { stickyMs: 5000 });
  try {
    await adpCommand(adpCommandOf("CreateSession", { session_id: sessionId, title: `任务 · ${cwd}`, cwd }));
    dispatchWebUiEdge("new.created", { session_id: sessionId });
    await refreshSessions();
    await refreshSelectedSession();
    closeMobileDrawer();
    setCommandStatus(`新任务已就绪 · cwd ${cwd}`, { stickyMs: 5000 });
  } catch (error) {
    setCommandStatus(`新建任务失败：${error.message}`, { stickyMs: 8000 });
  }
}

function setSessionList(projection) {
  state.sessions = topLevelPersistedSessions((projection && projection.sessions) || []);
  state.sessionListLoaded = true;
  const knownSessionIds = new Set(state.sessions.map((session) => session.session_id));
  for (const sessionId of Array.from(state.selectedSessionIds)) {
    if (!knownSessionIds.has(sessionId)) {
      state.selectedSessionIds.delete(sessionId);
    }
  }
  if (
    state.selectedSessionId &&
    !isDraftSessionId(state.selectedSessionId) &&
    !sessionTruthAllowsSessionId(state.selectedSessionId)
  ) {
    setSelectedSessionId(null);
  }
  if (state.sessions.length === 0 && !state.draftSessionId && !state.submitInFlight && !state.pendingUserInput) {
    clearLocalConversationTruth();
  } else if (state.turn && !sessionTruthAllowsTurn(state.turn)) {
    clearLocalConversationTruth({ preserveSelectedSession: true });
  }
  const selected = sessionSummaryForSelected();
  if (selected) {
    syncSelectedCwdFromProjection(selected);
  }
}

function internalRuntimeSessionId(sessionId) {
  const id = `${sessionId || ""}`.trim();
  return id.startsWith("worker-task-") || id.startsWith("master-lifecycle-") || id.startsWith("master-timer-");
}

function topLevelPersistedSessions(sessions) {
  return (sessions || []).filter((session) =>
    session && session.session_id && !session.temporary && !internalRuntimeSessionId(session.session_id)
  );
}

function selectedManagedSessionIds() {
  return Array.from(state.selectedSessionIds).filter((sessionId) =>
    state.sessions.some((session) => session.session_id === sessionId),
  );
}

function toggleSessionSelection(sessionId, selected) {
  if (!sessionId || isDraftSessionId(sessionId)) {
    return;
  }
  if (selected) {
    state.selectedSessionIds.add(sessionId);
  } else {
    state.selectedSessionIds.delete(sessionId);
  }
  renderSessions();
  renderMobileHomeDashboard();
}

function clearSessionSelection() {
  state.selectedSessionIds.clear();
  renderSessions();
  renderMobileHomeDashboard();
}

function selectAllSessions() {
  state.selectedSessionIds.clear();
  state.sessions.forEach((session) => {
    if (session && session.session_id && !isDraftSessionId(session.session_id) && !internalRuntimeSessionId(session.session_id)) {
      state.selectedSessionIds.add(session.session_id);
    }
  });
  renderSessions();
  renderMobileHomeDashboard();
}

async function deleteSelectedSessions() {
  const sessionIds = selectedManagedSessionIds();
  if (sessionIds.length === 0) {
    setCommandStatus("请选择要移除的会话", { stickyMs: 5000 });
    return;
  }
  const labelPreview = sessionIds
    .slice(0, 3)
    .map((sessionId) => {
      const session = state.sessions.find((candidate) => candidate.session_id === sessionId);
      return session?.title || session?.session_id || sessionId;
    })
    .join("、");
  const extraCount = Math.max(0, sessionIds.length - 3);
  const suffix = extraCount > 0 ? ` 等 ${sessionIds.length} 个会话` : `${sessionIds.length} 个会话`;
  if (!window.confirm(`批量移除「${labelPreview}」${suffix}？`)) {
    return;
  }
  setCommandStatus(`正在移除 ${sessionIds.length} 个会话...`, { stickyMs: 8000 });
  try {
    for (const sessionId of sessionIds) {
      dispatchWebUiEdge("home.delete_session", { session_id: sessionId });
      await adpCommand(adpCommandOf("DeleteSession", { session_id: sessionId }));
    }
    const deletedSelected = sessionIds.includes(state.selectedSessionId);
    state.selectedSessionIds.clear();
    if (deletedSelected) {
      setSelectedSessionId(null);
      state.sessionTurns = [];
      setTurnProjection(null, { preserveSessionTurns: true });
    }
    await refreshSessions();
    await refreshSelectedSession();
    setCommandStatus(`已移除 ${sessionIds.length} 个会话`, { stickyMs: 6000 });
  } catch (error) {
    setCommandStatus(`移除会话失败: ${error.message}`, { stickyMs: 9000 });
  }
}

async function renameCurrentSession() {
  const sessionId = state.selectedSessionId;
  if (!selectedSessionDetailRouteActive() || !sessionId || isDraftSessionId(sessionId)) {
    setCommandStatus("进入会话详情后才能重命名", { stickyMs: 5000 });
    return;
  }
  const current = sessionSummaryForSelected();
  const nextTitle = window.prompt("重命名会话", current?.title || current?.session_id || sessionId);
  if (nextTitle === null) {
    return;
  }
  const title = nextTitle.trim();
  if (!title) {
    setCommandStatus("重命名需要非空标题", { stickyMs: 6000 });
    return;
  }
  setCommandStatus("正在重命名会话...", { stickyMs: 5000 });
  try {
    dispatchWebUiEdge("session.rename_session", { session_id: sessionId, title });
    await adpCommand(adpCommandOf("RenameSession", { session_id: sessionId, title }));
    await refreshSessions();
    await refreshSelectedSession();
    setCommandStatus(`会话已重命名 · ${title}`, { stickyMs: 5000 });
  } catch (error) {
    setCommandStatus(`重命名失败: ${error.message}`, { stickyMs: 9000 });
  }
}

function latestRollbackUserText() {
  const turns = conversationTurnsForRender();
  const latest = turns[turns.length - 1];
  return `${(latest && latest.user_text) || ""}`.trim();
}

async function rollbackLatestSessionTurn() {
  if (!state.selectedSessionId || isDraftSessionId(state.selectedSessionId)) {
    setCommandStatus("回滚需要选中持久化会话", { stickyMs: 6000 });
    return;
  }
  const userText = latestRollbackUserText();
  setCommandStatus("正在回滚最新会话轮次...", { stickyMs: 8000 });
  try {
    await adpCommand(adpCommandOf("RollbackLatestSessionTurn", { session_id: state.selectedSessionId }));
    await refreshSessions();
    await refreshSelectedSession();
    if (userText) {
      composerInput.value = userText;
      composerInput.focus();
    }
    setCommandStatus("最新轮次已回滚；编辑后发送替换内容", { stickyMs: 7000 });
  } catch (error) {
    setCommandStatus(`回滚失败: ${error.message}`, { stickyMs: 9000 });
  }
}

function setSessionTranscript(projection) {
  if (
    projection &&
    projection.session_id &&
    state.selectedSessionId &&
    projection.session_id !== state.selectedSessionId
  ) {
    return;
  }
  if (projection && projection.session_id && !sessionTruthAllowsSessionId(projection.session_id)) {
    if (state.selectedSessionId === projection.session_id) {
      clearLocalConversationTruth();
    }
    return;
  }
  if (projection && projection.session_id) {
    clearSessionRefreshState(projection.session_id);
  }
  state.sessionTurns = logicalSessionTurns(
    guardedTranscriptTurns((projection && projection.turns) || []),
  );
  syncSelectedCwdFromProjection(projection);
  if (projection && projection.session_id) {
    setSelectedSessionId(projection.session_id);
    if (state.draftSessionId === projection.session_id) {
      state.draftSessionId = null;
    }
  }
  const latestTurn = state.sessionTurns[state.sessionTurns.length - 1] || null;
  setTurnProjection(latestTurn, { preserveSessionTurns: true });
}

function clearLocalConversationTruth(options = {}) {
  clearSessionRefreshRetryTimer();
  if (!options.preserveSelectedSession) {
    setSelectedSessionId(null);
  }
  state.sessionTurns = [];
  state.turn = null;
  state.publicConversation = [];
  state.debug = null;
  state.adpFailure = null;
  state.sessionRefreshInFlight = null;
  state.sessionRefreshError = null;
  state.pendingCancelTurnId = null;
  state.locallyCancelledTurnIds.clear();
  state.lifecycleClocks.clear();
  state.toolTimings.clear();
}

function sessionTruthAllowsTurn(turn) {
  if (!turn || !turn.session_id) {
    return false;
  }
  return sessionTruthAllowsSessionId(turn.session_id);
}

function sessionTruthAllowsSessionId(sessionId) {
  if (!sessionId) {
    return false;
  }
  if (state.draftSessionId && sessionId === state.draftSessionId) {
    return true;
  }
  if (state.pendingSubmitSessionId && sessionId === state.pendingSubmitSessionId) {
    return true;
  }
  if (!state.sessionListLoaded) {
    return true;
  }
  return (
    state.sessions.some((session) => session.session_id === sessionId) ||
    workerChildSessionForSessionId(sessionId) !== null
  );
}

function turnOrderKey(turnId) {
  const raw = `${turnId || ""}`;
  const runtimeMatch = raw.match(/^runtime-turn-(\d+)(?:-r(\d+))?$/);
  if (runtimeMatch) {
    return {
      prefix: "runtime-turn-",
      ordinal: Number.parseInt(runtimeMatch[1], 10),
      round: Number.parseInt(runtimeMatch[2] || "1", 10),
      raw,
    };
  }
  const match = raw.match(/^(.*?)(\d+)$/);
  if (!match) {
    return { prefix: raw, ordinal: 0, round: 1, raw };
  }
  return {
    prefix: match[1],
    ordinal: Number.parseInt(match[2], 10),
    round: 1,
    raw,
  };
}

function setTurnProjection(turn, options = {}) {
  const previousTurn = state.turn;
  if (shouldIgnoreCancelGuardedTurn(turn, options)) {
    return;
  }
  clearCancelGuardForTerminalTruth(turn);
  if (turn && !sessionTruthAllowsTurn(turn)) {
    if (state.turn && state.turn.session_id === turn.session_id) {
      clearLocalConversationTruth({ preserveSelectedSession: true });
    }
    return;
  }
  state.turn = turn || null;
  if (!state.turn) {
    state.debug = null;
  }
  if (state.turn && !state.selectedSessionId) {
    setSelectedSessionId(state.turn.session_id);
  }
  syncSelectedCwdFromProjection(state.turn);
  if (state.turn && !options.preserveSessionTurns) {
    const existingIndex = state.sessionTurns.findIndex(
      (existing) => sameRenderableTurn(existing, state.turn),
    );
    if (existingIndex >= 0) {
      state.sessionTurns[existingIndex] = state.turn;
    } else if (!state.selectedSessionId || state.turn.session_id === state.selectedSessionId) {
      state.sessionTurns.push(state.turn);
    }
    state.sessionTurns = logicalSessionTurns(state.sessionTurns);
  }
  state.publicConversation = derivePublicConversation(state.turn);
  syncToolTimings(conversationTurnsForRender());
  syncRenderLifecycleClocks();
  clearPendingUserInputIfMaterialized();
  maybeNotifyAndroidTurnFinished(previousTurn, state.turn);
}

function turnHasTerminalOutcome(turn) {
  return !!(
    turn &&
    (turn.terminal_text || isTerminalStatus(turn.terminal_status))
  );
}

function isCancelledTurn(turn) {
  return `${(turn && turn.terminal_status) || ""}`.toLowerCase() === "cancelled";
}

function turnIsCancelGuarded(turnId) {
  return !!(
    turnId &&
    (state.pendingCancelTurnId === turnId || state.locallyCancelledTurnIds.has(turnId))
  );
}

function shouldIgnoreCancelGuardedTurn(turn, options = {}) {
  if (!turn || options.allowCancelGuarded) {
    return false;
  }
  if (!turnIsCancelGuarded(turn.turn_id)) {
    return false;
  }
  return !turnHasTerminalOutcome(turn);
}

function clearCancelGuardForTerminalTruth(turn) {
  if (!turn || !turn.turn_id || !turnHasTerminalOutcome(turn)) {
    return;
  }
  if (state.pendingCancelTurnId === turn.turn_id) {
    state.pendingCancelTurnId = null;
  }
  if (isCancelledTurn(turn)) {
    state.locallyCancelledTurnIds.add(turn.turn_id);
  } else {
    state.locallyCancelledTurnIds.delete(turn.turn_id);
  }
}

function guardedTranscriptTurns(turns) {
  const currentCancelledTurn =
    state.turn && isCancelledTurn(state.turn) ? state.turn : null;
  return turns.map((turn) => {
    if (
      turn &&
      currentCancelledTurn &&
      turn.turn_id === currentCancelledTurn.turn_id &&
      shouldIgnoreCancelGuardedTurn(turn)
    ) {
      return currentCancelledTurn;
    }
    clearCancelGuardForTerminalTruth(turn);
    return turn;
  });
}

function locallyCancelledProjection(turnId) {
  const source =
    conversationTurnsForRender().find((turn) => turn && turn.turn_id === turnId) ||
    state.turn ||
    null;
  if (!source) {
    return null;
  }
  const userText = source.user_text || state.pendingUserInput || null;
  return {
    ...source,
    user_text: userText,
    model_request: null,
    tool_activities: (source.tool_activities || []).map((tool) => {
      const status = `${tool.status || ""}`.toLowerCase();
      if (status !== "waiting") {
        return tool;
      }
      return {
        ...tool,
        status: "Failed",
        result_summary: tool.result_summary || "用户已取消",
        error: tool.error || "用户已取消",
      };
    }),
    terminal_status: "Cancelled",
    terminal_text: source.terminal_text || "实时轮次已取消",
  };
}

function publishLocalCancelledTurn(turnId) {
  if (!turnId) {
    return;
  }
  state.pendingCancelTurnId = turnId;
  state.locallyCancelledTurnIds.add(turnId);
  const projection = locallyCancelledProjection(turnId);
  if (!projection) {
    renderAll();
    return;
  }
  setTurnProjection(projection, { allowCancelGuarded: true });
  renderAll();
}

function applyAdpQueryResult(result) {
  const turn = variantPayload(result, "Turn");
  if (turn !== undefined) {
    if (turn && !sessionTruthAllowsTurn(turn)) {
      renderAll();
      return;
    }
    if (turn && state.selectedSessionId && turn.session_id !== state.selectedSessionId) {
      renderAll();
      return;
    }
    setTurnProjection(turn);
    renderAll();
    if (state.turn) {
      refreshDebug().catch((error) => {
      setCommandStatus(`调试查询失败：${error.message}`);
      });
    }
    return;
  }
  const sessionListResult = variantPayload(result, "SessionList");
  if (sessionListResult !== undefined) {
    setSessionList(sessionListResult);
    renderAll();
    return;
  }
  const sessionTurns = variantPayload(result, "SessionTurns");
  if (sessionTurns !== undefined) {
    setSessionTranscript(sessionTurns);
    renderAll();
    return;
  }
  const debug = variantPayload(result, "Debug");
  if (debug !== undefined) {
    state.debug = debug || {
      status_text: "等待调试",
      detail_lines: ["等待调试快照"],
    };
    renderDebug();
    return;
  }
  const checkpoints = variantPayload(result, "Checkpoints");
  if (checkpoints !== undefined) {
    state.checkpoints = checkpoints.checkpoints || [];
    renderCheckpoints();
    return;
  }
  const configStatus = variantPayload(result, "ConfigStatus");
  if (configStatus !== undefined) {
    state.configStatus = configStatus;
    state.configStatusError = null;
    state.agentResourceDraftCount = Number(configStatus.agent_resource_count) || null;
    renderSettingsShell();
    renderPhase2Dashboard();
    renderSessions();
    return;
  }
  if (applyPhase2QueryResult(result)) {
    return;
  }
}

function applyAdpSubscriptionEvent(event) {
  const projection = event.projection || {};
  const turn = variantPayload(projection, "Turn");
  if (turn !== undefined) {
    if (turn && !sessionTruthAllowsTurn(turn)) {
      renderCommandStatus();
      return;
    }
    if (state.selectedSessionId && turn.session_id !== state.selectedSessionId) {
      renderCommandStatus();
      return;
    }
    setTurnProjection(turn);
    setBackgroundCommandStatus("会话已更新");
    renderAll();
    ensureDebugSubscription();
    return;
  }
  const debug = variantPayload(projection, "Debug");
  if (debug !== undefined) {
    state.debug = debug;
    renderDebug();
    return;
  }
  const checkpoints = variantPayload(projection, "Checkpoints");
  if (checkpoints !== undefined) {
    state.checkpoints = checkpoints.checkpoints || [];
    renderCheckpoints();
  }
}

function liveTurnStatus() {
  if (state.pendingSubmitError) {
    return "检查服务真源 · 提交回执未验证";
  }
  if (state.submitInFlight && !state.turn) {
    const elapsed = elapsedSince(state.submitStartedAt);
    return elapsed ? `派发中... ${elapsed}` : "派发中...";
  }
  const turn = activeTurnForSelectedSession();
  if (!turn) {
    return null;
  }

  if (turn.terminal_text || isTerminalStatus(turn.terminal_status) || isToolPendingStatus(turn.terminal_status)) {
    return terminalTurnStatusLabelForTurn(turn, turn.terminal_status);
  }

  const waitingTools = (turn.tool_activities || []).filter(
    (tool) => tool.status === "Waiting" || tool.status === "waiting",
  );
  if (waitingTools.length > 0) {
    if (!protocolConnectionCanRenderLive()) {
      return "连接已关闭，正在刷新服务真源";
    }
    return waitingToolStatus(waitingTools);
  }

  if (turnIsWaitingForModelResponse(turn)) {
    if (!protocolConnectionCanRenderLive()) {
      return "连接已关闭，正在刷新服务真源";
    }
    const elapsed = elapsedSince(lifecycleClockStartedAt(modelRequestTimingKey(turn)));
    const label = modelRequestLabel(turn);
    return elapsed ? `${label}... ${elapsed}` : `${label}...`;
  }

  if (state.submitInFlight) {
    const elapsed = elapsedSince(state.submitStartedAt);
    return elapsed ? `派发中... ${elapsed}` : "派发中...";
  }

  return null;
}

function activeTurnForSelectedSession() {
  if (!state.turn) {
    return null;
  }
  if (state.selectedSessionId && state.turn.session_id !== state.selectedSessionId) {
    return null;
  }
  return state.turn;
}

function renderCommandStatus() {
  if (state.commandStatusStickyUntil > Date.now()) {
    setText("command-status", state.commandStatusMessage);
    return;
  }
  const liveStatus = liveTurnStatus();
  setText("command-status", liveStatus || state.commandStatusMessage);
}

function renderMessages() {
  updateComposerClearance();
  renderComposerContextStrip();
  const wasNearBottom = messageListIsNearBottom();
  const shouldStickToBottom =
    state.forceScrollToBottom || (!state.userScrollLocked && wasNearBottom);
  state.forceScrollToBottom = false;
  const fragments = [];
  syncToolTimings(conversationTurnsForRender());
  syncRenderLifecycleClocks();
  const renderModel = buildConversationRenderModel();
  const hasSelectedSessionTranscript = renderModel.turns.length > 0;

  conversationTimelineItems(renderModel).forEach((item) => {
    fragments.push(timelineItemCycleCard(item));
  });

  if (fragments.length === 0 && renderModel.sessionLoading) {
    const waitingForWorkerTranscript =
      renderModel.sessionRefreshError &&
      renderModel.sessionRefreshError.kind === "worker_transcript_pending";
    fragments.push(cycleCardFromChatCards(
      {
        kind: "加载中",
        turnId: waitingForWorkerTranscript ? "worker-transcript-waiting" : "session-refresh-loading",
        sessionId: state.selectedSessionId || "",
        lifecycle: {
          className: "running",
          label: waitingForWorkerTranscript ? "等待 工作器记录" : "加载会话中",
          isLive: true,
        },
        terminal: false,
      },
      [
        waitingForWorkerTranscript
          ? workerTranscriptWaitingBubble(renderModel.sessionRefreshError)
          : loadingConversationBubble(),
      ],
    ));
  }

  if (fragments.length === 0) {
    const empty = document.createElement("div");
    empty.className = "chat-empty-state";
    const title = document.createElement("div");
    title.className = "chat-empty-title";
    title.textContent = "新会话";
    const copy = document.createElement("div");
    copy.className = "chat-empty-copy";
    copy.textContent = "发送消息开始这个会话。";
    empty.append(title, copy);
    fragments.push(empty);
  }

  renderConversationFragments(fragments, renderModel.selectedSessionId);
  if (shouldStickToBottom) {
    scrollMessagesToBottom();
  }
}

function renderConversationFragments(fragments, selectedSessionId) {
  const sessionKey = `${selectedSessionId || ""}`;
  const cycleOnly = fragments.length > 0 && fragments.every((fragment) =>
    fragment && fragment.classList && fragment.classList.contains("turn-cycle-card"),
  );
  const existingCycleOnly = Array.from(messageList.children).every((child) =>
    child.classList && child.classList.contains("turn-cycle-card"),
  );
  if (!cycleOnly || !existingCycleOnly || state.renderedCycleSessionId !== sessionKey) {
    messageList.replaceChildren(...fragments);
    state.renderedCycleSessionId = sessionKey;
    return;
  }
  reconcileCycleCardFragments(fragments);
  state.renderedCycleSessionId = sessionKey;
}

function reconcileCycleCardFragments(nextCards) {
  const existingCards = Array.from(messageList.children).filter((child) =>
    child.classList && child.classList.contains("turn-cycle-card"),
  );
  const existingByKey = new Map();
  existingCards.forEach((card) => {
    const key = cycleCardKeyFromNode(card);
    if (key && !existingByKey.has(key)) {
      existingByKey.set(key, card);
    }
  });

  const desiredCards = nextCards.map((nextCard) => {
    const key = cycleCardKeyFromNode(nextCard);
    const existing = key ? existingByKey.get(key) : null;
    if (
      existing &&
      existing.dataset.frozen === "true" &&
      !frozenCycleCardNeedsAuthoritativeMetadataRefresh(existing, nextCard)
    ) {
      return existing;
    }
    return nextCard;
  });
  const desired = new Set(desiredCards);

  desiredCards.forEach((card, index) => {
    const current = messageList.children[index] || null;
    if (current === card) {
      return;
    }
    messageList.insertBefore(card, current);
  });

  Array.from(messageList.children).forEach((child) => {
    if (!desired.has(child)) {
      child.remove();
    }
  });
}

function frozenCycleCardNeedsAuthoritativeMetadataRefresh(existing, nextCard) {
  if (!existing || !nextCard) {
    return false;
  }
  const existingCreatedAt = existing.dataset.createdAt || "";
  const nextCreatedAt = nextCard.dataset.createdAt || "";
  return (
    (!existing.dataset.timeToFirstResponseMs && !!nextCard.dataset.timeToFirstResponseMs) ||
    (!existing.dataset.totalElapsedMs && !!nextCard.dataset.totalElapsedMs) ||
    (!existingCreatedAt && !!nextCreatedAt) ||
    historicalRecoveryProjectionChanged(
      {
        recoveryState: existing.dataset.recoveryState || "",
        recoveryDebugDetails: existing.dataset.recoveryDebugDetails || "",
      },
      {
        recoveryState: nextCard.dataset.recoveryState || "",
        recoveryDebugDetails: nextCard.dataset.recoveryDebugDetails || "",
      },
    ) ||
    frozenCycleCardNeedsLifecycleClassificationRefresh(existing, nextCard)
  );
}

function frozenCycleCardNeedsLifecycleClassificationRefresh(existing, nextCard) {
  if (!existing || !nextCard) {
    return false;
  }
  const existingKey = cycleCardKeyFromNode(existing);
  const nextKey = cycleCardKeyFromNode(nextCard);
  if (!existingKey || existingKey !== nextKey) {
    return false;
  }
  const refreshablePhases = new Set(["waiting_lifecycle", "waiting_user"]);
  const existingPhase = existing.dataset.lifecyclePhase || "";
  const nextPhase = nextCard.dataset.lifecyclePhase || "";
  if (!refreshablePhases.has(existingPhase) && !refreshablePhases.has(nextPhase)) {
    return false;
  }
  return (
    existingPhase !== nextPhase ||
    (existing.dataset.lifecycleClass || "") !== (nextCard.dataset.lifecycleClass || "")
  );
}

function conversationTimelineItems(renderModel) {
  const items = (renderModel.turns || []).map((renderTurn) => ({
    kind: "turn",
    renderTurn,
  }));
  if (renderModel.pendingSubmit) {
    insertTimelineItem(
      items,
      { kind: "pending", pendingSubmit: renderModel.pendingSubmit },
      pendingSubmitTimelineIndex(items, renderModel.pendingSubmit),
    );
  }
  if (renderModel.acceptedSubmitReceipt) {
    insertTimelineItem(
      items,
      { kind: "accepted", acceptedSubmitReceipt: renderModel.acceptedSubmitReceipt },
      acceptedSubmitReceiptTimelineIndex(items, renderModel.acceptedSubmitReceipt),
    );
  }
  if (renderModel.adpFailure) {
    items.push({ kind: "failure", failure: renderModel.adpFailure });
  }
  return items;
}

function timelineItemChatCards(item) {
  if (item.kind === "turn") {
    return turnChatCards(item.renderTurn);
  }
  if (item.kind === "pending") {
    return pendingChatCards(item.pendingSubmit);
  }
  if (item.kind === "accepted") {
    return acceptedSubmitReceiptChatCards(item.acceptedSubmitReceipt);
  }
  if (item.kind === "failure") {
    return [failureChatBubble(item.failure)];
  }
  return [];
}

function timelineItemCycleCard(item) {
  return cycleCardFromChatCards(cycleCardMetaForTimelineItem(item), timelineItemChatCards(item));
}

function cycleCardFromChatCards(meta, chatCards) {
  const kind = `${(meta && meta.kind) || "turn"}`.trim() || "turn";
  const lifecycle = (meta && meta.lifecycle) || { className: "pending", label: "等待中", isLive: false };
  const article = document.createElement("article");
  article.className = `turn-cycle-card ${lifecycle.className || "pending"}-state`;
  article.dataset.cycleKey = cycleCardKey(meta);
  article.dataset.cycleKind = kind;
  article.dataset.turnId = `${(meta && meta.turnId) || ""}`;
  article.dataset.sessionId = `${(meta && meta.sessionId) || ""}`;
  article.dataset.submitId = `${(meta && meta.submitId) || ""}`;
  article.dataset.lifecycleClass = `${lifecycle.className || ""}`;
  article.dataset.lifecyclePhase = `${lifecycle.phase || ""}`;
  if (meta && meta.recoveryState) {
    article.dataset.recoveryState = `${meta.recoveryState}`;
    article.dataset.recoveryDebugDetails = meta.recoveryDebugDetails ? "true" : "false";
  }
  const createdAt = (meta && meta.createdAt) || "";
  if (createdAt) {
    article.dataset.createdAt = `${createdAt}`;
  }
  if (lifecycle.isLive) {
    article.dataset.live = "true";
  }
  const terminal = cycleCardIsTerminal(meta);
  if (terminal) {
    article.dataset.terminal = "true";
    article.dataset.frozen = "true";
  }
  const timing = meta && meta.timing;
  if (timing) {
    if (Number.isFinite(timing.timeToFirstResponseMs)) {
      article.dataset.timeToFirstResponseMs = `${timing.timeToFirstResponseMs}`;
    }
    if (Number.isFinite(timing.totalElapsedMs)) {
      article.dataset.totalElapsedMs = `${timing.totalElapsedMs}`;
    }
  }
  article.setAttribute("aria-label", `请求周期 ${kind} ${lifecycle.label || ""}`.trim());
  const header = cycleCardHeader(meta);
  if (header) {
    article.appendChild(header);
  }
  (chatCards || []).forEach((card) => article.appendChild(card));
  return article;
}

function cycleCardHeader(meta) {
  if (!meta) {
    return null;
  }
  const items = [];
  const createdAtMs = timestampToMilliseconds(meta.createdAt);
  if (createdAtMs) {
    items.push({ label: "时间", value: localChatTimeLabel(createdAtMs) });
  }
  const timingLine = turnTimingLine(meta.sourceTurn || null, { includeLiveWait: true });
  if (timingLine) {
    items.push({ label: "耗时", value: timingLine });
  }
  if (items.length === 0) {
    return null;
  }
  const header = document.createElement("div");
  header.className = "turn-cycle-header";
  items.forEach((item) => {
    const pill = document.createElement("span");
    pill.className = "turn-cycle-header-pill";
    pill.dataset.label = item.label;
    pill.textContent = item.value;
    header.appendChild(pill);
  });
  return header;
}

function cycleCardKey(meta) {
  const kind = `${(meta && meta.kind) || "turn"}`.trim() || "turn";
  const sessionId = `${(meta && meta.sessionId) || ""}`.trim();
  const turnId = `${(meta && meta.turnId) || ""}`.trim();
  if (kind === "turn" && turnId) {
    return `turn:${sessionId}:${turnId}`;
  }
  const submitId = `${(meta && meta.submitId) || ""}`.trim();
  if (submitId) {
    return `submit:${sessionId}:${submitId}`;
  }
  if (turnId) {
    return `${kind}:${sessionId}:${turnId}`;
  }
  const createdAt = `${(meta && meta.createdAt) || ""}`.trim();
  return `${kind}:${sessionId}:${createdAt}`;
}

function cycleCardKeyFromNode(node) {
  return (node && node.dataset && node.dataset.cycleKey) || "";
}

function cycleCardMetaForTimelineItem(item) {
  if (!item) {
    return { kind: "unknown", lifecycle: { className: "pending", label: "等待中", isLive: false } };
  }
  if (item.kind === "turn") {
    const renderTurn = item.renderTurn || {};
    return {
      kind: "turn",
      turnId: renderTurn.turnId || "",
      sessionId: renderTurn.sessionId || "",
      submitId: renderTurn.submitId || "",
      createdAt: renderTurn.createdAt || "",
      timing: renderTurn.timing || null,
      sourceTurn: renderTurn.sourceTurn || null,
      recoveryState: renderTurn.recoveryState || "",
      recoveryDebugDetails: renderTurn.recoveryDebugDetails === true,
      lifecycle: renderTurn.lifecycle,
      terminal: !((renderTurn.lifecycle && renderTurn.lifecycle.isLive) || false) &&
        !((renderTurn.lifecycle && renderTurn.lifecycle.neutral) || false),
    };
  }
  if (item.kind === "pending") {
    const pendingSubmit = item.pendingSubmit || {};
    const elapsed = pendingSubmit.elapsed;
    return {
      kind: "pending",
      turnId: "pending-submit",
      sessionId: pendingSubmit.sessionId || "",
      submitId: pendingSubmit.submitId || "",
      createdAt: pendingSubmit.startedAt || "",
      lifecycle: {
        className: pendingSubmit.error || pendingSubmit.isLive ? "running" : "pending",
        label: pendingSubmit.error
          ? "检查服务真源"
          : elapsed
            ? `派发中... ${elapsed}`
            : "派发中",
        isLive: pendingSubmit.isLive || !!pendingSubmit.error,
      },
      terminal: false,
    };
  }
  if (item.kind === "accepted") {
    const receipt = item.acceptedSubmitReceipt || {};
    return {
      kind: "accepted",
      turnId: "accepted-submit",
      sessionId: receipt.sessionId || "",
      submitId: receipt.submitId || "",
      createdAt: receipt.createdAt || receipt.created_at || "",
      lifecycle: { className: "running", label: "服务已接收", isLive: true },
      terminal: false,
    };
  }
  if (item.kind === "failure") {
    const failure = item.failure || {};
    return {
      kind: "failure",
      turnId: failure.sessionRefresh ? "session-refresh-failure" : "adp-failure",
      sessionId: state.selectedSessionId || "",
      lifecycle: {
        className: "failed",
        label: failure.sessionRefresh ? "会话刷新失败" : "失败",
        isLive: false,
      },
      terminal: true,
    };
  }
  return { kind: item.kind || "unknown", lifecycle: { className: "pending", label: "等待中", isLive: false } };
}

function cycleCardIsTerminal(meta) {
  if (!meta) {
    return false;
  }
  if (meta.terminal) {
    return true;
  }
  const lifecycle = meta.lifecycle || {};
  if (lifecycle.isLive || lifecycle.neutral) {
    return false;
  }
  return lifecycle.className === "success" || lifecycle.className === "failed";
}

function insertTimelineItem(items, item, index) {
  const safeIndex = Number.isFinite(index)
    ? Math.max(0, Math.min(items.length, index))
    : items.length;
  items.splice(safeIndex, 0, item);
}

function pendingSubmitTimelineIndex(items, pendingSubmit) {
  const submitId = `${(pendingSubmit && pendingSubmit.submitId) || ""}`.trim();
  if (submitId) {
    const exactSubmitIndex = items.findIndex((item) =>
      item.kind === "turn" && item.renderTurn && item.renderTurn.submitId === submitId,
    );
    if (exactSubmitIndex >= 0) {
      return exactSubmitIndex;
    }
  }
  const startedAt = Number((pendingSubmit && pendingSubmit.startedAt) || 0);
  return firstCycleTurnIndex(items, {
    sessionId: pendingSubmit && pendingSubmit.sessionId,
    startedAt,
    includeLive: true,
  });
}

function acceptedSubmitReceiptTimelineIndex(items, receipt) {
  return firstCycleTurnIndex(items, {
    sessionId: receipt && receipt.sessionId,
    startedAt: timestampToMilliseconds(receipt && receipt.createdAt),
    includeLive: true,
  });
}

function firstCycleTurnIndex(items, options = {}) {
  const sessionId = `${options.sessionId || ""}`.trim();
  const startedAt = Number(options.startedAt || 0);
  const threshold = startedAt > 0 ? startedAt - 2000 : 0;
  const liveIndex = options.includeLive
    ? items.findIndex((item) =>
        item.kind === "turn" &&
          renderTurnMatchesSession(item.renderTurn, sessionId) &&
          item.renderTurn.lifecycle &&
          item.renderTurn.lifecycle.isLive,
      )
    : -1;
  const timeIndex = threshold > 0
    ? items.findIndex((item) =>
        item.kind === "turn" &&
          renderTurnMatchesSession(item.renderTurn, sessionId) &&
          renderTurnCreatedAtMs(item.renderTurn) >= threshold,
      )
    : -1;
  const candidates = [timeIndex, liveIndex].filter((index) => index >= 0);
  if (candidates.length > 0) {
    return Math.min(...candidates);
  }
  return items.length;
}

function renderTurnMatchesSession(renderTurn, sessionId) {
  if (!sessionId || !renderTurn || !renderTurn.sessionId) {
    return true;
  }
  return renderTurn.sessionId === sessionId;
}

function renderTurnCreatedAtMs(renderTurn) {
  return timestampToMilliseconds(renderTurn && renderTurn.createdAt) || 0;
}

function messageListIsNearBottom() {
  return scrollHostRemaining(scrollHostForConversation()) < 96;
}

function scrollMessagesToBottom() {
  window.requestAnimationFrame(() => {
    const host = scrollHostForConversation();
    host.scrollTop = host.scrollHeight;
    messageList.scrollTop = messageList.scrollHeight;
    state.userScrollLocked = false;
  });
}

function scrollHostForConversation() {
  const streamStage = document.querySelector(".stream-stage");
  if (streamStage && streamStage.scrollHeight > streamStage.clientHeight + 2) {
    return streamStage;
  }
  return document.scrollingElement || document.documentElement;
}

function scrollHostRemaining(host) {
  return Math.max(0, host.scrollHeight - host.scrollTop - host.clientHeight);
}

function syncUserScrollLock() {
  state.userScrollLocked = scrollHostRemaining(scrollHostForConversation()) >= 96;
}

function updateComposerClearance() {
  const composerCard = document.querySelector(".composer-card");
  if (!composerCard || !shell) {
    return;
  }
  const height = Math.ceil(composerCard.getBoundingClientRect().height);
  const clearance = Math.max(96, height + 28);
  const conversationRegion = document.querySelector(".conversation-region");
  if (conversationRegion) {
    conversationRegion.style.setProperty("--measured-composer-clearance", `${clearance}px`);
  }
  shell.style.setProperty("--composer-clearance", `${clearance}px`);
  document.documentElement.style.setProperty("--composer-clearance", `${clearance}px`);
}

function formatTokenCount(tokens) {
  const value = Number(tokens) || 0;
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1)}M`;
  }
  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1)}k`;
  }
  return `${value}`;
}

function formatPercentBps(bps) {
  const value = Number(bps) || 0;
  if (value <= 0) {
    return "--";
  }
  return `${(value / 100).toFixed(1)}%`;
}

function currentSessionUsageProjections() {
  return conversationTurnsForRender()
    .map((turn) => turn && turn.usage_projection)
    .filter(Boolean);
}

function renderComposerContextStrip() {
  if (!composerContextStrip) {
    return;
  }
  const projections = currentSessionUsageProjections();
  const latest = projections[projections.length - 1] || null;
  const hasData = !!latest;
  composerContextStrip.hidden = !hasData;
  if (!hasData) {
    return;
  }

  const hit = latest.cache_hit_rate_bps || 0;
  const readTotal = projections.reduce((sum, p) => sum + (Number(p.cache_read_tokens) || 0), 0);
  const createTotal = projections.reduce((sum, p) => sum + (Number(p.cache_creation_tokens) || 0), 0);
  const cachePool = readTotal + createTotal;
  const avgBps = cachePool > 0 ? Math.round((readTotal / cachePool) * 10000) : 0;
  const thinking = latest.reasoning_tokens || 0;
  const context = Number(latest.context_tokens) || 0;
  const compacted = projections.reduce((sum, p) => sum + (Number(p.compacted_tokens) || 0), 0);

  setText("context-stat-cache-hit", `缓存 ${formatPercentBps(hit)}`);
  setText("context-stat-cache-avg", `平均 ${formatPercentBps(avgBps)}`);
  setText("context-stat-thinking", `思考 ${formatTokenCount(thinking)}`);
  setText("context-stat-context", `上下文 ${formatTokenCount(context)}`);
  const compactedText = compacted > 0 ? `压缩 ${formatTokenCount(compacted)}` : "压缩 --";
  setText("context-stat-compacted", compactedText);
}

function requestContextCompaction() {
  const latest = currentSessionUsageProjections()[currentSessionUsageProjections().length - 1];
  const sessionId = state.selectedSessionId || (latest && latest.session_id) || "";
  if (!sessionId) {
    return;
  }
  dispatchWebUiEdge("session.compact_context", {
    session_id: sessionId,
    reason: "manual compaction request",
  });
  if (compactContextButton) {
    compactContextButton.disabled = true;
  }
  setCommandStatus("正在请求压缩上下文...", { stickyMs: 8000 });
  adpCommand(adpCommandOf("CompactSessionContext", {
    session_id: sessionId,
    reason: "manual compaction request",
  }))
    .then((receipt) => {
      const status = (receipt && receipt.dispatch_status) || "";
      if (status.startsWith("compaction_hold") || status.startsWith("compaction_stale_prune") || status.startsWith("compaction_staged")) {
        setCommandStatus(`压缩暂不可执行（当前未接入模型摘要生成）: ${status}`, { stickyMs: 9000 });
      } else if (status.startsWith("compaction_soft_notice")) {
        setCommandStatus(`上下文接近压缩阈值，暂保持前缀: ${status}`, { stickyMs: 9000 });
      } else {
        setCommandStatus(`压缩请求已受理: ${status}`, { stickyMs: 9000 });
      }
      if (compactContextButton) {
        compactContextButton.disabled = false;
      }
    })
    .catch((error) => {
      setCommandStatus(`压缩请求失败: ${error && error.message}`, { stickyMs: 9000 });
      if (compactContextButton) {
        compactContextButton.disabled = false;
      }
    });
}

if (compactContextButton) {
  compactContextButton.addEventListener("click", requestContextCompaction);
}

function isDraftSessionId(sessionId) {
  return !!sessionId && state.draftSessionId === sessionId;
}

function appendSessionParts(item, label, title, meta) {
  const labelNode = document.createElement("div");
  labelNode.className = "meta-label session-state";
  labelNode.textContent = label;

  const titleNode = document.createElement("div");
  titleNode.className = "session-title";
  titleNode.textContent = title;
  titleNode.title = title;

  const metaNode = document.createElement("div");
  metaNode.className = "session-copy";
  metaNode.textContent = meta;
  metaNode.title = meta;

  item.append(labelNode, titleNode, metaNode);
}

function sessionKindLabel(session) {
  if (session && session.temporary) {
    return "Worker";
  }
  return normalizeCwd(session && session.cwd) ? "任务" : "全局";
}

function normalizeAgentId(value) {
  const agentId = `${value || ""}`.trim();
  return agentId || "master";
}

function workerSessionIdForTask(task) {
  const sessionId = `${(task && task.worker_session_id) || ""}`.trim();
  return sessionId || null;
}

function workerChildSessionsForParent(parentSessionId) {
  if (!parentSessionId || !state.taskBoard) {
    return [];
  }
  return ((state.taskBoard && state.taskBoard.tasks) || [])
    .filter((task) => task && taskVisibleInSession(task, parentSessionId))
    .map((task) => ({
      session_id: workerSessionIdForTask(task),
      title: task.title ? `Worker · ${task.title}` : `Worker · ${task.task_id}`,
      archived: false,
      cwd: task.target_cwd || null,
      latest_turn_id: null,
      active_turn_id: null,
      turn_count: 0,
      latest_status: task.status || "task",
      latest_summary: task.goal || task.task_id || null,
      temporary: true,
      parent_session_id: parentSessionId,
      task_id: task.task_id || null,
      assignee_agent_id: task.assignee_agent_id || null,
    }))
    .filter((session) => session.session_id);
}

function workerChildSessionForSessionId(sessionId) {
  const parents = state.sessions || [];
  for (const parent of parents) {
    const child = workerChildSessionsForParent(parent.session_id).find((session) => session.session_id === sessionId);
    if (child) {
      return child;
    }
  }
  return null;
}

function selectedParentSessionSummary() {
  const selectedWorkerSession = workerChildSessionForSessionId(state.selectedSessionId);
  const parentSessionId = selectedWorkerSession
    ? selectedWorkerSession.parent_session_id
    : state.selectedSessionId;
  if (!parentSessionId) {
    return null;
  }
  return (
    state.sessions.find((session) => session.session_id === parentSessionId) ||
    (state.draftSessionId === parentSessionId
      ? { session_id: parentSessionId, title: "草稿会话", temporary: false }
      : null)
  );
}

function currentTaskParentSessionId() {
  const selectedWorkerSession = workerChildSessionForSessionId(state.selectedSessionId);
  if (selectedWorkerSession) {
    return selectedWorkerSession.parent_session_id || null;
  }
  return state.selectedSessionId || null;
}

function currentSessionTasks() {
  const tasks = phase2SortedTasks((state.taskBoard && state.taskBoard.tasks) || []);
  const parentSessionId = currentTaskParentSessionId();
  if (!parentSessionId) {
    return tasks;
  }
  return tasks.filter((task) => task && taskVisibleInSession(task, parentSessionId));
}

function taskVisibleInSession(task, sessionId) {
  return Boolean(
    task &&
      sessionId &&
      (task.parent_session_id === sessionId ||
        (Array.isArray(task.attached_session_ids) && task.attached_session_ids.includes(sessionId)))
  );
}

function currentSessionTaskCounts(tasks = currentSessionTasks()) {
  const taskIds = new Set(tasks.map((task) => task.task_id).filter(Boolean));
  const staleTaskIds = new Set(
    ((state.taskBoard && state.taskBoard.stale) || [])
      .map((task) => task && task.task_id)
      .filter(Boolean)
  );
  const activeCount = tasks.filter((task) => taskLifecycleBucket(task.status) === "active").length;
  const blockedCount = tasks.filter((task) => taskLifecycleBucket(task.status) === "blocked").length;
  const reviewCount = tasks.filter((task) => taskLifecycleBucket(task.status) === "review").length;
  const closedCount = tasks.filter((task) => taskLifecycleBucket(task.status) === "closed").length;
  return {
    taskCount: tasks.length,
    activeCount,
    blockedCount,
    reviewCount,
    closedCount,
    staleCount: Array.from(taskIds).filter((taskId) => staleTaskIds.has(taskId)).length,
  };
}

function currentSessionTaskStatusLabel(tasks = currentSessionTasks()) {
  const counts = currentSessionTaskCounts(tasks);
  return `${counts.activeCount} 活动 · ${counts.reviewCount} 审核 · ${counts.blockedCount} 阻塞 · ${counts.closedCount} 关闭 · ${counts.staleCount} 过期`;
}

function sessionSummaryById(sessionId) {
  const id = `${sessionId || ""}`.trim();
  if (!id) {
    return null;
  }
  return (state.sessions || []).find((session) => session.session_id === id) ||
    workerChildSessionForSessionId(id) ||
    null;
}

function sessionTurnsForSession(sessionId) {
  const id = `${sessionId || ""}`.trim();
  if (!id) {
    return [];
  }
  return logicalSessionTurns(state.sessionTurns || []).filter((turn) => turn && turn.session_id === id);
}

function compareTurnIdOrder(leftTurnId, rightTurnId) {
  const left = turnOrderKey(leftTurnId);
  const right = turnOrderKey(rightTurnId);
  if (left.prefix === right.prefix) {
    if (left.ordinal !== right.ordinal) {
      return left.ordinal - right.ordinal;
    }
    if (left.round !== right.round) {
      return left.round - right.round;
    }
  }
  return left.raw.localeCompare(right.raw);
}

function compareSessionTurnPosition(left, right) {
  if (!left && !right) {
    return 0;
  }
  if (!left) {
    return -1;
  }
  if (!right) {
    return 1;
  }
  const turnOrder = compareTurnIdOrder(left.turn_id, right.turn_id);
  if (turnOrder !== 0) {
    return turnOrder;
  }
  const leftCreatedAt = timestampToMilliseconds(left.created_at) || 0;
  const rightCreatedAt = timestampToMilliseconds(right.created_at) || 0;
  return leftCreatedAt - rightCreatedAt;
}

function sessionTurnCandidates(sessionId) {
  const id = `${sessionId || ""}`.trim();
  if (!id) {
    return [];
  }
  const candidates = [];
  if (state.turn && state.turn.session_id === id) {
    candidates.push(state.turn);
  }
  candidates.push(...sessionTurnsForSession(id));
  return logicalSessionTurns(candidates);
}

function latestSessionTurnMatching(sessionId, predicate) {
  return sessionTurnCandidates(sessionId).reduce((latest, turn) => {
    if (!turn || (predicate && !predicate(turn))) {
      return latest;
    }
    return compareSessionTurnPosition(turn, latest) > 0 ? turn : latest;
  }, null);
}

function turnHasObservableSessionActivity(turn) {
  if (!turn || isTerminalStatus(turn.terminal_status)) {
    return false;
  }
  const waitingTools = (turn.tool_activities || []).some(
    (tool) => tool.status === "Waiting" || tool.status === "waiting",
  );
  return (isToolPendingStatus(turn.terminal_status) && toolPendingRepresentsLifecycle(turn)) ||
    turnIsWaitingForModelResponse(turn) ||
    waitingTools;
}

function latestClosedTurnForSession(sessionId) {
  return latestSessionTurnMatching(sessionId, (turn) => isTerminalStatus(turn.terminal_status));
}

function closedTurnObsoletesObservableTurn(closedTurn, observableTurn) {
  return !!(
    closedTurn &&
    observableTurn &&
    compareSessionTurnPosition(closedTurn, observableTurn) >= 0
  );
}

function closedTurnObsoletesSessionSummary(closedTurn, summary) {
  if (!closedTurn || !summary || !sessionHasObservableActiveStatus(summary)) {
    return false;
  }
  const activeTurnId = `${summary.active_turn_id || ""}`.trim();
  if (activeTurnId) {
    return compareTurnIdOrder(closedTurn.turn_id, activeTurnId) >= 0;
  }
  const latestTurnId = `${summary.latest_turn_id || ""}`.trim();
  return latestTurnId ? compareTurnIdOrder(closedTurn.turn_id, latestTurnId) >= 0 : false;
}

function activeTurnForSession(sessionId) {
  const id = `${sessionId || ""}`.trim();
  if (!id) {
    return null;
  }
  const summary = sessionSummaryById(id);
  const activeTurnId = `${(summary && summary.active_turn_id) || ""}`.trim();
  const candidates = sessionTurnCandidates(id);
  const latestClosedTurn = latestClosedTurnForSession(id);
  if (activeTurnId) {
    const exact = candidates.find((turn) => turn && turn.turn_id === activeTurnId);
    if (exact && turnHasObservableSessionActivity(exact) && !closedTurnObsoletesObservableTurn(latestClosedTurn, exact)) {
      return exact;
    }
  }
  return latestSessionTurnMatching(id, (turn) =>
    turnHasObservableSessionActivity(turn) &&
      !closedTurnObsoletesObservableTurn(latestClosedTurn, turn)
  );
}

function sessionHasObservableActiveStatus(session) {
  const status = `${(session && session.latest_status) || ""}`.toLowerCase();
  if (session && isToolPendingStatus(status)) {
    if (sessionHasOpenTaskLifecycle(session.session_id) || sessionHasOpenTimerLifecycle(session.session_id)) {
      return true;
    }
    return Boolean(session.active_turn_id);
  }
  return Boolean(
    session &&
      (session.active_turn_id ||
        ["waiting_model", "waiting", "running"].includes(status)),
  );
}

function sessionLiveObservation(sessionId) {
  const summary = sessionSummaryById(sessionId);
  const turn = activeTurnForSession(sessionId);
  if (!summary && !turn) {
    return null;
  }
  const latestClosedTurn = latestClosedTurnForSession(sessionId);
  if (
    !turn &&
    (!sessionHasObservableActiveStatus(summary) || closedTurnObsoletesSessionSummary(latestClosedTurn, summary))
  ) {
    return null;
  }
  const turnId = `${(turn && turn.turn_id) || (summary && summary.active_turn_id) || (summary && summary.latest_turn_id) || ""}`.trim();
  const status = `${(summary && summary.latest_status) || ""}`.trim();
  const label = turn && isToolPendingStatus(turn.terminal_status)
    ? toolPendingStatusLabelForTurn(turn)
    : turn && isAwaitingUserOptionsStatus(turn.terminal_status)
      ? "等待用户选择"
      : turn && turnIsWaitingForModelResponse(turn)
        ? modelRequestLabel(turn)
        : isToolPendingStatus(status) && lifecycleOwnerTaskProjectionLoaded() && !sessionHasOpenTaskLifecycle(sessionId) && !sessionHasOpenTimerLifecycle(sessionId)
          ? "等待用户选择"
          : statusLabel(status || "active");
  const detail = turn && turn.model_request && turn.model_request.detail
    ? turn.model_request.detail
    : (summary && summary.latest_summary) || "";
  return {
    sessionId: `${(summary && summary.session_id) || (turn && turn.session_id) || sessionId || ""}`,
    title: `${(summary && summary.title) || (summary && summary.session_id) || (turn && turn.session_id) || "会话"}`,
    turnId,
    status: status || (turn && turnIsWaitingForModelResponse(turn) ? "waiting_model" : "active"),
    label,
    detail,
    tone: turn && isToolPendingStatus(turn.terminal_status) && !toolPendingRepresentsLifecycle(turn)
      ? "phase2-muted"
      : turn && modelRequestTransportPhase(turn).startsWith("provider")
      ? "phase2-running"
      : phase2StatusClass(status || "running"),
  };
}

function globalLiveSessionObservation() {
  if (state.selectedSessionId) {
    const selected = sessionLiveObservation(state.selectedSessionId);
    if (selected) {
    return { ...selected, scope: "当前会话" };
    }
    const selectedWorkerSession = workerChildSessionForSessionId(state.selectedSessionId);
    const parentSessionId = selectedWorkerSession && selectedWorkerSession.parent_session_id;
    const parent = parentSessionId ? sessionLiveObservation(parentSessionId) : null;
    return parent ? { ...parent, scope: "父 Master" } : null;
  }
  const activeSummary = (state.sessions || []).find(sessionHasObservableActiveStatus);
  const active = activeSummary ? sessionLiveObservation(activeSummary.session_id) : null;
  return active ? { ...active, scope: "活动 Master" } : null;
}

function liveObservationLine(observation) {
  if (!observation) {
    return "";
  }
  return [
    observation.scope,
    observation.label,
    observation.turnId,
    observation.sessionId,
  ].filter(Boolean).join(" · ");
}

function currentSessionAgents() {
  const tasks = currentSessionTasks();
  if (!tasks.length) {
    return [];
  }
  const scopedTaskIds = new Set(tasks.map((task) => task.task_id).filter(Boolean));
  const scopedExecutionIds = new Set(tasks.map((task) => task.active_execution_id).filter(Boolean));
  return phase2SortedAgents((state.agentBoard && state.agentBoard.agents) || []).filter((agent) => {
    if (agent.current_task_id && scopedTaskIds.has(agent.current_task_id)) {
      return true;
    }
    if (agent.current_execution_id && scopedExecutionIds.has(agent.current_execution_id)) {
      return true;
    }
    return !!taskForAgent(agent, tasks);
  });
}

function currentSessionEvents() {
  const taskIds = new Set(currentSessionTasks().map((task) => task.task_id).filter(Boolean));
  if (!taskIds.size) {
    return [];
  }
  return ((state.eventInbox && state.eventInbox.events) || []).filter((event) =>
    event && taskIds.has(event.task_id)
  );
}

function renderSessionItem(session) {
  const item = document.createElement("section");
  item.className = `session-item session-row${session.session_id === state.selectedSessionId ? " active" : ""}`;
  item.dataset.sessionId = session.session_id;
  item.dataset.sessionKind = sessionKindLabel(session);

  const selector = document.createElement("input");
  selector.className = "session-selector";
  selector.type = "checkbox";
  selector.checked = state.selectedSessionIds.has(session.session_id);
  selector.setAttribute("aria-label", `选择会话 ${session.session_id}`);
  selector.disabled = !!session.temporary;
  selector.addEventListener("change", () => {
    toggleSessionSelection(session.session_id, selector.checked);
  });

  const button = document.createElement("button");
  button.className = "session-button";
  button.type = "button";
  button.dataset.sessionId = session.session_id;

  const cwd = normalizeCwd(session.cwd);
  const cwdTail = cwd ? ` · ${cwd.split("/").filter(Boolean).slice(-2).join("/") || cwd}` : "";
  const turnText = session.latest_turn_id
    ? `${session.latest_turn_id} · ${session.turn_count} 个 turn${cwdTail}`
    : `${session.turn_count} 个 turn${cwdTail}`;
  const observation = sessionLiveObservation(session.session_id);
  appendSessionParts(
    button,
    observation ? `活动 · ${observation.label}` : `${sessionKindLabel(session)} · ${statusLabel(session.latest_status || "session")}`,
    session.title || session.session_id,
    observation
      ? `${observation.turnId || session.active_turn_id || session.latest_turn_id} · ${statusLabel(observation.status)} · ${session.turn_count} 个 turn${cwdTail}`
      : turnText,
  );

  button.addEventListener("click", () => {
    closeMobileDrawer();
    switchConversationSession(session.session_id);
  });
  if (!session.temporary) {
    item.append(selector);
  }
  item.append(button);
  return item;
}

function renderSessionWithWorkerChildren(session) {
  const section = document.createElement("section");
  section.className = "session-with-workers";
  section.dataset.sessionId = session.session_id;
  section.appendChild(renderSessionItem(session));
  const children = workerChildSessionsForParent(session.session_id);
  if (children.length > 0) {
    const childrenNode = document.createElement("div");
    childrenNode.className = "session-worker-children";
    children.forEach((child) => {
      childrenNode.appendChild(renderSessionItem(child));
    });
    section.appendChild(childrenNode);
  }
  return section;
}

function renderSessionAgentGroup(sessions) {
  const group = document.createElement("section");
  group.className = "session-agent-group";
  group.dataset.expanded = "true";

  const toggle = document.createElement("button");
  toggle.className = "session-agent-button";
  toggle.type = "button";
  toggle.setAttribute("aria-expanded", "true");

  const main = document.createElement("span");
  main.className = "session-agent-main";
  const chevron = document.createElement("span");
  chevron.className = "session-agent-chevron";
  chevron.setAttribute("aria-hidden", "true");
  chevron.textContent = "›";
  const name = document.createElement("span");
  name.className = "session-agent-name";
  name.textContent = sessionAgentGroupLabel();
  main.append(chevron, name);

  const count = document.createElement("span");
  count.className = "session-agent-count";
  count.textContent = `${sessions.length} 个会话`;
  toggle.append(main, count);

  const sessionNodes = document.createElement("div");
  sessionNodes.className = "session-agent-sessions";
  sessions.forEach((session) => {
    sessionNodes.appendChild(renderSessionItem(session));
  });

  toggle.addEventListener("click", () => {
    const expanded = group.dataset.expanded !== "false";
    group.dataset.expanded = expanded ? "false" : "true";
    toggle.setAttribute("aria-expanded", expanded ? "false" : "true");
  });
  group.append(toggle, sessionNodes);
  return group;
}

function sessionAgentGroupLabel() {
  const agentName = `${state.configStatus?.agent_name || ""}`.trim();
  return agentName ? phase2AgentLabel(agentName) : "当前 Agent";
}

function renderSessionBulkToolbar() {
  if (!sessionBulkCount || !sessionDeleteSelectedButton) {
    return;
  }
  const selectedCount = selectedManagedSessionIds().length;
  const selectableCount = state.sessions.filter((session) => !isDraftSessionId(session.session_id)).length;
  sessionBulkCount.textContent = `已选 ${selectedCount} 个`;
  sessionDeleteSelectedButton.disabled = selectedCount === 0;
  if (sessionSelectAllButton) {
    sessionSelectAllButton.disabled = selectableCount === 0 || selectedCount === selectableCount;
  }
  if (sessionClearSelectionButton) {
    sessionClearSelectionButton.disabled = selectedCount === 0;
  }
}

function renderSessions() {
  if (!sessionList) {
    return;
  }
  sessionList.replaceChildren();
  renderSessionBulkToolbar();
  if (state.sessions.length === 0) {
    if (state.draftSessionId) {
      renderDraftSessionItem();
      return;
    }
    const empty = document.createElement("section");
    empty.className = "session-item active";
    appendSessionParts(empty, "空", "暂无会话", "等待第一轮对话");
    sessionList.appendChild(empty);
    return;
  }

  if (state.draftSessionId && !state.sessions.some((session) => session.session_id === state.draftSessionId)) {
    renderDraftSessionItem();
  }

  sessionList.appendChild(renderSessionAgentGroup(state.sessions));
  renderSessionBulkToolbar();
}

function renderMobileHomeDashboard() {
  if (!mobileHomeDashboard) {
    return;
  }
  const activeSessions = activeSessionsForHome();
  const historySessions = mobileHomeHistorySessions(activeSessions);
  renderHomeDashboardSurface(
    {
      activeSessions,
      historySessions,
      buckets: mobileHomeHistoryBuckets(historySessions),
    },
    mobileHomeDashboardContext(),
  );
}

function mobileHomeDashboardContext() {
  const context = {
    state,
    dom: {
      mobileHomeDashboard,
      mobileHomeActiveMarker,
      mobileHomeActiveList,
      mobileHomeSessionList,
    },
    setText,
    sessionSummaryById,
    liveObservationLine,
    statusLabel,
    workerChildSessionsForParent,
    sessionHasObservableActiveStatus,
    mobileHomeSessionMeta,
    sessionKindLabel,
    compactSentence,
    isDraftSessionId,
    isSessionSelected(sessionId) {
      return state.selectedSessionIds.has(sessionId);
    },
    toggleSessionSelection,
    selectedSessionCount() {
      return selectedManagedSessionIds().length;
    },
    selectableSessionCount() {
      return state.sessions.filter((session) => session && session.session_id && !isDraftSessionId(session.session_id)).length;
    },
    selectAllSessions,
    clearSelection: clearSessionSelection,
    deleteSelectedSessions,
    openSession(sessionId) {
      switchConversationSession(sessionId);
    },
    deleteSession(sessionId) {
      deleteSessionFromHome(sessionId);
    },
  };
  context.createSessionRow = (row) => createHomeSessionRow(row, context);
  return context;
}

function mobileHomeHistorySessions(activeSessions = activeSessionsForHome()) {
  const activeSessionIds = new Set(
    (activeSessions || [])
      .map((observation) => `${(observation && observation.sessionId) || ""}`.trim())
      .filter(Boolean),
  );
  return (state.sessions || [])
    .filter((session) => session && session.session_id && !activeSessionIds.has(session.session_id))
    .sort(compareSessionSummaryForDisplay);
}

function mobileHomeHistoryBuckets(sessions) {
  const buckets = [
    { id: "today", label: "今天", sessions: [] },
    { id: "week", label: "过去一周", sessions: [] },
    { id: "older", label: "所有更早的", sessions: [] },
  ];
  const byId = new Map(buckets.map((bucket) => [bucket.id, bucket]));
  (sessions || []).forEach((session) => {
    byId.get(mobileHomeHistoryBucketId(session)).sessions.push(session);
  });
  return buckets;
}

function mobileHomeHistoryBucketId(session, nowMs = Date.now()) {
  const rank = sessionSummaryTimeRank(session);
  if (!Number.isFinite(rank) || rank < 1000000000000) {
    return "older";
  }
  const now = new Date(nowMs);
  const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  if (rank >= todayStart) {
    return "today";
  }
  const weekStart = todayStart - 6 * 24 * 60 * 60 * 1000;
  return rank >= weekStart ? "week" : "older";
}

function compareSessionSummaryForDisplay(left, right) {
  const leftTime = sessionSummaryTimeRank(left);
  const rightTime = sessionSummaryTimeRank(right);
  if (leftTime !== rightTime) {
    return rightTime - leftTime;
  }
  return `${right && right.session_id || ""}`.localeCompare(`${left && left.session_id || ""}`);
}

function sessionSummaryTimeRank(session) {
  const latestTurn = latestSessionTurnMatching(session && session.session_id);
  const turnCreatedAt = timestampToMilliseconds(latestTurn && latestTurn.created_at) || 0;
  if (turnCreatedAt) {
    return turnCreatedAt;
  }
  const id = `${(session && session.session_id) || ""}`;
  const stamp = id.match(/(20\d{12})/);
  if (stamp) {
    const raw = stamp[1];
    const iso = `${raw.slice(0, 4)}-${raw.slice(4, 6)}-${raw.slice(6, 8)}T${raw.slice(8, 10)}:${raw.slice(10, 12)}:${raw.slice(12, 14)}Z`;
    const parsed = Date.parse(iso);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  const turnOrdinal = turnOrderKey(session && session.latest_turn_id).ordinal || 0;
  return turnOrdinal;
}

function activeSessionsForHome() {
  const bySession = new Map();
  (state.sessions || []).forEach((session) => {
    const observation = sessionLiveObservation(session.session_id);
    if (observation && observation.sessionId) {
      bySession.set(observation.sessionId, { ...observation, scope: observation.scope || "运行中" });
    }
  });
  const selectedObservation = globalLiveSessionObservation();
  if (selectedObservation && selectedObservation.sessionId) {
    bySession.set(selectedObservation.sessionId, selectedObservation);
  }
  return Array.from(bySession.values()).sort((left, right) =>
    compareSessionSummaryForDisplay(sessionSummaryById(left.sessionId), sessionSummaryById(right.sessionId))
  );
}


function renderMobileHomeActiveList(activeSessions = activeSessionsForHome()) {
  const historySessions = mobileHomeHistorySessions(activeSessions);
  renderHomeRunningList(
    createHomeDashboardModel({
      activeSessions,
      historySessions,
      buckets: mobileHomeHistoryBuckets(historySessions),
    }),
    mobileHomeDashboardContext(),
  );
}

function renderMobileHomeSessionList(sessions = mobileHomeHistorySessions()) {
  renderHomeHistoryList(
    createHomeDashboardModel({
      activeSessions: activeSessionsForHome(),
      historySessions: sessions,
      buckets: mobileHomeHistoryBuckets(sessions),
    }),
    mobileHomeDashboardContext(),
  );
}

function mobileHomeHistorySessionNode(session) {
  const children = workerChildSessionsForParent(session.session_id);
  return createHomeSessionRow({
    session: { ...session, child_count: children.length },
    markerClass: sessionHasObservableActiveStatus(session) ? "ok" : "",
    primary: session.title || session.session_id,
    meta: mobileHomeSessionMeta({ ...session, child_count: children.length }),
    status: statusLabel(session.latest_status || "session"),
    live: false,
  }, mobileHomeDashboardContext());
}

function mobileHomeSessionButton({ session, markerClass = "", primary, meta, status, turnId = "", live = false, child = false }) {
  return createHomeSessionRow({ session, markerClass, primary, meta, status, turnId, live, child }, mobileHomeDashboardContext());
}

async function deleteSessionFromHome(sessionId) {
  const current = state.sessions.find((session) => session.session_id === sessionId);
  const label = current?.title || current?.session_id || sessionId;
  if (!window.confirm(`移除会话「${label}」？`)) {
    return;
  }
  dispatchWebUiEdge("home.delete_session", { session_id: sessionId });
  setCommandStatus("正在移除会话...", { stickyMs: 5000 });
  try {
    await adpCommand(adpCommandOf("DeleteSession", { session_id: sessionId }));
    state.selectedSessionIds.delete(sessionId);
    if (state.selectedSessionId === sessionId) {
      setSelectedSessionId(null);
      state.sessionTurns = [];
      setTurnProjection(null, { preserveSessionTurns: true });
      dispatchWebUiEdge("root.open_home");
    }
    await refreshSessions();
    await refreshSelectedSession();
    setCommandStatus("会话已移除", { stickyMs: 5000 });
  } catch (error) {
    setCommandStatus(`移除会话失败: ${error.message}`, { stickyMs: 9000 });
  }
}

function mobileHomeSessionMeta(session) {
  const cwd = normalizeCwd(session && session.cwd);
  const cwdTail = cwd ? cwd.split("/").filter(Boolean).slice(-2).join("/") || cwd : "";
  const timeLabel = mobileHomeSessionTimeLabel(session);
  const childCount = Number(session && session.child_count) || 0;
  return [
    timeLabel,
    sessionKindLabel(session),
    `${(session && session.turn_count) || 0} 个 turn`,
    childCount > 0 ? `${childCount} 个工作器记录` : "",
    cwdTail,
  ].filter(Boolean).join(" · ");
}

function mobileHomeSessionTimeLabel(session) {
  const latestTurn = latestSessionTurnMatching(session && session.session_id);
  const createdAt = timestampToMilliseconds(latestTurn && latestTurn.created_at);
  if (createdAt) {
    return localChatTimeLabel(createdAt);
  }
  const rank = sessionSummaryTimeRank(session);
  return rank > 1000000000000 ? localChatTimeLabel(rank) : "时间未知";
}

function renderSettingsDiagnostics() {
  renderSettingsDiagnosticsSurface(settingsSurfaceContext());
}

function renderDiagnosticLogRow(file) {
  return renderDiagnosticLogRowSurface(file, settingsSurfaceContext());
}

function renderDraftSessionItem() {
  const item = document.createElement("button");
  item.className = `session-item session-button${state.draftSessionId === state.selectedSessionId ? " active" : ""}`;
  item.type = "button";
  item.dataset.sessionId = state.draftSessionId;
  item.dataset.sessionKind = state.selectedCwd ? "task" : "global";
  appendSessionParts(item, "草稿", state.draftSessionId, state.selectedCwd ? `cwd ${state.selectedCwd}` : "首次发送会创建会话");
  item.addEventListener("click", () => {
    setSelectedSessionId(state.draftSessionId);
    state.sessionTurns = [];
    setTurnProjection(null, { preserveSessionTurns: true });
    closeMobileDrawer();
    renderAll();
  });
  sessionList.appendChild(item);
}

function renderDebug() {
  if (!state.debug) {
    setText("debug-status", "等待中");
    setText("debug-lines", "-");
    return;
  }
  setText("debug-status", state.debug.status_text);
  setText("debug-lines", state.debug.detail_lines.join(" · "));
}

function renderCheckpoints() {
  setText("checkpoint-status", `${state.checkpoints.length} 个检查点`);
  const list = document.getElementById("checkpoint-list");
  if (!list) {
    return;
  }
  list.replaceChildren();
  if (state.checkpoints.length === 0) {
    list.textContent = "-";
    return;
  }
  state.checkpoints.slice(0, 4).forEach((checkpoint) => {
    const item = document.createElement("button");
    item.className = "checkpoint-item";
    item.type = "button";
    item.dataset.checkpointId = checkpoint.checkpoint_id;
    item.textContent = `${checkpoint.latest_status} · ${checkpoint.changed_paths.join(", ")}`;
    item.title = checkpoint.checkpoint_id;
    item.addEventListener("click", () => rewindCheckpoint(checkpoint.checkpoint_id));
    list.appendChild(item);
  });
}

function applyPhase2QueryResult(result) {
  const taskBoard = variantPayload(result, "TaskBoard");
  if (taskBoard !== undefined) {
    state.taskBoard = taskBoard;
    state.phase2StatusError = null;
    renderPhase2Dashboard();
    renderSessions();
    return true;
  }
  const agentBoard = variantPayload(result, "AgentBoard");
  if (agentBoard !== undefined) {
    state.agentBoard = agentBoard;
    state.phase2StatusError = null;
    renderPhase2Dashboard();
    return true;
  }
  const eventInbox = variantPayload(result, "EventInbox");
  if (eventInbox !== undefined) {
    state.eventInbox = eventInbox;
    state.phase2StatusError = null;
    renderPhase2Dashboard();
    return true;
  }
  const masterPoll = variantPayload(result, "MasterPoll");
  if (masterPoll !== undefined) {
    state.taskBoard = masterPoll.task_board || state.taskBoard;
    state.agentBoard = masterPoll.agent_board || state.agentBoard;
    state.eventInbox = masterPoll.event_inbox || state.eventInbox;
    state.phase2StatusError = null;
    renderPhase2Dashboard();
    renderSessions();
    return true;
  }
  const workerControl = variantPayload(result, "WorkerControl");
  if (workerControl !== undefined) {
    state.workerControl = workerControl;
    state.phase2StatusError = null;
    renderPhase2Dashboard();
    return true;
  }
  const taskHistory = variantPayload(result, "TaskHistory");
  if (taskHistory !== undefined) {
    state.taskHistory = taskHistory;
    state.phase2StatusError = null;
    renderPhase2Dashboard();
    return true;
  }
  const timerList = variantPayload(result, "TimerList");
  if (timerList !== undefined) {
    state.timerList = timerList;
    state.timerStatusError = null;
    renderTimerDashboard();
    renderMobileHomeDashboard();
    return true;
  }
  const toolRegistry = variantPayload(result, "ToolRegistry");
  if (toolRegistry !== undefined) {
    state.toolRegistry = toolRegistry;
    state.toolRegistryError = null;
    renderToolsDashboard();
    return true;
  }
  const diagnostics = variantPayload(result, "Diagnostics");
  if (diagnostics !== undefined) {
    state.diagnostics = diagnostics;
    state.diagnosticsError = null;
    renderSettingsDiagnostics();
    return true;
  }
  const sessionSearch = variantPayload(result, "SessionSearch");
  if (sessionSearch !== undefined) {
    state.sessionSearch = sessionSearch;
    state.sessionSearchError = null;
    renderSessionSearchDashboard();
    return true;
  }
  return false;
}

function buildMobileAgentDashboardModel() {
  const taskBoard = state.taskBoard;
  const tasks = currentSessionTasks();
  const agents = state.selectedSessionId
    ? currentSessionAgents()
    : (state.configStatus?.local_agent_directory || []).map((entry) => ({
        agent_id: entry.agent_name,
        role: entry.agent_mode,
        alive: false,
        state: "configured",
        local_web_url: entry.web_url || null,
        relay_web_url: entry.relay_web_url || null,
        is_local: !!entry.is_local,
      }));
  const counts = currentSessionTaskCounts(tasks);
  const liveObservation = globalLiveSessionObservation();
  const selectedTurns = logicalSessionTurns(state.sessionTurns || []).filter((turn) =>
    !state.selectedSessionId || turn.session_id === state.selectedSessionId
  );
  const latestSelectedTurn = selectedTurns[selectedTurns.length - 1] || activeTurnForSelectedSession();
  const terminalStatus = `${latestSelectedTurn?.terminal_status || ""}`.toLowerCase();
  let tone = "不可用";
  if (liveObservation) {
    tone = "active";
  } else if (taskBoard) {
    if (tasks.some((task) => ["blocked", "failed", "cancelled"].includes(`${task.status || ""}`.toLowerCase()))) {
      tone = "blocked";
    } else if (tasks.some((task) => ["review_ready", "review_submitted"].includes(`${task.status || ""}`.toLowerCase()))) {
      tone = "evaluating";
    } else if (tasks.some((task) => taskKeepsSessionLifecycleRunning(task))) {
      tone = "active";
    } else {
      tone = "neutral";
    }
  }

  return {
    tone,
    terminalStatus,
    taskBoardStatus: taskBoard
      ? currentSessionTaskStatusLabel(tasks)
      : state.phase2StatusError
        ? `状态不可用：${state.phase2StatusError}`
        : "等待中",
    counts,
    tasks,
    agents,
    liveObservation,
  };
}

function renderMobileAgentSummaryStrip(model = buildMobileAgentDashboardModel()) {
  if (!mobileAgentSummaryStrip) {
    return;
  }
  mobileAgentSummaryStrip.hidden = selectedSessionDetailRouteActive();
  mobileAgentSummaryStrip.dataset.tone = model.tone;
  const runningAgents = model.agents.filter((agent) => agentIsActive(agent));
  const counts = model.counts || currentSessionTaskCounts(model.tasks);
  const lifecycleSummary = mobileAgentLifecycleSummary(counts);
  const workerLimit = Number(state.configStatus?.agent_resource_count);
  const resourceSummary = Number.isFinite(workerLimit) && workerLimit > 0
    ? `/${workerLimit}`
    : "";
  const liveObservation = model.liveObservation || null;
  const directoryMode = !state.selectedSessionId;
 setText(
   "mobile-agent-summary-title",
   liveObservation
     ? `${liveObservation.label} · ${liveObservation.turnId || "活动 turn"}`
     : directoryMode
     ? `${model.agents.length} 个 Agent · ${runningAgents.length} 个活动`
     : `${runningAgents.length} 运行中 · ${lifecycleSummary}${resourceSummary}`,
 );
  // copy omitted on mobile - title and dot carry the signal; full detail in Agent sheet
  setText("mobile-agent-summary-dot", "");
  if (shell) {
    shell.dataset.lifecycleClockCount = `${state.lifecycleClocks.size}`;
    shell.dataset.selectedTerminalStatus = model.terminalStatus || "";
  }
}

function renderMobileAgentSheet(model = buildMobileAgentDashboardModel()) {
  if (!mobileAgentSheet) {
    return;
  }
  mobileAgentSheet.dataset.tone = model.tone;
  const liveObservation = model.liveObservation || null;
  setText(
    "mobile-agent-task-status",
    liveObservation
      ? `${liveObservation.label} · ${liveObservation.turnId || "活动 turn"} · ${liveObservation.sessionId}`
      : !state.selectedSessionId
      ? `${model.agents.length} 个已配置 Agent`
      : model.taskBoardStatus,
  );
  renderMobileAgentTaskList(model);
  applyMobileAgentSheetState();
}

function renderSystemAgentResourceConfig() {
  const status = state.configStatus;
  const isMaster = status?.agent_mode === "master";
  const workerLimit = Number(state.agentResourceDraftCount ?? status?.agent_resource_count ?? 1);
  const systemMax = Number(status?.agent_resource_limit || 5);
  const providerMode = status?.agent_resource_provider_mode || "不可用";
  const providerId = status?.agent_resource_provider_id || "不可用";
  setText("settings-agent-resource-count", `${workerLimit}`);
  setText("settings-agent-resource-limit", `${systemMax}`);
  setText("settings-agent-resource-summary", status ? `上限 ${workerLimit} · 最大 ${systemMax}` : "加载中");
  setText(
    "settings-agent-resource-provider",
    providerMode === "shared" ? `共享 · ${providerId}` : statusLabel(providerMode),
  );
  const disabled = !status || !isMaster || state.agentResourceSaveInFlight;
  if (settingsAgentResourceDecrement) {
    settingsAgentResourceDecrement.disabled = disabled || workerLimit <= 1;
  }
  if (settingsAgentResourceIncrement) {
    settingsAgentResourceIncrement.disabled = disabled || workerLimit >= systemMax;
  }
  if (settingsAgentResourceSave) {
    settingsAgentResourceSave.disabled = disabled || workerLimit === Number(status?.agent_resource_count);
    settingsAgentResourceSave.textContent = state.agentResourceSaveInFlight ? "保存中..." : "保存工作器上限";
  }
  const statusText = state.agentResourceSaveError
    ? `保存失败：${state.agentResourceSaveError}`
    : state.agentResourceSaveMessage
      ? state.agentResourceSaveMessage
      : !status
        ? "等待配置真源。"
        : !isMaster
          ? "工作器上限只能从活动 Master 配置。"
          : "工作器上限 1-5 · 需要重启并启动 Worker 进程。";
  setText("settings-agent-resource-status", statusText);
}

function adjustAgentResourceDraft(delta) {
  const status = state.configStatus;
  if (!status || status.agent_mode !== "master" || state.agentResourceSaveInFlight) {
    return;
  }
  const systemMax = Number(status.agent_resource_limit || 5);
  const current = Number(state.agentResourceDraftCount ?? status.agent_resource_count ?? 1);
  state.agentResourceDraftCount = Math.min(systemMax, Math.max(1, current + delta));
  state.agentResourceSaveMessage = null;
  state.agentResourceSaveError = null;
  renderSettingsShell();
}

async function submitAgentResourceConfigUpdate() {
  const status = state.configStatus;
  if (!status || status.agent_mode !== "master") {
    state.agentResourceSaveError = "活动 Master 配置不可用";
    renderSettingsShell();
    return;
  }
  const resourceCount = Number(state.agentResourceDraftCount ?? status.agent_resource_count);
  state.agentResourceSaveInFlight = true;
  state.agentResourceSaveMessage = null;
  state.agentResourceSaveError = null;
  renderSettingsShell();
  try {
    const receipt = await adpCommand(adpCommandOf("UpdateAgentResourceConfig", {
        update: {
          agent_name: status.agent_name,
          resource_count: resourceCount,
        },
      }));
    setCommandStatus(agentResourceConfigReceiptStatus(receipt, resourceCount), { stickyMs: 5000 });
    await refreshConfigStatus();
    state.agentResourceSaveMessage = "已保存。重启并启动 Worker 进程后生效。";
  } catch (error) {
    state.agentResourceSaveError = error.message;
  } finally {
    state.agentResourceSaveInFlight = false;
    renderSettingsShell();
  }
}

function renderMobileAgentTaskList(model) {
  if (!mobileAgentTaskList) {
    return;
  }
  mobileAgentTaskList.replaceChildren();
  if (!state.selectedSessionId) {
    if (model.agents.length === 0) {
      mobileAgentTaskList.appendChild(mobileAgentEmptyCard("当前配置中没有可访问的 Agent。"));
      return;
    }
    model.agents.forEach((agent) => {
      const agentWebUrl = agentNavigationUrl(agent);
      const card = mobileAgentCard({
        title: phase2AgentLabel(agent.agent_id),
        meta: [statusLabel(agent.state), `${agent.role || "Agent"}`.toUpperCase()].join(" · "),
        copy: agentWebUrl ? "进入独立会话" : "当前设备没有可访问地址",
        tone: agentIsActive(agent) ? "phase2-agent-active" : "phase2-muted",
        interactive: !!agentWebUrl,
      });
      card.dataset.agentId = `${agent.agent_id || ""}`;
      if (agentWebUrl) {
        card.addEventListener("click", () => window.location.assign(agentWebUrl));
      }
      mobileAgentTaskList.appendChild(card);
    });
    return;
  }
  if (model.tasks.length === 0) {
    mobileAgentTaskList.appendChild(mobileAgentEmptyCard("当前投影中没有 工作器任务。"));
    return;
  }
  const total = model.tasks.length;
  model.tasks.forEach((task, index) => {
    const card = mobileAgentCard({
      title: taskTitle(task),
      meta: [`${index + 1}/${total}`, statusLabel(task.status), assigneeLabel(task.assignee_agent_id), freshnessLabel(task.last_progress_at || task.updated_at)]
        .filter(Boolean)
        .join(" · "),
      copy: compactSentence(task.goal || "任务目标不可用", 132),
      tone: phase2StatusClass(task.status),
      interactive: true,
    });
    card.dataset.taskId = `${task.task_id || ""}`;
    card.dataset.workerSessionId = `${(task && task.worker_session_id) || ""}`;
    card.addEventListener("click", () => {
      setMobileAgentSheetOpen(false);
      openWorkerTaskSession(task);
    });
    mobileAgentTaskList.appendChild(card);
  });
}

function mobileAgentLifecycleSummary(counts) {
  const pieces = [];
  if (counts.activeCount > 0) {
    pieces.push(`${counts.activeCount} 个运行任务`);
  }
  if (counts.reviewCount > 0) {
    pieces.push(`${counts.reviewCount} 个审核任务`);
  }
  if (counts.blockedCount > 0) {
    pieces.push(`${counts.blockedCount} 个阻塞任务`);
  }
  if (pieces.length === 0 && counts.closedCount > 0) {
    pieces.push(`${counts.closedCount} 个关闭任务`);
  }
  return pieces.length > 0 ? pieces.join(" · ") : "0 个任务";
}

function mobileAgentCard({ title, meta, copy, tone = "phase2-muted", interactive = false }) {
  const card = document.createElement(interactive ? "button" : "section");
  card.className = `mobile-agent-card ${tone}`;
  if (interactive) {
    card.type = "button";
  }
  const titleNode = document.createElement("div");
  titleNode.className = "mobile-agent-card-title";
  titleNode.textContent = title;
  const metaNode = document.createElement("div");
  metaNode.className = "mobile-agent-card-meta";
  metaNode.textContent = meta || "状态不可用";
  const copyNode = document.createElement("div");
  copyNode.className = "mobile-agent-card-copy";
  copyNode.textContent = copy || "详情不可用";
  card.append(titleNode, metaNode, copyNode);
  return card;
}

function mobileAgentEmptyCard(copy) {
  return mobileAgentCard({
    title: "不可用",
    meta: "owner 投影",
    copy,
  });
}

function renderPhase2Dashboard() {
  renderTaskBoardProjection();
  renderAgentBoardProjection();
  renderEventInboxProjection();
  renderTaskHistoryProjection();
  renderWorkerControlProjection();
  const mobileModel = buildMobileAgentDashboardModel();
  renderSessionRelationHeader(mobileModel);
  renderMobileAgentSummaryStrip(mobileModel);
  renderMobileAgentSheet(mobileModel);
}

function renderTaskBoardProjection() {
  if (!taskBoardStatus || !taskBoardList) {
    return;
  }
  const board = state.taskBoard;
  if (state.phase2StatusError && !board) {
    taskBoardStatus.textContent = `状态不可用：${state.phase2StatusError}`;
    taskBoardList.textContent = "-";
    return;
  }
  if (!board) {
    taskBoardStatus.textContent = "等待中";
    taskBoardList.textContent = "-";
    return;
  }
  const tasks = currentSessionTasks();
  taskBoardStatus.textContent = currentSessionTaskStatusLabel(tasks);
  taskBoardList.replaceChildren();
  if (tasks.length === 0) {
    taskBoardList.textContent = state.selectedSessionId ? "选中会话没有任务" : "暂无任务";
    return;
  }
  tasks.slice(0, 8).forEach((task) => taskBoardList.appendChild(taskBoardItem(task)));
}

function taskBoardItem(task) {
  const item = document.createElement("section");
  item.className = `phase2-card ${phase2StatusClass(task.status)}`;
  item.dataset.taskId = task.task_id || "";
  if (task.active_execution_id) {
    item.dataset.executionId = task.active_execution_id;
  }
  const title = document.createElement("div");
  title.className = "phase2-card-title";
  title.textContent = taskTitle(task);
  const meta = document.createElement("div");
  meta.className = "phase2-card-meta";
  meta.textContent = [statusLabel(task.status), assigneeLabel(task.assignee_agent_id), freshnessLabel(task.last_progress_at || task.updated_at)]
    .filter(Boolean)
    .join(" · ");
  const goal = document.createElement("div");
  goal.className = "phase2-card-copy";
  goal.textContent = compactSentence(task.goal || task.target_cwd || "任务已注册", 120);
  item.append(title, meta, goal);
  return item;
}

function renderAgentBoardProjection() {
  if (!agentBoardStatus || !agentBoardList) {
    return;
  }
  const board = state.agentBoard;
  if (state.phase2StatusError && !board) {
    agentBoardStatus.textContent = `状态不可用：${state.phase2StatusError}`;
    agentBoardList.textContent = "-";
    return;
  }
  if (!board) {
    agentBoardStatus.textContent = "等待中";
    agentBoardList.textContent = "-";
    return;
  }
  const agents = state.selectedSessionId
    ? currentSessionAgents()
    : (state.configStatus?.local_agent_directory || []).map((entry) => ({
        agent_id: entry.agent_name,
        role: entry.agent_mode,
        alive: false,
        state: "configured",
        local_web_url: entry.web_url || null,
        relay_web_url: entry.relay_web_url || null,
        is_local: !!entry.is_local,
      }));
  const activeCount = agents.filter((agent) => agent.alive).length;
  agentBoardStatus.textContent = `${agents.length} 个当前 Agent · ${activeCount} 个活动`;
  agentBoardList.replaceChildren();
  if (agents.length === 0) {
    agentBoardList.textContent = state.selectedSessionId ? "选中会话没有 Worker" : "暂无本地 Agent";
    return;
  }
  agents.slice(0, 8).forEach((agent, index) => agentBoardList.appendChild(agentBoardItem(agent, index)));
}

function agentBoardItem(agent, index) {
  const boundTask = taskForAgent(agent);
  const agentWebUrl = agentNavigationUrl(agent);
  const item = document.createElement(boundTask || agentWebUrl ? "button" : "section");
  item.className = `phase2-card phase2-agent-card ${agent.alive ? "phase2-agent-active" : "phase2-muted"}`;
  if (boundTask || agentWebUrl) {
    item.type = "button";
    item.dataset.taskId = (boundTask && boundTask.task_id) || "";
    item.dataset.sessionId = boundTask ? workerSessionIdForTask(boundTask) : "";
    item.addEventListener("click", () => {
      if (boundTask) {
        openWorkerTaskSession(boundTask);
        return;
      }
      window.location.assign(agentWebUrl);
    });
  }
  const title = document.createElement("div");
  title.className = "phase2-card-title";
  title.textContent = phase2AgentLabel(agent.agent_id);
  const meta = document.createElement("div");
  meta.className = "phase2-card-meta";
  meta.textContent = [statusLabel(agent.state), agent.role || "Agent", agent.current_model ? `模型 ${agent.current_model}` : null]
    .filter(Boolean)
    .join(" · ");
  const activity = document.createElement("div");
  activity.className = "phase2-card-copy";
  activity.textContent = lifecycleActivityLabel(agent) || (boundTask ? taskTitle(boundTask) : "空闲");
  item.append(title, meta, activity);
  return item;
}

function renderEventInboxProjection() {
  if (!eventInboxStatus || !eventInboxList) {
    return;
  }
  const inbox = state.eventInbox;
  if (state.phase2StatusError && !inbox) {
    eventInboxStatus.textContent = `状态不可用：${state.phase2StatusError}`;
    eventInboxList.textContent = "-";
    return;
  }
  if (!inbox) {
    eventInboxStatus.textContent = "等待中";
    eventInboxList.textContent = "-";
    return;
  }
  const events = currentSessionEvents();
  eventInboxStatus.textContent = `${events.length} 个当前事件${inbox.next_cursor ? " · 已更新" : ""}`;
  eventInboxList.replaceChildren();
  if (events.length === 0) {
    eventInboxList.textContent = state.selectedSessionId ? "选中会话没有事件" : "暂无待处理任务事件";
    return;
  }
  events.slice(-10).reverse().forEach((event) => eventInboxList.appendChild(eventInboxItem(event)));
}

function eventInboxItem(event) {
  const item = document.createElement("section");
  item.className = `phase2-event ${phase2EventClass(event.kind)}`;
  const title = document.createElement("div");
  title.className = "phase2-card-title";
  title.textContent = eventKindLabel(event.kind);
  const meta = document.createElement("div");
  meta.className = "phase2-card-meta";
  meta.textContent = [eventPayloadStatus(event), assigneeLabel(event.agent_id), freshnessLabel(event.created_at)]
    .filter(Boolean)
    .join(" · ");
  const copy = document.createElement("div");
  copy.className = "phase2-card-copy";
  copy.textContent = compactSentence(eventPayloadSummary(event), 110);
  item.append(title, meta, copy);
  return item;
}

function renderTaskHistoryProjection() {
  if (!taskHistoryStatus || !taskHistoryList) {
    return;
  }
  const history = state.taskHistory;
  if (state.phase2StatusError && !history) {
    taskHistoryStatus.textContent = `历史不可用：${state.phase2StatusError}`;
    taskHistoryList.textContent = "-";
    return;
  }
  if (!history) {
    if (state.taskBoard) {
      taskHistoryStatus.textContent = "没有任务历史";
      taskHistoryList.textContent = "未选择任务";
      return;
    }
    taskHistoryStatus.textContent = "等待中";
    taskHistoryList.textContent = "-";
    return;
  }
  const events = history.events || [];
  taskHistoryStatus.textContent = `${events.length} 个执行事件`;
  taskHistoryList.replaceChildren();
  if (events.length === 0) {
    taskHistoryList.textContent = "没有记录执行事件";
    return;
  }
  events.slice(-10).reverse().forEach((event) => taskHistoryList.appendChild(taskHistoryItem(event)));
}

function taskHistoryItem(event) {
  const item = document.createElement("section");
  item.className = `phase2-event ${phase2EventClass(event.event_type || event.to_status)}`;
  const title = document.createElement("div");
  title.className = "phase2-card-title";
  title.textContent = eventKindLabel(event.event_type);
  const meta = document.createElement("div");
  meta.className = "phase2-card-meta";
  meta.textContent = [statusLabel(event.to_status), freshnessLabel(event.timestamp)]
    .filter(Boolean)
    .join(" · ");
  const copy = document.createElement("div");
  copy.className = "phase2-card-copy";
  copy.textContent = compactSentence(eventPayloadSummary({ kind: event.event_type, payload: event.payload || {} }), 110);
  item.append(title, meta, copy);
  return item;
}

function renderWorkerControlProjection() {
  if (!workerControlStatus || !workerControlList) {
    return;
  }
  const target = currentWorkerControlTarget();
  const control = state.workerControl;
  if (state.phase2StatusError && !control) {
    workerControlStatus.textContent = `状态不可用：${state.phase2StatusError}`;
    workerControlList.textContent = "-";
    return;
  }
  if (!target && !control) {
    workerControlStatus.textContent = "没有活动执行";
    workerControlList.textContent = "任务进入执行后会显示 工作器控制";
    return;
  }
  const events = (control && control.events) || [];
  const currentTask = (control && control.task) || (target && target.task) || null;
  workerControlStatus.textContent = `${statusLabel(currentTask && currentTask.status)} · ${events.length} 个控制事件`;
  workerControlList.replaceChildren();
  workerControlList.appendChild(workerControlSummaryCard(currentTask, target));
  workerControlList.appendChild(workerControlActionRow(currentTask, target));
  if (events.length === 0) {
    const empty = document.createElement("div");
    empty.className = "phase2-empty-note";
    empty.textContent = "没有记录控制事件";
    workerControlList.appendChild(empty);
    return;
  }
  events.slice(-6).reverse().forEach((event) => workerControlList.appendChild(workerControlEventItem(event)));
}

function workerControlSummaryCard(task, target) {
  const item = document.createElement("section");
  item.className = `phase2-card ${phase2StatusClass(task && task.status)}`;
  const title = document.createElement("div");
  title.className = "phase2-card-title";
  title.textContent = task ? taskTitle(task) : "Worker 执行";
  const meta = document.createElement("div");
  meta.className = "phase2-card-meta";
  meta.textContent = [statusLabel(task && task.status), assigneeLabel((task && task.assignee_agent_id) || (target && target.agent_id))]
    .filter(Boolean)
    .join(" · ");
  const copy = document.createElement("div");
  copy.className = "phase2-card-copy";
  copy.textContent = task && task.active_execution_id ? "执行由服务跟踪" : "没有活动执行";
  item.append(title, meta, copy);
  return item;
}

function workerControlActionRow(task, target) {
  const row = document.createElement("div");
  row.className = "phase2-action-row";
  const disabled = !workerControlCanMutate(task, target) || state.workerControlInFlight;
  [
    ["query_status", "查询状态"],
    ["request_checkpoint", "请求检查点"],
    ["request_submission_now", "立即提交"],
    ["pause", "暂停"],
    ["resume", "继续"],
    ["cancel", "取消"],
  ].forEach(([op, label]) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = `phase2-action ${op === "cancel" ? "danger" : ""}`;
    button.dataset.workerControlOp = op;
    button.textContent = label;
    button.disabled = disabled;
    row.appendChild(button);
  });
  return row;
}

function workerControlEventItem(event) {
  const item = document.createElement("section");
  item.className = `phase2-event ${phase2ControlStatusClass(event.status)}`;
  const title = document.createElement("div");
  title.className = "phase2-card-title";
  title.textContent = workerControlOpLabel(event.op);
  const meta = document.createElement("div");
  meta.className = "phase2-card-meta";
  meta.textContent = [statusLabel(event.status), assigneeLabel(event.agent_id), freshnessLabel(event.created_at)]
    .filter(Boolean)
    .join(" · ");
  const copy = document.createElement("div");
  copy.className = "phase2-card-copy";
  copy.textContent = workerControlPayloadSummary(event);
  item.append(title, meta, copy);
  return item;
}

function phase2SortedTasks(tasks) {
  return [...tasks].sort((left, right) => {
    const leftTerminal = terminalTaskStatus(left.status) ? 1 : 0;
    const rightTerminal = terminalTaskStatus(right.status) ? 1 : 0;
    if (leftTerminal !== rightTerminal) {
      return leftTerminal - rightTerminal;
    }
    return Number(right.updated_at || 0) - Number(left.updated_at || 0);
  });
}

function phase2SortedAgents(agents) {
  return [...agents].sort((left, right) => {
    const leftActive = agentIsActive(left) ? 1 : 0;
    const rightActive = agentIsActive(right) ? 1 : 0;
    if (leftActive !== rightActive) {
      return rightActive - leftActive;
    }
    return normalizeAgentId(left.agent_id).localeCompare(normalizeAgentId(right.agent_id));
  });
}

function agentIsActive(agent) {
  if (!agent) {
    return false;
  }
  if (agent.current_task_id || agent.current_execution_id) {
    return true;
  }
  return !!agent.alive && !["idle", "available", ""].includes(`${agent.state || ""}`.toLowerCase());
}

function agentNavigationUrl(agent) {
  const browserIsLoopback = ["127.0.0.1", "::1", "[::1]", "localhost"].includes(window.location.hostname);
  if (browserIsLoopback && agent.is_local && agent.local_web_url) {
    return agent.local_web_url;
  }
  return agent.relay_web_url;
}

function taskForAgent(agent, taskCandidates = null) {
  const agentId = normalizeAgentId(agent && agent.agent_id);
  if (!agentId || !state.taskBoard) {
    return null;
  }
  const tasks = taskCandidates
    ? phase2SortedTasks(taskCandidates)
    : phase2SortedTasks(state.taskBoard.tasks || []);
  return tasks.find((task) => {
    const taskAgent = normalizeAgentId(task.assignee_agent_id);
    if (agent.current_task_id && task.task_id === agent.current_task_id) {
      return true;
    }
    if (agent.current_execution_id && task.active_execution_id === agent.current_execution_id) {
      return true;
    }
    return taskAgent === agentId && taskKeepsSessionLifecycleRunning(task);
  }) || null;
}

function openWorkerTaskSession(task) {
  if (!task || !task.task_id) {
    return;
  }
  const sessionId = workerSessionIdForTask(task);
  if (!sessionId) {
    setCommandStatus("任务面板 投影中没有可用 工作器会话", { stickyMs: 8000 });
    return;
  }
  switchConversationSession(sessionId, { edgeId: "session.open_worker_session", payload: { worker_session_id: sessionId } });
  state.taskHistory = null;
  state.workerControl = null;
  adpQuery(adpQueryOf("QueryTaskHistory", { task_id: task.task_id }))
    .then((result) => applyPhase2QueryResult(result))
    .catch((error) => {
      state.phase2StatusError = error.message;
      renderPhase2Dashboard();
    });
  if (task.active_execution_id) {
    adpQuery(adpQueryOf("QueryWorkerControl", {
        task_id: task.task_id,
        execution_id: task.active_execution_id,
      }))
      .then((result) => applyPhase2QueryResult(result))
      .catch((error) => {
        state.phase2StatusError = error.message;
        renderPhase2Dashboard();
      });
  }
}

function returnToParentSession() {
  const selectedWorkerSession = workerChildSessionForSessionId(state.selectedSessionId);
  const parentSessionId = selectedWorkerSession && selectedWorkerSession.parent_session_id;
  if (!parentSessionId) {
    setCommandStatus("选中 Worker 的父 主控会话不可用", { stickyMs: 8000 });
    return;
  }
  switchConversationSession(parentSessionId, { edgeId: "session.open_parent_session", payload: { session_id: parentSessionId } });
}

function renderWorkerSessionNavigation(selectedWorkerSession = workerChildSessionForSessionId(state.selectedSessionId)) {
  if (!workerSessionNav) {
    return;
  }
  if (!selectedWorkerSession) {
    workerSessionNav.hidden = true;
    return;
  }
  workerSessionNav.hidden = false;
  setText("worker-session-nav-title", selectedWorkerSession.title || selectedWorkerSession.session_id);
}

function renderSessionRelationHeader(model = buildMobileAgentDashboardModel()) {
  if (!sessionRelationHeader) {
    return;
  }
  const selectedWorkerSession = workerChildSessionForSessionId(state.selectedSessionId);
  const parentSession = selectedParentSessionSummary();
  const parentSessionId = parentSession && parentSession.session_id;
  const tasks = parentSessionId ? workerChildSessionsForParent(parentSessionId) : [];
  const counts = currentSessionTaskCounts(model.tasks || []);
  const runningAgents = (model.agents || []).filter((agent) => agentIsActive(agent)).length;
  const liveObservation = model.liveObservation || globalLiveSessionObservation();
  const workerLimit = Number(state.configStatus?.agent_resource_count);
  const workerLimitText = Number.isFinite(workerLimit) && workerLimit > 0 ? ` · 上限 ${workerLimit}` : "";
  const title = selectedWorkerSession
    ? selectedWorkerSession.title || selectedWorkerSession.session_id
    : parentSession
      ? parentSession.title || parentSession.session_id
      : state.selectedSessionId || "未选择会话";
  const activeTask =
    (model.tasks || []).find((task) => taskLifecycleBucket(task.status) === "active") ||
    (model.tasks || []).find((task) => taskLifecycleBucket(task.status) === "review") ||
    (model.tasks || []).find((task) => taskLifecycleBucket(task.status) === "blocked") ||
    (model.tasks || [])[0];
  const copy = selectedWorkerSession
    ? liveObservation
      ? liveObservationLine(liveObservation)
      : `父 Master：${parentSession ? parentSession.title || parentSession.session_id : selectedWorkerSession.parent_session_id || "不可用"}`
    : activeTask
      ? liveObservation
        ? liveObservationLine(liveObservation)
        : `${statusLabel(activeTask.status)}: ${taskTitle(activeTask)}`
      : "点开查看当前会话内容";

  sessionRelationHeader.dataset.open = state.sessionTreeOpen ? "true" : "false";
  sessionRelationHeader.dataset.selectedKind = selectedWorkerSession ? "worker" : "master";
  sessionRelationHeader.dataset.liveSessionId = liveObservation ? liveObservation.sessionId : "";
  sessionRelationHeader.dataset.liveTurnId = liveObservation ? liveObservation.turnId : "";
  setText("session-relation-kicker", selectedWorkerSession ? "工作器会话" : "当前会话");
  setText("session-relation-title", compactSentence(title, 96));
 setText(
   "session-relation-metrics",
   liveObservation
     ? `${liveObservation.label} · ${liveObservation.turnId || "活动 turn"} · ${runningAgents} 个 Agent${workerLimitText}`
     : `${counts.activeCount}A · ${counts.reviewCount}R · ${counts.blockedCount}B · ${counts.closedCount}C`,
 );
  // copy omitted on mobile - compact metrics pill + title carry the signal
  if (sessionRelationToggleButton) {
    sessionRelationToggleButton.setAttribute("aria-expanded", state.sessionTreeOpen ? "true" : "false");
  }
  if (sessionTreeDropdown) {
    sessionTreeDropdown.hidden = !state.sessionTreeOpen;
  }
  renderWorkerSessionNavigation(selectedWorkerSession);
  renderSessionWorkerRail(parentSession, model.tasks || [], selectedWorkerSession);
  renderSessionTree(parentSession, tasks, selectedWorkerSession);
}

function renderSessionWorkerRail(parentSession, tasks = [], selectedWorkerSession = null) {
  if (!sessionWorkerRail) {
    return;
  }
  const scopedTasks = Array.isArray(tasks) ? tasks : [];
  const expandedTaskId = `${state.workerRailExpandedTaskId || ""}`.trim();
  if (expandedTaskId && !scopedTasks.some((task) => `${task.task_id || ""}` === expandedTaskId)) {
    state.workerRailExpandedTaskId = null;
  }
  const activeExpandedTaskId = `${state.workerRailExpandedTaskId || ""}`.trim();
  sessionWorkerRail.replaceChildren();
  sessionWorkerRail.dataset.workerCount = `${scopedTasks.length}`;
  sessionWorkerRail.dataset.expandedTaskId = activeExpandedTaskId;
  sessionWorkerRail.dataset.parentSessionId = parentSession?.session_id || "";
  sessionWorkerRail.dataset.hasLiveTask = scopedTasks.some((task) => taskKeepsSessionLifecycleRunning(task)) ? "true" : "false";
  if (!parentSession || scopedTasks.length === 0) {
    sessionWorkerRail.hidden = true;
    return;
  }
  sessionWorkerRail.hidden = false;

  const list = document.createElement("div");
  list.className = "session-worker-list";
  list.setAttribute("role", "list");

  scopedTasks.forEach((task, index) => {
    const taskId = `${task.task_id || ""}`.trim();
    const workerSessionId = `${workerSessionIdForTask(task) || ""}`.trim();
    const status = workerTaskStatusLabel(task);
    const duration = taskDurationLabel(task);
    const durationState = taskDurationState(task);
    const tone = phase2StatusClass(task.status);
    const expanded = activeExpandedTaskId !== "" && activeExpandedTaskId === taskId;
    const selected = selectedWorkerSession && selectedWorkerSession.task_id === taskId;
    const title = compactSentence(taskTitle(task), 72);
    const workerLabel = assigneeLabel(task.assignee_agent_id) || `Worker ${index + 1}`;
    const meta = compactSentence(
      [status, task.active_execution_id ? "有执行" : null, freshnessLabel(task.last_progress_at || task.updated_at)]
        .filter(Boolean)
        .join(" · "),
      96,
    );

    const row = document.createElement("div");
    row.className = [
      "session-worker-row",
      expanded ? "is-expanded" : "",
      selected ? "is-selected" : "",
      tone,
    ].filter(Boolean).join(" ");
    row.dataset.taskId = taskId;
    row.dataset.workerSessionId = workerSessionId;
    row.dataset.relationSchema = "UiTaskSnapshotProjection";
    row.dataset.relationSource = "TaskBoard.worker_session_id";
    row.dataset.durationState = durationState;
    row.dataset.status = `${task.status || ""}`;
    row.dataset.workerLabel = workerLabel;
    row.setAttribute("role", "listitem");

    const pill = document.createElement("button");
    pill.type = "button";
    pill.className = "session-worker-pill";
    pill.setAttribute("aria-expanded", expanded ? "true" : "false");
    if (taskId) {
      pill.setAttribute("aria-controls", `session-worker-detail-${taskId}`);
    }
    pill.dataset.taskId = taskId;
    pill.dataset.workerSessionId = workerSessionId;

    const statusDot = document.createElement("span");
    statusDot.className = "session-worker-status-dot";
    statusDot.setAttribute("aria-hidden", "true");

    const labelNode = document.createElement("span");
    labelNode.className = "session-worker-label";
    labelNode.textContent = workerLabel;

    const copy = document.createElement("span");
    copy.className = "session-worker-copy";
    const titleNode = document.createElement("span");
    titleNode.className = "session-worker-title";
    titleNode.textContent = title;
    const metaNode = document.createElement("span");
    metaNode.className = "session-worker-meta";
    metaNode.textContent = meta || status;
    copy.append(titleNode, metaNode);

    const durationNode = document.createElement("span");
    durationNode.className = "session-worker-duration";
    durationNode.dataset.durationState = durationState;
    durationNode.textContent = duration;

    const chevron = document.createElement("span");
    chevron.className = "session-worker-chevron";
    chevron.setAttribute("aria-hidden", "true");
    chevron.textContent = expanded ? "⌃" : "⌄";

    pill.append(statusDot, labelNode, copy, durationNode, chevron);
    pill.addEventListener("click", () => {
      if (parentSession?.session_id && taskId) {
        dispatchWebUiEdge("session.expand_worker_status", { session_id: parentSession.session_id, task_id: taskId });
      }
      state.workerRailExpandedTaskId = expanded ? null : taskId || null;
      renderSessionRelationHeader();
    });
    row.appendChild(pill);

    if (expanded) {
      const detail = document.createElement("section");
      detail.className = "session-worker-detail";
      if (taskId) {
        detail.id = `session-worker-detail-${taskId}`;
      }
      detail.dataset.taskId = taskId;
      detail.dataset.workerSessionId = workerSessionId;

      const detailTitle = document.createElement("div");
      detailTitle.className = "session-worker-detail-title";
      detailTitle.textContent = compactSentence(taskTitle(task), 110);

      const detailMeta = document.createElement("div");
      detailMeta.className = "session-worker-detail-meta";
      detailMeta.textContent = [
        status,
        assigneeLabel(task.assignee_agent_id),
        `持续 ${duration}`,
        task.active_execution_id ? "执行跟踪中" : null,
      ].filter(Boolean).join(" · ");

      const detailCopy = document.createElement("div");
      detailCopy.className = "session-worker-detail-copy";
      detailCopy.textContent = compactSentence(task.goal || task.target_cwd || "任务目标不可用", 160);

      const actions = document.createElement("div");
      actions.className = "session-worker-detail-actions";
      const openButton = document.createElement("button");
      openButton.type = "button";
      openButton.className = "session-worker-open-button";
      openButton.textContent = workerSessionId ? "打开工作器会话" : "工作器会话不可用";
      openButton.disabled = !workerSessionId;
      openButton.dataset.taskId = taskId;
      openButton.dataset.workerSessionId = workerSessionId;
      openButton.addEventListener("click", () => {
        openWorkerTaskSession(task);
      });
      actions.appendChild(openButton);

      detail.append(detailTitle, detailMeta, detailCopy, actions);
      row.appendChild(detail);
    }

    list.appendChild(row);
  });

  sessionWorkerRail.appendChild(list);
}

function renderSessionTree(parentSession, workerSessions, selectedWorkerSession) {
  if (!sessionTree) {
    return;
  }
  sessionTree.replaceChildren();
  if (!parentSession) {
    const empty = document.createElement("section");
    empty.className = "session-tree-node";
    empty.append(
      sessionTreeBranch(""),
      sessionTreeText("未选择持久化 主控会话", "创建或选择一个会话来查看 Worker 关系。"),
      sessionTreeStatus("等待中", "phase2-muted"),
    );
    sessionTree.appendChild(empty);
    return;
  }
  sessionTree.appendChild(
    sessionTreeNode({
      kind: "master",
      sessionId: parentSession.session_id,
      title: parentSession.title || parentSession.session_id,
      meta: [parentSession.session_id, normalizeCwd(parentSession.cwd)].filter(Boolean).join(" · "),
      status: sessionLiveObservation(parentSession.session_id)?.label || (selectedWorkerSession ? "返回" : "已选中"),
      statusClass: sessionLiveObservation(parentSession.session_id)?.tone || "phase2-muted",
      selected: state.selectedSessionId === parentSession.session_id,
      onClick: () => switchConversationSession(parentSession.session_id),
    }),
  );
  if (workerSessions.length === 0) {
    const empty = document.createElement("section");
    empty.className = "session-tree-node is-worker";
    empty.append(
      sessionTreeBranch("worker"),
      sessionTreeText("没有 Worker 子会话", "任务面板 中没有属于这个 主控会话的当前子任务。"),
      sessionTreeStatus("0 个任务", "phase2-muted"),
    );
    sessionTree.appendChild(empty);
    return;
  }
  workerSessions.forEach((workerSession) => {
    sessionTree.appendChild(
      sessionTreeNode({
        kind: "worker",
        sessionId: workerSession.session_id,
        taskId: workerSession.task_id,
        title: workerSession.title || workerSession.session_id,
        meta: [
          assigneeLabel(workerSession.assignee_agent_id),
          statusLabel(workerSession.latest_status),
          workerSession.session_id,
        ].filter(Boolean).join(" · "),
        status: statusLabel(workerSession.latest_status),
        statusClass: phase2StatusClass(workerSession.latest_status),
        selected: state.selectedSessionId === workerSession.session_id,
        onClick: () => switchConversationSession(workerSession.session_id),
      }),
    );
  });
}

function sessionTreeNode({ kind, sessionId = "", taskId = "", title, meta, status, statusClass = "phase2-muted", selected = false, onClick }) {
  const node = document.createElement(onClick ? "button" : "section");
  node.className = [
    "session-tree-node",
    kind === "worker" ? "is-worker" : "is-master",
    selected ? "is-selected" : "",
  ].filter(Boolean).join(" ");
  node.dataset.relationSchema = kind === "worker" ? "UiTaskSnapshotProjection" : "UiSessionMetadataProjection";
  node.dataset.relationSource = kind === "worker" ? "TaskBoard.worker_session_id" : "SessionMetadata.session_id";
  if (sessionId) {
    node.dataset.sessionId = sessionId;
  }
  if (taskId) {
    node.dataset.taskId = taskId;
  }
  if (onClick) {
    node.type = "button";
    node.addEventListener("click", onClick);
  }
  node.append(sessionTreeBranch(kind), sessionTreeText(title, meta), sessionTreeStatus(status, statusClass));
  return node;
}

function sessionTreeBranch(kind) {
  const branch = document.createElement("span");
  branch.className = "session-tree-branch";
  branch.dataset.kind = kind || "";
  return branch;
}

function sessionTreeText(title, meta) {
  const copy = document.createElement("span");
  copy.className = "session-tree-node-copy";
  const titleNode = document.createElement("span");
  titleNode.className = "session-tree-node-title";
  titleNode.textContent = compactSentence(title || "会话", 110);
  const metaNode = document.createElement("span");
  metaNode.className = "session-tree-node-meta";
  metaNode.textContent = compactSentence(meta || "关系真源不可用", 150);
  copy.append(titleNode, metaNode);
  return copy;
}

function sessionTreeStatus(status, statusClass) {
  const node = document.createElement("span");
  node.className = ["session-tree-node-status", statusClass || "phase2-muted"].join(" ");
  node.textContent = status || "状态";
  return node;
}

function currentWorkerControlTarget() {
  const tasks = currentSessionTasks();
  const task = tasks.find((candidate) => candidate.active_execution_id) || null;
  if (!task) {
    return null;
  }
  return {
    task,
    task_id: task.task_id,
    execution_id: task.active_execution_id,
    agent_id: task.assignee_agent_id || null,
  };
}

function currentTaskHistoryTarget() {
  const tasks = currentSessionTasks();
  const active = tasks.find((candidate) => candidate.active_execution_id);
  return active || tasks[0] || null;
}

function workerControlCanMutate(task, target) {
  return !!(
    task &&
    target &&
    target.task_id &&
    target.execution_id &&
    target.agent_id &&
    taskKeepsSessionLifecycleRunning(task)
  );
}

function taskTitle(task) {
  const title = `${(task && task.title) || ""}`.trim();
  return compactSentence(title || "任务", 80);
}

function statusLabel(status) {
  const normalized = `${status || ""}`.trim().toLowerCase();
  if (!normalized) {
    return "未知";
  }
  const labels = {
    active: "活动",
    assigned: "已分配",
    available: "可用",
    approved: "已批准",
    blocked: "已阻塞",
    cancelled: "已取消",
    closed: "已关闭",
    completed: "已完成",
    created: "已创建",
    failed: "失败",
    idle: "空闲",
    interrupted: "已中断",
    paused: "已暂停",
    pending: "等待中",
    ready: "就绪",
    recovering: "恢复中",
    review: "审核中",
    review_ready: "待审核",
    review_submitted: "审核已提交",
    running: "运行中",
    session: "会话",
    stale: "过期",
    submitted: "已提交",
    success: "成功",
    task: "任务",
    tool_pending: "等待中",
    toolpending: "等待中",
    awaiting_user_options: "等待用户选择",
    awaitinguseroptions: "等待用户选择",
    waiting_for_user_options: "等待用户选择",
    waiting_user: "等待用户选择",
    waiting: "等待中",
    waiting_agent: "等待 Agent",
    waiting_model: "等待模型",
  };
  return labels[normalized] || normalized.replace(/_/g, " ");
}

function phase2StatusClass(status) {
  const normalized = `${status || ""}`.toLowerCase();
  if (["blocked", "failed", "cancelled"].includes(normalized)) {
    return "phase2-failed";
  }
  if (["review_ready", "approved", "closed", "completed"].includes(normalized)) {
    return "phase2-success";
  }
  if (["running", "recovering", "assigned", "waiting_agent", "paused", "waiting_model", "waiting"].includes(normalized)) {
    return "phase2-running";
  }
  return "phase2-muted";
}

function phase2ControlStatusClass(status) {
  const normalized = `${status || ""}`.toLowerCase();
  if (normalized === "applied" || normalized === "observed") {
    return "phase2-success";
  }
  if (normalized === "queued" || normalized === "deferred") {
    return "phase2-running";
  }
  if (normalized === "rejected" || normalized === "failed") {
    return "phase2-failed";
  }
  return "phase2-muted";
}

function phase2EventClass(kind) {
  const normalized = `${kind || ""}`.toLowerCase();
  if (normalized.includes("blocked") || normalized.includes("failed") || normalized.includes("cancelled") || normalized.includes("rejected")) {
    return "phase2-failed";
  }
  if (normalized.includes("review") || normalized.includes("approved") || normalized.includes("closed")) {
    return "phase2-success";
  }
  if (normalized.includes("running") || normalized.includes("assigned") || normalized.includes("resumed") || normalized.includes("recorded")) {
    return "phase2-running";
  }
  return "phase2-muted";
}

function terminalTaskStatus(status) {
  return ["approved", "closed", "cancelled", "failed"].includes(`${status || ""}`.toLowerCase());
}

function taskBlockedStatus(status) {
  return `${status || ""}`.toLowerCase() === "blocked";
}

function taskLifecycleBucket(status) {
  const normalized = `${status || ""}`.toLowerCase();
  if (["blocked", "failed", "cancelled"].includes(normalized)) {
    return "blocked";
  }
  if (["review_ready", "review_submitted", "approved"].includes(normalized)) {
    return "review";
  }
  if (["closed", "completed"].includes(normalized)) {
    return "closed";
  }
  if (normalized === "") {
    return "unknown";
  }
  return terminalTaskStatus(normalized) ? "closed" : "active";
}

function phase2AgentLabel(agentId, explicitIndex = null) {
  const normalized = normalizeAgentId(agentId);
  if (normalized === "master") {
    return "Master";
  }
  const stableOrdinal = workerOrdinalFromAgentId(normalized);
  if (stableOrdinal) {
    return `Worker ${stableOrdinal}`;
  }
  const agents = ((state.agentBoard && state.agentBoard.agents) || []).filter(
    (agent) => normalizeAgentId(agent.agent_id) !== "master",
  );
  const index = explicitIndex === null
    ? agents.findIndex((agent) => normalizeAgentId(agent.agent_id) === normalized)
    : explicitIndex;
  const ordinal = index >= 0 ? index + 1 : null;
  return `Worker ${ordinal || ""}`.trim();
}

function workerOrdinalFromAgentId(agentId) {
  const normalized = `${agentId || ""}`.trim();
  if (normalized === "worker") {
    return 1;
  }
  const match = normalized.match(/^worker-(\d+)$/);
  return match ? Number.parseInt(match[1], 10) : null;
}

function assigneeLabel(agentId) {
  if (!agentId) {
    return null;
  }
  return phase2AgentLabel(agentId);
}

function lifecycleActivityLabel(agent) {
  const activity = agent && (agent.current_activity || agent.last_activity);
  if (!activity) {
    return null;
  }
  const elapsed = activity.elapsed_ms ? formatDuration(activity.elapsed_ms) : "";
  return [compactSentence(activity.semantic_summary || activity.kind || "活动", 90), elapsed]
    .filter(Boolean)
    .join(" · ");
}

function freshnessLabel(timestamp) {
  const ms = timestampToMilliseconds(timestamp);
  if (!ms) {
    return null;
  }
  const elapsed = Date.now() - ms;
  if (!Number.isFinite(elapsed) || elapsed < 0) {
    return "刚刚";
  }
  return `${formatDuration(elapsed)} 前`;
}

function taskStartedAtMs(task) {
  return timestampToMilliseconds(task && task.created_at);
}

function taskDurationEndMs(task) {
  if (!task) {
    return null;
  }
  if (terminalTaskStatus(task.status) || taskBlockedStatus(task.status)) {
    return timestampToMilliseconds(task.updated_at || task.last_progress_at || task.created_at);
  }
  return Date.now();
}

function taskDurationLabel(task) {
  const startedAt = taskStartedAtMs(task);
  if (!startedAt) {
    return "时间不可用";
  }
  const endedAt = taskDurationEndMs(task);
  if (!endedAt) {
    return "时间不可用";
  }
  const elapsed = Math.max(0, endedAt - startedAt);
  const label = formatDuration(elapsed);
  const value = label || "0s";
  return taskBlockedStatus(task.status) ? `已阻塞 · ${value}` : value;
}

function taskDurationState(task) {
  if (!taskStartedAtMs(task)) {
    return "unavailable";
  }
  return terminalTaskStatus(task.status) || taskBlockedStatus(task.status) ? "frozen" : "live";
}

function workerTaskStatusLabel(task) {
  if (!task) {
    return "未知";
  }
  const bucket = taskLifecycleBucket(task.status);
  if (task.active_execution_id && bucket === "active") {
    return "执行中";
  }
  return statusLabel(task.status);
}

function headerWorkerRailNeedsClock() {
  if (!sessionWorkerRail || sessionWorkerRail.hidden) {
    return false;
  }
  return currentSessionTasks().some((task) => taskDurationState(task) === "live");
}

function headerWorkerRailHasOpenTasks() {
  if (!sessionWorkerRail || sessionWorkerRail.hidden) {
    return false;
  }
  return currentSessionTasks().some((task) => task && taskKeepsSessionLifecycleRunning(task));
}

function refreshHeaderWorkerRailStatusIfNeeded() {
  if (!headerWorkerRailHasOpenTasks() || state.phase2LiveRefreshInFlight) {
    return;
  }
  const lastRefreshAt = Number(state.phase2LastRefreshAt) || 0;
  if (Date.now() - lastRefreshAt < headerWorkerRailStatusRefreshMs) {
    return;
  }
  state.phase2LiveRefreshInFlight = true;
  refreshPhase2Status()
    .catch((error) => {
      state.phase2StatusError = error.message;
    })
    .finally(() => {
      state.phase2LiveRefreshInFlight = false;
    });
}

function eventKindLabel(kind) {
  const raw = `${kind || "TaskEvent"}`;
  return raw
    .replace(/^Task/, "")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/_/g, " ")
    .trim()
    .toLowerCase() || "任务事件";
}

function eventPayloadStatus(event) {
  const payload = (event && event.payload) || {};
  return statusLabel(payload.to_status || payload.status || event.kind);
}

function eventPayloadSummary(event) {
  const payload = (event && event.payload) || {};
  if (payload.summary) {
    return payload.summary;
  }
  if (payload.reason) {
    return payload.reason;
  }
  if (payload.phase) {
    return payload.phase;
  }
  return eventKindLabel(event && event.kind);
}

function workerControlOpLabel(op) {
  const normalized = `${op || ""}`.toLowerCase();
  const labels = {
    query_status: "已查询状态",
    ask_at_safe_point: "问题已排队",
    add_constraint: "约束已排队",
    request_checkpoint: "已请求检查点",
    request_submission_now: "已请求立即提交",
    pause: "已请求暂停",
    resume: "已请求继续",
    cancel: "已请求取消",
  };
  return labels[normalized] || statusLabel(normalized || "控制事件");
}

function workerControlPayloadSummary(event) {
  const payload = (event && event.payload) || {};
  if (payload.question) {
    return compactSentence(payload.question, 110);
  }
  if (payload.constraint) {
    return compactSentence(payload.constraint, 110);
  }
  if (payload.note) {
    return compactSentence(payload.note, 110);
  }
  return `${workerControlOpLabel(event && event.op)} · ${statusLabel(event && event.status)}`;
}

function compactSentence(value, maxLength = 96) {
  const text = `${value || ""}`.replace(/\s+/g, " ").trim();
  if (text.length <= maxLength) {
    return text;
  }
  return `${text.slice(0, Math.max(0, maxLength - 1)).trim()}…`;
}

function formatUnixTime(value) {
  const seconds = Number(value);
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return "无到期时间";
  }
  return new Date(seconds * 1000).toLocaleString(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function renderTimerDashboard() {
  renderTimerDashboardSurface(timerDashboardSurfaceContext());
}

function timerDashboardSurfaceContext() {
  return {
    state,
    dom: {
      dialog: timerDashboardDialog,
      status: timerDashboardStatus,
      list: timerDashboardList,
      history: timerDashboardHistory,
      sourceSessionInput: timerSourceSessionInput,
    },
    currentSourceSessionId: currentTimerSourceSessionId,
    internalRuntimeSessionId,
    compactSentence,
    statusLabel,
    formatUnixTime,
    cancelTimer,
    dispatchEdge: dispatchWebUiEdge,
    adpQuery,
    adpCommand,
    applyPhase2QueryResult,
    setCommandStatus,
    renderTimerDashboard,
    refreshTimerDashboard,
    renderMobileHomeDashboard,
    buildTimerSchedulePayload,
    timerScheduleReceiptStatus,
    timerCancelReceiptStatus,
  };
}


function currentTimerSourceSessionId() {
  const selected = selectedParentSessionSummary() || sessionSummaryForSelected();
  if (selected && selected.session_id && !internalRuntimeSessionId(selected.session_id)) {
    return selected.session_id;
  }
  return "";
}


async function openTimerDashboard() {
  await openTimerDashboardSurface(timerDashboardSurfaceContext());
}

async function refreshTimerDashboard() {
  await refreshTimerDashboardSurface(timerDashboardSurfaceContext());
}

function buildTimerSchedulePayload() {
  const mode = (timerModeInput && timerModeInput.value) || "relative";
  const maxRuns = positiveIntegerValue(timerMaxRunsInput, 1);
  const payload = {
    mode,
    reason: (timerReasonInput && timerReasonInput.value || "").trim(),
    prompt: (timerPromptInput && timerPromptInput.value || "").trim(),
    max_runs: maxRuns,
  };
  const sourceSession = (timerSourceSessionInput && timerSourceSessionInput.value || "").trim();
  if (sourceSession) {
    payload.source_session_id = sourceSession;
  }
  if (mode === "relative") {
    payload.delay_seconds = positiveIntegerValue(timerDelayInput, 0);
  } else if (mode === "absolute") {
    payload.run_at_unix_seconds = nonNegativeIntegerValue(timerRunAtInput);
  } else if (mode === "recurring") {
    payload.repeat = buildTimerRepeatPayload(maxRuns);
  }
  return payload;
}

function buildTimerRepeatPayload(maxRuns) {
  const kind = (timerRepeatKindInput && timerRepeatKindInput.value) || "interval";
  if (kind === "interval") {
    return {
      kind,
      interval_seconds: positiveIntegerValue(timerIntervalInput, 0),
      max_runs: maxRuns,
    };
  }
  if (kind === "daily") {
    return {
      kind,
      time_of_day_seconds_local: nonNegativeIntegerValue(timerTimeOfDayInput),
      skip_weekends: false,
      max_runs: maxRuns,
    };
  }
  if (kind === "weekly") {
    return {
      kind,
      time_of_day_seconds_local: nonNegativeIntegerValue(timerTimeOfDayInput),
      weekdays: parseWeekdayList(timerWeekdaysInput && timerWeekdaysInput.value),
      max_runs: maxRuns,
    };
  }
  if (kind === "cron") {
    return {
      kind,
      expression: (timerCronInput && timerCronInput.value || "").trim(),
      max_runs: maxRuns,
    };
  }
  throw new Error(`unsupported timer repeat kind: ${kind}`);
}

function positiveIntegerValue(input, fallback) {
  const value = Number.parseInt(input && input.value, 10);
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function nonNegativeIntegerValue(input) {
  const value = Number.parseInt(input && input.value, 10);
  return Number.isFinite(value) && value >= 0 ? value : 0;
}

function parseWeekdayList(value) {
  const days = `${value || ""}`
    .split(",")
    .map((item) => Number.parseInt(item.trim(), 10))
    .filter((item) => Number.isInteger(item));
  if (days.length === 0 || days.some((day) => day < 0 || day > 6)) {
    throw new Error("weekdays must be comma-separated integers 0..6");
  }
  return days;
}

async function scheduleTimerFromForm() {
  await scheduleTimerFromSurface(timerDashboardSurfaceContext());
}

async function cancelTimer(timerId) {
  await cancelTimerFromSurface(timerDashboardSurfaceContext(), timerId);
}

function renderToolsDashboard() {
  renderToolsRegistrySurface(toolsRegistrySurfaceContext());
}

function toolsRegistrySurfaceContext() {
  return {
    state,
    dom: {
      dialog: toolsDashboardDialog,
      status: toolsDashboardStatus,
      refreshButton: toolsDashboardRefreshButton,
      guidance: toolsDashboardGuidance,
      list: toolsDashboardList,
    },
    toolRegistryTools,
    dispatchEdge: dispatchWebUiEdge,
    adpQuery,
    applyPhase2QueryResult,
    setCommandStatus,
    renderToolsDashboard,
    refreshToolsDashboard,
  };
}

function toolRegistryTools() {
  return Array.isArray(state.toolRegistry?.tools) ? state.toolRegistry.tools : [];
}


async function openToolsDashboard() {
  await openToolsRegistrySurface(toolsRegistrySurfaceContext());
}

async function refreshToolsDashboard() {
  await refreshToolsRegistrySurface(toolsRegistrySurfaceContext());
}

function renderSessionSearchDashboard() {
  renderSessionSearchSurface(sessionSearchSurfaceContext());
}

function sessionSearchResultsList() {
  return Array.isArray(state.sessionSearch?.results) ? state.sessionSearch.results : [];
}

function renderSessionSearchResult(result) {
  return renderSessionSearchResultSurface(result, sessionSearchSurfaceContext());
}

function sessionSearchSurfaceContext() {
  return {
    state,
    dom: {
      status: sessionSearchStatus,
      submitButton: sessionSearchSubmitButton,
      results: sessionSearchResults,
    },
    compactSentence,
    openResult: openSessionSearchResult,
  };
}

function openSessionSearchResult(sessionId) {
  if (!sessionId) {
    return;
  }
  sessionSearchDialog?.close();
  closeMobileDrawer();
  switchConversationSession(sessionId, { edgeId: "search.open_result", payload: { session_id: sessionId } });
}

async function openSessionSearchDashboard() {
  dispatchWebUiEdge("home.open_search");
  if (sessionSearchDialog && typeof sessionSearchDialog.showModal === "function" && !sessionSearchDialog.open) {
    sessionSearchDialog.showModal();
  }
  window.setTimeout(() => sessionSearchInput?.focus(), 0);
  renderSessionSearchDashboard();
}

async function submitSessionSearch(event) {
  event?.preventDefault?.();
  const query = `${sessionSearchInput?.value || ""}`.trim();
  if (!query) {
    state.sessionSearchError = "请输入非空搜索关键词。";
    renderSessionSearchDashboard();
    return;
  }
  state.sessionSearchInFlight = true;
  state.sessionSearchError = null;
  renderSessionSearchDashboard();
  try {
    const result = await adpQuery(adpQueryOf("QuerySessionSearch", { query, limit: 20 }));
    applyPhase2QueryResult(result);
    setCommandStatus("持久化会话搜索已刷新。");
  } catch (error) {
    state.sessionSearchError = error.message;
    setCommandStatus(`会话搜索失败: ${error.message}`, { stickyMs: 9000 });
  } finally {
    state.sessionSearchInFlight = false;
    renderSessionSearchDashboard();
  }
}

async function refreshPhase2Status() {
  try {
    applyPhase2QueryResult(await adpQuery(adpQueryOf("QueryTaskBoard", { include_terminal: true })));
    applyPhase2QueryResult(await adpQuery(adpQueryOf("QueryAgentBoard")));
    applyPhase2QueryResult(await adpQuery(adpQueryOf("QueryEventInbox", { limit: 30 })));
    applyPhase2QueryResult(await adpQuery(adpQueryOf("QueryTimerList", { include_terminal: true })));
    const historyTarget = currentTaskHistoryTarget();
    if (historyTarget && historyTarget.task_id) {
      applyPhase2QueryResult(await adpQuery(adpQueryOf("QueryTaskHistory", { task_id: historyTarget.task_id })));
    } else {
      state.taskHistory = null;
    }
    const target = currentWorkerControlTarget();
    if (target && target.task_id && target.execution_id) {
      applyPhase2QueryResult(await adpQuery(adpQueryOf("QueryWorkerControl", { task_id: target.task_id, execution_id: target.execution_id })));
    } else {
      state.workerControl = null;
    }
    state.phase2LastRefreshAt = Date.now();
    state.phase2StatusError = null;
    if (selectedWorkerTranscriptRefreshRetryable()) {
      scheduleSessionRefreshRetry(0);
    }
  } catch (error) {
    state.phase2StatusError = error.message;
    setCommandStatus(`task status refresh failed: ${error.message}`, { stickyMs: 9000 });
  }
  renderPhase2Dashboard();
}

async function sendWorkerControl(op) {
  const target = currentWorkerControlTarget();
  const task = target && target.task;
  if (!workerControlCanMutate(task, target)) {
    setCommandStatus("工作器控制需要一个未终止且已分配的执行", { stickyMs: 7000 });
    return;
  }
  state.workerControlInFlight = true;
  renderWorkerControlProjection();
  try {
    const control = {
      control_id: `webui-control-${browserRandomId().slice(0, 12)}`,
      task_id: target.task_id,
      execution_id: target.execution_id,
      agent_id: target.agent_id,
      op,
    };
    const result = await adpCommand(adpCommandOf("WorkerControl", { control }));
    const statusText = `${result.dispatch_status || "accepted"}`.toLowerCase().replace(/_/g, " ");
    setCommandStatus(`工作器控制 ${statusText}`, { stickyMs: 6000 });
    await refreshPhase2Status();
  } catch (error) {
    setCommandStatus(`工作器控制失败: ${error.message}`, { stickyMs: 9000 });
  } finally {
    state.workerControlInFlight = false;
    renderWorkerControlProjection();
  }
}

function showInspectorPanel(panel) {
  state.inspectorPanel = panel === "settings" ? "settings" : "debug";
  const showingSettings = state.inspectorPanel === "settings";
  if (inspectorDebugPanel) {
    inspectorDebugPanel.hidden = showingSettings;
  }
  if (settingsShell) {
    settingsShell.hidden = !showingSettings;
  }
  if (inspectorEyebrow) {
    inspectorEyebrow.textContent = showingSettings ? "设置" : "生命周期观察";
  }
  if (inspectorTitle) {
    inspectorTitle.textContent = showingSettings ? "系统设置" : "任务与智能体生命周期";
  }
  if (inspectorCopy) {
    inspectorCopy.textContent = showingSettings
      ? "打开拆分后的设置页：模型服务配置、模型服务策略、诊断、运行时和安卓外壳控制。"
      : "观察 Master/工作器生命周期、当前执行、事件和必要的调试摘要；活跃 Worker 会高亮并可点击查看对应任务进度。";
  }
  if (settingsShellToggle) {
    settingsShellToggle.classList.toggle("is-active", showingSettings);
    settingsShellToggle.setAttribute("aria-pressed", showingSettings ? "true" : "false");
  }
}

function renderSettingsShell() {
  renderSettingsShellSurface(settingsSurfaceContext());
}

function accountConfigSyncStatusLabel(status) {
  switch (`${status || ""}`) {
    case "synced":
      return "已同步";
    case "conflict":
      return "存在冲突";
    case "failed":
      return "同步失败";
    case "not_configured":
      return "未配置";
    default:
      return "未连接";
  }
}

function renderAccountConfigSync() {
  const sync = state.configStatus?.account_config_sync || {};
  const status = `${sync.status || "not_configured"}`;
  const summary = accountConfigSyncStatusLabel(status);
  const document = sync.server_document || null;
  const contentSummary = document
    ? `${document.provider_count || 0} 个模型服务 · ${document.model_group_count || 0} 个模型组 · ${document.relay_endpoint_count || 0} 个连接端点 · ${document.remote_daemon_count || 0} 个远端 daemon`
    : "未同步";
  setText("settings-account-config-sync-summary", summary);
  setText("settings-account-config-sync-account", sync.account_id || "未配置");
  setText(
    "settings-account-config-sync-revision",
    sync.revision === undefined || sync.revision === null ? "未同步" : `revision ${sync.revision}`,
  );
  setText("settings-account-config-sync-content", contentSummary);
  setText(
    "settings-account-config-sync-status",
    sync.error_message ||
      (status === "conflict"
        ? "服务器文档已变化。请先拉取并检查，再决定是否重新上传本机非 Secret 配置。"
        : status === "synced"
          ? "设备镜像与服务器文档已同步。上传只发送 schema 允许的非 Secret 配置。"
          : status === "not_configured"
            ? "当前账号尚无共享配置文档。上传入口保持显式，不会因登录自动上传。"
            : "登录后可显式拉取同账号配置。"),
  );
  if (settingsAccountConfigSyncMarker) {
    settingsAccountConfigSyncMarker.className = `settings-status-marker ${
      status === "synced" ? "ok" : status === "failed" || status === "conflict" ? "attention" : "partial"
    }`;
  }
  const disabled = state.accountConfigSyncInFlight || !state.configStatus;
  if (settingsAccountConfigPullButton) {
    settingsAccountConfigPullButton.disabled = disabled;
    settingsAccountConfigPullButton.textContent = state.accountConfigSyncInFlight ? "同步中..." : "拉取账号配置";
  }
  if (settingsAccountConfigPushButton) {
    settingsAccountConfigPushButton.disabled = disabled;
  }
}

function renderSettingsNavigation() {
  renderSettingsNavigationSurface(settingsSurfaceContext());
}

function openSettingsPage(page) {
  const target = `${page || ""}`.trim();
  const exists = Array.from(document.querySelectorAll("[data-settings-page]"))
    .some((panel) => panel.dataset.settingsPage === target);
  if (!target || !exists) {
    setCommandStatus(`设置页不可用: ${target || "unknown"}`, { stickyMs: 5000 });
    return;
  }
  dispatchWebUiEdge("settings.navigate", { page_id: target });
  state.settingsPage = target;
  renderSettingsNavigation();
  if (settingsShell) {
    settingsShell.scrollTop = 0;
  }
}

function settingsSurfaceContext() {
  return {
    state,
    dom: {
      settingsShell,
      modelSelector,
      diagnosticsStatus: settingsDiagnosticsStatus,
      diagnosticsRefreshButton: settingsDiagnosticsRefreshButton,
      diagnosticsList: settingsDiagnosticsList,
    },
    setText,
    setTitle,
    webSearchStatusLabel,
    settingsAuthTypeLabel,
    compactSentence,
    formatUnixTime,
    syncProviderSelectionControls,
    syncSettingsProviderForm,
    renderSettingsProviderRegistry,
    syncModelGroupSelectionControls,
    syncSettingsModelGroupForm,
    renderSettingsModelGroupRegistry,
    renderSystemAgentResourceConfig,
    renderAndroidApkUpdateSettings,
    renderAccountConfigSync,
    showInspectorPanel,
  };
}

function androidApkUpdateManifestUrlForDisplay() {
  try {
    return new URL("android/update.json", window.location.href).toString();
  } catch (_) {
    return "守护进程 /android/update.json";
  }
}

function androidApkUpdateBridge() {
  const bridge = window.FreehandAndroidApkUpdate;
  if (bridge && typeof bridge.check === "function") {
    return bridge;
  }
  return null;
}

function androidApkUpdateBridgeAvailable() {
  return layoutClient() === "android-webview" && !!androidApkUpdateBridge();
}

function androidApkUpdatePhaseInFlight(phase) {
  return ["checking", "available", "downloading", "downloaded", "already_checking"].includes(`${phase || ""}`);
}

function optionalAndroidApkUpdateNumber(value) {
  if (value === undefined || value === null || `${value}`.trim() === "") {
    return null;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function normalizeAndroidApkUpdateStatus(payload = {}) {
  const phase = `${payload.phase || "unknown"}`.trim() || "unknown";
  const message = `${payload.message || phase}`.trim() || phase;
  return {
    phase,
    message,
    versionCode: optionalAndroidApkUpdateNumber(payload.versionCode),
    versionName: `${payload.versionName || ""}`.trim(),
    apkUrl: `${payload.apkUrl || ""}`.trim(),
    bytes: optionalAndroidApkUpdateNumber(payload.bytes),
  };
}

function receiveAndroidApkUpdateStatus(payload = {}) {
  const status = normalizeAndroidApkUpdateStatus(payload);
  state.androidApkUpdateStatus = status;
  state.androidApkUpdateInFlight = androidApkUpdatePhaseInFlight(status.phase);
  renderAndroidApkUpdateSettings();
  setCommandStatus(`APK update: ${status.message}`, { stickyMs: 7000 });
}

function androidApkUpdateStatusText() {
  const status = state.androidApkUpdateStatus;
  if (!status) {
    if (layoutClient() !== "android-webview") {
      return "请在安卓 App 内打开此设置页检查并下载新版 APK。";
    }
    if (!androidApkUpdateBridge()) {
      return "安卓 APK 升级桥不可用；安装最新 APK 后请重新加载安卓 App。";
    }
    return "可以检查此守护进程的 Android 升级清单。";
  }
  const parts = [status.message];
  if (status.versionName) {
    parts.push(`versionName=${status.versionName}`);
  }
  if (status.bytes !== null) {
    parts.push(`bytes=${status.bytes}`);
  }
  if (status.apkUrl) {
    parts.push(status.apkUrl);
  }
  return parts.join(" · ");
}

function renderAndroidApkUpdateSettings() {
  const bridgeAvailable = androidApkUpdateBridgeAvailable();
  const phase = state.androidApkUpdateStatus?.phase || "";
  const summary = bridgeAvailable
    ? phase
      ? phase.replace(/_/g, " ")
      : "就绪"
    : layoutClient() === "android-webview"
      ? "bridge 不可用"
      : "仅安卓 App";
  setText("settings-apk-update-summary", summary);
  setText("settings-apk-update-source", androidApkUpdateManifestUrlForDisplay());
  setText("settings-apk-update-status", androidApkUpdateStatusText());
  if (settingsApkUpdateCheckButton) {
    settingsApkUpdateCheckButton.disabled = !bridgeAvailable || state.androidApkUpdateInFlight;
    settingsApkUpdateCheckButton.textContent = state.androidApkUpdateInFlight
      ? "正在检查 APK 升级..."
      : "检查 APK 升级";
  }
  if (settingsApkUpdateSummary) {
    settingsApkUpdateSummary.dataset.phase = phase || summary;
  }
  if (settingsApkUpdateStatus) {
    settingsApkUpdateStatus.dataset.phase = phase || summary;
  }
}

function requestAndroidApkUpdateCheck() {
  const bridge = androidApkUpdateBridge();
  if (layoutClient() !== "android-webview" || !bridge) {
    receiveAndroidApkUpdateStatus({
      phase: "failed",
      message: "APK 升级检查仅在 Freehand 安卓 App 内可用。",
    });
    return;
  }
  state.androidApkUpdateInFlight = true;
  state.androidApkUpdateStatus = {
    phase: "checking",
    message: "正在检查升级清单...",
    versionCode: null,
    versionName: "",
    apkUrl: "",
    bytes: null,
  };
  renderAndroidApkUpdateSettings();
  try {
    bridge.check();
  } catch (error) {
    receiveAndroidApkUpdateStatus({
      phase: "failed",
      message: error && error.message ? error.message : "安卓 APK 升级桥调用失败",
    });
  }
}

function settingsAuthTypeLabel(authType) {
  return authType === "apikey" ? "credential" : (authType || "credential");
}

function configProviderRegistry() {
  const registry = Array.isArray(state.configStatus?.provider_registry)
    ? state.configStatus.provider_registry
    : [];
  if (registry.length > 0) {
    return registry;
  }
  if (!state.configStatus?.provider_id) {
    return [];
  }
  return [{
    provider_id: state.configStatus.provider_id,
    enabled: true,
    provider_type: state.configStatus.provider_type,
    provider_protocol: state.configStatus.provider_protocol,
    provider_base_url: state.configStatus.provider_base_url || "",
    provider_base_url_host: state.configStatus.provider_base_url_host,
    default_model: state.configStatus.default_model,
    provider_web_search: state.configStatus.provider_web_search || "auto",
    provider_web_search_effective: state.configStatus.provider_web_search_effective || "",
    provider_web_search_reason: state.configStatus.provider_web_search_reason || "",
    provider_auth_type: state.configStatus.provider_auth_type,
    provider_auth_source: state.configStatus.provider_auth_source,
  }];
}

function configProviderById(providerId) {
  return configProviderRegistry().find((provider) => provider.provider_id === providerId) || null;
}

function providerOptionLabel(provider) {
  const stateLabel = provider.enabled === false ? "disabled" : "enabled";
  return `${provider.provider_id} · ${provider.provider_protocol} · ${provider.default_model} · ${stateLabel}`;
}

function replaceSelectOptions(select, providers, value, options = {}) {
  if (!select) {
    return;
  }
  const previous = select.value;
  const hasExplicitValue = value !== undefined && value !== null;
  select.replaceChildren();
  if (options.includeNone) {
    const none = document.createElement("option");
    none.value = "";
    none.textContent = "No fallback";
    select.append(none);
  }
  providers.forEach((provider) => {
    if (options.excludeProviderId && provider.provider_id === options.excludeProviderId) {
      return;
    }
    const option = document.createElement("option");
    option.value = provider.provider_id;
    option.disabled = provider.enabled === false;
    option.textContent = providerOptionLabel(provider);
    select.append(option);
  });
  if (hasExplicitValue) {
    select.value = value;
  }
  if (!hasExplicitValue && !select.value && previous && [...select.options].some((option) => option.value === previous)) {
    select.value = previous;
  }
  select.dataset.optionsInitialized = "true";
}

function syncProviderSelectionControls() {
  const status = state.configStatus;
  const providers = configProviderRegistry();
  const routeSource = status?.route_source || "agent";
  const modelGroupId = status?.model_group_id || "";
  const modelGroupLocked = routeSource === "model_group";
  const primaryId = state.providerSelectionDraft?.providerId || status?.provider_id || "";
  replaceSelectOptions(settingsProviderCurrentSelect, providers, primaryId);
  const selectedPrimary = settingsProviderCurrentSelect?.value || status?.provider_id || "";
  const fallbackDraft = state.providerSelectionDraft
    ? state.providerSelectionDraft.fallbackProviderId || ""
    : status?.fallback_provider_id || "";
  replaceSelectOptions(settingsProviderFallbackSelect, providers, fallbackDraft, {
    includeNone: true,
    excludeProviderId: selectedPrimary,
  });
  const currentFallback = settingsProviderFallbackSelect?.value || "";
  if (state.providerSelectionDraft) {
    state.providerSelectionDraft = {
      providerId: selectedPrimary,
      fallbackProviderId: currentFallback,
    };
  }
  const selectionChanged =
    Boolean(status) &&
    (selectedPrimary !== (status.provider_id || "") ||
      currentFallback !== (status.fallback_provider_id || ""));
  if (settingsProviderSwitchButton) {
    settingsProviderSwitchButton.disabled =
      state.providerSelectionInFlight ||
      !status ||
      !selectedPrimary ||
      !selectionChanged ||
      modelGroupLocked;
    settingsProviderSwitchButton.textContent = state.providerSelectionInFlight
      ? "保存中..."
      : "切换模型服务";
  }
  const strategyStatus = document.getElementById("settings-provider-switch-status");
  if (strategyStatus) {
    if (modelGroupLocked) {
      strategyStatus.textContent = `当前 Agent 由模型组 ${modelGroupId || "(未命名)"} 决定主/备用路由；请到「模型组」页修改，切换模型服务在此处已锁定。`;
    } else {
      strategyStatus.textContent = "切换会保存选中的 Agent 模型服务；重启后活动运行时才会生效。";
    }
  }
  if (settingsProviderCurrentSelect) {
    settingsProviderCurrentSelect.disabled = modelGroupLocked;
  }
  if (settingsProviderFallbackSelect) {
    settingsProviderFallbackSelect.disabled = modelGroupLocked;
  }
}

function updateProviderSelectionDraftFromControls() {
  state.providerSelectionDraft = {
    providerId: settingsProviderCurrentSelect?.value || "",
    fallbackProviderId: settingsProviderFallbackSelect?.value || "",
  };
}

function fillSettingsProviderFormFromProvider(provider) {
  if (!provider) {
    return;
  }
  if (settingsProviderIdInput) {
    settingsProviderIdInput.value = provider.provider_id || "";
  }
  if (settingsProviderTypeInput) {
    settingsProviderTypeInput.value = provider.provider_type || "openai";
  }
  if (settingsProviderProtocolInput) {
    settingsProviderProtocolInput.value = provider.provider_protocol || "responses";
  }
  if (settingsProviderUrlInput) {
    settingsProviderUrlInput.value = provider.provider_base_url || "";
  }
  if (settingsProviderModelInput) {
    settingsProviderModelInput.value = provider.default_model || "";
  }
  if (settingsProviderWebSearchInput) {
    settingsProviderWebSearchInput.value = provider.provider_web_search || "auto";
  }
  if (settingsProviderEnvInput && document.activeElement !== settingsProviderEnvInput) {
    settingsProviderEnvInput.value = "";
  }
  setText(
    "settings-provider-save-status",
    provider.provider_auth_source === "inline"
      ? "此模型服务在配置中使用内联鉴权。从界面保存会改写为环境变量鉴权。"
      : "已加载安全的模型服务字段。保存前请输入凭证环境变量。",
  );
}

function renderSettingsProviderRegistry() {
  if (!settingsProviderRegistryList) {
    return;
  }
  const providers = configProviderRegistry();
  if (!state.configStatus) {
    settingsProviderRegistryList.textContent = state.configStatusError || "正在加载模型服务注册表";
    return;
  }
  if (providers.length === 0) {
    settingsProviderRegistryList.textContent = "尚未配置模型服务。";
    return;
  }
  const cards = providers.map((provider) => {
    const card = document.createElement("article");
    card.className = "settings-provider-card";
    card.classList.toggle("is-active", provider.provider_id === state.configStatus.provider_id);
    card.classList.toggle("is-disabled", provider.enabled === false);
    card.dataset.providerId = provider.provider_id || "";
    const title = document.createElement("div");
    title.className = "settings-provider-card-title";
    const name = document.createElement("span");
    name.textContent = provider.provider_id || "unknown";
    const badge = document.createElement("span");
    badge.textContent = provider.provider_id === state.configStatus.provider_id
      ? "current"
      : provider.enabled === false
        ? "disabled"
        : "available";
    title.append(name, badge);
    const meta = document.createElement("div");
    meta.className = "settings-provider-card-meta";
    meta.textContent = [
      `${provider.provider_type}/${provider.provider_protocol}`,
      provider.default_model,
      `web_search=${webSearchStatusLabel(provider)}`,
      provider.provider_base_url_host || provider.provider_base_url,
      `${settingsAuthTypeLabel(provider.provider_auth_type)} ${provider.provider_auth_source}`,
    ].filter(Boolean).join(" · ");
    if (provider.provider_web_search_reason) {
      meta.title = provider.provider_web_search_reason;
    }
    const action = document.createElement("button");
    action.className = "settings-secondary-action";
    action.type = "button";
    action.textContent = "载入表单";
    action.addEventListener("click", () => fillSettingsProviderFormFromProvider(provider));
    const testAction = document.createElement("button");
    testAction.className = "settings-secondary-action";
    testAction.type = "button";
    testAction.disabled = provider.enabled === false || Boolean(state.providerWebSearchTestInFlight);
    testAction.textContent = state.providerWebSearchTestInFlight === provider.provider_id
      ? "正在测试联网搜索..."
      : "测试联网搜索";
    testAction.addEventListener("click", () => {
      fillSettingsProviderFormFromProvider(provider);
      testProviderWebSearch(provider.provider_id).catch((error) => {
        setText("settings-provider-web-search-test-status", `模型服务联网搜索测试失败: ${error.message}`);
      });
    });
    card.append(title, meta, action, testAction);
    return card;
  });
  settingsProviderRegistryList.replaceChildren(...cards);
}

function syncSettingsProviderForm() {
  const status = state.configStatus;
  const provider = configProviderById(status?.provider_id) || null;
  setInputValueIfNotFocused(settingsProviderIdInput, provider?.provider_id || status?.provider_id || "");
  setInputValueIfNotFocused(settingsProviderTypeInput, provider?.provider_type || status?.provider_type || "openai");
  setInputValueIfNotFocused(settingsProviderProtocolInput, provider?.provider_protocol || status?.provider_protocol || "responses");
  setInputValueIfNotFocused(settingsProviderUrlInput, provider?.provider_base_url || status?.provider_base_url || "");
  setInputValueIfNotFocused(settingsProviderModelInput, provider?.default_model || status?.default_model || "");
  setInputValueIfNotFocused(settingsProviderWebSearchInput, provider?.provider_web_search || status?.provider_web_search || "auto");
  if (settingsProviderEnvInput && document.activeElement !== settingsProviderEnvInput && !settingsProviderEnvInput.value) {
    settingsProviderEnvInput.value = "";
  }
  if (settingsProviderSaveButton) {
    settingsProviderSaveButton.disabled = state.configSaveInFlight;
    settingsProviderSaveButton.textContent = state.configSaveInFlight ? "保存中..." : "新增/更新模型服务";
  }
  if (settingsProviderWebSearchTestButton) {
    const providerId = settingsProviderIdInput?.value.trim() || status?.provider_id || "";
    settingsProviderWebSearchTestButton.disabled = !state.configStatus || !providerId || Boolean(state.providerWebSearchTestInFlight);
    settingsProviderWebSearchTestButton.textContent = state.providerWebSearchTestInFlight === providerId
      ? "正在测试联网搜索..."
      : "测试联网搜索";
  }
}

function setInputValueIfNotFocused(input, value) {
  if (!input || document.activeElement === input) {
    return;
  }
  input.value = value || "";
}

async function submitProviderConfigUpdate(event) {
  event.preventDefault();
  if (!state.configStatus) {
    setText("settings-provider-save-status", "配置状态尚未加载。");
    return;
  }
  const update = {
    agent_name: state.configStatus.agent_name,
    provider_id: settingsProviderIdInput?.value.trim() || "",
    provider_type: settingsProviderTypeInput?.value.trim() || "",
    provider_protocol: settingsProviderProtocolInput?.value.trim() || "",
    base_url: settingsProviderUrlInput?.value.trim() || "",
    default_model: settingsProviderModelInput?.value.trim() || "",
    web_search: settingsProviderWebSearchInput?.value.trim() || "auto",
    api_key_env: settingsProviderEnvInput?.value.trim() || "",
  };
  state.configSaveInFlight = true;
  setText("settings-provider-save-status", "正在保存配置...");
  renderSettingsShell();
  try {
    const receipt = await adpCommand(adpCommandOf("UpsertProviderConfig", { update }));
    setCommandStatus(providerConfigUpsertReceiptStatus(receipt), { stickyMs: 5000 });
    await refreshConfigStatus();
    setText("settings-provider-save-status", "模型服务定义已保存。重启后活动运行时才会生效。");
  } catch (error) {
    state.configStatusError = error.message;
    setText("settings-provider-save-status", `保存失败: ${error.message}`);
    renderSettingsShell();
  } finally {
    state.configSaveInFlight = false;
    renderSettingsShell();
  }
}

async function testProviderWebSearch(providerId) {
  const targetProviderId = (providerId || settingsProviderIdInput?.value || "").trim();
  if (!targetProviderId) {
    setText("settings-provider-web-search-test-status", "测试联网搜索前请选择模型服务 ID。");
    return;
  }
  state.providerWebSearchTestInFlight = targetProviderId;
  setText("settings-provider-web-search-test-status", `正在测试 ${targetProviderId} 的模型服务托管联网搜索...`);
  renderSettingsShell();
  try {
    const receipt = await adpCommand(adpCommandOf("TestProviderWebSearch", {
        provider_id: targetProviderId,
        query: "Use web_search to find the current UTC date and one current news headline from openai.com today. Do not answer from memory.",
      }));
    const status = providerWebSearchTestReceiptStatus(receipt);
    setText("settings-provider-web-search-test-status", status);
    setCommandStatus(status, { stickyMs: 8000 });
  } catch (error) {
    const message = `模型服务 ${targetProviderId} 联网搜索测试失败: ${error.message}`;
    setText("settings-provider-web-search-test-status", message);
    setCommandStatus(message, { stickyMs: 10000 });
  } finally {
    state.providerWebSearchTestInFlight = "";
    renderSettingsShell();
  }
}

async function submitProviderSelectionUpdate() {
  if (!state.configStatus) {
    setText("settings-provider-switch-status", "配置状态尚未加载。");
    return;
  }
  const providerId = settingsProviderCurrentSelect?.value.trim() || "";
  const fallbackProviderId = settingsProviderFallbackSelect?.value.trim() || "";
  const selection = {
    agent_name: state.configStatus.agent_name,
    provider_id: providerId,
    fallback_provider_id: fallbackProviderId || null,
  };
  state.providerSelectionInFlight = true;
  setText("settings-provider-switch-status", "正在保存模型服务选择...");
  renderSettingsShell();
  try {
    const receipt = await adpCommand(adpCommandOf("UpdateAgentProviderSelection", { selection }));
    setCommandStatus(providerSelectionReceiptStatus(receipt), { stickyMs: 5000 });
    await refreshConfigStatus();
    state.providerSelectionDraft = null;
    setText("settings-provider-switch-status", "模型服务选择已保存。重启后活动运行时才会生效。");
  } catch (error) {
    state.configStatusError = error.message;
    setText("settings-provider-switch-status", `切换失败: ${error.message}`);
    renderSettingsShell();
  } finally {
    state.providerSelectionInFlight = false;
    renderSettingsShell();
  }
}

function configModelGroupRegistry() {
  return Array.isArray(state.configStatus?.model_group_registry)
    ? state.configStatus.model_group_registry
    : [];
}

function configModelGroupById(groupId) {
  return configModelGroupRegistry().find((group) => group.group_id === groupId) || null;
}

function activeModelGroup() {
  return configModelGroupById(state.configStatus?.model_group_id || "");
}

function modelRouteLabel(route) {
  return route ? `${route.provider_id}:${route.model}` : "无";
}

function modelGroupOptionLabel(group) {
  const stateLabel = group.enabled === false ? "disabled" : "enabled";
  return `${group.group_id} · ${group.label || group.group_id} · ${modelRouteLabel(group.primary)} · ${stateLabel}`;
}

function replaceModelGroupOptions(select, groups, value) {
  if (!select) {
    return;
  }
  select.replaceChildren();
  const none = document.createElement("option");
  none.value = "";
  none.textContent = "没有启用模型组";
  select.append(none);
  groups.forEach((group) => {
    const option = document.createElement("option");
    option.value = group.group_id;
    option.disabled = group.enabled === false;
    option.textContent = modelGroupOptionLabel(group);
    select.append(option);
  });
  select.value = value || "";
  select.dataset.optionsInitialized = "true";
}

function syncModelGroupSelectionControls() {
  const status = state.configStatus;
  const groups = configModelGroupRegistry();
  const activeGroupId = state.modelGroupSelectionDraft?.modelGroupId || status?.model_group_id || "";
  replaceModelGroupOptions(settingsModelGroupCurrentSelect, groups, activeGroupId);
  const selectedGroupId = settingsModelGroupCurrentSelect?.value || "";
  if (state.modelGroupSelectionDraft) {
    state.modelGroupSelectionDraft = { modelGroupId: selectedGroupId };
  }
  const changed = Boolean(status) && selectedGroupId !== (status.model_group_id || "");
  setText(
    "settings-model-group-summary",
    status
      ? `${status.model_group_id || "无"} · ${groups.length} configured`
      : state.configStatusError || "加载中",
  );
  if (settingsModelGroupSwitchButton) {
    settingsModelGroupSwitchButton.disabled =
      state.modelGroupSelectionInFlight || !status || !changed;
    settingsModelGroupSwitchButton.textContent = state.modelGroupSelectionInFlight
      ? "保存中..."
      : "切换模型组";
  }
}

function updateModelGroupSelectionDraftFromControl() {
  state.modelGroupSelectionDraft = {
    modelGroupId: settingsModelGroupCurrentSelect?.value || "",
  };
}

function replaceProviderRouteOptions(select, value, includeNone = true) {
  if (!select) {
    return;
  }
  const providers = configProviderRegistry();
  select.replaceChildren();
  if (includeNone) {
    const none = document.createElement("option");
    none.value = "";
    none.textContent = "No route";
    select.append(none);
  }
  providers.forEach((provider) => {
    const option = document.createElement("option");
    option.value = provider.provider_id;
    option.disabled = provider.enabled === false;
    option.textContent = providerOptionLabel(provider);
    select.append(option);
  });
  select.value = value || "";
}

function syncSettingsModelGroupForm() {
  const status = state.configStatus;
  const group = activeModelGroup() || configModelGroupRegistry()[0] || null;
  const primaryProvider = group?.primary?.provider_id || status?.provider_id || "";
  const primaryModel = group?.primary?.model || status?.default_model || "";
  setInputValueIfNotFocused(settingsModelGroupIdInput, group?.group_id || "default");
  setInputValueIfNotFocused(settingsModelGroupLabelInput, group?.label || "默认模型组");
  if (settingsModelGroupEnabledInput && document.activeElement !== settingsModelGroupEnabledInput) {
    settingsModelGroupEnabledInput.checked = group ? group.enabled !== false : true;
  }
  replaceProviderRouteOptions(settingsModelGroupPrimaryProviderInput, primaryProvider, false);
  replaceProviderRouteOptions(settingsModelGroupSubProviderInput, group?.sub?.provider_id || "");
  replaceProviderRouteOptions(settingsModelGroupSearchProviderInput, group?.search?.provider_id || "");
  replaceProviderRouteOptions(settingsModelGroupTitleProviderInput, group?.title?.provider_id || "");
  replaceProviderRouteOptions(settingsModelGroupFallbackProviderInput, group?.fallback?.provider_id || status?.fallback_provider_id || "");
  setInputValueIfNotFocused(settingsModelGroupPrimaryModelInput, primaryModel);
  setInputValueIfNotFocused(settingsModelGroupSubModelInput, group?.sub?.model || "");
  setInputValueIfNotFocused(settingsModelGroupSearchModelInput, group?.search?.model || "");
  setInputValueIfNotFocused(settingsModelGroupTitleModelInput, group?.title?.model || "");
  setInputValueIfNotFocused(settingsModelGroupFallbackModelInput, group?.fallback?.model || "");
  setInputValueIfNotFocused(
    settingsModelGroupLoadBalanceInput,
    Array.isArray(group?.load_balance)
      ? group.load_balance
          .map((route) => `${route.provider_id}:${route.model}:${route.weight}`)
          .join(", ")
      : "",
  );
  if (settingsModelGroupSaveButton) {
    settingsModelGroupSaveButton.disabled = state.modelGroupSaveInFlight || !status;
    settingsModelGroupSaveButton.textContent = state.modelGroupSaveInFlight
      ? "保存中..."
      : "新增/更新模型组";
  }
}

function renderSettingsModelGroupRegistry() {
  if (!settingsModelGroupRegistryList) {
    return;
  }
  if (!state.configStatus) {
    settingsModelGroupRegistryList.textContent = state.configStatusError || "正在加载模型组";
    return;
  }
  const groups = configModelGroupRegistry();
  if (groups.length === 0) {
    settingsModelGroupRegistryList.textContent = "尚未配置模型组。可在下方新增并绑定主、子任务、搜索、标题和备用路由。";
    return;
  }
  const cards = groups.map((group) => {
    const card = document.createElement("article");
    card.className = "settings-model-group-card";
    card.classList.toggle("is-active", group.group_id === state.configStatus.model_group_id);
    card.classList.toggle("is-disabled", group.enabled === false);
    card.dataset.modelGroupId = group.group_id || "";
    const title = document.createElement("div");
    title.className = "settings-model-group-card-title";
    const name = document.createElement("span");
    name.textContent = group.label || group.group_id || "unknown";
    const badge = document.createElement("span");
    badge.textContent = group.group_id === state.configStatus.model_group_id
      ? "active"
      : group.enabled === false
        ? "disabled"
        : "available";
    title.append(name, badge);
    const meta = document.createElement("div");
    meta.className = "settings-model-group-card-meta";
    meta.textContent = [
      `id=${group.group_id}`,
      `primary=${modelRouteLabel(group.primary)}`,
      `sub=${modelRouteLabel(group.sub)}`,
      `search=${modelRouteLabel(group.search)}`,
      `title=${modelRouteLabel(group.title)}`,
      `fallback=${modelRouteLabel(group.fallback)}`,
      `load_balance=${Array.isArray(group.load_balance) ? group.load_balance.length : 0}`,
      `context=${formatTokenCount(group.context_window_tokens)}`,
      `compact_at=${formatTokenCount(group.compaction_threshold_tokens)}`,
    ].join(" · ");
    const action = document.createElement("button");
    action.className = "settings-secondary-action";
    action.type = "button";
    action.textContent = "载入表单";
    action.addEventListener("click", () => fillSettingsModelGroupForm(group));
    card.append(title, meta, action);
    return card;
  });
  settingsModelGroupRegistryList.replaceChildren(...cards);
}

function fillSettingsModelGroupForm(group) {
  if (!group) {
    return;
  }
  setInputDirect(settingsModelGroupIdInput, group.group_id || "");
  setInputDirect(settingsModelGroupLabelInput, group.label || group.group_id || "");
  if (settingsModelGroupEnabledInput) {
    settingsModelGroupEnabledInput.checked = group.enabled !== false;
  }
  fillModelRouteInputs("primary", group.primary);
  fillModelRouteInputs("sub", group.sub);
  fillModelRouteInputs("search", group.search);
  fillModelRouteInputs("title", group.title);
  fillModelRouteInputs("fallback", group.fallback);
  setInputDirect(
    settingsModelGroupLoadBalanceInput,
    Array.isArray(group.load_balance)
      ? group.load_balance
          .map((route) => `${route.provider_id}:${route.model}:${route.weight}`)
          .join(", ")
      : "",
  );
  setInputDirect(
    settingsModelGroupContextWindowInput,
    group.context_window_tokens || 128000,
  );
  setInputDirect(
    settingsModelGroupCompactionThresholdInput,
    group.compaction_threshold_tokens || 100000,
  );
}

function setInputDirect(input, value) {
  if (input) {
    input.value = value || "";
  }
}

function fillModelRouteInputs(routeName, route) {
  const controls = modelRouteControls(routeName);
  if (controls.provider) {
    controls.provider.value = route?.provider_id || "";
  }
  if (controls.model) {
    controls.model.value = route?.model || "";
  }
}

function modelRouteControls(routeName) {
  const map = {
    primary: [settingsModelGroupPrimaryProviderInput, settingsModelGroupPrimaryModelInput],
    sub: [settingsModelGroupSubProviderInput, settingsModelGroupSubModelInput],
    search: [settingsModelGroupSearchProviderInput, settingsModelGroupSearchModelInput],
    title: [settingsModelGroupTitleProviderInput, settingsModelGroupTitleModelInput],
    fallback: [settingsModelGroupFallbackProviderInput, settingsModelGroupFallbackModelInput],
  };
  const [provider, model] = map[routeName] || [];
  return { provider, model };
}

function routeUpdateFromControls(routeName, required = false) {
  const controls = modelRouteControls(routeName);
  const providerId = controls.provider?.value.trim() || "";
  const model = controls.model?.value.trim() || "";
  if (!providerId && !model && !required) {
    return null;
  }
  if (!providerId || !model) {
    throw new Error(`${routeName} route requires both provider and model`);
  }
  return {
    provider_id: providerId,
    model,
  };
}

function parseLoadBalanceRoutes(text) {
  const trimmed = (text || "").trim();
  if (!trimmed) {
    return [];
  }
  return trimmed.split(",").map((entry) => {
    const parts = entry.trim().split(":").map((part) => part.trim());
    if (parts.length !== 3 || !parts[0] || !parts[1] || !parts[2]) {
      throw new Error("Load balance routes must use provider:model:weight");
    }
    const weight = Number.parseInt(parts[2], 10);
    if (!Number.isInteger(weight) || weight <= 0) {
      throw new Error("Load balance weight must be a positive integer");
    }
    return {
      provider_id: parts[0],
      model: parts[1],
      weight,
    };
  });
}

async function submitModelGroupConfigUpdate(event) {
  event.preventDefault();
  if (!state.configStatus) {
    setText("settings-model-group-save-status", "配置状态尚未加载。");
    return;
  }
  let group;
  try {
    group = {
      agent_name: state.configStatus.agent_name,
      group_id: settingsModelGroupIdInput?.value.trim() || "",
      enabled: settingsModelGroupEnabledInput ? settingsModelGroupEnabledInput.checked : true,
      label: settingsModelGroupLabelInput?.value.trim() || "",
      primary: routeUpdateFromControls("primary", true),
      sub: routeUpdateFromControls("sub"),
      search: routeUpdateFromControls("search"),
      title: routeUpdateFromControls("title"),
      fallback: routeUpdateFromControls("fallback"),
      load_balance: parseLoadBalanceRoutes(settingsModelGroupLoadBalanceInput?.value || ""),
      context_window_tokens: Number.parseInt(settingsModelGroupContextWindowInput?.value || "0", 10),
      compaction_threshold_tokens: Number.parseInt(settingsModelGroupCompactionThresholdInput?.value || "0", 10),
    };
    if (!Number.isInteger(group.context_window_tokens) || group.context_window_tokens <= 0
      || !Number.isInteger(group.compaction_threshold_tokens)
      || group.compaction_threshold_tokens <= 0
      || group.compaction_threshold_tokens >= group.context_window_tokens) {
      throw new Error("压缩门限必须大于 0 且小于上下文窗口");
    }
  } catch (error) {
    setText("settings-model-group-save-status", `模型组无效: ${error.message}`);
    return;
  }
  state.modelGroupSaveInFlight = true;
  setText("settings-model-group-save-status", "正在保存模型组...");
  renderSettingsShell();
  try {
    const receipt = await adpCommand(adpCommandOf("UpsertModelGroupConfig", { group }));
    setCommandStatus(modelGroupUpsertReceiptStatus(receipt), { stickyMs: 5000 });
    await refreshConfigStatus();
    setText("settings-model-group-save-status", "模型组已保存。重启后活动运行时才会生效。");
  } catch (error) {
    state.configStatusError = error.message;
    setText("settings-model-group-save-status", `保存失败: ${error.message}`);
    renderSettingsShell();
  } finally {
    state.modelGroupSaveInFlight = false;
    renderSettingsShell();
  }
}

async function submitModelGroupSelectionUpdate() {
  if (!state.configStatus) {
    setText("settings-model-group-switch-status", "配置状态尚未加载。");
    return;
  }
  const selection = {
    agent_name: state.configStatus.agent_name,
    model_group_id: settingsModelGroupCurrentSelect?.value.trim() || null,
  };
  state.modelGroupSelectionInFlight = true;
  setText("settings-model-group-switch-status", "正在保存模型组选择...");
  renderSettingsShell();
  try {
    const receipt = await adpCommand(adpCommandOf("UpdateAgentModelGroupSelection", { selection }));
    setCommandStatus(modelGroupSelectionReceiptStatus(receipt), { stickyMs: 5000 });
    await refreshConfigStatus();
    state.modelGroupSelectionDraft = null;
    setText("settings-model-group-switch-status", "模型组选择已保存。重启后活动运行时才会生效。");
  } catch (error) {
    state.configStatusError = error.message;
    setText("settings-model-group-switch-status", `切换失败: ${error.message}`);
    renderSettingsShell();
  } finally {
    state.modelGroupSelectionInFlight = false;
    renderSettingsShell();
  }
}

function renderTurnMeta() {
  if (state.pendingSubmitError) {
    setText("turn-status", "检查服务真源 · 提交回执未验证");
  }
  const turn = activeTurnForSelectedSession();
  if (!turn) {
    const selectedSummary = sessionSummaryForSelected();
    setText("session-title", selectedSummary?.title || state.selectedSessionId || "等待服务状态");
    setText("session-copy", state.selectedSessionId ? `${selectedSummary?.session_id || state.selectedSessionId} · 选中会话暂无轮次` : "暂无活跃轮次");
    setShellDataset("selectedSession", state.selectedSessionId || "");
    setShellDataset("selectedTurn", "");
    setShellDataset("selectedCwd", state.selectedCwd || "");
    if (!state.pendingSubmitError) {
      setText("turn-status", liveTurnStatus() || "等待中");
    }
    syncSelectedSessionActions();
    return;
  }

  const selectedSummary = sessionSummaryForSelected();
  setText("session-title", selectedSummary?.title || turn.session_id);
  setText("session-copy", turn.cwd ? `${turn.turn_id} · ${turn.cwd}` : turn.turn_id);
  setShellDataset("selectedSession", turn.session_id || "");
  setShellDataset("selectedTurn", turn.turn_id || "");
  setShellDataset("selectedCwd", turn.cwd || state.selectedCwd || "");
  const runningTools = (turn.tool_activities || []).filter((tool) => tool.status === "Waiting" || tool.status === "waiting");
  const turnStatus = turn.terminal_text || isTerminalStatus(turn.terminal_status) || isToolPendingStatus(turn.terminal_status)
    ? terminalTurnStatusLabelForTurn(turn, turn.terminal_status)
    : runningTools.length > 0
      ? waitingToolStatus(runningTools).replace("tool executing", "tool running")
      : turnIsWaitingForModelResponse(turn)
        ? liveTurnStatus()
        : state.submitInFlight
          ? liveTurnStatus()
          : "等待中";
  if (!state.pendingSubmitError) {
    setText("turn-status", turnStatus);
  }
  syncSelectedSessionActions();
}

function syncSelectedSessionActions() {
  if (!selectedSessionRenameButton) {
    return;
  }
  const canRename = selectedSessionDetailRouteActive() && !isDraftSessionId(state.selectedSessionId) && !!sessionSummaryForSelected();
  selectedSessionRenameButton.hidden = !canRename;
  selectedSessionRenameButton.disabled = !canRename;
  selectedSessionRenameButton.dataset.sessionId = canRename ? state.selectedSessionId : "";
}

setInterval(() => {
  if (renderModelHasLiveLifecycle() || headerWorkerRailNeedsClock()) {
    renderMessages();
    renderTurnMeta();
    renderCommandStatus();
    if (headerWorkerRailNeedsClock()) {
      renderSessionRelationHeader();
    }
  }
  refreshHeaderWorkerRailStatusIfNeeded();
}, 1000);

setInterval(() => {
  if (document.visibilityState === "hidden" || !hasNonTerminalProtocolActivity()) {
    return;
  }
  refreshProtocolStateAfterForeground("live truth watchdog");
}, liveTruthWatchdogIntervalMs);

function renderModelHasLiveLifecycle() {
  if (state.submitInFlight || !!state.pendingUserInput) {
    return true;
  }
  return buildConversationRenderModel().turns.some((turn) => turn.lifecycle.isLive);
}

function hasNonTerminalProtocolActivity() {
  if (state.submitInFlight || !!state.pendingUserInput) {
    return true;
  }
  if (selectedWorkerTranscriptRefreshRetryable()) {
    return true;
  }
  if (turnRequiresLifecycleTruthRefresh(activeTurnForSelectedSession())) {
    return true;
  }
  if (headerWorkerRailHasOpenTasks()) {
    return true;
  }
  return conversationTurnsForRender().some((turn) => {
    if (!turn || turn.terminal_text || isTerminalStatus(turn.terminal_status) || isToolPendingStatus(turn.terminal_status)) {
      return false;
    }
    const waitingTools = (turn.tool_activities || []).some(
      (tool) => tool.status === "Waiting" || tool.status === "waiting",
    );
    return waitingTools || !!turn.model_request;
  });
}

function turnRequiresLifecycleTruthRefresh(turn) {
  if (!turn || !isToolPendingStatus(turn.terminal_status)) {
    return false;
  }
  return !lifecycleOwnerProjectionLoaded() || toolPendingRepresentsLifecycle(turn);
}

function renderAll() {
  setText("workspace-status", state.adpStatus);
  renderDebugDetailsToggle();
  renderSessions();
  renderTurnMeta();
  renderSessionRelationHeader();
  renderMessages();
  renderAttachmentTray();
  renderDebug();
  renderCheckpoints();
  renderPhase2Dashboard();
  renderMobileHomeDashboard();
  renderSettingsShell();
  renderToolsDashboard();
  renderCommandStatus();
}

async function refreshTurn() {
  const result = await adpQuery(adpQueryOf("QueryLatestActiveTurn"));
  applyAdpQueryResult(result);
  await refreshCheckpoints();
}

async function refreshSessions() {
  const result = await adpQuery(adpQueryOf("QuerySessionList"));
  setSessionList(variantPayload(result, "SessionList") || { sessions: [] });
  renderAll();
}

async function refreshSelectedSession() {
  if (!state.selectedSessionId) {
    state.sessionTurns = [];
    clearSessionRefreshState(null);
    setTurnProjection(null, { preserveSessionTurns: true });
    renderAll();
    return;
  }
  const requestedSessionId = state.selectedSessionId;
  state.sessionRefreshInFlight = requestedSessionId;
  const result = await adpQuery(adpQueryOf("QuerySessionTurns", { session_id: requestedSessionId }));
  if (state.selectedSessionId !== requestedSessionId) {
    return;
  }
  clearSessionRefreshState(requestedSessionId);
  applyAdpQueryResult(result);
  if (state.turn) {
    refreshDebug().catch((error) => {
      setCommandStatus(`debug query failed: ${error.message}`);
    });
  }
}

async function refreshDebug() {
  if (!state.turn) {
    state.debug = null;
    renderDebug();
    return;
  }
  const result = await adpQuery(adpQueryOf("QueryDebugState", { turn_id: state.turn.turn_id }));
  applyAdpQueryResult(result);
}

async function refreshCheckpoints() {
  const result = await adpQuery(adpQueryOf("QueryCheckpoints"));
  applyAdpQueryResult(result);
  renderAll();
}

async function refreshConfigStatus() {
  try {
    const result = await adpQuery(adpQueryOf("QueryConfigStatus"));
    applyAdpQueryResult(result);
  } catch (error) {
    state.configStatus = null;
    state.configStatusError = error.message;
    renderSettingsShell();
    renderPhase2Dashboard();
  }
}

async function dispatchAccountConfigSync(commandName) {
  if (state.accountConfigSyncInFlight) {
    return;
  }
  state.accountConfigSyncInFlight = true;
  renderSettingsShell();
  try {
    const receipt = await adpCommand(adpCommandOf(commandName));
    setCommandStatus(commandReceiptStatus(receipt), { stickyMs: 7000 });
    await refreshConfigStatus();
  } catch (error) {
    setCommandStatus(`账号配置同步失败：${error.message}`, { stickyMs: 9000 });
    await refreshConfigStatus().catch(() => {});
  } finally {
    state.accountConfigSyncInFlight = false;
    renderSettingsShell();
  }
}

async function refreshDiagnosticsStatus() {
  state.diagnosticsInFlight = true;
  renderSettingsDiagnostics();
  try {
    const result = await adpQuery(adpQueryOf("QueryDiagnostics"));
    applyPhase2QueryResult(result);
  } catch (error) {
    state.diagnosticsError = error.message;
    setCommandStatus(`诊断刷新失败: ${error.message}`, { stickyMs: 9000 });
  } finally {
    state.diagnosticsInFlight = false;
    renderSettingsDiagnostics();
  }
}

async function refreshAllProtocolState() {
  await refreshSessions();
  try {
    await refreshSelectedSession();
  } catch (error) {
    renderSessionRefreshFailure(error, state.selectedSessionId);
  }
  if (!state.selectedSessionId && !state.sessionListLoaded) {
    await refreshTurn();
  }
  await refreshCheckpoints();
  await refreshConfigStatus();
  await refreshDiagnosticsStatus();
  await refreshPhase2Status();
}

function shouldRefreshAfterForeground() {
  if (document.visibilityState === "hidden") {
    return false;
  }
  if (state.foregroundRefreshInFlight) {
    return false;
  }
  if (!hasRecoverableProtocolState()) {
    return false;
  }
  const now = Date.now();
  if (now - state.foregroundRefreshLastAt < foregroundRefreshMinIntervalMs) {
    return false;
  }
  state.foregroundRefreshLastAt = now;
  return true;
}

function refreshProtocolStateAfterForeground(reason) {
  if (!shouldRefreshAfterForeground()) {
    return;
  }
  state.foregroundRefreshInFlight = true;
  setBackgroundCommandStatus(`${reason} 后检查服务真源...`);
  refreshAllProtocolState()
    .then(() => {
      clearPendingUserInputIfMaterialized();
      renderAll();
      setBackgroundCommandStatus(`${reason} 后已刷新服务真源`);
    })
    .catch((error) => {
      setCommandStatus(`${reason} 后刷新服务失败：${error.message}`, { stickyMs: 8000 });
      scheduleAdpReconnect(reason);
    })
    .finally(() => {
      state.foregroundRefreshInFlight = false;
    });
}

async function refreshAfterAmbiguousSubmitFailure(error) {
  const refreshErrors = [];
  const captureRefresh = async (refresh) => {
    try {
      await refresh();
    } catch (refreshError) {
      refreshErrors.push(refreshError && refreshError.message ? refreshError.message : `${refreshError}`);
    }
  };
  await captureRefresh(refreshSessions);
  await captureRefresh(refreshSelectedSession);
  if (!pendingUserInputIsMaterialized()) {
    await captureRefresh(refreshTurn);
  }
  await captureRefresh(refreshPhase2Status);
  clearPendingUserInputIfMaterialized();
  const materialized = !state.pendingUserInput;
  const baseMessage = error && error.message ? error.message : `${error}`;
  return {
    materialized,
    message: refreshErrors.length > 0
      ? `${baseMessage}；刷新也失败：${refreshErrors.join("；")}`
      : baseMessage,
  };
}

function stopAmbiguousSubmitRecoveryPolling() {
  if (state.ambiguousSubmitRecoveryTimer) {
    window.clearInterval(state.ambiguousSubmitRecoveryTimer);
  }
  state.ambiguousSubmitRecoveryTimer = null;
}

function startAmbiguousSubmitRecoveryPolling(startedAtMs) {
  stopAmbiguousSubmitRecoveryPolling();
  state.ambiguousSubmitRecoveryStartedAt = startedAtMs || Date.now();
  let attempts = 0;
  state.ambiguousSubmitRecoveryTimer = window.setInterval(async () => {
    if (!state.pendingUserInput) {
      stopAmbiguousSubmitRecoveryPolling();
      return;
    }
    attempts += 1;
    try {
      const recovery = await refreshAfterAmbiguousSubmitFailure(new Error("服务真源刷新未完成"));
      if (recovery.materialized) {
        state.pendingSubmitError = null;
        state.pendingAttachments = [];
        renderAll();
        setCommandStatus("服务真源已接收请求；生命周期可见", { stickyMs: 5000 });
        stopAmbiguousSubmitRecoveryPolling();
      } else {
        renderAll();
      }
    } catch (error) {
      state.pendingSubmitError = error.message;
      renderAll();
    }
    if (attempts >= 20) {
      setCommandStatus("服务检查后提交回执仍未验证；重试前请先刷新", { stickyMs: 8000 });
      stopAmbiguousSubmitRecoveryPolling();
    }
  }, 2000);
}

function ensureTurnSubscription() {
  if (state.adpSubscriptions.has("latest-turn")) {
    return;
  }
  state.adpSubscriptions.add("latest-turn");
  adpSubscribe(adpSubscribeOf("SubscribeLatestActiveTurn", { client: adpClientKind() }), "sub-turn").catch((error) => {
    setCommandStatus(`turn subscription failed: ${error.message}`);
  });
}

function ensureSseTurnSubscription() {
  if (state.sseTurnStream || typeof EventSource === "undefined") {
    return;
  }
  const endpoint = shellConfig().turnSubscribe;
  if (!endpoint) {
    return;
  }
  const stream = new EventSource(endpoint);
  state.sseTurnStream = stream;  stream.addEventListener("open", () => {
    setBackgroundCommandStatus("SSE turn refresh connected");
  });
  stream.addEventListener("turn", (event) => {
    try {
      const turn = JSON.parse(event.data);
      if (turn && !sessionTruthAllowsTurn(turn)) {
        renderCommandStatus();
        return;
      }
      if (state.selectedSessionId && turn.session_id !== state.selectedSessionId) {
        renderCommandStatus();
        return;
      }
      setTurnProjection(turn);
      setBackgroundCommandStatus("SSE turn refresh received");
      renderAll();
    } catch (error) {
      setCommandStatus(`SSE turn refresh decode failed: ${error.message}`, { stickyMs: 8000 });
    }
  });
  stream.addEventListener("error", () => {
    setBackgroundCommandStatus("SSE turn refresh reconnecting...");
  });
}

function closeSseTurnSubscription() {
  if (!state.sseTurnStream) {
    return;
  }
  state.sseTurnStream.close();
  state.sseTurnStream = null;
}

window.addEventListener("pagehide", () => {
  closeSseTurnSubscription();
});

function ensureDebugSubscription() {
  if (!state.turn) {
    return;
  }
  const key = `debug:${state.turn.turn_id}`;
  if (state.adpSubscriptions.has(key)) {
    return;
  }
  state.adpSubscriptions.add(key);
  if (!state.debug) {
      state.debug = {
        status_text: "调试流等待中",
        detail_lines: ["等待调试订阅"],
      };
      renderDebug();
  }
  adpSubscribe(adpSubscribeOf("SubscribeDebugState", { client: adpClientKind(), turn_id: state.turn.turn_id }), "sub-debug").catch((error) => {
    state.debug = {
      status_text: "调试流失败",
      detail_lines: [error.message],
    };
    renderDebug();
  });
}

async function submitUserInput(text, submitMetadata = null) {
  const submitPayload = { text };
  if (state.selectedSessionId) {
    submitPayload.session_id = state.selectedSessionId;
  }
  const cwd = normalizeCwd(state.selectedCwd);
  if (cwd) {
    submitPayload.cwd = cwd;
  }
  if (submitMetadata && Array.isArray(submitMetadata.attachments) && submitMetadata.attachments.length > 0) {
    submitPayload.metadata = submitMetadata;
  }
  const payload = await adpCommand(adpCommandOf("SubmitUserInput", submitPayload));
  setCommandStatus(commandReceiptStatus(payload));
  return payload;
}

function activeTurnId() {
  if (!turnCanBeCancelled(state.turn)) {
    return null;
  }
  return state.turn && state.turn.turn_id ? state.turn.turn_id : null;
}

function turnCanBeCancelled(turn) {
  return !!(
    turn &&
    turn.turn_id &&
    !turn.terminal_text &&
    !isTerminalStatus(turn.terminal_status) &&
    !isToolPendingStatus(turn.terminal_status)
  );
}

async function cancelActiveTurn() {
  const turnId = activeTurnId();
  if (!turnId && !state.submitInFlight && !state.pendingUserInput) {
    composerInput.value = "";
    state.pendingUserInput = null;
    state.pendingSubmitId = null;
    state.pendingSubmitSessionId = null;
    state.pendingSubmitError = null;
    state.pendingAttachments = [];
    state.acceptedSubmitReceipt = null;
    state.lifecycleClocks.clear();
    state.submitStartedAt = null;
    setCommandStatus("没有活跃轮次；输入已清空", { stickyMs: 3000 });
    renderMessages();
    return;
  }
  const command = turnId
    ? adpCommandOf("CancelTurn", { turn_id: turnId })
    : adpCommandOf("CancelLatestActiveTurn", {});
  setCommandStatus(`正在取消 ${turnId || "最新活跃轮次"}...`);
  if (turnId) {
    state.pendingCancelTurnId = turnId;
  }
  let payload;
  try {
    payload = await adpCommand(command);
  } catch (error) {
    if (turnId && state.pendingCancelTurnId === turnId) {
      state.pendingCancelTurnId = null;
    }
    setCommandStatus(`取消失败: ${error.message}`);
    return;
  }
  if (turnId) {
    publishLocalCancelledTurn(turnId);
  }
  state.pendingUserInput = null;
  state.pendingSubmitId = null;
  state.pendingSubmitSessionId = null;
  state.pendingSubmitError = null;
  state.pendingAttachments = [];
  state.acceptedSubmitReceipt = null;
  state.lifecycleClocks.clear();
  state.submitStartedAt = null;
  state.submitInFlight = false;
  composerInput.value = "";
  setCommandStatus(commandReceiptStatus(payload));
  refreshCheckpoints().catch((error) => {
    setCommandStatus(`${commandReceiptStatus(payload)} (checkpoint refresh failed: ${error.message})`);
  });
}

async function rewindCheckpoint(checkpointId) {
  setCommandStatus(`rewinding ${checkpointId}...`);
  let payload;
  try {
    payload = await adpCommand(adpCommandOf("RewindCheckpoint", { checkpoint_id: checkpointId }));
  } catch (error) {
    setCommandStatus(`rewind failed: ${error.message}`);
    return;
  }
  setCommandStatus(commandReceiptStatus(payload));
  await refreshCheckpoints();
}

async function runSlashCommand(rawText) {
  const command = rawText.trim();
  const firstLine = command.split(/\s+/, 1)[0] || "";
  const knownSlashCommands = new Set([
    "/help",
    "/new",
    "/task",
    "/设置",
    "/cwd",
    "/sessions",
    "/reload",
    "/success",
    "/failure",
    "/cancel",
    "/clear",
    "/附件",
    "/model",
  ]);
  if (command.startsWith("/") && !knownSlashCommands.has(firstLine)) {
    return false;
  }
  if (knownSlashCommands.has(firstLine)) {
    composerInput.value = "";
    state.pendingUserInput = null;
    state.pendingSubmitId = null;
    state.pendingSubmitSessionId = null;
    state.pendingSubmitError = null;
    state.pendingAttachments = [];
    state.acceptedSubmitReceipt = null;
    state.lifecycleClocks.clear();
    state.submitStartedAt = null;
  }
  switch (firstLine) {
    case "/help":
      setCommandStatus(shortcutHelp, { stickyMs: 10000 });
      return true;
    case "/new":
      openNewSessionDialog("conversation");
      return true;
    case "/task":
      openNewSessionDialog("task");
      return true;
    case "/设置":
      showInspectorPanel("settings");
      setMobileDrawer("settings");
      renderAll();
      setCommandStatus("设置已打开", { stickyMs: 4000 });
      return true;
    case "/cwd": {
      const cwd = requireTaskCwd("task cwd selection");
      if (cwd) {
        setCommandStatus(`已选择任务目标目录: ${cwd}`, { stickyMs: 5000 });
        renderAll();
      }
      return true;
    }
    case "/sessions":
      setCommandStatus("正在刷新会话...", { stickyMs: 3000 });
      await refreshSessions();
      await refreshSelectedSession();
      setCommandStatus("会话已刷新", { stickyMs: 5000 });
      return true;
    case "/reload":
      setCommandStatus("正在刷新服务状态...", { stickyMs: 3000 });
      await refreshAllProtocolState();
      setCommandStatus("服务状态已刷新", { stickyMs: 5000 });
      return true;
    case "/success":
      loadSamplePrompt("success");
      return true;
    case "/failure":
      loadSamplePrompt("failure");
      return true;
    case "/cancel":
      await cancelActiveTurn();
      return true;
    case "/clear":
      composerInput.value = "";
      state.pendingUserInput = null;
      state.pendingSubmitId = null;
      state.pendingSubmitSessionId = null;
      state.pendingSubmitError = null;
      state.pendingAttachments = [];
      state.acceptedSubmitReceipt = null;
      state.lifecycleClocks.clear();
      state.submitStartedAt = null;
      state.submitInFlight = false;
      setCommandStatus("本地输入框已清空", { stickyMs: 3000 });
      renderMessages();
      return true;
    case "/附件":
      state.attachmentsPreviewOpen = !state.attachmentsPreviewOpen;
      renderAttachmentTray();
      setCommandStatus(`附件 ${state.attachmentsPreviewOpen ? "预览已显示" : "预览已收起"}: ${attachmentSummary()}`, { stickyMs: 5000 });
      return true;
    case "/model":
      setCommandStatus("模型选择由运行时配置控制", { stickyMs: 6000 });
      return true;
    default:
      return false;
  }
}

function rememberInputHistory(text) {
  const value = `${text || ""}`.trim();
  if (!value) {
    return;
  }
  if (state.inputHistory[state.inputHistory.length - 1] !== value) {
    state.inputHistory.push(value);
  }
  if (state.inputHistory.length > 100) {
    state.inputHistory = state.inputHistory.slice(-100);
  }
  state.inputHistoryIndex = null;
}

function recallInputHistory(direction) {
  if (state.inputHistory.length === 0) {
    return false;
  }
  if (state.inputHistoryIndex === null) {
    if (direction > 0) {
      return false;
    }
    state.inputHistoryIndex = state.inputHistory.length - 1;
  } else {
    state.inputHistoryIndex += direction;
  }
  if (state.inputHistoryIndex < 0) {
    state.inputHistoryIndex = 0;
  }
  if (state.inputHistoryIndex >= state.inputHistory.length) {
    state.inputHistoryIndex = null;
    composerInput.value = "";
    return true;
  }
  composerInput.value = state.inputHistory[state.inputHistoryIndex];
  composerInput.setSelectionRange(composerInput.value.length, composerInput.value.length);
  return true;
}

composerForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const text = composerInput.value.trim();
  const attachments = currentAttachments();
  if (!text && attachments.length === 0) {
    setCommandStatus("已拒绝空输入", { stickyMs: 3000 });
    return;
  }
  if (attachments.some((attachment) => attachment.kind !== "image")) {
    setCommandStatus("当前版本只支持提交图片附件", { stickyMs: 6000 });
    return;
  }
  try {
    if (text && attachments.length === 0 && await runSlashCommand(text)) {
      return;
    }
  } catch (error) {
    setCommandStatus(`斜杠命令失败: ${error.message}`, { stickyMs: 8000 });
    return;
  }
  let submitMetadata = { attachments: [] };
  try {
    submitMetadata = {
      attachments: await attachmentsForSubmit(attachments),
    };
  } catch (error) {
    setCommandStatus(`图片派发前提交失败: ${error.message}`, { stickyMs: 8000 });
    return;
  }
  const commandText = text || "分析附件图片。";
  setCommandStatus("派发中...");
  if (!state.selectedSessionId) {
    const sessionId = newDraftSessionId();
    state.draftSessionId = sessionId;
    setSelectedSessionId(sessionId);
  }
  dispatchWebUiEdge("session.submit", { session_id: state.selectedSessionId });
  rememberInputHistory(text);
  state.pendingUserInput = commandText;
  state.pendingSubmitId = null;
  state.pendingSubmitSessionId = state.selectedSessionId;
  state.pendingSubmitError = null;
  state.pendingAttachments = attachments;
  state.acceptedSubmitReceipt = null;
  state.submitStartedAt = Date.now();
  state.ambiguousSubmitRecoveryStartedAt = state.submitStartedAt;
  stopAmbiguousSubmitRecoveryPolling();
  state.submitInFlight = true;
  composerInput.value = "";
  state.forceScrollToBottom = true;
  renderMessages();
  try {
    const receipt = await submitUserInput(commandText, submitMetadata);
    state.pendingSubmitId = receipt && receipt.ingress ? receipt.ingress.submit_id : null;
    clearCurrentAttachments();
    state.submitInFlight = false;
    state.submitStartedAt = null;
    state.pendingSubmitError = null;
    state.pendingAttachments = [];
    state.acceptedSubmitReceipt = null;
    try {
      await refreshAllProtocolState();
      renderCommandStatus();
    } catch (error) {
      setCommandStatus(`${commandReceiptStatus(receipt)} (refresh failed: ${error.message})`);
    }
  } catch (error) {
    const submittedAt = state.submitStartedAt || state.ambiguousSubmitRecoveryStartedAt || Date.now();
    state.submitInFlight = false;
    state.submitStartedAt = null;
    composerInput.value = "";
    state.forceScrollToBottom = true;
    const recovery = await refreshAfterAmbiguousSubmitFailure(error);
    if (recovery.materialized) {
      clearCurrentAttachments();
      state.pendingSubmitError = null;
      state.pendingAttachments = [];
      renderAll();
      setCommandStatus("服务刷新后请求已可见；请从当前会话状态继续", { stickyMs: 5000 });
      return;
    }
    state.pendingSubmitError = recovery.message;
    startAmbiguousSubmitRecoveryPolling(submittedAt);
    renderMessages();
    renderTurnMeta();
    setCommandStatus(`服务刷新后仍未验证提交回执；重复发送前正在检查服务真源。可用 ↑ 召回输入。附件草稿已保留：${recovery.message}`);
  }
});

cancelButton.addEventListener("click", () => {
  cancelActiveTurn().catch((error) => {
    setCommandStatus(`取消失败: ${error.message}`);
  });
});

function loadSamplePrompt(kind) {
  const prompt = samplePrompts[kind];
  if (!prompt) {
    return;
  }
  composerInput.value = prompt;
  composerInput.focus();
  setCommandStatus(`${kind} scenario loaded; press 发送 to run`, { stickyMs: 5000 });
}

function renderDebugDetailsToggle() {
  if (!debugDetailsToggle) {
    return;
  }
  debugDetailsToggle.classList.toggle("is-active", state.debugDetailsVisible);
  debugDetailsToggle.setAttribute("aria-pressed", state.debugDetailsVisible ? "true" : "false");
  debugDetailsToggle.textContent = state.debugDetailsVisible ? "调试开" : "调试关";
}

function openSettingsPanel() {
  dispatchWebUiEdge("root.open_settings");
  state.settingsPage = "root";
  showInspectorPanel("settings");
  setMobileDrawer("settings");
  renderAll();
}

newConversationButton.addEventListener("click", () => openNewSessionDialog("conversation"));
newTaskButton.addEventListener("click", () => {
  openNewSessionDialog("task");
});
sessionSelectAllButton.addEventListener("click", () => {
  selectAllSessions();
});
sessionClearSelectionButton.addEventListener("click", () => {
  clearSessionSelection();
});
sessionDeleteSelectedButton.addEventListener("click", () => {
  deleteSelectedSessions();
});
if (selectedSessionRenameButton) {
  selectedSessionRenameButton.addEventListener("click", () => {
    renameCurrentSession();
  });
}
if (debugDetailsToggle) {
  debugDetailsToggle.addEventListener("click", () => {
    state.debugDetailsVisible = !state.debugDetailsVisible;
    state.publicConversation = derivePublicConversation(state.turn);
    renderAll();
  });
}
if (openSessionDrawerButton) {
  openSessionDrawerButton.addEventListener("click", () => {
    openSessionSearchDashboard().catch((error) => {
      state.sessionSearchError = error.message;
      setCommandStatus(`会话搜索失败: ${error.message}`, { stickyMs: 9000 });
      renderSessionSearchDashboard();
    });
  });
}
if (mobileNewEntryButton) {
  mobileNewEntryButton.addEventListener("click", () => openNewSessionDialog("conversation"));
}
if (openTimerDashboardButton) {
  openTimerDashboardButton.addEventListener("click", () => {
    openTimerDashboard().catch((error) => {
      state.timerStatusError = error.message;
      setCommandStatus(`timer dashboard failed: ${error.message}`, { stickyMs: 9000 });
      renderTimerDashboard();
    });
  });
}
if (timerDashboardCloseButton) {
  timerDashboardCloseButton.addEventListener("click", () => {
    timerDashboardDialog?.close();
    dispatchWebUiEdge("root.open_home");
  });
}
if (timerDashboardRefreshButton) {
  timerDashboardRefreshButton.addEventListener("click", () => {
    refreshTimerDashboard();
  });
}
if (timerDashboardForm) {
  timerDashboardForm.addEventListener("submit", (event) => {
    event.preventDefault();
    scheduleTimerFromForm();
  });
}
if (openToolsDashboardButton) {
  openToolsDashboardButton.addEventListener("click", () => {
    openToolsDashboard().catch((error) => {
      state.toolRegistryError = error.message;
      setCommandStatus(`工具注册表面板打开失败: ${error.message}`, { stickyMs: 9000 });
      renderToolsDashboard();
    });
  });
}
if (toolsDashboardCloseButton) {
  toolsDashboardCloseButton.addEventListener("click", () => {
    toolsDashboardDialog?.close();
    dispatchWebUiEdge("root.open_home");
  });
}
if (toolsDashboardRefreshButton) {
  toolsDashboardRefreshButton.addEventListener("click", () => {
    refreshToolsDashboard();
  });
}
if (sessionSearchCloseButton) {
  sessionSearchCloseButton.addEventListener("click", () => {
    sessionSearchDialog?.close();
    dispatchWebUiEdge("root.open_home");
  });
}
if (sessionSearchForm) {
  sessionSearchForm.addEventListener("submit", (event) => {
    submitSessionSearch(event);
  });
}
if (closeSessionDrawerButton) {
  closeSessionDrawerButton.addEventListener("click", closeMobileDrawer);
}
if (openDetailDrawerButton) {
  openDetailDrawerButton.addEventListener("click", () => {
    setMobileAgentSheetOpen(!state.mobileAgentSheetOpen);
  });
}
if (openMobileAgentSheetButton) {
  openMobileAgentSheetButton.addEventListener("click", () => {
    setMobileAgentSheetOpen(!state.mobileAgentSheetOpen);
  });
}
if (closeMobileAgentSheetButton) {
  closeMobileAgentSheetButton.addEventListener("click", () => {
    setMobileAgentSheetOpen(false);
  });
}
if (workerSessionBackButton) {
  workerSessionBackButton.addEventListener("click", returnToParentSession);
}

if (sessionRelationToggleButton) {
  sessionRelationToggleButton.addEventListener("click", () => {
    state.sessionTreeOpen = !state.sessionTreeOpen;
    renderSessionRelationHeader();
  });
}
if (settingsAgentResourceDecrement) {
  settingsAgentResourceDecrement.addEventListener("click", () => adjustAgentResourceDraft(-1));
}
if (settingsAgentResourceIncrement) {
  settingsAgentResourceIncrement.addEventListener("click", () => adjustAgentResourceDraft(1));
}
if (settingsAgentResourceSave) {
  settingsAgentResourceSave.addEventListener("click", () => {
    submitAgentResourceConfigUpdate().catch((error) => {
      state.agentResourceSaveInFlight = false;
      state.agentResourceSaveError = error.message;
      renderSettingsShell();
    });
  });
}
if (openSettingsDrawerButton) {
  openSettingsDrawerButton.addEventListener("click", () => {
    if (state.mobileDrawer === "settings" && state.inspectorPanel === "settings") {
      closeMobileDrawer();
      return;
    }
    openSettingsPanel();
  });
}
if (settingsShellToggle) {
  settingsShellToggle.addEventListener("click", () => {
    const openingSettings = state.inspectorPanel !== "settings";
    if (openingSettings) {
      dispatchWebUiEdge("root.open_settings");
      state.settingsPage = "root";
    } else if (state.route === "settings") {
      dispatchWebUiEdge("root.open_home");
    }
    showInspectorPanel(openingSettings ? "settings" : "debug");
    renderAll();
  });
}
document.querySelectorAll("[data-settings-target]").forEach((control) => {
  control.addEventListener("click", () => {
    openSettingsPage(control.dataset.settingsTarget);
  });
});
if (settingsProviderForm) {
  settingsProviderForm.addEventListener("submit", (event) => {
    submitProviderConfigUpdate(event).catch((error) => {
      state.configSaveInFlight = false;
      setText("settings-provider-save-status", `保存失败: ${error.message}`);
      renderSettingsShell();
    });
  });
}
if (settingsProviderCurrentSelect) {
  settingsProviderCurrentSelect.addEventListener("change", () => {
    updateProviderSelectionDraftFromControls();
    syncProviderSelectionControls();
  });
}
if (settingsProviderFallbackSelect) {
  settingsProviderFallbackSelect.addEventListener("change", () => {
    updateProviderSelectionDraftFromControls();
    syncProviderSelectionControls();
  });
}
if (settingsProviderSwitchButton) {
  settingsProviderSwitchButton.addEventListener("click", () => {
    submitProviderSelectionUpdate().catch((error) => {
      state.providerSelectionInFlight = false;
      setText("settings-provider-switch-status", `切换失败: ${error.message}`);
      renderSettingsShell();
    });
  });
}
if (settingsProviderWebSearchTestButton) {
  settingsProviderWebSearchTestButton.addEventListener("click", () => {
    testProviderWebSearch().catch((error) => {
      state.providerWebSearchTestInFlight = "";
      setText("settings-provider-web-search-test-status", `模型服务联网搜索测试失败: ${error.message}`);
      renderSettingsShell();
    });
  });
}
if (settingsModelGroupForm) {
  settingsModelGroupForm.addEventListener("submit", (event) => {
    submitModelGroupConfigUpdate(event).catch((error) => {
      state.modelGroupSaveInFlight = false;
      setText("settings-model-group-save-status", `保存失败: ${error.message}`);
      renderSettingsShell();
    });
  });
}
if (settingsModelGroupCurrentSelect) {
  settingsModelGroupCurrentSelect.addEventListener("change", () => {
    updateModelGroupSelectionDraftFromControl();
    syncModelGroupSelectionControls();
  });
}
if (settingsModelGroupSwitchButton) {
  settingsModelGroupSwitchButton.addEventListener("click", () => {
    submitModelGroupSelectionUpdate().catch((error) => {
      state.modelGroupSelectionInFlight = false;
      setText("settings-model-group-switch-status", `切换失败: ${error.message}`);
      renderSettingsShell();
    });
  });
}
if (settingsApkUpdateCheckButton) {
  settingsApkUpdateCheckButton.addEventListener("click", () => {
    requestAndroidApkUpdateCheck();
  });
}
if (settingsAccountConfigPullButton) {
  settingsAccountConfigPullButton.addEventListener("click", () => {
    dispatchAccountConfigSync("PullAccountConfig");
  });
}
if (settingsAccountConfigPushButton) {
  settingsAccountConfigPushButton.addEventListener("click", () => {
    dispatchAccountConfigSync("PushAccountConfig");
  });
}
if (settingsDiagnosticsRefreshButton) {
  settingsDiagnosticsRefreshButton.addEventListener("click", () => {
    refreshDiagnosticsStatus().catch((error) => {
      state.diagnosticsInFlight = false;
      state.diagnosticsError = error.message;
      renderSettingsDiagnostics();
    });
  });
}
if (closeDetailDrawerButton) {
  closeDetailDrawerButton.addEventListener("click", closeMobileDrawer);
}
if (mobileDrawerScrim) {
  mobileDrawerScrim.addEventListener("click", closeMobileOverlays);
}
if (newSessionForm) {
  newSessionForm.addEventListener("change", (event) => {
    if (event.target && event.target.name === "new-session-kind") {
      syncNewSessionDialogMode();
    }
  });
  newSessionForm.addEventListener("submit", (event) => {
    event.preventDefault();
    submitNewSessionDialog().catch((error) => {
      setCommandStatus(`新建会话失败: ${error.message}`, { stickyMs: 8000 });
    });
  });
}
if (newSessionCancelButton) {
  newSessionCancelButton.addEventListener("click", closeNewSessionDialog);
}
if (newSessionCloseButton) {
  newSessionCloseButton.addEventListener("click", closeNewSessionDialog);
}
if (newSessionBrowseButton) {
  newSessionBrowseButton.addEventListener("click", () => {
    chooseNewTaskDirectory();
  });
}
if (newTaskPathPresets) {
  newTaskPathPresets.addEventListener("click", (event) => {
    const button = event.target && event.target.closest && event.target.closest(".path-preset-button");
    if (!button) {
      return;
    }
    const cwd = normalizeCwd(button.dataset.cwd);
    if (newSessionCwdInput) {
      newSessionCwdInput.value = cwd;
    }
    setCommandStatus(`已选择任务目标目录: ${cwd}`, { stickyMs: 5000 });
  });
}
composerInput.addEventListener("focus", () => {
  setComposerFocused(true);
});
composerInput.addEventListener("blur", () => {
  window.setTimeout(() => {
    const activeElement = document.activeElement;
    if (activeElement && activeElement.closest && activeElement.closest(".composer-card")) {
      return;
    }
    setComposerFocused(false);
  }, 120);
});
if (composerForm) {
  composerForm.addEventListener("focusin", () => {
    setComposerFocused(true);
  });
  composerForm.addEventListener("focusout", () => {
    window.setTimeout(() => {
      const activeElement = document.activeElement;
      if (activeElement && activeElement.closest && activeElement.closest(".composer-card")) {
        return;
      }
      setComposerFocused(false);
    }, 120);
  });
}
attachmentFileInput.addEventListener("change", (event) => {
  addAttachmentFiles(event.target.files, "file");
  event.target.value = "";
});
attachmentImageInput.addEventListener("change", (event) => {
  addAttachmentFiles(event.target.files, "image");
  event.target.value = "";
});
attachmentVideoInput.addEventListener("change", (event) => {
  addAttachmentFiles(event.target.files, "video");
  event.target.value = "";
});
function bindAndroidAttachmentBridge(button, kind) {
  const invoke = (event) => {
    const picker = window.FreehandAndroidFilePicker;
    if (!picker || typeof picker.request !== "function") {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    picker.request(kind);
  };
  button.addEventListener("click", invoke);
  button.addEventListener("pointerup", invoke);
  button.addEventListener("touchend", invoke, { passive: false });
}

bindAndroidAttachmentBridge(attachFileButton, "file");
bindAndroidAttachmentBridge(attachImageButton, "image");
bindAndroidAttachmentBridge(attachVideoButton, "video");
previewAttachmentsButton.addEventListener("click", () => {
  state.attachmentsPreviewOpen = !state.attachmentsPreviewOpen;
  renderAttachmentTray();
  setCommandStatus(`附件 ${state.attachmentsPreviewOpen ? "预览已显示" : "预览已收起"}: ${attachmentSummary()}`, { stickyMs: 5000 });
});
refreshSessionButton.addEventListener("click", () => {
  setCommandStatus("refreshing selected session...", { stickyMs: 3000 });
  refreshSelectedSession()
    .then(() => refreshPhase2Status())
    .then(() => setCommandStatus("selected session refreshed", { stickyMs: 4000 }))
    .catch((error) => {
      renderSessionRefreshFailure(error, state.selectedSessionId);
      if (selectedWorkerTranscriptRefreshRetryable()) {
        setCommandStatus("工作器记录未就绪；正在重试刷新选中会话", { stickyMs: 6000 });
      } else {
        setCommandStatus(`selected session refresh failed: ${error.message}`, { stickyMs: 8000 });
      }
    });
});
if (workerControlList) {
  workerControlList.addEventListener("click", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const button = target ? target.closest("[data-worker-control-op]") : null;
    if (!button) {
      return;
    }
    sendWorkerControl(button.dataset.workerControlOp).catch((error) => {
      setCommandStatus(`工作器控制失败: ${error.message}`, { stickyMs: 9000 });
    });
  });
}
modelSelector.addEventListener("change", () => {
  modelSelector.value = "运行时";
  setCommandStatus("模型选择器只读；活动模型由运行时配置拥有", { stickyMs: 6000 });
});
cwdInput.value = state.selectedCwd;
taskCwdInput.value = state.selectedCwd;
cwdInput.addEventListener("change", () => {
  setSelectedCwd(cwdInput.value);
  setCommandStatus(state.selectedCwd ? `已选择会话工作目录: ${state.selectedCwd}` : "会话工作目录已清空；将使用运行时默认值", { stickyMs: 5000 });
  renderAll();
});
taskCwdInput.addEventListener("change", () => {
  setSelectedCwd(taskCwdInput.value);
  setCommandStatus(state.selectedCwd ? `已选择任务目标目录: ${state.selectedCwd}` : "已清空任务目标目录", { stickyMs: 5000 });
  renderAll();
});

applyLayoutShape();
markWebUiJavascriptReady();
syncMobileDrawerForLayout();
installMobileSessionSwipeGesture();
installDesktopLayoutResizers();
updateComposerClearance();
window.addEventListener("resize", () => {
  applyLayoutShape();
  syncMobileDrawerForLayout();
  applySavedLayoutWidths();
  updateComposerClearance();
});
window.addEventListener("orientationchange", () => {
  applyLayoutShape();
  syncMobileDrawerForLayout();
  applySavedLayoutWidths();
  updateComposerClearance();
});
if (window.visualViewport) {
  window.visualViewport.addEventListener("resize", () => {
    applyLayoutShape();
    syncMobileDrawerForLayout();
    applySavedLayoutWidths();
    updateComposerClearance();
  });
  window.visualViewport.addEventListener("scroll", updateComposerClearance, { passive: true });
}
const streamStageForScrollLock = document.querySelector(".stream-stage");
if (streamStageForScrollLock) {
  streamStageForScrollLock.addEventListener("scroll", syncUserScrollLock, { passive: true });
}
window.addEventListener("scroll", syncUserScrollLock, { passive: true });
window.addEventListener("pageshow", () => {
  refreshProtocolStateAfterForeground("页面恢复");
});
window.addEventListener("focus", () => {
  refreshProtocolStateAfterForeground("应用聚焦");
});
window.addEventListener("online", () => {
  refreshProtocolStateAfterForeground("网络恢复");
});
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible") {
    refreshProtocolStateAfterForeground("应用恢复");
  }
});
if (window.ResizeObserver) {
  const composerResizeObserver = new ResizeObserver(updateComposerClearance);
  const composerCard = document.querySelector(".composer-card");
  if (composerCard) {
    composerResizeObserver.observe(composerCard);
  }
}

document.addEventListener("keydown", (event) => {
  if (document.activeElement === composerInput && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
    if (recallInputHistory(event.key === "ArrowUp" ? -1 : 1)) {
      event.preventDefault();
      return;
    }
  }
  if (event.key !== "Escape") {
    const usesModifier = event.metaKey || event.ctrlKey;
    if (usesModifier && event.key === "Enter") {
      event.preventDefault();
      composerForm.requestSubmit();
      return;
    }
    if (usesModifier && event.key.toLowerCase() === "r") {
      event.preventDefault();
      setCommandStatus("正在刷新服务状态...", { stickyMs: 3000 });
      refreshAllProtocolState()
        .then(() => {
          setCommandStatus("服务状态已刷新", { stickyMs: 5000 });
        })
        .catch((error) => {
          setCommandStatus(`refresh failed: ${error.message}`, { stickyMs: 8000 });
        });
      return;
    }
    if (usesModifier && event.key.toLowerCase() === "k") {
      event.preventDefault();
      composerInput.focus();
      setCommandStatus("输入框已聚焦", { stickyMs: 3000 });
      return;
    }
    if (usesModifier && event.key === "1") {
      event.preventDefault();
      loadSamplePrompt("success");
      return;
    }
    if (usesModifier && event.key === "2") {
      event.preventDefault();
      loadSamplePrompt("failure");
      return;
    }
    return;
  }
  event.preventDefault();
  if (closeVisibleNavigationSurface()) {
    return;
  }
  const hasLiveTurn = state.submitInFlight || (state.turn && turnIsCurrentLiveTurn(state.turn));
  if (hasLiveTurn) {
    state.rollbackArmedAt = 0;
    cancelActiveTurn().catch((error) => {
      setCommandStatus(`取消失败: ${error.message}`);
    });
    return;
  }
  if (composerInput.value.trim()) {
    state.rollbackArmedAt = 0;
    composerInput.value = "";
    setCommandStatus("输入框已清空", { stickyMs: 3000 });
    return;
  }
  const now = Date.now();
  if (state.rollbackArmedAt && now - state.rollbackArmedAt <= 900) {
    state.rollbackArmedAt = 0;
    rollbackLatestSessionTurn();
    return;
  }
  state.rollbackArmedAt = now;
  setCommandStatus("再次按 Esc 回滚最新会话轮次", { stickyMs: 1200 });
});

function installWebUiTestHooks() {
  if (!globalThis.__freehandEnableTestHooks) {
    return;
  }
  globalThis.__freehandWebUiTest = {
    projectHomeSharedStateForTest({ loaded, sessions }) {
      if (typeof loaded !== "boolean") {
        throw new Error("Home shared-state fixture loaded must be boolean");
      }
      if (!Array.isArray(sessions)) {
        throw new Error("Home shared-state fixture sessions must be an array");
      }
      state.sessions = sessions;
      state.sessionListLoaded = loaded;
      state.turn = null;
      state.sessionTurns = [];
      state.taskBoard = null;
      state.agentBoard = null;
      renderMobileHomeDashboard();
      return {
        activeState: mobileHomeActiveList?.dataset.sharedState || "",
        historyStates: Array.from(
          mobileHomeSessionList?.querySelectorAll("[data-shared-state]") || [],
          (element) => element.dataset.sharedState || "",
        ),
        historySessionIds: Array.from(
          mobileHomeSessionList?.querySelectorAll("[data-session-id]") || [],
          (element) => element.dataset.sessionId || "",
        ),
      };
    },
    resetAmbiguousSubmitState(sessionId, prompt) {
      state.sessions = [];
      state.sessionListLoaded = false;
      state.sessionTurns = [];
      state.turn = null;
      state.publicConversation = [];
      state.debug = null;
      state.taskBoard = null;
      state.agentBoard = null;
      state.eventInbox = null;
      state.taskHistory = null;
      state.workerControl = null;
      state.draftSessionId = sessionId;
      state.pendingUserInput = prompt;
      state.pendingSubmitId = null;
      state.pendingSubmitSessionId = sessionId;
      state.pendingSubmitError = null;
      state.pendingAttachments = [];
      state.acceptedSubmitReceipt = null;
      state.submitInFlight = false;
      state.submitStartedAt = Date.now();
      state.ambiguousSubmitRecoveryStartedAt = state.submitStartedAt;
      setSelectedSessionId(sessionId);
      renderAll();
    },
    setAdpQueryForTest(handler) {
      adpQuery = handler;
    },
    setAdpCommandForTest(handler) {
      adpCommandForTest = handler;
    },
    prepareAttachmentProofSession(sessionId) {
      const id = sessionId || "webui-image-attachment-proof-fixed";
      state.sessions = [{
        session_id: id,
        title: "图片附件验证",
        active_turn_id: null,
        archived: false,
      }];
      state.sessionListLoaded = true;
      state.sessionTurns = [];
      state.turn = null;
      state.publicConversation = [];
      state.draftSessionId = null;
      state.pendingUserInput = null;
      state.pendingSubmitId = null;
      state.pendingSubmitSessionId = null;
      state.pendingSubmitError = null;
      state.pendingAttachments = [];
      state.acceptedSubmitReceipt = null;
      state.submitInFlight = false;
      state.submitStartedAt = null;
      setSelectedSessionId(id);
      clearCurrentAttachments();
      renderAll();
      return this.captureAttachmentState();
    },
    addImageAttachmentForTest({ name, type, size, dataBase64 }) {
      addAndroidAttachmentDrafts("image", [{
        name: name || "test-image.png",
        type: type || "image/png",
        size: Number.isFinite(size) ? size : 0,
        data_base64: dataBase64 || "",
      }]);
      renderAttachmentTray();
      return this.captureAttachmentState();
    },
    captureAttachmentState() {
      const messageText = document.getElementById("message-list")?.innerText || "";
      const shell = document.querySelector('[data-webui-shell="true"]');
      return {
        selectedSession: shell?.dataset.selectedSession || "",
        selectedCwd: shell?.dataset.selectedCwd || "",
        composerValue: composerInput.value || "",
        turnStatus: document.getElementById("turn-status")?.textContent?.trim() || "",
        commandStatus: document.getElementById("command-status")?.textContent?.trim() || "",
        attachmentCount: currentAttachments().length,
        trayText: document.getElementById("attachment-tray")?.innerText || "",
        thumbCount: document.querySelectorAll(".attachment-thumb").length,
        removeCount: document.querySelectorAll(".attachment-remove").length,
        overlayCount: document.querySelectorAll(".attachment-preview-overlay").length,
        messageText,
        pendingAttachments: state.pendingAttachments.length,
        pendingUserInput: state.pendingUserInput,
        pendingSubmitSessionId: state.pendingSubmitSessionId,
        pendingSubmitError: state.pendingSubmitError,
      };
    },
    clickFirstAttachmentPreviewForTest() {
      document.querySelector(".attachment-thumb-button")?.click();
      return this.captureAttachmentState();
    },
    closeAttachmentPreviewForTest() {
      document.querySelector(".attachment-preview-close")?.click();
      return this.captureAttachmentState();
    },
    removeFirstAttachmentForTest() {
      document.querySelector(".attachment-remove")?.click();
      return this.captureAttachmentState();
    },
    async submitComposerForTest(text) {
      composerInput.value = text || "";
      composerForm.requestSubmit();
      await new Promise((resolve) => setTimeout(resolve, 0));
      return this.captureAttachmentState();
    },
    closeAdpSocketForTest() {
      if (state.adpSocket) {
        state.adpSocket.close();
      }
      state.adpSocket = null;
      state.adpOpened = null;
      clearAdpReconnectTimer();
      return this.captureAttachmentState();
    },
    simulateSessionRefreshFailureForTest(message, sessionId) {
      const targetSessionId = sessionId || state.selectedSessionId || "webui-session-refresh-failure-test";
      if (!state.selectedSessionId) {
        setSelectedSessionId(targetSessionId);
      }
      renderSessionRefreshFailure(new Error(message || "模拟会话刷新失败"), targetSessionId);
      return this.captureSessionRefreshExitState();
    },
    captureSessionRefreshExitState() {
      const shell = document.querySelector('[data-webui-shell="true"]');
      return {
        selectedSession: shell?.dataset.selectedSession || "",
        selectedTurn: shell?.dataset.selectedTurn || "",
        commandStatus: document.getElementById("command-status")?.textContent?.trim() || "",
        turnStatus: document.getElementById("turn-status")?.textContent?.trim() || "",
        messageText: document.getElementById("message-list")?.innerText || "",
        sessionRefreshError: state.sessionRefreshError,
        adpFailure: state.adpFailure,
        mobileDrawer: state.mobileDrawer,
        actionLabels: Array.from(document.querySelectorAll(".session-refresh-action-bar button"))
          .map((button) => button.textContent?.trim() || "")
          .filter(Boolean),
      };
    },
    applyTurnProjectionForTest(turn) {
      setTurnProjection(turn || null);
      renderAll();
      return this.captureAttachmentState();
    },
    clearAndroidNotificationMemoryForTest() {
      state.androidNotifiedTurns.clear();
      state.androidObservedNonTerminalTurns.clear();
      persistAndroidNotifiedTurns();
    },
    refreshAfterAmbiguousSubmitFailure(message) {
      return refreshAfterAmbiguousSubmitFailure(new Error(message || "模拟提交失败"));
    },
    receiveAndroidApkUpdateStatus(payload) {
      receiveAndroidApkUpdateStatus(payload || {});
      return {
        inFlight: state.androidApkUpdateInFlight,
        status: state.androidApkUpdateStatus,
      };
    },
    markPendingSubmitError(message) {
      state.pendingSubmitError = message;
      renderMessages();
      renderTurnMeta();
    },
    renderAll() {
      renderAll();
    },
    captureAmbiguousSubmitState() {
      const shell = document.querySelector('[data-webui-shell="true"]');
      const messageText = document.getElementById("message-list")?.innerText || "";
      return {
        selectedSession: shell?.dataset.selectedSession || "",
        turnStatus: document.getElementById("turn-status")?.textContent?.trim() || "",
        commandStatus: document.getElementById("command-status")?.textContent?.trim() || "",
        pendingSubmitCardCount: document.querySelectorAll('[data-turn-id="pending-submit"]').length,
        messageText,
        pendingUserInput: state.pendingUserInput,
        pendingSubmitError: state.pendingSubmitError,
        pendingSubmitAcceptedByTaskTruth: pendingSubmitAcceptedByTaskTruth(),
        acceptedSubmitReceipt: state.acceptedSubmitReceipt,
        sessionTurns: state.sessionTurns.length,
        foregroundRefreshInFlight: state.foregroundRefreshInFlight,
        foregroundRefreshLastAt: state.foregroundRefreshLastAt,
      };
    },
    refreshProtocolStateAfterForeground(reason) {
      refreshProtocolStateAfterForeground(reason || "测试恢复");
    },
  };
}

installWebUiTestHooks();

ensureAdpSocket()
  .then(async () => {
    ensureTurnSubscription();
    ensureSseTurnSubscription();
    await refreshAllProtocolState();
  })
  .catch((error) => {
    setCommandStatus(`启动连接失败: ${error.message}`);
    renderAll();
    scheduleAdpReconnect("startup failure");
  });
