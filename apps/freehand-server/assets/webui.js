import { initializeThemeToggle } from "/assets/theme.js?v=20260725-diagnostics-ui";

initializeThemeToggle(document);

export function classifyLayoutShape(width, height) {
  return classifyLayoutShapeForClient(width, height, "");
}

export function classifyLayoutShapeForClient(width, height, client) {
  const safeWidth = Math.max(1, Number(width) || 1);
  const safeHeight = Math.max(1, Number(height) || 1);
  const ratio = safeWidth / safeHeight;
  if (client === "android-webview") {
    if (ratio > 4 / 3) {
      return "phone_landscape";
    }
    if (safeWidth >= 600) {
      return "tablet_portrait";
    }
    if (ratio <= 9 / 16) {
      return "tall_phone";
    }
    return "phone_portrait";
  }
  if (safeWidth >= 1180 && ratio > 1.15) {
    return "desktop_large";
  }
  if (safeWidth >= 720 && ratio >= 0.85 && ratio <= 1.35) {
    return "foldable_unfolded";
  }
  if (safeWidth >= 880 && ratio > 1) {
    return "tablet_landscape";
  }
  if (safeWidth >= 600 && safeWidth <= 1023 && ratio <= 1) {
    return "tablet_portrait";
  }
  if (safeWidth < 880 && ratio > 4 / 3) {
    return "phone_landscape";
  }
  if (safeWidth < 720 && ratio <= 9 / 16) {
    return "tall_phone";
  }
  return "phone_portrait";
}

function viewportDimensionsForLayout() {
  const isAndroidWebView = layoutClient() === "android-webview";
  const widths = [
    window.visualViewport && window.visualViewport.width,
    document.documentElement && document.documentElement.clientWidth,
    window.innerWidth,
    isAndroidWebView && window.screen && window.screen.width,
  ].map(Number).filter((value) => Number.isFinite(value) && value > 0);
  const heights = [
    window.visualViewport && window.visualViewport.height,
    document.documentElement && document.documentElement.clientHeight,
    window.innerHeight,
    isAndroidWebView && window.screen && window.screen.height,
  ].map(Number).filter((value) => Number.isFinite(value) && value > 0);
  return {
    width: widths.length > 0 ? Math.min(...widths) : 1,
    height: heights.length > 0 ? Math.max(...heights) : 1,
  };
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
const mobileHomeSessionList = document.getElementById("mobile-home-session-list");
const mobileHomeTimerMarker = document.getElementById("mobile-home-timer-marker");
const mobileHomeTimerList = document.getElementById("mobile-home-timer-list");
const settingsReviewTree = document.getElementById("settings-review-tree");
const mobileAgentSummaryStrip = document.getElementById("mobile-agent-summary-strip");
const openMobileAgentSheetButton = document.getElementById("open-mobile-agent-sheet-button");
const closeMobileAgentSheetButton = document.getElementById("close-mobile-agent-sheet-button");
const mobileAgentSheet = document.getElementById("mobile-agent-sheet");
const mobileAgentTaskList = document.getElementById("mobile-agent-task-list");
const sessionRelationHeader = document.getElementById("session-relation-header");
const sessionRelationToggleButton = document.getElementById("session-relation-toggle-button");
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
const settingsModelGroupSaveButton = document.getElementById("settings-model-group-save-button");
const settingsApkUpdateSummary = document.getElementById("settings-apk-update-summary");
const settingsApkUpdateSource = document.getElementById("settings-apk-update-source");
const settingsApkUpdateStatus = document.getElementById("settings-apk-update-status");
const settingsApkUpdateCheckButton = document.getElementById("settings-apk-update-check-button");
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
const sessionRenameSelectedButton = document.getElementById("session-rename-selected-button");
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

const samplePrompts = {
  success:
    "Answer with one short sentence and a valid Freehand completion schema. Do not call tools.",
  failure:
    'Call the task tool exactly once with {"op":"query","task_id":"definitely-missing-freehand-task"}, then use the failed tool result to continue and report success through the required Freehand completion schema.',
};

const phaseOneSettingsTree = [
  {
    title: "LLM 提供商",
    items: [
      ["Active provider", "当前 provider / model / auth / web_search safe projection", "ok"],
      ["Provider registry", "已配置 provider 列表和加载到表单", "ok"],
      ["Add provider family", "OpenAI / Anthropic / Gemini / xAI / OpenRouter family review UI", "attention"],
      ["Provider detail", "API key / OAuth / Base URL / protocol / capability test", "ok"],
      ["Model group", "primary / sub / search / title / fallback / load balance owner-backed config", "ok"],
      ["Token 用量", "provider / session 用量投影，Phase 2 接 owner truth", "attention"],
    ],
  },
  {
    title: "外观",
    items: [["外观", "主题、字号、密度、手机显示策略", "attention"]],
  },
  {
    title: "Agent 运行时",
    items: [
      ["Skills", "Freehand skills、项目 skills、兼容导入审计", "attention"],
      ["记忆", "daemon runtime home 中的 session/turn/history", "attention"],
      ["MCP / 集成", "外部工具服务和账号连接", "attention"],
      ["环境变量", "daemon 注入变量，只显示安全投影", "attention"],
      ["运行时目录", "只读展示 runtime home 和状态目录", "attention"],
    ],
  },
  {
    title: "Daemon 与手机壳",
    items: [
      ["Daemon 连接", "本机、Tailscale、Relay、二维码导入", "attention"],
      ["Worker 能力", "数量上限、capability、状态一致性", "ok"],
      ["Android 更新与权限", "APK 更新、通知、文件访问授权", "ok"],
    ],
  },
  {
    title: "观测与关于",
    items: [
      ["日志", "导出 UI / daemon / provider 诊断包", "attention"],
      ["关于 Freehand", "版本、隐私、反馈", "attention"],
    ],
  },
];

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
const workerTranscriptRefreshRetryDelayMs = 3000;
const shortcutHelp =
  "Shortcuts: Cmd/Ctrl+Enter send · Esc cancel · Cmd/Ctrl+R refresh · Cmd/Ctrl+K focus · Cmd/Ctrl+1 success · Cmd/Ctrl+2 failure. Slash: /help /new /task /settings /cwd /sessions /reload /success /failure /cancel /clear /attachments /model";
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
  workerControlInFlight: false,
  configStatus: null,
  configStatusError: null,
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
  sessionTreeOpen: false,
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
  commandStatusMessage: "connecting to service...",
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
  setMobileDrawer(null);
}

function setMobileAgentSheetOpen(open) {
  const shape = document.body.dataset.layoutShape || applyLayoutShape();
  state.mobileAgentSheetOpen = !!open && isMobileDrawerLayout(shape);
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
  if (state.sessionTreeOpen) {
    state.sessionTreeOpen = false;
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
    setCommandStatus("input focus cleared", { stickyMs: 2000 });
    return true;
  }
  return closeVisibleNavigationSurface();
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
  const endpoint = shellConfig().adpEndpoint || "/adp";
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}${endpoint}`;
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
  setBackgroundCommandStatus(`connection closed; reconnecting after ${reason}...`);
  state.adpReconnectTimer = window.setTimeout(() => {
    state.adpReconnectTimer = null;
    refreshAllProtocolStateAfterReconnect(reason).catch((error) => {
      setCommandStatus(`service reconnect failed: ${error.message}`, { stickyMs: 5000 });
      scheduleAdpReconnect("retry failure");
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
  setBackgroundCommandStatus(`service truth refreshed after ${reason}`);
}

function ensureAdpSocket() {
  if (state.adpSocket && state.adpSocket.readyState === WebSocket.OPEN) {
    return Promise.resolve(state.adpSocket);
  }
  if (state.adpOpened) {
    return state.adpOpened;
  }

  const socket = new WebSocket(adpUrl());
  state.adpSocket = socket;
  state.adpStatus = "connecting";
  setCommandStatus("connecting to service...");

  state.adpOpened = new Promise((resolve, reject) => {
    socket.addEventListener("open", () => {
      state.adpStatus = "connected";
      state.adpFailure = null;
      state.adpReconnectAttempt = 0;
      clearAdpReconnectTimer();
      setCommandStatus("connected; waiting for updates...");
      renderAll();
      resolve(socket);
    });
    socket.addEventListener("message", (event) => {
      try {
        handleAdpFrame(JSON.parse(event.data));
      } catch (error) {
        state.adpFailure = `connection decode failed: ${error.message}`;
        setCommandStatus(state.adpFailure);
        renderAll();
      }
    });
    socket.addEventListener("error", () => {
      state.adpStatus = "error";
      setCommandStatus("connection error");
      renderAll();
      reject(new Error("connection error"));
    });
    socket.addEventListener("close", () => {
      state.adpStatus = "closed";
      setCommandStatus("connection closed");
      state.adpSocket = null;
      state.adpOpened = null;
      state.adpSubscriptions.clear();
      for (const { reject: rejectRequest } of state.adpRequests.values()) {
        rejectRequest(new Error("connection closed"));
      }
      for (const { timeoutId } of state.adpRequests.values()) {
        window.clearTimeout(timeoutId);
      }
      state.adpRequests.clear();
      renderAll();
      scheduleAdpReconnect("transport close");
    });
  });

  return state.adpOpened;
}

async function sendAdpFrame(frame) {
  const socket = await ensureAdpSocket();
  socket.send(JSON.stringify(frame));
}

function requestAdp(kind, payloadKey, payload, prefix) {
  const requestId = nextRequestId(prefix);
  const frame = { kind, request_id: requestId };
  frame[payloadKey] = payload;
  const promise = new Promise((resolve, reject) => {
    const timeoutId = window.setTimeout(() => {
      if (!state.adpRequests.has(requestId)) {
        return;
      }
      state.adpRequests.delete(requestId);
      reject(new Error(`request timed out after ${formatDuration(adpRequestTimeoutMs)}`));
    }, adpRequestTimeoutMs);
    state.adpRequests.set(requestId, { resolve, reject, kind, timeoutId });
  });
  sendAdpFrame(frame).catch((error) => {
    const request = state.adpRequests.get(requestId);
    state.adpRequests.delete(requestId);
    if (request) {
      window.clearTimeout(request.timeoutId);
      request.reject(error);
    }
  });
  return promise;
}

function adpQuery(query) {
  return requestAdp("query", "query", query, "query");
}

let adpCommandForTest = null;

function adpCommand(command) {
  if (adpCommandForTest) {
    return adpCommandForTest(command);
  }
  return requestAdp("command", "command", command, "cmd");
}

function adpSubscribe(subscription, prefix) {
  return requestAdp("subscribe", "subscription", subscription, prefix);
}

function setCommandStatus(message, options = {}) {
  state.commandStatusMessage = message;
  state.commandStatusStickyUntil = options.stickyMs ? Date.now() + options.stickyMs : 0;
  renderCommandStatus();
}

function providerConfigReceiptStatus(receipt) {
  if (receipt && receipt.dispatch_status === "provider_config_saved_restart_required") {
    return "Provider config saved. Restart required.";
  }
  throw new Error("Config save returned an unexpected service status.");
}

function providerConfigUpsertReceiptStatus(receipt) {
  if (receipt && receipt.dispatch_status === "provider_config_upserted_restart_required") {
    return "Provider definition saved. Restart required.";
  }
  throw new Error("Provider definition save returned an unexpected service status.");
}

function providerWebSearchTestReceiptStatus(receipt) {
  const status = receipt?.dispatch_status || "";
  if (status.startsWith("provider_web_search_test_passed:")) {
    return `Provider web_search test passed: ${status}`;
  }
  throw new Error("Provider web_search test returned an unexpected service status.");
}

function providerSelectionReceiptStatus(receipt) {
  if (receipt && receipt.dispatch_status === "agent_provider_selection_saved_restart_required") {
    return "Provider selection saved. Restart required.";
  }
  throw new Error("Provider selection save returned an unexpected service status.");
}

function modelGroupUpsertReceiptStatus(receipt) {
  if (receipt && receipt.dispatch_status === "model_group_config_upserted_restart_required") {
    return "Model group saved. Restart required.";
  }
  throw new Error("Model group save returned an unexpected service status.");
}

function modelGroupSelectionReceiptStatus(receipt) {
  if (receipt && receipt.dispatch_status === "model_group_selection_saved_restart_required") {
    return "Model group selection saved. Restart required.";
  }
  throw new Error("Model group selection save returned an unexpected service status.");
}

function agentResourceConfigReceiptStatus(receipt, expectedCount) {
  const expected = `agent_resource_config_saved_restart_required:count=${expectedCount}`;
  if (receipt && receipt.dispatch_status === expected) {
    return `Worker limit saved: ${expectedCount}. Restart required.`;
  }
  throw new Error("Agent resource save returned an unexpected service status.");
}

function timerScheduleReceiptStatus(receipt) {
  const status = receipt?.dispatch_status || "";
  if (status.startsWith("timer_scheduled:timer_id=")) {
    return `Timer scheduled: ${status}`;
  }
  throw new Error("Timer schedule returned an unexpected service status.");
}

function timerCancelReceiptStatus(receipt) {
  const status = receipt?.dispatch_status || "";
  if (status.startsWith("timer_cancelled:timer_id=")) {
    return `Timer cancelled: ${status}`;
  }
  throw new Error("Timer cancel returned an unexpected service status.");
}

function setBackgroundCommandStatus(message) {
  if (state.commandStatusStickyUntil > Date.now()) {
    return;
  }
  setCommandStatus(message);
}

function handleAdpFrame(frame) {
  const request = state.adpRequests.get(frame.request_id);
  switch (frame.kind) {
    case "query_result":
      state.adpFailure = null;
      if (request) {
        state.adpRequests.delete(frame.request_id);
        window.clearTimeout(request.timeoutId);
        request.resolve(frame.result);
      }
      return;
    case "command_receipt":
      state.adpFailure = null;
      if (request) {
        state.adpRequests.delete(frame.request_id);
        window.clearTimeout(request.timeoutId);
        request.resolve(frame.receipt);
      }
      setBackgroundCommandStatus(commandReceiptStatus(frame.receipt));
      return;
    case "subscription_accepted":
      state.adpFailure = null;
      if (request) {
        state.adpRequests.delete(frame.request_id);
        window.clearTimeout(request.timeoutId);
        request.resolve(frame.selector);
      }
      state.adpSubscriptions.add(frame.request_id);
      setBackgroundCommandStatus(`updates connected: ${frame.selector.stream_kind}`);
      return;
    case "subscription_event":
      state.adpFailure = null;
      applyAdpSubscriptionEvent(frame.event);
      return;
    case "failure":
      state.adpFailure = frame.failure.message || frame.failure.code;
      if (request) {
        state.adpRequests.delete(frame.request_id);
        window.clearTimeout(request.timeoutId);
        request.reject(new Error(frame.failure.message || frame.failure.code));
      }
      setCommandStatus(`request failed: ${frame.failure.code}`);
      return;
    default:
      setCommandStatus(`unsupported service message: ${frame.kind}`);
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
    return "loading";
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
    return "unknown size";
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
  setCommandStatus(`${next.length} attachment draft(s) in selected session`, { stickyMs: 4000 });
}

function addAndroidAttachmentDrafts(kind, files) {
  const next = [...currentAttachments()];
  Array.from(files || []).forEach((file) => {
    next.push({
      id: browserRandomId(),
      name: file.name || "attachment",
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
  setCommandStatus(`${next.length} attachment draft(s) in selected session`, { stickyMs: 4000 });
}

window.__freehandAndroidAttachmentSelected = (kind, files) => {
  addAndroidAttachmentDrafts(kind, files);
};

function removeAttachment(id) {
  const next = currentAttachments().filter((attachment) => attachment.id !== id);
  setCurrentAttachments(next);
  setCommandStatus("attachment removed", { stickyMs: 3000 });
}

function clearCurrentAttachments() {
  setCurrentAttachments([]);
}

function attachmentDisplayLines(attachments = currentAttachments(), options = {}) {
  if (attachments.length === 0) {
    return [];
  }
  const lines = [options.heading || "Attachments"];
  attachments.forEach((attachment) => {
    const mediaType = attachment.type || attachment.media_type || "unknown";
    const sizeBytes = Number.isFinite(attachment.size) ? attachment.size : attachment.size_bytes;
    const availability = attachment.available ? "ready" : options.defaultAvailability || "metadata-only";
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
    heading: "Submitted attachments",
    defaultAvailability: "metadata-only",
  });
  if (lines.length === 0) {
    return text;
  }
  return `${text}\n\n${lines.join("\n")}`;
}

function attachmentSummary(attachments = currentAttachments()) {
  if (attachments.length === 0) {
    return "no draft attachments";
  }
  const ready = attachments.filter((attachment) => attachment.available).length;
  return `${attachments.length} draft attachment(s), ${ready} ready in this page`;
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
      imageButton.title = "Preview selected image";
      const img = document.createElement("img");
      img.className = "attachment-thumb";
      img.alt = attachment.name || "selected image";
      img.src = attachment.previewUrl;
      imageButton.appendChild(img);
      imageButton.addEventListener("click", () => showAttachmentPreview(attachment));
      chip.appendChild(imageButton);
    }

    const text = document.createElement("span");
    text.className = "attachment-chip-text";
    text.textContent = `${attachment.kind} · ${attachment.name} · ${formatBytes(attachment.size)}`;
    text.title = attachment.available
      ? "This page still holds the File handle for retry."
      : "Metadata restored from session; reselect the file before sending binary payload.";

    const remove = document.createElement("button");
    remove.className = "attachment-remove";
    remove.type = "button";
    remove.textContent = "×";
    remove.setAttribute("aria-label", `Remove ${attachment.name}`);
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
    setCommandStatus("image preview is not available; reselect the image if it was restored from metadata", { stickyMs: 5000 });
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
  close.setAttribute("aria-label", "Close image preview");
  const img = document.createElement("img");
  img.className = "attachment-preview-image";
  img.src = attachment.previewUrl;
  img.alt = attachment.name || "selected image";
  const caption = document.createElement("div");
  caption.className = "attachment-preview-caption";
  caption.textContent = `${attachment.name || "image"} · ${attachment.type || "unknown"} · ${formatBytes(attachment.size)}`;
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
    reader.onerror = () => reject(reader.error || new Error("failed to read image"));
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
      throw new Error(`image ${attachment.name || attachment.id} is metadata-only; reselect it before sending`);
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
    return { phase: "neutral", className: "pending", label: "idle", isLive: false, elapsed: "" };
  }
  if (turn.terminal_text || isTerminalStatus(turn.terminal_status) || isToolPendingStatus(turn.terminal_status)) {
    const terminal = `${turn.terminal_status || "success"}`.toLowerCase();
    const phase = terminal === "success" ? "completed" : terminal;
    if (terminal === "running" || isToolPendingStatus(terminal)) {
      return {
        phase: "waiting_lifecycle",
        className: "running",
        label: "waiting lifecycle",
        isLive: false,
        elapsed: "",
      };
    }
    const label = terminalTurnStatusLabel(terminal);
    return {
      phase,
      className: label === "completed" ? "success" : "failed",
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
      label: elapsed ? `dispatching... ${elapsed}` : "dispatching...",
      isLive: true,
      elapsed,
    };
  }
  const inactiveToolLifecycle = inactiveToolLifecycleForRender(turn);
  if (inactiveToolLifecycle) {
    return inactiveToolLifecycle;
  }
  return { phase: "neutral", className: "pending", label: "waiting", isLive: false, neutral: true, elapsed: "" };
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
      label: "failed",
      isLive: false,
      elapsed: "",
    };
  }
  if (statuses.every((status) => status === "completed" || status === "success")) {
    return {
      phase: "tool_completed",
      className: "success",
      label: "completed",
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
      label: elapsed ? `dispatching · ${elapsed}` : "dispatching",
    },
    live: renderPending.isLive,
  });
  const body = article.querySelector(".execution-body");
  body.appendChild(executionRow({
    kind: "user",
    title: "User",
    body: [textWithAttachmentDisplay(renderPending.text, renderPending.attachments)],
    status: "submitted",
  }));
  body.appendChild(executionRow({
    kind: "system",
    title: "Client",
    body: ["Request accepted. Waiting for service dispatch."],
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
    title: "User",
    body: [textWithAttachmentDisplay(renderPending.text, renderPending.attachments)],
    status: "submitted",
  };
  const assistantRows = [{
    kind: "system",
    title: "Client",
    body: renderPending.error
      ? [
          "Submit receipt is being verified against service truth.",
          "Do not send a duplicate until the service refresh finishes.",
          retainedAttachmentCount > 0
            ? `Draft attachments retained for retry: ${retainedAttachmentCount}.`
            : "Draft attachments retained for retry: none.",
        ]
      : ["Request accepted. Waiting for service dispatch."],
    status: renderPending.error ? "checking service truth" : elapsed || "0s",
  }];
  const renderTurn = {
    turnId: "pending-submit",
    createdAt: renderPending.startedAt || Date.now(),
    lifecycle: {
      className: renderPending.error ? "running" : renderPending.isLive ? "running" : "pending",
      label: renderPending.error
        ? "checking service truth"
        : elapsed
          ? `dispatching... ${elapsed}`
          : "dispatching",
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
    title: "User",
    body: [text],
    status: "submitted",
  };
  const assistantRows = [{
    kind: "system",
    title: "Service",
    body: [
      "Service accepted this request through TaskBoard truth.",
      taskLine ? `Worker task: ${taskLine}` : "Worker lifecycle is visible in the Agent task list.",
    ],
    status: "accepted",
  }];
  const renderTurn = {
    turnId: "accepted-submit",
    createdAt: receipt.createdAt || receipt.created_at || Date.now(),
    lifecycle: {
      className: "running",
      label: "service accepted",
      isLive: true,
    },
  };
  return [userChatBubble(renderTurn, userRow), assistantChatBubble(renderTurn, assistantRows)];
}

function failureChatBubble(message) {
  return assistantChatBubble(
    {
      turnId: "adp-failure",
      lifecycle: { className: "failed", label: "failed", isLive: false },
    },
    [{
      kind: "error",
      title: "Connection",
      body: [message],
      status: "failed",
    }],
  );
}

function workerTranscriptWaitingBubble(error) {
  const detail = error || {};
  const body = [
    "Worker transcript is not persisted yet; TaskBoard still shows this Worker task as active.",
  ];
  const taskLine = [detail.task_id, detail.task_status, detail.assignee_agent_id]
    .filter(Boolean)
    .join(" · ");
  if (taskLine) {
    body.push(`TaskBoard: ${taskLine}`);
  }
  if (detail.session_id) {
    body.push(`Worker session: ${detail.session_id}`);
  }
  if (detail.message) {
    body.push(`Last refresh: ${compactSentence(detail.message, 180)}`);
  }
  body.push("Refreshing the same owner-projected Worker session; this is not a task dispatch failure.");
  return assistantChatBubble(
    {
      turnId: "worker-transcript-waiting",
      lifecycle: { className: "running", label: "worker transcript waiting", isLive: true },
    },
    [{
      kind: "system",
      title: "Worker transcript",
      body,
      status: "waiting",
    }],
  );
}

function loadingConversationBubble() {
  return assistantChatBubble(
    {
      turnId: "session-refresh-loading",
      lifecycle: { className: "running", label: "loading conversation", isLive: true },
    },
    [{
      kind: "system",
      title: "Conversation",
      body: ["Loading selected session transcript from runtime truth."],
      status: "loading",
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
    body.appendChild(executionRow({ kind: "system", title: "Turn", body: ["Waiting for projection."], status: lifecycle.label }));
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
      body: ["Waiting for projection."],
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
  label.textContent = "User";
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
  label.textContent = "Assistant";
  const status = document.createElement("span");
  status.className = `chat-state-pill ${className}`;
  status.textContent = chatAssistantStatusLabel(lifecycle, rows);
  meta.append(label, status);
  appendChatMessageTime(meta, renderTurn);
  article.appendChild(meta);

  rows.forEach((row) => {
    article.appendChild(chatAssistantSection(row));
  });
  appendTurnActionBar(article, renderTurn, rows);
  return article;
}

function appendTurnActionBar(article, renderTurn, rows) {
  const bar = document.createElement("div");
  bar.className = "turn-action-bar";
  bar.dataset.turnId = renderTurn.turnId || "";
  const copyButton = turnActionButton("Copy");
  copyButton.addEventListener("click", () => {
    copyTurnActionText(renderTurn, rows).catch((error) => {
      setCommandStatus(`copy failed: ${error.message}`, { stickyMs: 6000 });
    });
  });
  const editButton = turnActionButton("Edit from here");
  editButton.addEventListener("click", () => {
    editAndRerunFromTurn(renderTurn).catch((error) => {
      setCommandStatus(`edit from here failed: ${error.message}`, { stickyMs: 9000 });
    });
  });
  const newSessionButton = turnActionButton("New session");
  newSessionButton.addEventListener("click", () => {
    newSessionFromTurn(renderTurn, rows).catch((error) => {
      setCommandStatus(`new session from here failed: ${error.message}`, { stickyMs: 9000 });
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
    setCommandStatus("nothing to copy", { stickyMs: 4000 });
    return;
  }
  await copyTextToClipboard(text);
  setCommandStatus("copied turn text", { stickyMs: 3000 });
}

async function editAndRerunFromTurn(renderTurn) {
  if (!renderTurn || !renderTurn.turnId) {
    setCommandStatus("selected turn has no durable id", { stickyMs: 6000 });
    return;
  }
  if (!state.selectedSessionId || isDraftSessionId(state.selectedSessionId)) {
    setCommandStatus("edit from here requires a persisted selected session", { stickyMs: 6000 });
    return;
  }
  const userText = turnUserTextForAction(renderTurn.turnId);
  if (!userText) {
    setCommandStatus("selected turn has no editable user prompt", { stickyMs: 6000 });
    return;
  }
  await rollbackEffectiveTranscriptThroughTurn(renderTurn.turnId);
  composerInput.value = userText;
  composerInput.focus();
  setCommandStatus("rolled back to this turn; edit and send replacement", { stickyMs: 8000 });
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
    await adpCommand({ RollbackLatestSessionTurn: { session_id: state.selectedSessionId } });
    await refreshSessions();
    await refreshSelectedSession();
  }
  throw new Error("rollback guard reached before selected turn was removed");
}

async function startNewConversationFromText(text) {
  const draftText = `${text || ""}`.trim();
  if (!draftText) {
    setCommandStatus("selected turn has no text for a new session", { stickyMs: 6000 });
    return;
  }
  await startNewConversation();
  composerInput.value = draftText;
  composerInput.focus();
  setCommandStatus("new session ready; edit and send from copied turn", { stickyMs: 7000 });
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
    throw new Error("clipboard unavailable");
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
    return "tool failed";
  }
  if (lifecycle.isLive) {
    return lifecycle.label || "running";
  }
  if (rows.some((row) => row.kind === "final" && `${row.status || ""}`.toLowerCase() === "running")) {
    return "waiting lifecycle";
  }
  const finalRow = rows.find((row) => row.kind === "final");
  if (finalRow) {
    return finalRow.status || lifecycle.label || "completed";
  }
  return lifecycle.label || "received";
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
  return section;
}

function assistantSectionHeadingLabel(row) {
  if (!row) {
    return "";
  }
  if (row.kind === "final") {
    return `${row.status || ""}`.toLowerCase() === "running" ? "Lifecycle" : "Final";
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
  if (normalized.includes("等待") || normalized === "waiting") {
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
    return "unsupported command receipt: missing dispatch status";
  }
  switch (statusCode) {
    case "reason_live_turn_cancel_requested":
      return "cancel requested";
    case "reason_turn_cancelled":
      return "request cancelled";
    case "runtime_checkpoint_rewound":
      return "checkpoint restored";
    case "session_metadata_updated":
    case "session_turn_rolled_back":
      return "session updated";
    case "reason_turn_started":
      return "request accepted";
    case "reason_live_turn_completed":
      return "request completed";
    case "provider_config_saved_restart_required":
    case "provider_config_upserted_restart_required":
    case "agent_provider_selection_saved_restart_required":
    case "model_group_config_upserted_restart_required":
    case "model_group_selection_saved_restart_required":
    case "agent_resource_config_saved_restart_required":
      return "settings saved";
    case "node_direct_message_dispatched":
      return "worker message sent";
    case "worker_control_applied":
      return "worker control accepted";
    case "task_agent_created":
      return "worker updated";
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
      return "task updated";
    case "queued_by_static_dispatch_port":
      return "command queued";
    default:
      return `unsupported command receipt: ${truncateForChat(statusCode, 80)}`;
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
  const pendingStartedAt =
    state.submitStartedAt || state.ambiguousSubmitRecoveryStartedAt || Date.now();
  return {
    selectedSessionId: state.selectedSessionId,
    turns: turnsForRender.map((turn) => buildRenderTurn(turn)),
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
  return { message: error.message || "selected session refresh failed" };
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

function buildRenderTurn(turn) {
  const lifecycle = turnLifecycleForRender(turn);
  return {
    turnId: turn.turn_id || "",
    sessionId: turn.session_id || "",
    submitId: turn.submit_id || "",
    createdAt: turn.created_at || null,
    timing: turnTimingProjection(turn),
    sourceTurn: turn,
    orderKey: turnOrderKey(turn.turn_id),
    lifecycle,
    rows: buildRenderRows(turn, lifecycle),
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
  return modelRequestPhase(turn) === "schema_retry" ? "Schema" : "Model";
}

function modelRequestDisplayLines(turn) {
  const request = (turn && turn.model_request) || {};
  const lines = [];
  const mainDetail = request.detail || "Waiting for model response.";
  if (mainDetail) {
    lines.push(mainDetail);
  }
  const timingLine = turnTimingLine(turn, { includeLiveWait: true });
  if (timingLine) {
    lines.push(`timing: ${timingLine}`);
  }
  const transport = modelRequestTransport(turn);
  if (transport && transport.detail) {
    lines.push(`${modelRequestTransportLabel(transport)}: ${transport.detail}`);
  }
  return lines;
}

function modelRequestStaticStatus(turn) {
  const transportPhase = modelRequestTransportPhase(turn);
  if (transportPhase === "provider_retry") {
    return "transport retrying";
  }
  if (transportPhase === "provider_failover") {
    return "transport switching";
  }
  const timing = turnTimingProjection(turn);
  if (timing && Number.isFinite(timing.timeToFirstResponseMs)) {
    return `wait ${formatDuration(timing.timeToFirstResponseMs)}`;
  }
  return "waiting";
}

function buildObservableLiveTurnRenderRow(turn, lifecycle) {
  const status = lifecycle.elapsed ? `${lifecycle.label || "working"}... ${lifecycle.elapsed}` : lifecycle.label || "working";
  return {
    kind: "system",
    title: "Turn",
    body: ["request accepted; waiting for protocol-visible turn details"],
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
    identity: { turnId: turn.turn_id },
  };
}

function assistantRowStatus(turn, status) {
  if (isToolPendingStatus(turn.terminal_status)) {
    return "running";
  }
  if (turn.terminal_text || isTerminalStatus(turn.terminal_status)) {
    return terminalTurnStatusLabel(turn.terminal_status);
  }
  if (turnIsCurrentLiveTurn(turn)) {
    return status || "streaming";
  }
  return status === "streaming" ? "received" : status;
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
  return ["success", "failed", "blocked", "interrupted", "cancelled"].includes(normalized);
}

function isToolPendingStatus(status) {
  const normalized = `${status || ""}`.toLowerCase().replace(/[_-]/g, "");
  return normalized === "toolpending";
}

function terminalTurnStatusLabel(status) {
  const normalized = `${status || ""}`.toLowerCase().replace(/[_-]/g, "");
  if (normalized === "toolpending" || normalized === "running") {
    return "waiting lifecycle";
  }
  if (normalized === "failed") {
    return "failed";
  }
  if (normalized === "blocked") {
    return "blocked";
  }
  if (normalized === "interrupted") {
    return "interrupted";
  }
  if (normalized === "cancelled") {
    return "cancelled";
  }
  return "completed";
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
  const label = terminalTurnStatusLabel(nextTurn.terminal_status);
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
    return "schema polishing";
  }
  if (kind === "toolresultcontinuation" || kind === "tool_result_continuation") {
    return "thinking after tool result";
  }
  return "thinking";
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
    return "transport retry";
  }
  if (kind === "providerfailover" || kind === "provider_failover") {
    return "transport switch";
  }
  return "transport";
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
    parts.push(`wait ${formatDuration(waitMs)}`);
  }
  if (Number.isFinite(timing.timeToFirstResponseMs)) {
    parts.push(`first response ${formatDuration(timing.timeToFirstResponseMs)}`);
  }
  if (Number.isFinite(timing.totalElapsedMs)) {
    parts.push(`total ${formatDuration(timing.totalElapsedMs)}`);
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
  return elapsed ? `tool executing: ${names} · ${elapsed}` : `tool executing: ${names}`;
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
      title: "User",
      body: textWithSubmittedAttachmentDisplay(turn.user_text, turn.attachments || []),
      status: "submitted",
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
      title: "Assistant",
      body: assistantBodies.join("\n"),
      status: "streaming",
    });
  }
  (turn.tool_activities || []).forEach((tool) => {
    const status = `${tool.status || "waiting"}`.toLowerCase();
    items.push({
      kind: "ToolSummary",
      title: tool.display && tool.display.action ? tool.display.action : tool.tool_name || "Tool",
      body: tool.detail || status,
      status,
      tool_call_id: tool.tool_call_id,
      display: tool.display || null,
    });
  });
  if (turn.terminal_text) {
    const terminalStatus = `${turn.terminal_status || "Success"}`.toLowerCase();
    const isToolPending = isToolPendingStatus(terminalStatus);
    const status =
      isToolPending
        ? "running"
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
      title: isToolPending ? "Lifecycle" : "Final",
      body: terminalBodyForDisplay(turn.terminal_text),
      status,
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

function terminalBodyForDisplay(text) {
  const stripped = stripFreehandCompletionBlock(text);
  if (state.debugDetailsVisible) {
    return stripped;
  }
  const summary = terminalSummaryBlock(stripped);
  return summary || stripDebugTerminalLines(stripped);
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
  const nextSessionId = sessionId || null;
  if (state.selectedSessionId !== nextSessionId) {
    state.taskHistory = null;
    state.workerControl = null;
  }
  state.selectedSessionId = nextSessionId;
  if (state.selectedSessionId) {
    window.localStorage.setItem(selectedSessionStorageKey, state.selectedSessionId);
  } else {
    window.localStorage.removeItem(selectedSessionStorageKey);
  }
}

function clearConversationForSessionSwitch(sessionId) {
  clearSessionRefreshRetryTimer();
  setSelectedSessionId(sessionId);
  state.sessionTurns = [];
  state.turn = null;
  state.publicConversation = [];
  state.debug = null;
  state.adpFailure = null;
  state.sessionRefreshInFlight = sessionId;
  state.sessionRefreshError = null;
}

function switchConversationSession(sessionId) {
  if (!sessionId) {
    return;
  }
  const requestedSessionId = sessionId;
  state.sessionTreeOpen = false;
  clearConversationForSessionSwitch(sessionId);
  renderAll();
  refreshSelectedSession().catch((error) => {
    renderSessionRefreshFailure(error, requestedSessionId);
  });
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
  return !terminalTaskStatus(task.status);
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
  const message = `session refresh failed: ${error && error.message ? error.message : error}`;
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
    if (`${state.adpFailure || ""}`.startsWith("session refresh failed:")) {
      state.adpFailure = null;
    }
    setCommandStatus(
      `Worker transcript not ready · task ${retryContext.task_id || "unknown"} is ${retryContext.task_status || "active"}; retrying`,
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
  state.adpFailure = message;
  setCommandStatus(message, { stickyMs: 8000 });
  renderAll();
}

function clearSessionRefreshState(sessionId) {
  if (!sessionId || state.sessionRefreshInFlight === sessionId) {
    state.sessionRefreshInFlight = null;
  }
  if (!sessionId || (state.sessionRefreshError && state.sessionRefreshError.session_id === sessionId)) {
    state.sessionRefreshError = null;
    clearSessionRefreshRetryTimer();
    if (`${state.adpFailure || ""}`.startsWith("session refresh failed:")) {
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
  setCommandStatus(`${action} requires a task target directory`, { stickyMs: 6000 });
  (taskCwdInput || cwdInput || composerInput).focus();
  return "";
}

function selectedNewSessionKind() {
  const checked = newSessionForm
    ? newSessionForm.querySelector("input[name=\"new-session-kind\"]:checked")
    : null;
  return (checked && checked.value) || state.newSessionKind || "conversation";
}

function syncNewSessionDialogMode() {
  const kind = selectedNewSessionKind();
  state.newSessionKind = kind;
  if (newSessionDialog) {
    newSessionDialog.dataset.kind = kind;
  }
  if (newSessionConfirmButton) {
    newSessionConfirmButton.textContent = kind === "task" ? "Create task" : "Create session";
  }
}

function openNewSessionDialog(kind = "conversation") {
  state.newSessionKind = kind === "task" ? "task" : "conversation";
  if (!newSessionDialog || !newSessionForm) {
    if (state.newSessionKind === "task") {
      startNewTask().catch((error) => {
        setCommandStatus(`new task failed: ${error.message}`, { stickyMs: 8000 });
      });
    } else {
      startNewConversation().catch((error) => {
        setCommandStatus(`new conversation failed: ${error.message}`, { stickyMs: 8000 });
      });
    }
    return;
  }
  const radio = newSessionForm.querySelector(`input[name="new-session-kind"][value="${state.newSessionKind}"]`);
  if (radio) {
    radio.checked = true;
  }
  if (newSessionCwdInput) {
    newSessionCwdInput.value = selectedWorkspaceCwd();
  }
  syncNewSessionDialogMode();
  newSessionDialog.showModal();
  window.setTimeout(() => {
    if (state.newSessionKind === "task") {
      (newSessionBrowseButton || newSessionCwdInput || newSessionConfirmButton)?.focus();
    } else {
      newSessionConfirmButton?.focus();
    }
  }, 0);
}

function closeNewSessionDialog() {
  if (newSessionDialog && newSessionDialog.open) {
    newSessionDialog.close();
  }
}

async function chooseNewTaskDirectory() {
  const firstPreset = newTaskPathPresets?.querySelector(".path-preset-button");
  if (firstPreset) {
    firstPreset.focus();
    setCommandStatus("choose a directory preset or type a path", { stickyMs: 5000 });
    return;
  }
  newSessionCwdInput?.focus();
  setCommandStatus("type a task target directory", { stickyMs: 5000 });
}

async function submitNewSessionDialog() {
  const kind = selectedNewSessionKind();
  if (kind === "task") {
    const cwd = normalizeCwd(newSessionCwdInput && newSessionCwdInput.value);
    if (!cwd) {
      setCommandStatus("new task requires a target directory", { stickyMs: 6000 });
      newSessionCwdInput?.focus();
      return;
    }
    setSelectedCwd(cwd);
    closeNewSessionDialog();
    await startNewTask({ cwd });
    return;
  }
  closeNewSessionDialog();
  await startNewConversation();
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
  setCommandStatus("creating conversation session...", { stickyMs: 5000 });
  try {
    await adpCommand({
      CreateSession: {
        session_id: sessionId,
        title: "New conversation",
      },
    });
    state.draftSessionId = null;
    await refreshSessions();
    await refreshSelectedSession();
    closeMobileDrawer();
    setCommandStatus("new conversation ready", { stickyMs: 5000 });
  } catch (error) {
    setCommandStatus(`new conversation failed: ${error.message}`, { stickyMs: 8000 });
    throw error;
  }
}

async function startNewTask(options = {}) {
  const cwd = normalizeCwd(options.cwd) || requireTaskCwd("new task");
  if (!cwd) {
    return;
  }
  const sessionId = newDraftSessionId();
  resetLocalConversationState(sessionId);
  setSelectedCwd(cwd);
  setCommandStatus(`creating task session · cwd ${cwd}`, { stickyMs: 5000 });
  try {
    await adpCommand({
      CreateSession: {
        session_id: sessionId,
        title: `Task · ${cwd}`,
        cwd,
      },
    });
    await refreshSessions();
    await refreshSelectedSession();
    closeMobileDrawer();
    setCommandStatus(`new task ready · cwd ${cwd}`, { stickyMs: 5000 });
  } catch (error) {
    setCommandStatus(`new task failed: ${error.message}`, { stickyMs: 8000 });
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
  if (!state.selectedSessionId && state.sessions.length > 0) {
    const active = state.sessions.find((session) => session.active_turn_id);
    setSelectedSessionId((active || state.sessions[state.sessions.length - 1]).session_id);
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
}

function clearSessionSelection() {
  state.selectedSessionIds.clear();
  renderSessions();
}

function selectAllSessions() {
  state.selectedSessionIds.clear();
  state.sessions.forEach((session) => {
    if (session && session.session_id && !isDraftSessionId(session.session_id) && !internalRuntimeSessionId(session.session_id)) {
      state.selectedSessionIds.add(session.session_id);
    }
  });
  renderSessions();
}

async function deleteSelectedSessions() {
  const sessionIds = selectedManagedSessionIds();
  if (sessionIds.length === 0) {
    setCommandStatus("select sessions to remove", { stickyMs: 5000 });
    return;
  }
  setCommandStatus(`removing ${sessionIds.length} session(s)...`, { stickyMs: 8000 });
  try {
    for (const sessionId of sessionIds) {
      await adpCommand({ DeleteSession: { session_id: sessionId } });
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
    setCommandStatus(`removed ${sessionIds.length} session(s)`, { stickyMs: 6000 });
  } catch (error) {
    setCommandStatus(`remove session failed: ${error.message}`, { stickyMs: 9000 });
  }
}

async function renameSelectedSession() {
  const sessionIds = selectedManagedSessionIds();
  const sessionId = sessionIds.length === 1 ? sessionIds[0] : state.selectedSessionId;
  if (!sessionId || isDraftSessionId(sessionId)) {
    setCommandStatus("select one persisted session to rename", { stickyMs: 5000 });
    return;
  }
  const current = state.sessions.find((session) => session.session_id === sessionId);
  const nextTitle = window.prompt("Rename session", current?.title || current?.session_id || sessionId);
  if (nextTitle === null) {
    return;
  }
  const title = nextTitle.trim();
  if (!title) {
    setCommandStatus("rename requires a non-empty title", { stickyMs: 6000 });
    return;
  }
  setCommandStatus("renaming session...", { stickyMs: 5000 });
  try {
    await adpCommand({ RenameSession: { session_id: sessionId, title } });
    await refreshSessions();
    await refreshSelectedSession();
    setCommandStatus(`renamed session · ${title}`, { stickyMs: 5000 });
  } catch (error) {
    setCommandStatus(`rename failed: ${error.message}`, { stickyMs: 9000 });
  }
}

function latestRollbackUserText() {
  const turns = conversationTurnsForRender();
  const latest = turns[turns.length - 1];
  return `${(latest && latest.user_text) || ""}`.trim();
}

async function rollbackLatestSessionTurn() {
  if (!state.selectedSessionId || isDraftSessionId(state.selectedSessionId)) {
    setCommandStatus("rollback requires a persisted selected session", { stickyMs: 6000 });
    return;
  }
  const userText = latestRollbackUserText();
  setCommandStatus("rolling back latest session turn...", { stickyMs: 8000 });
  try {
    await adpCommand({ RollbackLatestSessionTurn: { session_id: state.selectedSessionId } });
    await refreshSessions();
    await refreshSelectedSession();
    if (userText) {
      composerInput.value = userText;
      composerInput.focus();
    }
    setCommandStatus("latest turn rolled back; edit and send replacement", { stickyMs: 7000 });
  } catch (error) {
    setCommandStatus(`rollback failed: ${error.message}`, { stickyMs: 9000 });
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
        result_summary: tool.result_summary || "cancelled by user",
        error: tool.error || "cancelled by user",
      };
    }),
    terminal_status: "Cancelled",
    terminal_text: source.terminal_text || "live turn cancelled",
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
      setCommandStatus(`debug query failed: ${error.message}`);
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
      status_text: "debug pending",
      detail_lines: ["waiting for debug snapshot"],
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
    setBackgroundCommandStatus("conversation updated");
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
    return "checking service truth · submit receipt not verified";
  }
  if (state.submitInFlight && !state.turn) {
    const elapsed = elapsedSince(state.submitStartedAt);
    return elapsed ? `dispatching... ${elapsed}` : "dispatching...";
  }
  const turn = activeTurnForSelectedSession();
  if (!turn) {
    return null;
  }

  if (turn.terminal_text || isTerminalStatus(turn.terminal_status) || isToolPendingStatus(turn.terminal_status)) {
    return terminalTurnStatusLabel(turn.terminal_status);
  }

  const waitingTools = (turn.tool_activities || []).filter(
    (tool) => tool.status === "Waiting" || tool.status === "waiting",
  );
  if (waitingTools.length > 0) {
    if (!protocolConnectionCanRenderLive()) {
      return "connection closed; refreshing service truth";
    }
    return waitingToolStatus(waitingTools);
  }

  if (turnIsWaitingForModelResponse(turn)) {
    if (!protocolConnectionCanRenderLive()) {
      return "connection closed; refreshing service truth";
    }
    const elapsed = elapsedSince(lifecycleClockStartedAt(modelRequestTimingKey(turn)));
    const label = modelRequestLabel(turn);
    return elapsed ? `${label}... ${elapsed}` : `${label}...`;
  }

  if (state.submitInFlight) {
    const elapsed = elapsedSince(state.submitStartedAt);
    return elapsed ? `dispatching... ${elapsed}` : "dispatching...";
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
        kind: "loading",
        turnId: waitingForWorkerTranscript ? "worker-transcript-waiting" : "session-refresh-loading",
        sessionId: state.selectedSessionId || "",
        lifecycle: {
          className: "running",
          label: waitingForWorkerTranscript ? "worker transcript waiting" : "loading conversation",
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
    title.textContent = "New conversation";
    const copy = document.createElement("div");
    copy.className = "chat-empty-copy";
    copy.textContent = "Send a message to start this session.";
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
    (!existingCreatedAt && !!nextCreatedAt)
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
    return [failureChatBubble(item.failure.message)];
  }
  return [];
}

function timelineItemCycleCard(item) {
  return cycleCardFromChatCards(cycleCardMetaForTimelineItem(item), timelineItemChatCards(item));
}

function cycleCardFromChatCards(meta, chatCards) {
  const kind = `${(meta && meta.kind) || "turn"}`.trim() || "turn";
  const lifecycle = (meta && meta.lifecycle) || { className: "pending", label: "waiting", isLive: false };
  const article = document.createElement("article");
  article.className = `turn-cycle-card ${lifecycle.className || "pending"}-state`;
  article.dataset.cycleKey = cycleCardKey(meta);
  article.dataset.cycleKind = kind;
  article.dataset.turnId = `${(meta && meta.turnId) || ""}`;
  article.dataset.sessionId = `${(meta && meta.sessionId) || ""}`;
  article.dataset.submitId = `${(meta && meta.submitId) || ""}`;
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
  article.setAttribute("aria-label", `request cycle ${kind} ${lifecycle.label || ""}`.trim());
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
    items.push({ label: "time", value: localChatTimeLabel(createdAtMs) });
  }
  const timingLine = turnTimingLine(meta.sourceTurn || null, { includeLiveWait: true });
  if (timingLine) {
    items.push({ label: "timing", value: timingLine });
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
    return { kind: "unknown", lifecycle: { className: "pending", label: "waiting", isLive: false } };
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
          ? "checking service truth"
          : elapsed
            ? `dispatching... ${elapsed}`
            : "dispatching",
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
      lifecycle: { className: "running", label: "service accepted", isLive: true },
      terminal: false,
    };
  }
  if (item.kind === "failure") {
    return {
      kind: "failure",
      turnId: "adp-failure",
      sessionId: state.selectedSessionId || "",
      lifecycle: { className: "failed", label: "failed", isLive: false },
      terminal: true,
    };
  }
  return { kind: item.kind || "unknown", lifecycle: { className: "pending", label: "waiting", isLive: false } };
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
  shell.style.setProperty("--composer-clearance", `${clearance}px`);
  document.documentElement.style.setProperty("--composer-clearance", `${clearance}px`);
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
    return "worker";
  }
  return normalizeCwd(session && session.cwd) ? "task" : "global";
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
      ? { session_id: parentSessionId, title: "Draft session", temporary: false }
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
  return `${counts.activeCount} active · ${counts.reviewCount} review · ${counts.blockedCount} blocked · ${counts.closedCount} closed · ${counts.staleCount} stale`;
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
  return isToolPendingStatus(turn.terminal_status) || turnIsWaitingForModelResponse(turn) || waitingTools;
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
  return Boolean(
    session &&
      (session.active_turn_id ||
        ["waiting_model", "waiting", "running", "toolpending", "tool_pending"].includes(status)),
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
  const label = turn && turnIsWaitingForModelResponse(turn)
    ? modelRequestLabel(turn)
    : statusLabel(status || "active");
  const detail = turn && turn.model_request && turn.model_request.detail
    ? turn.model_request.detail
    : (summary && summary.latest_summary) || "";
  return {
    sessionId: `${(summary && summary.session_id) || (turn && turn.session_id) || sessionId || ""}`,
    title: `${(summary && summary.title) || (summary && summary.session_id) || (turn && turn.session_id) || "session"}`,
    turnId,
    status: status || (turn && turnIsWaitingForModelResponse(turn) ? "waiting_model" : "active"),
    label,
    detail,
    tone: turn && modelRequestTransportPhase(turn).startsWith("provider")
      ? "phase2-running"
      : phase2StatusClass(status || "running"),
  };
}

function globalLiveSessionObservation() {
  if (state.selectedSessionId) {
    const selected = sessionLiveObservation(state.selectedSessionId);
    if (selected) {
      return { ...selected, scope: "selected" };
    }
    const selectedWorkerSession = workerChildSessionForSessionId(state.selectedSessionId);
    const parentSessionId = selectedWorkerSession && selectedWorkerSession.parent_session_id;
    const parent = parentSessionId ? sessionLiveObservation(parentSessionId) : null;
    return parent ? { ...parent, scope: "parent Master" } : null;
  }
  const activeSummary = (state.sessions || []).find(sessionHasObservableActiveStatus);
  const active = activeSummary ? sessionLiveObservation(activeSummary.session_id) : null;
  return active ? { ...active, scope: "active Master" } : null;
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
  selector.setAttribute("aria-label", `Select session ${session.session_id}`);
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
    ? `${session.latest_turn_id} · ${session.turn_count} turn(s)${cwdTail}`
    : `${session.turn_count} turn(s)${cwdTail}`;
  const observation = sessionLiveObservation(session.session_id);
  appendSessionParts(
    button,
    observation ? `active · ${observation.label}` : `${sessionKindLabel(session)} · ${session.latest_status || "session"}`,
    session.title || session.session_id,
    observation
      ? `${observation.turnId || session.active_turn_id || session.latest_turn_id} · ${observation.status} · ${session.turn_count} turn(s)${cwdTail}`
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
  name.textContent = "Master";
  main.append(chevron, name);

  const count = document.createElement("span");
  count.className = "session-agent-count";
  count.textContent = `${sessions.length} session(s)`;
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

function renderSessionBulkToolbar() {
  if (!sessionBulkCount || !sessionDeleteSelectedButton) {
    return;
  }
  const selectedCount = selectedManagedSessionIds().length;
  const selectableCount = state.sessions.filter((session) => !isDraftSessionId(session.session_id)).length;
  sessionBulkCount.textContent = `${selectedCount} selected`;
  sessionDeleteSelectedButton.disabled = selectedCount === 0;
  if (sessionRenameSelectedButton) {
    sessionRenameSelectedButton.disabled = selectedCount > 1;
  }
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
    appendSessionParts(empty, "empty", "no sessions", "waiting for first turn");
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
  const selected = selectedParentSessionSummary() || sessionSummaryForSelected() || state.sessions[state.sessions.length - 1] || null;
  const tasks = currentSessionTasks();
  const counts = currentSessionTaskCounts(tasks);
  const liveObservation = globalLiveSessionObservation();
  const title = selected
    ? selected.title || selected.session_id
    : state.draftSessionId || "No session selected";
  const copy = liveObservation
    ? liveObservationLine(liveObservation)
    : selected
      ? compactSentence(selected.latest_summary || selected.latest_status || "Persisted session selected", 132)
      : "Select a persisted session or create a new conversation.";
  const timerStats = timerDashboardStats();
  const timerCopy = state.timerStatusError
    ? `Timer query failed: ${compactSentence(state.timerStatusError, 96)}`
    : state.timerList
      ? timerDashboardSummary(timerStats)
      : "waiting for owner-backed timer projection";
  setText("mobile-home-current-title", compactSentence(title, 80));
  setText("mobile-home-current-copy", copy);
  setText(
    "mobile-home-current-metrics",
    `${counts.activeCount} running · ${counts.reviewCount} review · ${counts.blockedCount} blocked · ${counts.closedCount} closed`,
  );
  setText("mobile-home-timer-title", state.timerList ? "Timer owner truth" : "Timer loading");
  setText("mobile-home-timer-copy", timerCopy);
  if (mobileHomeTimerMarker) {
    mobileHomeTimerMarker.classList.toggle("ok", !state.timerStatusError && timerStats.activeCount > 0);
    mobileHomeTimerMarker.classList.toggle("attention", !!state.timerStatusError || timerStats.activeCount === 0);
  }
  renderMobileHomeTimerList();
  setText("mobile-home-session-count", `${state.sessions.length} persisted session(s)`);
  renderMobileHomeSessionList();
}

function timerDashboardStats() {
  const timers = (state.timerList && state.timerList.timers) || [];
  return {
    totalCount: timers.length,
    activeCount: timers.filter((timer) => ["active", "running"].includes(timer.status)).length,
    terminalCount: timers.filter((timer) => ["completed", "cancelled"].includes(timer.status)).length,
    next: timers
      .filter((timer) => ["active", "running"].includes(timer.status))
      .slice()
      .sort((left, right) => Number(left.next_due_at || 0) - Number(right.next_due_at || 0))[0] || null,
  };
}

function timerDashboardSummary(stats = timerDashboardStats()) {
  if (!state.timerList) {
    return "waiting for timer truth";
  }
  const next = stats.next ? ` · next ${formatUnixTime(stats.next.next_due_at)}` : "";
  return `${stats.activeCount} active · ${stats.terminalCount} terminal${next}`;
}

function renderMobileHomeTimerList() {
  if (!mobileHomeTimerList) {
    return;
  }
  mobileHomeTimerList.replaceChildren();
  if (state.timerStatusError) {
    mobileHomeTimerList.textContent = compactSentence(state.timerStatusError, 96);
    return;
  }
  const timers = ((state.timerList && state.timerList.timers) || [])
    .slice()
    .sort((left, right) => Number(left.next_due_at || 0) - Number(right.next_due_at || 0))
    .slice(0, 3);
  if (timers.length === 0) {
    mobileHomeTimerList.textContent = state.timerList ? "No timers." : "waiting for timer truth";
    return;
  }
  timers.forEach((timer) => {
    const item = document.createElement("button");
    item.className = "mobile-home-session-item";
    item.type = "button";
    item.dataset.timerId = timer.timer_id || "";
    const marker = document.createElement("span");
    marker.className = `settings-status-marker ${["active", "running"].includes(timer.status) ? "ok" : "attention"}`;
    marker.setAttribute("aria-hidden", "true");
    const copy = document.createElement("span");
    copy.className = "mobile-home-session-copy";
    const title = document.createElement("strong");
    title.textContent = compactSentence(timer.reason || timer.timer_id, 72);
    const meta = document.createElement("small");
    meta.textContent = compactSentence(`${timer.status} · ${formatUnixTime(timer.next_due_at)}`, 72);
    copy.append(title, meta);
    item.append(marker, copy);
    item.addEventListener("click", () => openTimerDashboard());
    mobileHomeTimerList.appendChild(item);
  });
}

function renderMobileHomeSessionList() {
  if (!mobileHomeSessionList) {
    return;
  }
  mobileHomeSessionList.replaceChildren();
  const sessions = state.sessions.slice(-3).reverse();
  if (sessions.length === 0) {
    mobileHomeSessionList.textContent = state.sessionListLoaded ? "No persisted sessions." : "waiting for session truth";
    return;
  }
  sessions.forEach((session) => {
    const item = document.createElement("button");
    item.className = "mobile-home-session-item";
    item.type = "button";
    item.dataset.sessionId = session.session_id || "";
    const marker = document.createElement("span");
    marker.className = `settings-status-marker ${sessionHasObservableActiveStatus(session) ? "ok" : ""}`;
    marker.setAttribute("aria-hidden", "true");
    const copy = document.createElement("span");
    copy.className = "mobile-home-session-copy";
    const title = document.createElement("strong");
    title.textContent = compactSentence(session.title || session.session_id, 72);
    const meta = document.createElement("small");
    meta.textContent = compactSentence(`${sessionKindLabel(session)} · ${session.latest_status || "session"}`, 72);
    copy.append(title, meta);
    item.append(marker, copy);
    item.addEventListener("click", () => switchConversationSession(session.session_id));
    mobileHomeSessionList.appendChild(item);
  });
}

function renderSettingsReviewTree() {
  if (!settingsReviewTree) {
    return;
  }
  const sections = phaseOneSettingsTree.map((section) => {
    const block = document.createElement("section");
    block.className = "settings-review-section";
    const title = document.createElement("h3");
    title.textContent = section.title;
    const list = document.createElement("div");
    list.className = "settings-review-list";
    section.items.forEach(([name, detail, tone]) => {
      const row = document.createElement("article");
      row.className = "settings-review-row";
      const marker = document.createElement("span");
      marker.className = `settings-status-marker ${tone === "ok" ? "ok" : "attention"}`;
      marker.setAttribute("aria-hidden", "true");
      const copy = document.createElement("span");
      const label = document.createElement("strong");
      label.textContent = name;
      const note = document.createElement("small");
      note.textContent = detail;
      copy.append(label, note);
      row.append(marker, copy);
      list.append(row);
    });
    block.append(title, list);
    return block;
  });
  const note = document.createElement("p");
  note.className = "settings-review-note";
  note.textContent = "Phase 1 audit tree: unconnected rows are UI-only placeholders until owner-backed Phase 2 wiring lands. Android remains a daemon-hosted WebUI shell; phone-local filesystem management is not part of this surface.";
  settingsReviewTree.replaceChildren(note, ...sections);
}

function renderSettingsDiagnostics() {
  const files = Array.isArray(state.diagnostics?.files) ? state.diagnostics.files : [];
  setText(
    "settings-diagnostics-summary",
    state.diagnosticsError
      ? "query failed"
      : state.diagnostics
        ? `${files.length} log file(s)`
        : "loading",
  );
  setText("settings-diagnostics-runtime-home", state.diagnostics?.runtime_home || "loading");
  if (settingsDiagnosticsStatus) {
    settingsDiagnosticsStatus.textContent = state.diagnosticsError
      ? `Diagnostics query failed: ${state.diagnosticsError}`
      : state.diagnostics
        ? `Redacted log metadata from ${state.diagnostics.logs_dir || "logs"} · generated ${formatUnixTime(state.diagnostics.generated_at)}`
        : "Diagnostics show service-owned log metadata and redacted tail lines.";
  }
  if (settingsDiagnosticsRefreshButton) {
    settingsDiagnosticsRefreshButton.disabled = state.diagnosticsInFlight;
    settingsDiagnosticsRefreshButton.textContent = state.diagnosticsInFlight
      ? "Refreshing diagnostics..."
      : "Refresh diagnostics";
  }
  if (!settingsDiagnosticsList) {
    return;
  }
  settingsDiagnosticsList.replaceChildren();
  if (state.diagnosticsError) {
    settingsDiagnosticsList.textContent = state.diagnosticsError;
    return;
  }
  if (files.length === 0) {
    settingsDiagnosticsList.textContent = state.diagnostics ? "No log files projected." : "waiting for diagnostics projection";
    return;
  }
  files.slice(0, 8).forEach((file) => {
    settingsDiagnosticsList.appendChild(renderDiagnosticLogRow(file));
  });
}

function renderDiagnosticLogRow(file) {
  const row = document.createElement("article");
  row.className = "settings-review-row diagnostic-log-row";
  row.dataset.logName = file.name || "";
  row.dataset.relativePath = file.relative_path || "";
  const marker = document.createElement("span");
  marker.className = "settings-status-marker ok";
  marker.setAttribute("aria-hidden", "true");
  const copy = document.createElement("span");
  const label = document.createElement("strong");
  label.textContent = file.name || file.relative_path || "log file";
  const meta = document.createElement("small");
  meta.textContent = compactSentence(
    `${file.relative_path || "logs"} · ${Number(file.size_bytes || 0)} bytes · ${formatUnixTime(file.modified_at)}`,
    150,
  );
  const tail = document.createElement("small");
  tail.textContent = compactSentence((file.tail_lines || []).join(" / ") || "no tail lines", 180);
  copy.append(label, meta, tail);
  row.append(marker, copy);
  return row;
}

function renderDraftSessionItem() {
  const item = document.createElement("button");
  item.className = `session-item session-button${state.draftSessionId === state.selectedSessionId ? " active" : ""}`;
  item.type = "button";
  item.dataset.sessionId = state.draftSessionId;
  item.dataset.sessionKind = state.selectedCwd ? "task" : "global";
  appendSessionParts(item, "draft", state.draftSessionId, state.selectedCwd ? `cwd ${state.selectedCwd}` : "first send creates session");
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
    setText("debug-status", "waiting");
    setText("debug-lines", "-");
    return;
  }
  setText("debug-status", state.debug.status_text);
  setText("debug-lines", state.debug.detail_lines.join(" · "));
}

function renderCheckpoints() {
  setText("checkpoint-status", `${state.checkpoints.length} checkpoint(s)`);
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
  const agents = currentSessionAgents();
  const counts = currentSessionTaskCounts(tasks);
  const liveObservation = globalLiveSessionObservation();
  const selectedTurns = logicalSessionTurns(state.sessionTurns || []).filter((turn) =>
    !state.selectedSessionId || turn.session_id === state.selectedSessionId
  );
  const latestSelectedTurn = selectedTurns[selectedTurns.length - 1] || activeTurnForSelectedSession();
  const terminalStatus = `${latestSelectedTurn?.terminal_status || ""}`.toLowerCase();
  let tone = "unavailable";
  if (liveObservation) {
    tone = "active";
  } else if (taskBoard) {
    if (tasks.some((task) => ["blocked", "failed", "cancelled"].includes(`${task.status || ""}`.toLowerCase()))) {
      tone = "blocked";
    } else if (tasks.some((task) => ["review_ready", "review_submitted"].includes(`${task.status || ""}`.toLowerCase()))) {
      tone = "evaluating";
    } else if (tasks.some((task) => !terminalTaskStatus(task.status))) {
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
        ? `status unavailable: ${state.phase2StatusError}`
        : "waiting",
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
  mobileAgentSummaryStrip.dataset.tone = model.tone;
  const runningAgents = model.agents.filter((agent) => agentIsActive(agent));
  const counts = model.counts || currentSessionTaskCounts(model.tasks);
  const lifecycleSummary = mobileAgentLifecycleSummary(counts);
  const workerLimit = Number(state.configStatus?.agent_resource_count);
  const resourceSummary = Number.isFinite(workerLimit) && workerLimit > 0
    ? ` · limit ${workerLimit}`
    : "";
  const liveObservation = model.liveObservation || null;
  setText(
    "mobile-agent-summary-title",
    liveObservation
      ? `${liveObservation.label} · ${liveObservation.turnId || "active turn"}`
      : `${runningAgents.length} running · ${lifecycleSummary}${resourceSummary}`,
  );
  const activeTask =
    model.tasks.find((task) => taskLifecycleBucket(task.status) === "active") ||
    model.tasks.find((task) => taskLifecycleBucket(task.status) === "review") ||
    model.tasks.find((task) => taskLifecycleBucket(task.status) === "blocked") ||
    model.tasks[0];
  setText(
    "mobile-agent-summary-copy",
    liveObservation
      ? compactSentence(`${liveObservation.scope}: ${liveObservation.title} · ${liveObservation.sessionId}`, 96)
      : activeTask
      ? compactSentence(`${statusLabel(activeTask.status)}: ${taskTitle(activeTask)}`, 72)
      : "No Worker task in this session",
  );
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
      ? `${liveObservation.label} · ${liveObservation.turnId || "active turn"} · ${liveObservation.sessionId}`
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
  const providerMode = status?.agent_resource_provider_mode || "unavailable";
  const providerId = status?.agent_resource_provider_id || "unavailable";
  setText("settings-agent-resource-count", `${workerLimit}`);
  setText("settings-agent-resource-limit", `${systemMax}`);
  setText("settings-agent-resource-summary", status ? `limit ${workerLimit} · max ${systemMax}` : "loading");
  setText(
    "settings-agent-resource-provider",
    providerMode === "shared" ? `shared · ${providerId}` : providerMode.replaceAll("_", " "),
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
    settingsAgentResourceSave.textContent = state.agentResourceSaveInFlight ? "Saving..." : "Save Worker limit";
  }
  const statusText = state.agentResourceSaveError
    ? `Save failed: ${state.agentResourceSaveError}`
    : state.agentResourceSaveMessage
      ? state.agentResourceSaveMessage
      : !status
        ? "Waiting for config truth."
        : !isMaster
          ? "Worker limit is configurable only from the active Master."
          : "Worker limit 1-5 · restart and Worker process startup required.";
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
    state.agentResourceSaveError = "active Master config is unavailable";
    renderSettingsShell();
    return;
  }
  const resourceCount = Number(state.agentResourceDraftCount ?? status.agent_resource_count);
  state.agentResourceSaveInFlight = true;
  state.agentResourceSaveMessage = null;
  state.agentResourceSaveError = null;
  renderSettingsShell();
  try {
    const receipt = await adpCommand({
      UpdateAgentResourceConfig: {
        update: {
          agent_name: status.agent_name,
          resource_count: resourceCount,
        },
      },
    });
    setCommandStatus(agentResourceConfigReceiptStatus(receipt, resourceCount), { stickyMs: 5000 });
    await refreshConfigStatus();
    state.agentResourceSaveMessage = "Saved. Restart and start Worker processes up to this limit.";
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
  if (model.tasks.length === 0) {
    mobileAgentTaskList.appendChild(mobileAgentEmptyCard("No Worker tasks in the current projection."));
    return;
  }
  const total = model.tasks.length;
  model.tasks.forEach((task, index) => {
    const card = mobileAgentCard({
      title: taskTitle(task),
      meta: [`${index + 1}/${total}`, statusLabel(task.status), assigneeLabel(task.assignee_agent_id), freshnessLabel(task.last_progress_at || task.updated_at)]
        .filter(Boolean)
        .join(" · "),
      copy: compactSentence(task.goal || "Task goal unavailable", 132),
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
    pieces.push(`${counts.activeCount} running task${counts.activeCount === 1 ? "" : "s"}`);
  }
  if (counts.reviewCount > 0) {
    pieces.push(`${counts.reviewCount} review task${counts.reviewCount === 1 ? "" : "s"}`);
  }
  if (counts.blockedCount > 0) {
    pieces.push(`${counts.blockedCount} blocked task${counts.blockedCount === 1 ? "" : "s"}`);
  }
  if (pieces.length === 0 && counts.closedCount > 0) {
    pieces.push(`${counts.closedCount} closed task${counts.closedCount === 1 ? "" : "s"}`);
  }
  return pieces.length > 0 ? pieces.join(" · ") : "0 tasks";
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
  metaNode.textContent = meta || "status unavailable";
  const copyNode = document.createElement("div");
  copyNode.className = "mobile-agent-card-copy";
  copyNode.textContent = copy || "detail unavailable";
  card.append(titleNode, metaNode, copyNode);
  return card;
}

function mobileAgentEmptyCard(copy) {
  return mobileAgentCard({
    title: "Unavailable",
    meta: "owner projection",
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
    taskBoardStatus.textContent = `status unavailable: ${state.phase2StatusError}`;
    taskBoardList.textContent = "-";
    return;
  }
  if (!board) {
    taskBoardStatus.textContent = "waiting";
    taskBoardList.textContent = "-";
    return;
  }
  const tasks = currentSessionTasks();
  taskBoardStatus.textContent = currentSessionTaskStatusLabel(tasks);
  taskBoardList.replaceChildren();
  if (tasks.length === 0) {
    taskBoardList.textContent = state.selectedSessionId ? "no tasks for selected session" : "no tasks yet";
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
  goal.textContent = compactSentence(task.goal || task.target_cwd || "task registered", 120);
  item.append(title, meta, goal);
  return item;
}

function renderAgentBoardProjection() {
  if (!agentBoardStatus || !agentBoardList) {
    return;
  }
  const board = state.agentBoard;
  if (state.phase2StatusError && !board) {
    agentBoardStatus.textContent = `status unavailable: ${state.phase2StatusError}`;
    agentBoardList.textContent = "-";
    return;
  }
  if (!board) {
    agentBoardStatus.textContent = "waiting";
    agentBoardList.textContent = "-";
    return;
  }
  const agents = currentSessionAgents();
  const activeCount = agents.filter((agent) => agent.alive).length;
  agentBoardStatus.textContent = `${agents.length} current agent(s) · ${activeCount} active`;
  agentBoardList.replaceChildren();
  if (agents.length === 0) {
    agentBoardList.textContent = state.selectedSessionId ? "no workers for selected session" : "no workers yet";
    return;
  }
  agents.slice(0, 8).forEach((agent, index) => agentBoardList.appendChild(agentBoardItem(agent, index)));
}

function agentBoardItem(agent, index) {
  const boundTask = taskForAgent(agent);
  const item = document.createElement(boundTask ? "button" : "section");
  item.className = `phase2-card phase2-agent-card ${agent.alive ? "phase2-agent-active" : "phase2-muted"}`;
  if (boundTask) {
    item.type = "button";
    item.dataset.taskId = boundTask.task_id || "";
    item.dataset.sessionId = workerSessionIdForTask(boundTask);
    item.addEventListener("click", () => {
      openWorkerTaskSession(boundTask);
    });
  }
  const title = document.createElement("div");
  title.className = "phase2-card-title";
  title.textContent = phase2AgentLabel(agent.agent_id, index);
  const meta = document.createElement("div");
  meta.className = "phase2-card-meta";
  meta.textContent = [statusLabel(agent.state), agent.role || "agent", agent.current_model ? `model ${agent.current_model}` : null]
    .filter(Boolean)
    .join(" · ");
  const activity = document.createElement("div");
  activity.className = "phase2-card-copy";
  activity.textContent = lifecycleActivityLabel(agent) || (boundTask ? taskTitle(boundTask) : "idle");
  item.append(title, meta, activity);
  return item;
}

function renderEventInboxProjection() {
  if (!eventInboxStatus || !eventInboxList) {
    return;
  }
  const inbox = state.eventInbox;
  if (state.phase2StatusError && !inbox) {
    eventInboxStatus.textContent = `status unavailable: ${state.phase2StatusError}`;
    eventInboxList.textContent = "-";
    return;
  }
  if (!inbox) {
    eventInboxStatus.textContent = "waiting";
    eventInboxList.textContent = "-";
    return;
  }
  const events = currentSessionEvents();
  eventInboxStatus.textContent = `${events.length} current event(s)${inbox.next_cursor ? " · updated" : ""}`;
  eventInboxList.replaceChildren();
  if (events.length === 0) {
    eventInboxList.textContent = state.selectedSessionId ? "no events for selected session" : "no pending task events";
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
    taskHistoryStatus.textContent = `history unavailable: ${state.phase2StatusError}`;
    taskHistoryList.textContent = "-";
    return;
  }
  if (!history) {
    if (state.taskBoard) {
      taskHistoryStatus.textContent = "no task history";
      taskHistoryList.textContent = "no task selected";
      return;
    }
    taskHistoryStatus.textContent = "waiting";
    taskHistoryList.textContent = "-";
    return;
  }
  const events = history.events || [];
  taskHistoryStatus.textContent = `${events.length} execution event(s)`;
  taskHistoryList.replaceChildren();
  if (events.length === 0) {
    taskHistoryList.textContent = "no execution events recorded";
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
    workerControlStatus.textContent = `status unavailable: ${state.phase2StatusError}`;
    workerControlList.textContent = "-";
    return;
  }
  if (!target && !control) {
    workerControlStatus.textContent = "no active execution";
    workerControlList.textContent = "worker control appears when a task has an execution";
    return;
  }
  const events = (control && control.events) || [];
  const currentTask = (control && control.task) || (target && target.task) || null;
  workerControlStatus.textContent = `${statusLabel(currentTask && currentTask.status)} · ${events.length} control event(s)`;
  workerControlList.replaceChildren();
  workerControlList.appendChild(workerControlSummaryCard(currentTask, target));
  workerControlList.appendChild(workerControlActionRow(currentTask, target));
  if (events.length === 0) {
    const empty = document.createElement("div");
    empty.className = "phase2-empty-note";
    empty.textContent = "no control events recorded";
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
  title.textContent = task ? taskTitle(task) : "Worker execution";
  const meta = document.createElement("div");
  meta.className = "phase2-card-meta";
  meta.textContent = [statusLabel(task && task.status), assigneeLabel((task && task.assignee_agent_id) || (target && target.agent_id))]
    .filter(Boolean)
    .join(" · ");
  const copy = document.createElement("div");
  copy.className = "phase2-card-copy";
  copy.textContent = task && task.active_execution_id ? "execution is tracked by the service" : "no active execution";
  item.append(title, meta, copy);
  return item;
}

function workerControlActionRow(task, target) {
  const row = document.createElement("div");
  row.className = "phase2-action-row";
  const disabled = !workerControlCanMutate(task, target) || state.workerControlInFlight;
  [
    ["query_status", "Query status"],
    ["request_checkpoint", "Checkpoint"],
    ["request_submission_now", "Submit now"],
    ["pause", "Pause"],
    ["resume", "Resume"],
    ["cancel", "Cancel"],
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
    return taskAgent === agentId && !terminalTaskStatus(task.status);
  }) || null;
}

function openWorkerTaskSession(task) {
  if (!task || !task.task_id) {
    return;
  }
  const sessionId = workerSessionIdForTask(task);
  if (!sessionId) {
    setCommandStatus("worker session unavailable in TaskBoard projection", { stickyMs: 8000 });
    return;
  }
  switchConversationSession(sessionId);
  state.taskHistory = null;
  state.workerControl = null;
  adpQuery({ QueryTaskHistory: { task_id: task.task_id } })
    .then((result) => applyPhase2QueryResult(result))
    .catch((error) => {
      state.phase2StatusError = error.message;
      renderPhase2Dashboard();
    });
  if (task.active_execution_id) {
    adpQuery({
      QueryWorkerControl: {
        task_id: task.task_id,
        execution_id: task.active_execution_id,
      },
    })
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
    setCommandStatus("parent Master session unavailable for selected Worker", { stickyMs: 8000 });
    return;
  }
  switchConversationSession(parentSessionId);
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
  const workerLimitText = Number.isFinite(workerLimit) && workerLimit > 0 ? ` · limit ${workerLimit}` : "";
  const title = selectedWorkerSession
    ? selectedWorkerSession.title || selectedWorkerSession.session_id
    : parentSession
      ? parentSession.title || parentSession.session_id
      : state.selectedSessionId || "No session selected";
  const activeTask =
    (model.tasks || []).find((task) => taskLifecycleBucket(task.status) === "active") ||
    (model.tasks || []).find((task) => taskLifecycleBucket(task.status) === "review") ||
    (model.tasks || []).find((task) => taskLifecycleBucket(task.status) === "blocked") ||
    (model.tasks || [])[0];
  const copy = selectedWorkerSession
    ? liveObservation
      ? liveObservationLine(liveObservation)
      : `Parent Master: ${parentSession ? parentSession.title || parentSession.session_id : selectedWorkerSession.parent_session_id || "unavailable"}`
    : activeTask
      ? liveObservation
        ? liveObservationLine(liveObservation)
        : `${statusLabel(activeTask.status)}: ${taskTitle(activeTask)}`
      : "Click to open session tree";

  sessionRelationHeader.dataset.open = state.sessionTreeOpen ? "true" : "false";
  sessionRelationHeader.dataset.selectedKind = selectedWorkerSession ? "worker" : "master";
  sessionRelationHeader.dataset.liveSessionId = liveObservation ? liveObservation.sessionId : "";
  sessionRelationHeader.dataset.liveTurnId = liveObservation ? liveObservation.turnId : "";
  setText("session-relation-kicker", selectedWorkerSession ? "Worker session" : "Master session");
  setText("session-relation-title", compactSentence(title, 96));
  setText(
    "session-relation-metrics",
    liveObservation
      ? `${liveObservation.label} · ${liveObservation.turnId || "active turn"} · ${runningAgents} agents${workerLimitText}`
      : `${counts.activeCount} running · ${counts.reviewCount} review · ${counts.blockedCount} blocked · ${counts.closedCount} closed · ${runningAgents} agents${workerLimitText}`,
  );
  setText("session-relation-copy", compactSentence(copy, 132));
  if (sessionRelationToggleButton) {
    sessionRelationToggleButton.setAttribute("aria-expanded", state.sessionTreeOpen ? "true" : "false");
  }
  if (sessionTreeDropdown) {
    sessionTreeDropdown.hidden = !state.sessionTreeOpen;
  }
  renderWorkerSessionNavigation(selectedWorkerSession);
  renderSessionTree(parentSession, tasks, selectedWorkerSession);
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
      sessionTreeText("No persisted Master session selected", "Create or select a session to inspect Worker relationships."),
      sessionTreeStatus("waiting", "phase2-muted"),
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
      status: sessionLiveObservation(parentSession.session_id)?.label || (selectedWorkerSession ? "Back" : "Selected"),
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
      sessionTreeText("No Worker child session", "TaskBoard has no current child tasks for this Master session."),
      sessionTreeStatus("0 tasks", "phase2-muted"),
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
  titleNode.textContent = compactSentence(title || "Session", 110);
  const metaNode = document.createElement("span");
  metaNode.className = "session-tree-node-meta";
  metaNode.textContent = compactSentence(meta || "relationship truth unavailable", 150);
  copy.append(titleNode, metaNode);
  return copy;
}

function sessionTreeStatus(status, statusClass) {
  const node = document.createElement("span");
  node.className = ["session-tree-node-status", statusClass || "phase2-muted"].join(" ");
  node.textContent = status || "status";
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
    !terminalTaskStatus(task.status)
  );
}

function taskTitle(task) {
  const title = `${(task && task.title) || ""}`.trim();
  return compactSentence(title || "Task", 80);
}

function statusLabel(status) {
  const normalized = `${status || ""}`.trim().toLowerCase();
  if (!normalized) {
    return "unknown";
  }
  return normalized.replace(/_/g, " ");
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
  const agents = ((state.agentBoard && state.agentBoard.agents) || []).filter(
    (agent) => normalizeAgentId(agent.agent_id) !== "master",
  );
  const index = explicitIndex === null
    ? agents.findIndex((agent) => normalizeAgentId(agent.agent_id) === normalized)
    : explicitIndex;
  const ordinal = index >= 0 ? index + 1 : workerOrdinalFromAgentId(normalized);
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
  return [compactSentence(activity.semantic_summary || activity.kind || "activity", 90), elapsed]
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
    return "just now";
  }
  return `${formatDuration(elapsed)} ago`;
}

function eventKindLabel(kind) {
  const raw = `${kind || "TaskEvent"}`;
  return raw
    .replace(/^Task/, "")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/_/g, " ")
    .trim()
    .toLowerCase() || "task event";
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
    query_status: "Status queried",
    ask_at_safe_point: "Question queued",
    add_constraint: "Constraint queued",
    request_checkpoint: "Checkpoint requested",
    request_submission_now: "Submission requested",
    pause: "Pause requested",
    resume: "Resume requested",
    cancel: "Cancel requested",
  };
  return labels[normalized] || statusLabel(normalized || "control event");
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
    return "no due time";
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
  renderTimerSourceOptions();
  if (timerDashboardStatus) {
    timerDashboardStatus.textContent = state.timerStatusError
      ? `timer query failed: ${state.timerStatusError}`
      : state.timerList
        ? timerDashboardSummary()
        : "waiting for timer projection";
  }
  renderTimerDashboardList();
  renderTimerDashboardHistory();
}

function renderTimerSourceOptions() {
  if (!timerSourceSessionInput) {
    return;
  }
  const selectedValue = timerSourceSessionInput.value || currentTimerSourceSessionId() || "";
  timerSourceSessionInput.replaceChildren();
  const internal = document.createElement("option");
  internal.value = "";
  internal.textContent = "Internal wakeup";
  timerSourceSessionInput.appendChild(internal);
  state.sessions.forEach((session) => {
    if (!session || !session.session_id || internalRuntimeSessionId(session.session_id)) {
      return;
    }
    const option = document.createElement("option");
    option.value = session.session_id;
    option.textContent = compactSentence(session.title || session.session_id, 80);
    timerSourceSessionInput.appendChild(option);
  });
  timerSourceSessionInput.value = Array.from(timerSourceSessionInput.options)
    .some((option) => option.value === selectedValue)
    ? selectedValue
    : "";
}

function currentTimerSourceSessionId() {
  const selected = selectedParentSessionSummary() || sessionSummaryForSelected();
  if (selected && selected.session_id && !internalRuntimeSessionId(selected.session_id)) {
    return selected.session_id;
  }
  return "";
}

function renderTimerDashboardList() {
  if (!timerDashboardList) {
    return;
  }
  timerDashboardList.replaceChildren();
  if (state.timerStatusError) {
    timerDashboardList.textContent = state.timerStatusError;
    return;
  }
  const timers = ((state.timerList && state.timerList.timers) || [])
    .slice()
    .sort((left, right) => Number(left.next_due_at || 0) - Number(right.next_due_at || 0));
  if (timers.length === 0) {
    timerDashboardList.textContent = state.timerList ? "No timer schedules." : "waiting for timer truth";
    return;
  }
  timers.forEach((timer) => {
    const row = document.createElement("section");
    row.className = "timer-row";
    row.dataset.timerId = timer.timer_id || "";
    const marker = document.createElement("span");
    marker.className = `settings-status-marker ${["active", "running"].includes(timer.status) ? "ok" : "attention"}`;
    marker.setAttribute("aria-hidden", "true");
    const body = document.createElement("div");
    body.className = "timer-row-body";
    const title = document.createElement("strong");
    title.textContent = compactSentence(timer.reason || timer.timer_id, 96);
    const meta = document.createElement("small");
    const repeat = timer.repeat_summary ? ` · ${timer.repeat_summary}` : "";
    meta.textContent = compactSentence(`${timer.status} · due ${formatUnixTime(timer.next_due_at)} · ${timer.fired_count}/${timer.max_runs}${repeat}`, 150);
    const prompt = document.createElement("p");
    prompt.textContent = compactSentence(timer.prompt || "no wakeup prompt", 180);
    body.append(title, meta, prompt);
    row.append(marker, body);
    if (["active", "running"].includes(timer.status)) {
      const cancel = document.createElement("button");
      cancel.className = "session-bulk-button timer-cancel-button";
      cancel.type = "button";
      cancel.textContent = "Cancel";
      cancel.addEventListener("click", () => cancelTimer(timer.timer_id));
      row.appendChild(cancel);
    }
    timerDashboardList.appendChild(row);
  });
}

function renderTimerDashboardHistory() {
  if (!timerDashboardHistory) {
    return;
  }
  timerDashboardHistory.replaceChildren();
  const events = ((state.timerList && state.timerList.events) || []).slice(-8).reverse();
  if (events.length === 0) {
    timerDashboardHistory.textContent = state.timerList ? "No timer ledger events." : "waiting for timer ledger";
    return;
  }
  events.forEach((event) => {
    const row = document.createElement("div");
    row.className = "timer-event-row";
    const marker = document.createElement("span");
    marker.className = `settings-status-marker ${event.event_type === "TimerScheduled" || event.event_type === "TimerFired" ? "ok" : "attention"}`;
    marker.setAttribute("aria-hidden", "true");
    const copy = document.createElement("span");
    copy.textContent = compactSentence(`${event.event_type} · ${formatUnixTime(event.occurred_at)} · ${event.summary}`, 180);
    row.append(marker, copy);
    timerDashboardHistory.appendChild(row);
  });
}

async function openTimerDashboard() {
  if (timerDashboardDialog && typeof timerDashboardDialog.showModal === "function" && !timerDashboardDialog.open) {
    timerDashboardDialog.showModal();
  }
  renderTimerDashboard();
  await refreshTimerDashboard();
}

async function refreshTimerDashboard() {
  try {
    const result = await adpQuery({ QueryTimerList: { include_terminal: true } });
    applyPhase2QueryResult(result);
    setCommandStatus("Timer projection refreshed.");
  } catch (error) {
    state.timerStatusError = error.message;
    setCommandStatus(`timer refresh failed: ${error.message}`, { stickyMs: 9000 });
    renderTimerDashboard();
    renderMobileHomeDashboard();
  }
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
  const timer = buildTimerSchedulePayload();
  state.timerCommandInFlight = true;
  renderTimerDashboard();
  try {
    const receipt = await adpCommand({ ScheduleTimer: { timer } });
    const message = timerScheduleReceiptStatus(receipt);
    setCommandStatus(message, { stickyMs: 8000 });
    await refreshTimerDashboard();
  } catch (error) {
    state.timerStatusError = error.message;
    setCommandStatus(`timer schedule failed: ${error.message}`, { stickyMs: 9000 });
  } finally {
    state.timerCommandInFlight = false;
    renderTimerDashboard();
    renderMobileHomeDashboard();
  }
}

async function cancelTimer(timerId) {
  if (!timerId) {
    return;
  }
  state.timerCommandInFlight = true;
  renderTimerDashboard();
  try {
    const receipt = await adpCommand({ CancelTimer: { timer_id: timerId } });
    const message = timerCancelReceiptStatus(receipt);
    setCommandStatus(message, { stickyMs: 8000 });
    await refreshTimerDashboard();
  } catch (error) {
    state.timerStatusError = error.message;
    setCommandStatus(`timer cancel failed: ${error.message}`, { stickyMs: 9000 });
  } finally {
    state.timerCommandInFlight = false;
    renderTimerDashboard();
    renderMobileHomeDashboard();
  }
}

function renderToolsDashboard() {
  if (toolsDashboardStatus) {
    toolsDashboardStatus.textContent = state.toolRegistryError
      ? `tool registry query failed: ${state.toolRegistryError}`
      : state.toolRegistry
        ? toolRegistrySummary()
        : "waiting for tool registry projection";
  }
  if (toolsDashboardRefreshButton) {
    toolsDashboardRefreshButton.disabled = state.toolRegistryInFlight;
    toolsDashboardRefreshButton.textContent = state.toolRegistryInFlight ? "Refreshing..." : "Refresh tools";
  }
  renderToolRegistryGuidance();
  renderToolRegistryList();
}

function toolRegistrySummary() {
  const tools = toolRegistryTools();
  const masterCount = tools.filter((tool) => tool.exposed_to_master).length;
  const workerCount = tools.filter((tool) => tool.exposed_to_worker).length;
  const unimplementedCount = tools.filter((tool) => !tool.implemented).length;
  return `registry ${state.toolRegistry.registry_version || "unknown"} · ${tools.length} tools · master ${masterCount} · worker ${workerCount} · unimplemented ${unimplementedCount}`;
}

function renderToolRegistryGuidance() {
  if (!toolsDashboardGuidance) {
    return;
  }
  toolsDashboardGuidance.replaceChildren();
  if (state.toolRegistryError) {
    toolsDashboardGuidance.textContent = state.toolRegistryError;
    return;
  }
  const guidance = Array.isArray(state.toolRegistry?.guidance) ? state.toolRegistry.guidance : [];
  if (guidance.length === 0) {
    toolsDashboardGuidance.textContent = state.toolRegistry ? "No registry guidance." : "waiting for registry guidance";
    return;
  }
  guidance.forEach((line) => {
    const item = document.createElement("p");
    item.textContent = line;
    toolsDashboardGuidance.appendChild(item);
  });
}

function renderToolRegistryList() {
  if (!toolsDashboardList) {
    return;
  }
  toolsDashboardList.replaceChildren();
  if (state.toolRegistryError) {
    toolsDashboardList.textContent = state.toolRegistryError;
    return;
  }
  const tools = toolRegistryTools();
  if (tools.length === 0) {
    toolsDashboardList.textContent = state.toolRegistry ? "No tool registry rows." : "waiting for tool registry truth";
    return;
  }
  tools.forEach((tool) => {
    toolsDashboardList.appendChild(renderToolRegistryCard(tool));
  });
}

function toolRegistryTools() {
  return Array.isArray(state.toolRegistry?.tools)
    ? state.toolRegistry.tools.slice().sort((left, right) => `${left.name || ""}`.localeCompare(`${right.name || ""}`))
    : [];
}

function renderToolRegistryCard(tool) {
  const card = document.createElement("article");
  card.className = "tool-registry-card";
  card.dataset.toolName = tool.name || "";
  card.dataset.scope = tool.execution_scope || "";
  card.dataset.implemented = String(tool.implemented === true);
  card.dataset.readOnly = String(tool.read_only === true);
  card.dataset.exposedToMaster = String(tool.exposed_to_master === true);
  card.dataset.exposedToWorker = String(tool.exposed_to_worker === true);

  const header = document.createElement("div");
  header.className = "tool-registry-card-head";
  const marker = document.createElement("span");
  marker.className = `settings-status-marker ${toolRegistryTone(tool)}`;
  marker.setAttribute("aria-hidden", "true");
  const title = document.createElement("div");
  title.className = "tool-registry-title";
  const name = document.createElement("strong");
  name.textContent = tool.name || "unnamed";
  const meta = document.createElement("small");
  meta.textContent = [
    `scope=${tool.execution_scope || "unknown"}`,
    `read_only=${tool.read_only === true}`,
    `implemented=${tool.implemented === true}`,
    `master=${tool.exposed_to_master === true}`,
    `worker=${tool.exposed_to_worker === true}`,
  ].join(" · ");
  title.append(name, meta);
  header.append(marker, title);
  card.appendChild(header);

  const description = document.createElement("p");
  description.className = "tool-registry-description";
  description.textContent = tool.description || "No description projected.";
  card.appendChild(description);

  const badges = document.createElement("div");
  badges.className = "tool-registry-badges";
  badges.append(
    toolRegistryBadge(tool.execution_scope || "unknown", "scope"),
    toolRegistryBadge(tool.read_only ? "read only" : "mutating", "read_only"),
    toolRegistryBadge(tool.implemented ? "implemented" : "unimplemented", "implemented"),
    toolRegistryBadge(tool.exposed_to_master ? "Master visible" : "Master hidden", "master"),
    toolRegistryBadge(tool.exposed_to_worker ? "Worker visible" : "Worker hidden", "worker"),
  );
  card.appendChild(badges);

  appendToolRegistryListSection(card, "Examples", tool.examples, "example");
  appendToolRegistryListSection(card, "Guidance", tool.guidance, "guidance");

  const details = document.createElement("details");
  details.className = "tool-registry-schema";
  const summary = document.createElement("summary");
  summary.textContent = "Input schema";
  const pre = document.createElement("pre");
  pre.textContent = schemaPreview(tool.input_schema);
  details.append(summary, pre);
  card.appendChild(details);
  return card;
}

function toolRegistryTone(tool) {
  if (!tool || tool.implemented !== true) {
    return "attention";
  }
  return tool.exposed_to_master || tool.exposed_to_worker ? "ok" : "attention";
}

function toolRegistryBadge(label, key) {
  const badge = document.createElement("span");
  badge.className = "tool-registry-badge";
  badge.dataset.badge = key || "";
  badge.textContent = label;
  return badge;
}

function appendToolRegistryListSection(card, title, items, kind) {
  const values = Array.isArray(items) ? items.filter(Boolean) : [];
  if (values.length === 0) {
    return;
  }
  const section = document.createElement("section");
  section.className = `tool-registry-section tool-registry-section-${kind}`;
  const heading = document.createElement("div");
  heading.className = "tool-registry-section-heading";
  heading.textContent = title;
  section.appendChild(heading);
  values.forEach((value) => {
    const item = document.createElement(kind === "example" ? "code" : "p");
    item.textContent = `${value}`;
    section.appendChild(item);
  });
  card.appendChild(section);
}

function schemaPreview(schema) {
  try {
    return JSON.stringify(schema || {}, null, 2);
  } catch (error) {
    return String(schema || "");
  }
}

async function openToolsDashboard() {
  if (toolsDashboardDialog && typeof toolsDashboardDialog.showModal === "function" && !toolsDashboardDialog.open) {
    toolsDashboardDialog.showModal();
  }
  renderToolsDashboard();
  await refreshToolsDashboard();
}

async function refreshToolsDashboard() {
  state.toolRegistryInFlight = true;
  renderToolsDashboard();
  try {
    const result = await adpQuery("QueryToolRegistry");
    applyPhase2QueryResult(result);
    setCommandStatus("Tool registry projection refreshed.");
  } catch (error) {
    state.toolRegistryError = error.message;
    setCommandStatus(`tool registry refresh failed: ${error.message}`, { stickyMs: 9000 });
  } finally {
    state.toolRegistryInFlight = false;
    renderToolsDashboard();
  }
}

function renderSessionSearchDashboard() {
  if (sessionSearchStatus) {
    sessionSearchStatus.textContent = state.sessionSearchError
      ? `session search failed: ${state.sessionSearchError}`
      : state.sessionSearch
        ? `query "${state.sessionSearch.query || ""}" · ${sessionSearchResultsList().length} parent results`
        : "Enter a query to search persisted sessions.";
  }
  if (sessionSearchSubmitButton) {
    sessionSearchSubmitButton.disabled = state.sessionSearchInFlight;
    sessionSearchSubmitButton.textContent = state.sessionSearchInFlight ? "Searching..." : "Search";
  }
  if (!sessionSearchResults) {
    return;
  }
  sessionSearchResults.replaceChildren();
  if (state.sessionSearchError) {
    sessionSearchResults.textContent = state.sessionSearchError;
    return;
  }
  if (state.sessionSearchInFlight) {
    sessionSearchResults.textContent = "querying persisted session index...";
    return;
  }
  const results = sessionSearchResultsList();
  if (results.length === 0) {
    sessionSearchResults.textContent = state.sessionSearch ? "No persisted session matches." : "No query yet.";
    return;
  }
  results.forEach((result) => {
    sessionSearchResults.appendChild(renderSessionSearchResult(result));
  });
}

function sessionSearchResultsList() {
  return Array.isArray(state.sessionSearch?.results) ? state.sessionSearch.results : [];
}

function renderSessionSearchResult(result) {
  const card = document.createElement("article");
  card.className = "session-search-card";
  card.dataset.sessionId = result.session_id || "";
  const head = document.createElement("button");
  head.className = "session-search-card-head";
  head.type = "button";
  const marker = document.createElement("span");
  marker.className = "settings-status-marker ok";
  marker.setAttribute("aria-hidden", "true");
  const title = document.createElement("span");
  title.className = "session-search-title";
  const strong = document.createElement("strong");
  strong.textContent = compactSentence(result.title || result.session_id || "session", 96);
  const small = document.createElement("small");
  small.textContent = compactSentence([
    result.latest_status || "session",
    result.latest_turn_id ? `turn ${result.latest_turn_id}` : "",
    result.cwd || "",
  ].filter(Boolean).join(" · "), 110);
  title.append(strong, small);
  head.append(marker, title);
  head.addEventListener("click", () => openSessionSearchResult(result.session_id));
  card.appendChild(head);

  const snippet = document.createElement("p");
  snippet.className = "session-search-snippet";
  snippet.textContent = result.snippet || "Matched persisted session metadata.";
  card.appendChild(snippet);

  const fields = document.createElement("div");
  fields.className = "session-search-fields";
  fields.textContent = `matched: ${(result.matched_fields || []).join(", ") || "session"}`;
  card.appendChild(fields);

  const childMatches = Array.isArray(result.child_matches) ? result.child_matches : [];
  if (childMatches.length > 0) {
    const children = document.createElement("div");
    children.className = "session-search-child-list";
    childMatches.forEach((child) => {
      const childRow = document.createElement("button");
      childRow.className = "session-search-child";
      childRow.type = "button";
      childRow.dataset.parentSessionId = result.session_id || "";
      childRow.dataset.childSessionId = child.session_id || "";
      childRow.textContent = compactSentence([
        `Worker child: ${child.title || child.task_id || child.session_id}`,
        child.latest_status || "session",
        child.snippet || "",
      ].filter(Boolean).join(" · "), 180);
      childRow.addEventListener("click", () => openSessionSearchResult(result.session_id));
      children.appendChild(childRow);
    });
    card.appendChild(children);
  }
  return card;
}

function openSessionSearchResult(sessionId) {
  if (!sessionId) {
    return;
  }
  sessionSearchDialog?.close();
  closeMobileDrawer();
  switchConversationSession(sessionId);
}

async function openSessionSearchDashboard() {
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
    state.sessionSearchError = "Enter a non-empty search query.";
    renderSessionSearchDashboard();
    return;
  }
  state.sessionSearchInFlight = true;
  state.sessionSearchError = null;
  renderSessionSearchDashboard();
  try {
    const result = await adpQuery({ QuerySessionSearch: { query, limit: 20 } });
    applyPhase2QueryResult(result);
    setCommandStatus("Persisted session search refreshed.");
  } catch (error) {
    state.sessionSearchError = error.message;
    setCommandStatus(`session search failed: ${error.message}`, { stickyMs: 9000 });
  } finally {
    state.sessionSearchInFlight = false;
    renderSessionSearchDashboard();
  }
}

async function refreshPhase2Status() {
  try {
    applyPhase2QueryResult(await adpQuery({ QueryTaskBoard: { include_terminal: true } }));
    applyPhase2QueryResult(await adpQuery("QueryAgentBoard"));
    applyPhase2QueryResult(await adpQuery({ QueryEventInbox: { limit: 30 } }));
    applyPhase2QueryResult(await adpQuery({ QueryTimerList: { include_terminal: true } }));
    const historyTarget = currentTaskHistoryTarget();
    if (historyTarget && historyTarget.task_id) {
      applyPhase2QueryResult(await adpQuery({ QueryTaskHistory: { task_id: historyTarget.task_id } }));
    } else {
      state.taskHistory = null;
    }
    const target = currentWorkerControlTarget();
    if (target && target.task_id && target.execution_id) {
      applyPhase2QueryResult(await adpQuery({
        QueryWorkerControl: {
          task_id: target.task_id,
          execution_id: target.execution_id,
        },
      }));
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
    setCommandStatus("worker control requires a non-terminal assigned execution", { stickyMs: 7000 });
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
    const result = await adpCommand({ WorkerControl: { control } });
    const statusText = `${result.dispatch_status || "accepted"}`.toLowerCase().replace(/_/g, " ");
    setCommandStatus(`worker control ${statusText}`, { stickyMs: 6000 });
    await refreshPhase2Status();
  } catch (error) {
    setCommandStatus(`worker control failed: ${error.message}`, { stickyMs: 9000 });
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
    inspectorEyebrow.textContent = showingSettings ? "settings" : "lifecycle observer";
  }
  if (inspectorTitle) {
    inspectorTitle.textContent = showingSettings ? "Provider Settings" : "Task and Agent Lifecycle";
  }
  if (inspectorCopy) {
    inspectorCopy.textContent = showingSettings
      ? "Edit provider endpoint, model, and credential environment variable. Runtime status lives under Status."
      : "观察 Master/Worker 生命周期、当前执行、事件和必要的调试摘要；活跃 Worker 会高亮并可点击查看对应任务进度。";
  }
  if (settingsShellToggle) {
    settingsShellToggle.classList.toggle("is-active", showingSettings);
    settingsShellToggle.setAttribute("aria-pressed", showingSettings ? "true" : "false");
  }
}

function renderSettingsShell() {
  const modelLabel =
    state.configStatus?.default_model ||
    modelSelector?.selectedOptions?.[0]?.textContent ||
    modelSelector?.value ||
    "Runtime config";
  const providerSummary = state.configStatus
    ? `${state.configStatus.provider_id} · ${state.configStatus.provider_protocol}`
    : state.configStatusError
      ? "unavailable"
      : "loading";
  setText("settings-status-pill", state.adpStatus || "connecting");
  setText("settings-model-value", modelLabel);
  setText("settings-provider-summary", providerSummary);
  setText("settings-provider-id", state.configStatus?.provider_id || "loading");
  setText("settings-provider-type", state.configStatus?.provider_type || "loading");
  setText("settings-provider-protocol", state.configStatus?.provider_protocol || "loading");
  setText("settings-provider-host", state.configStatus?.provider_base_url_host || "loading");
  setText("settings-provider-web-search", webSearchStatusLabel(state.configStatus));
  setTitle("settings-provider-web-search", [
    state.configStatus?.provider_web_search_reason,
    state.configStatus?.provider_web_search_route_summary,
  ].filter(Boolean).join("\n"));
  setText("settings-provider-auth", state.configStatus ? `${settingsAuthTypeLabel(state.configStatus.provider_auth_type)} · ${state.configStatus.provider_auth_source}` : "loading");
  setText("settings-restart-required", state.configStatus?.restart_required_on_change ? "restart required after changes" : "no restart flag");
  setText("settings-config-error", state.configStatusError || "none");
  syncProviderSelectionControls();
  syncSettingsProviderForm();
  renderSettingsProviderRegistry();
  syncModelGroupSelectionControls();
  syncSettingsModelGroupForm();
  renderSettingsModelGroupRegistry();
  renderSystemAgentResourceConfig();
  renderAndroidApkUpdateSettings();
  renderSettingsDiagnostics();
  renderSettingsReviewTree();
  showInspectorPanel(state.inspectorPanel);
}

function androidApkUpdateManifestUrlForDisplay() {
  try {
    return new URL("android/update.json", window.location.href).toString();
  } catch (_) {
    return "daemon /android/update.json";
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
      return "Open this Settings page inside the Android app to check and download a newer APK.";
    }
    if (!androidApkUpdateBridge()) {
      return "Android APK update bridge is unavailable; reload the Android app after installing the latest APK.";
    }
    return "Ready to check this daemon's Android update manifest.";
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
      : "ready"
    : layoutClient() === "android-webview"
      ? "bridge unavailable"
      : "Android app only";
  setText("settings-apk-update-summary", summary);
  setText("settings-apk-update-source", androidApkUpdateManifestUrlForDisplay());
  setText("settings-apk-update-status", androidApkUpdateStatusText());
  if (settingsApkUpdateCheckButton) {
    settingsApkUpdateCheckButton.disabled = !bridgeAvailable || state.androidApkUpdateInFlight;
    settingsApkUpdateCheckButton.textContent = state.androidApkUpdateInFlight
      ? "Checking APK update..."
      : "Check APK update";
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
      message: "APK update check is available only inside the Freehand Android app.",
    });
    return;
  }
  state.androidApkUpdateInFlight = true;
  state.androidApkUpdateStatus = {
    phase: "checking",
    message: "Checking update manifest...",
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
      message: error && error.message ? error.message : "Android APK update bridge call failed",
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
      state.providerSelectionInFlight || !status || !selectedPrimary || !selectionChanged;
    settingsProviderSwitchButton.textContent = state.providerSelectionInFlight
      ? "Saving..."
      : "Switch provider";
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
      ? "This provider uses inline auth in config. Saving from UI will rewrite it to env-var auth."
      : "Loaded safe provider fields. Enter the credential env var before saving.",
  );
}

function renderSettingsProviderRegistry() {
  if (!settingsProviderRegistryList) {
    return;
  }
  const providers = configProviderRegistry();
  if (!state.configStatus) {
    settingsProviderRegistryList.textContent = state.configStatusError || "loading provider registry";
    return;
  }
  if (providers.length === 0) {
    settingsProviderRegistryList.textContent = "No providers configured.";
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
    action.textContent = "Load into form";
    action.addEventListener("click", () => fillSettingsProviderFormFromProvider(provider));
    const testAction = document.createElement("button");
    testAction.className = "settings-secondary-action";
    testAction.type = "button";
    testAction.disabled = provider.enabled === false || Boolean(state.providerWebSearchTestInFlight);
    testAction.textContent = state.providerWebSearchTestInFlight === provider.provider_id
      ? "Testing web_search..."
      : "Test web_search";
    testAction.addEventListener("click", () => {
      fillSettingsProviderFormFromProvider(provider);
      testProviderWebSearch(provider.provider_id).catch((error) => {
        setText("settings-provider-web-search-test-status", `Provider web_search test failed: ${error.message}`);
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
    settingsProviderSaveButton.textContent = state.configSaveInFlight ? "Saving..." : "Add/update provider";
  }
  if (settingsProviderWebSearchTestButton) {
    const providerId = settingsProviderIdInput?.value.trim() || status?.provider_id || "";
    settingsProviderWebSearchTestButton.disabled = !state.configStatus || !providerId || Boolean(state.providerWebSearchTestInFlight);
    settingsProviderWebSearchTestButton.textContent = state.providerWebSearchTestInFlight === providerId
      ? "Testing web_search..."
      : "Test web_search";
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
    setText("settings-provider-save-status", "Config status is not loaded yet.");
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
  setText("settings-provider-save-status", "Saving config...");
  renderSettingsShell();
  try {
    const receipt = await adpCommand({ UpsertProviderConfig: { update } });
    setCommandStatus(providerConfigUpsertReceiptStatus(receipt), { stickyMs: 5000 });
    await refreshConfigStatus();
    setText("settings-provider-save-status", "Provider definition saved. Restart required before active runtime changes.");
  } catch (error) {
    state.configStatusError = error.message;
    setText("settings-provider-save-status", `Save failed: ${error.message}`);
    renderSettingsShell();
  } finally {
    state.configSaveInFlight = false;
    renderSettingsShell();
  }
}

async function testProviderWebSearch(providerId) {
  const targetProviderId = (providerId || settingsProviderIdInput?.value || "").trim();
  if (!targetProviderId) {
    setText("settings-provider-web-search-test-status", "Choose a provider id before testing web_search.");
    return;
  }
  state.providerWebSearchTestInFlight = targetProviderId;
  setText("settings-provider-web-search-test-status", `Testing provider-hosted web_search for ${targetProviderId}...`);
  renderSettingsShell();
  try {
    const receipt = await adpCommand({
      TestProviderWebSearch: {
        provider_id: targetProviderId,
        query: "Use web_search to find the current UTC date and one current news headline from openai.com today. Do not answer from memory.",
      },
    });
    const status = providerWebSearchTestReceiptStatus(receipt);
    setText("settings-provider-web-search-test-status", status);
    setCommandStatus(status, { stickyMs: 8000 });
  } catch (error) {
    const message = `Provider web_search test failed for ${targetProviderId}: ${error.message}`;
    setText("settings-provider-web-search-test-status", message);
    setCommandStatus(message, { stickyMs: 10000 });
  } finally {
    state.providerWebSearchTestInFlight = "";
    renderSettingsShell();
  }
}

async function submitProviderSelectionUpdate() {
  if (!state.configStatus) {
    setText("settings-provider-switch-status", "Config status is not loaded yet.");
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
  setText("settings-provider-switch-status", "Saving provider selection...");
  renderSettingsShell();
  try {
    const receipt = await adpCommand({ UpdateAgentProviderSelection: { selection } });
    setCommandStatus(providerSelectionReceiptStatus(receipt), { stickyMs: 5000 });
    await refreshConfigStatus();
    state.providerSelectionDraft = null;
    setText("settings-provider-switch-status", "Provider selection saved. Restart required before active runtime changes.");
  } catch (error) {
    state.configStatusError = error.message;
    setText("settings-provider-switch-status", `Switch failed: ${error.message}`);
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
  return route ? `${route.provider_id}:${route.model}` : "none";
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
  none.textContent = "No active model group";
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
      ? `${status.model_group_id || "none"} · ${groups.length} configured`
      : state.configStatusError || "loading",
  );
  if (settingsModelGroupSwitchButton) {
    settingsModelGroupSwitchButton.disabled =
      state.modelGroupSelectionInFlight || !status || !changed;
    settingsModelGroupSwitchButton.textContent = state.modelGroupSelectionInFlight
      ? "Saving..."
      : "Switch group";
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
  setInputValueIfNotFocused(settingsModelGroupLabelInput, group?.label || "Default model group");
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
      ? "Saving..."
      : "Add/update model group";
  }
}

function renderSettingsModelGroupRegistry() {
  if (!settingsModelGroupRegistryList) {
    return;
  }
  if (!state.configStatus) {
    settingsModelGroupRegistryList.textContent = state.configStatusError || "loading model groups";
    return;
  }
  const groups = configModelGroupRegistry();
  if (groups.length === 0) {
    settingsModelGroupRegistryList.textContent = "No model groups configured. Add one below to bind primary/sub/search/title/fallback routes.";
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
    ].join(" · ");
    const action = document.createElement("button");
    action.className = "settings-secondary-action";
    action.type = "button";
    action.textContent = "Load into form";
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
    setText("settings-model-group-save-status", "Config status is not loaded yet.");
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
    };
  } catch (error) {
    setText("settings-model-group-save-status", `Model group invalid: ${error.message}`);
    return;
  }
  state.modelGroupSaveInFlight = true;
  setText("settings-model-group-save-status", "Saving model group...");
  renderSettingsShell();
  try {
    const receipt = await adpCommand({ UpsertModelGroupConfig: { group } });
    setCommandStatus(modelGroupUpsertReceiptStatus(receipt), { stickyMs: 5000 });
    await refreshConfigStatus();
    setText("settings-model-group-save-status", "Model group saved. Restart required before active runtime changes.");
  } catch (error) {
    state.configStatusError = error.message;
    setText("settings-model-group-save-status", `Save failed: ${error.message}`);
    renderSettingsShell();
  } finally {
    state.modelGroupSaveInFlight = false;
    renderSettingsShell();
  }
}

async function submitModelGroupSelectionUpdate() {
  if (!state.configStatus) {
    setText("settings-model-group-switch-status", "Config status is not loaded yet.");
    return;
  }
  const selection = {
    agent_name: state.configStatus.agent_name,
    model_group_id: settingsModelGroupCurrentSelect?.value.trim() || null,
  };
  state.modelGroupSelectionInFlight = true;
  setText("settings-model-group-switch-status", "Saving model group selection...");
  renderSettingsShell();
  try {
    const receipt = await adpCommand({ UpdateAgentModelGroupSelection: { selection } });
    setCommandStatus(modelGroupSelectionReceiptStatus(receipt), { stickyMs: 5000 });
    await refreshConfigStatus();
    state.modelGroupSelectionDraft = null;
    setText("settings-model-group-switch-status", "Model group selection saved. Restart required before active runtime changes.");
  } catch (error) {
    state.configStatusError = error.message;
    setText("settings-model-group-switch-status", `Switch failed: ${error.message}`);
    renderSettingsShell();
  } finally {
    state.modelGroupSelectionInFlight = false;
    renderSettingsShell();
  }
}

function renderTurnMeta() {
  if (state.pendingSubmitError) {
    setText("turn-status", "checking service truth · submit receipt not verified");
  }
  const turn = activeTurnForSelectedSession();
  if (!turn) {
    setText("session-title", state.selectedSessionId || "waiting for service state");
    setText("session-copy", state.selectedSessionId ? "no turns in selected session" : "no active turn yet");
    setShellDataset("selectedSession", state.selectedSessionId || "");
    setShellDataset("selectedTurn", "");
    setShellDataset("selectedCwd", state.selectedCwd || "");
    if (!state.pendingSubmitError) {
      setText("turn-status", liveTurnStatus() || "waiting");
    }
    return;
  }

  setText("session-title", turn.session_id);
  setText("session-copy", turn.cwd ? `${turn.turn_id} · ${turn.cwd}` : turn.turn_id);
  setShellDataset("selectedSession", turn.session_id || "");
  setShellDataset("selectedTurn", turn.turn_id || "");
  setShellDataset("selectedCwd", turn.cwd || state.selectedCwd || "");
  const runningTools = (turn.tool_activities || []).filter((tool) => tool.status === "Waiting" || tool.status === "waiting");
  const turnStatus = turn.terminal_text || isTerminalStatus(turn.terminal_status) || isToolPendingStatus(turn.terminal_status)
    ? terminalTurnStatusLabel(turn.terminal_status)
    : runningTools.length > 0
      ? waitingToolStatus(runningTools).replace("tool executing", "tool running")
      : turnIsWaitingForModelResponse(turn)
        ? liveTurnStatus()
        : state.submitInFlight
          ? liveTurnStatus()
          : "waiting";
  if (!state.pendingSubmitError) {
    setText("turn-status", turnStatus);
  }
}

setInterval(() => {
  if (renderModelHasLiveLifecycle()) {
    renderMessages();
    renderTurnMeta();
    renderCommandStatus();
  }
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
  return !!(turn && isToolPendingStatus(turn.terminal_status));
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
  const result = await adpQuery("QueryLatestActiveTurn");
  applyAdpQueryResult(result);
  await refreshCheckpoints();
}

async function refreshSessions() {
  const result = await adpQuery("QuerySessionList");
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
  const result = await adpQuery({
    QuerySessionTurns: { session_id: requestedSessionId },
  });
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
  const result = await adpQuery({ QueryDebugState: { turn_id: state.turn.turn_id } });
  applyAdpQueryResult(result);
}

async function refreshCheckpoints() {
  const result = await adpQuery("QueryCheckpoints");
  applyAdpQueryResult(result);
  renderAll();
}

async function refreshConfigStatus() {
  try {
    const result = await adpQuery("QueryConfigStatus");
    applyAdpQueryResult(result);
  } catch (error) {
    state.configStatus = null;
    state.configStatusError = error.message;
    renderSettingsShell();
    renderPhase2Dashboard();
  }
}

async function refreshDiagnosticsStatus() {
  state.diagnosticsInFlight = true;
  renderSettingsDiagnostics();
  try {
    const result = await adpQuery("QueryDiagnostics");
    applyPhase2QueryResult(result);
  } catch (error) {
    state.diagnosticsError = error.message;
    setCommandStatus(`diagnostics refresh failed: ${error.message}`, { stickyMs: 9000 });
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
  setBackgroundCommandStatus(`checking service truth after ${reason}...`);
  refreshAllProtocolState()
    .then(() => {
      clearPendingUserInputIfMaterialized();
      renderAll();
      setBackgroundCommandStatus(`service truth refreshed after ${reason}`);
    })
    .catch((error) => {
      setCommandStatus(`service refresh after ${reason} failed: ${error.message}`, { stickyMs: 8000 });
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
      ? `${baseMessage}; refresh also failed: ${refreshErrors.join("; ")}`
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
      const recovery = await refreshAfterAmbiguousSubmitFailure(new Error("service truth refresh pending"));
      if (recovery.materialized) {
        state.pendingSubmitError = null;
        state.pendingAttachments = [];
        renderAll();
        setCommandStatus("request accepted by service truth; lifecycle is visible", { stickyMs: 5000 });
        stopAmbiguousSubmitRecoveryPolling();
      } else {
        renderAll();
      }
    } catch (error) {
      state.pendingSubmitError = error.message;
      renderAll();
    }
    if (attempts >= 20) {
      setCommandStatus("submit receipt still not verified after service checks; refresh before retry", { stickyMs: 8000 });
      stopAmbiguousSubmitRecoveryPolling();
    }
  }, 2000);
}

function ensureTurnSubscription() {
  if (state.adpSubscriptions.has("latest-turn")) {
    return;
  }
  state.adpSubscriptions.add("latest-turn");
  adpSubscribe(
    { SubscribeLatestActiveTurn: { client: adpClientKind() } },
    "sub-turn",
  ).catch((error) => {
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
  state.sseTurnStream = stream;
  stream.addEventListener("open", () => {
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
        status_text: "debug stream waiting",
        detail_lines: ["waiting for debug subscription"],
      };
      renderDebug();
  }
  adpSubscribe(
    { SubscribeDebugState: { client: adpClientKind(), turn_id: state.turn.turn_id } },
    "sub-debug",
  ).catch((error) => {
    state.debug = {
      status_text: "debug stream failed",
      detail_lines: [error.message],
    };
    renderDebug();
  });
}

async function submitUserInput(text, submitMetadata = null) {
  const command = { SubmitUserInput: { text } };
  if (state.selectedSessionId) {
    command.SubmitUserInput.session_id = state.selectedSessionId;
  }
  const cwd = normalizeCwd(state.selectedCwd);
  if (cwd) {
    command.SubmitUserInput.cwd = cwd;
  }
  if (submitMetadata && Array.isArray(submitMetadata.attachments) && submitMetadata.attachments.length > 0) {
    command.SubmitUserInput.metadata = submitMetadata;
  }
  const payload = await adpCommand(command);
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
    setCommandStatus("no active turn; input cleared", { stickyMs: 3000 });
    renderMessages();
    return;
  }
  const command = turnId
    ? { CancelTurn: { turn_id: turnId } }
    : { CancelLatestActiveTurn: {} };
  setCommandStatus(`cancelling ${turnId || "latest active turn"}...`);
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
    setCommandStatus(`cancel failed: ${error.message}`);
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
    payload = await adpCommand({ RewindCheckpoint: { checkpoint_id: checkpointId } });
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
    "/settings",
    "/cwd",
    "/sessions",
    "/reload",
    "/success",
    "/failure",
    "/cancel",
    "/clear",
    "/attachments",
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
    case "/settings":
      showInspectorPanel("settings");
      setMobileDrawer("settings");
      renderAll();
      setCommandStatus("settings opened", { stickyMs: 4000 });
      return true;
    case "/cwd": {
      const cwd = requireTaskCwd("task cwd selection");
      if (cwd) {
        setCommandStatus(`task target directory selected: ${cwd}`, { stickyMs: 5000 });
        renderAll();
      }
      return true;
    }
    case "/sessions":
      setCommandStatus("refreshing sessions...", { stickyMs: 3000 });
      await refreshSessions();
      await refreshSelectedSession();
      setCommandStatus("sessions refreshed", { stickyMs: 5000 });
      return true;
    case "/reload":
      setCommandStatus("refreshing service state...", { stickyMs: 3000 });
      await refreshAllProtocolState();
      setCommandStatus("service state refreshed", { stickyMs: 5000 });
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
      setCommandStatus("local composer cleared", { stickyMs: 3000 });
      renderMessages();
      return true;
    case "/attachments":
      state.attachmentsPreviewOpen = !state.attachmentsPreviewOpen;
      renderAttachmentTray();
      setCommandStatus(`attachments ${state.attachmentsPreviewOpen ? "preview visible" : "preview collapsed"}: ${attachmentSummary()}`, { stickyMs: 5000 });
      return true;
    case "/model":
      setCommandStatus("model selection is controlled by runtime config", { stickyMs: 6000 });
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
    setCommandStatus("empty input rejected", { stickyMs: 3000 });
    return;
  }
  if (attachments.some((attachment) => attachment.kind !== "image")) {
    setCommandStatus("only image attachments can be submitted in this version", { stickyMs: 6000 });
    return;
  }
  try {
    if (text && attachments.length === 0 && await runSlashCommand(text)) {
      return;
    }
  } catch (error) {
    setCommandStatus(`slash command failed: ${error.message}`, { stickyMs: 8000 });
    return;
  }
  let submitMetadata = { attachments: [] };
  try {
    submitMetadata = {
      attachments: await attachmentsForSubmit(attachments),
    };
  } catch (error) {
    setCommandStatus(`image submit failed before dispatch: ${error.message}`, { stickyMs: 8000 });
    return;
  }
  const commandText = text || "Analyze the attached image.";
  setCommandStatus("dispatching...");
  if (!state.selectedSessionId) {
    const sessionId = newDraftSessionId();
    state.draftSessionId = sessionId;
    setSelectedSessionId(sessionId);
  }
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
      setCommandStatus("request is visible after service refresh; continue from current conversation state", { stickyMs: 5000 });
      return;
    }
    state.pendingSubmitError = recovery.message;
    startAmbiguousSubmitRecoveryPolling(submittedAt);
    renderMessages();
    renderTurnMeta();
    setCommandStatus(`submit receipt not verified after service refresh; checking service truth before duplicate send. Use ↑ to recall input. Draft attachments retained: ${recovery.message}`);
  }
});

cancelButton.addEventListener("click", () => {
  cancelActiveTurn().catch((error) => {
    setCommandStatus(`cancel failed: ${error.message}`);
  });
});

function loadSamplePrompt(kind) {
  const prompt = samplePrompts[kind];
  if (!prompt) {
    return;
  }
  composerInput.value = prompt;
  composerInput.focus();
  setCommandStatus(`${kind} scenario loaded; press Send to run`, { stickyMs: 5000 });
}

function renderDebugDetailsToggle() {
  if (!debugDetailsToggle) {
    return;
  }
  debugDetailsToggle.classList.toggle("is-active", state.debugDetailsVisible);
  debugDetailsToggle.setAttribute("aria-pressed", state.debugDetailsVisible ? "true" : "false");
  debugDetailsToggle.textContent = state.debugDetailsVisible ? "Debug on" : "Debug off";
}

function openSettingsPanel() {
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
if (sessionRenameSelectedButton) {
  sessionRenameSelectedButton.addEventListener("click", () => {
    renameSelectedSession();
  });
}
sessionDeleteSelectedButton.addEventListener("click", () => {
  deleteSelectedSessions();
});
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
      setCommandStatus(`session search failed: ${error.message}`, { stickyMs: 9000 });
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
      setCommandStatus(`tool registry dashboard failed: ${error.message}`, { stickyMs: 9000 });
      renderToolsDashboard();
    });
  });
}
if (toolsDashboardCloseButton) {
  toolsDashboardCloseButton.addEventListener("click", () => {
    toolsDashboardDialog?.close();
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
    showInspectorPanel(state.inspectorPanel === "settings" ? "debug" : "settings");
    renderAll();
  });
}
if (settingsProviderForm) {
  settingsProviderForm.addEventListener("submit", (event) => {
    submitProviderConfigUpdate(event).catch((error) => {
      state.configSaveInFlight = false;
      setText("settings-provider-save-status", `Save failed: ${error.message}`);
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
      setText("settings-provider-switch-status", `Switch failed: ${error.message}`);
      renderSettingsShell();
    });
  });
}
if (settingsProviderWebSearchTestButton) {
  settingsProviderWebSearchTestButton.addEventListener("click", () => {
    testProviderWebSearch().catch((error) => {
      state.providerWebSearchTestInFlight = "";
      setText("settings-provider-web-search-test-status", `Provider web_search test failed: ${error.message}`);
      renderSettingsShell();
    });
  });
}
if (settingsModelGroupForm) {
  settingsModelGroupForm.addEventListener("submit", (event) => {
    submitModelGroupConfigUpdate(event).catch((error) => {
      state.modelGroupSaveInFlight = false;
      setText("settings-model-group-save-status", `Save failed: ${error.message}`);
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
      setText("settings-model-group-switch-status", `Switch failed: ${error.message}`);
      renderSettingsShell();
    });
  });
}
if (settingsApkUpdateCheckButton) {
  settingsApkUpdateCheckButton.addEventListener("click", () => {
    requestAndroidApkUpdateCheck();
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
      setCommandStatus(`new session failed: ${error.message}`, { stickyMs: 8000 });
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
    setCommandStatus(`task target directory selected: ${cwd}`, { stickyMs: 5000 });
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
  setCommandStatus(`attachments ${state.attachmentsPreviewOpen ? "preview visible" : "preview collapsed"}: ${attachmentSummary()}`, { stickyMs: 5000 });
});
refreshSessionButton.addEventListener("click", () => {
  setCommandStatus("refreshing selected session...", { stickyMs: 3000 });
  refreshSelectedSession()
    .then(() => refreshPhase2Status())
    .then(() => setCommandStatus("selected session refreshed", { stickyMs: 4000 }))
    .catch((error) => {
      renderSessionRefreshFailure(error, state.selectedSessionId);
      if (selectedWorkerTranscriptRefreshRetryable()) {
        setCommandStatus("Worker transcript not ready; retrying selected session refresh", { stickyMs: 6000 });
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
      setCommandStatus(`worker control failed: ${error.message}`, { stickyMs: 9000 });
    });
  });
}
modelSelector.addEventListener("change", () => {
  modelSelector.value = "runtime";
  setCommandStatus("model selector is read-only; runtime config owns active model", { stickyMs: 6000 });
});
cwdInput.value = state.selectedCwd;
taskCwdInput.value = state.selectedCwd;
cwdInput.addEventListener("change", () => {
  setSelectedCwd(cwdInput.value);
  setCommandStatus(state.selectedCwd ? `session cwd selected: ${state.selectedCwd}` : "session cwd cleared; runtime default will be used", { stickyMs: 5000 });
  renderAll();
});
taskCwdInput.addEventListener("change", () => {
  setSelectedCwd(taskCwdInput.value);
  setCommandStatus(state.selectedCwd ? `task target directory selected: ${state.selectedCwd}` : "task target directory cleared", { stickyMs: 5000 });
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
  refreshProtocolStateAfterForeground("page restore");
});
window.addEventListener("focus", () => {
  refreshProtocolStateAfterForeground("app focus");
});
window.addEventListener("online", () => {
  refreshProtocolStateAfterForeground("network restore");
});
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible") {
    refreshProtocolStateAfterForeground("app resume");
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
      setCommandStatus("refreshing service state...", { stickyMs: 3000 });
      refreshAllProtocolState()
        .then(() => {
          setCommandStatus("service state refreshed", { stickyMs: 5000 });
        })
        .catch((error) => {
          setCommandStatus(`refresh failed: ${error.message}`, { stickyMs: 8000 });
        });
      return;
    }
    if (usesModifier && event.key.toLowerCase() === "k") {
      event.preventDefault();
      composerInput.focus();
      setCommandStatus("composer focused", { stickyMs: 3000 });
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
      setCommandStatus(`cancel failed: ${error.message}`);
    });
    return;
  }
  if (composerInput.value.trim()) {
    state.rollbackArmedAt = 0;
    composerInput.value = "";
    setCommandStatus("composer cleared", { stickyMs: 3000 });
    return;
  }
  const now = Date.now();
  if (state.rollbackArmedAt && now - state.rollbackArmedAt <= 900) {
    state.rollbackArmedAt = 0;
    rollbackLatestSessionTurn();
    return;
  }
  state.rollbackArmedAt = now;
  setCommandStatus("press Esc again to rollback latest session turn", { stickyMs: 1200 });
});

function installWebUiTestHooks() {
  if (!globalThis.__freehandEnableTestHooks) {
    return;
  }
  globalThis.__freehandWebUiTest = {
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
        title: "Image attachment proof",
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
      return refreshAfterAmbiguousSubmitFailure(new Error(message || "simulated submit failure"));
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
      refreshProtocolStateAfterForeground(reason || "test resume");
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
    setCommandStatus(`startup connection failed: ${error.message}`);
    renderAll();
    scheduleAdpReconnect("startup failure");
  });
