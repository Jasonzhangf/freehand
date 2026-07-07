import { initializeThemeToggle } from "/assets/theme.js";

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
const closeDetailDrawerButton = document.getElementById("close-detail-drawer-button");
const settingsShellToggle = document.getElementById("settings-shell-toggle");
const inspectorEyebrow = document.getElementById("inspector-eyebrow");
const inspectorTitle = document.getElementById("inspector-title");
const inspectorCopy = document.getElementById("inspector-copy");
const inspectorDebugPanel = document.getElementById("inspector-debug-panel");
const settingsShell = document.getElementById("settings-shell");
const settingsProviderForm = document.getElementById("settings-provider-form");
const settingsProviderIdInput = document.getElementById("settings-provider-id-input");
const settingsProviderTypeInput = document.getElementById("settings-provider-type-input");
const settingsProviderProtocolInput = document.getElementById("settings-provider-protocol-input");
const settingsProviderUrlInput = document.getElementById("settings-provider-url-input");
const settingsProviderModelInput = document.getElementById("settings-provider-model-input");
const settingsProviderEnvInput = document.getElementById("settings-provider-env-input");
const settingsProviderSaveButton = document.getElementById("settings-provider-save-button");
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
    "Call the read_file tool exactly once with path definitely-missing-freehand-file.txt, then use the failed tool result to continue and report success through the required Freehand completion schema.",
};

const selectedSessionStorageKey = "freehand-webui-selected-session";
const selectedCwdStorageKey = "freehand-webui-selected-cwd";
const attachmentDraftStorageKey = "freehand-webui-attachment-drafts-v1";
const adpRequestTimeoutMs = 8000;
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

const state = {
  turn: null,
  sessions: [],
  sessionListLoaded: false,
  selectedSessionIds: new Set(),
  expandedAgentIds: new Set(["master"]),
  selectedSessionId: initialSelectedSessionId,
  selectedCwd: initialSelectedCwd,
  draftSessionId: null,
  sessionTurns: [],
  publicConversation: [],
  debug: null,
  checkpoints: [],
  configStatus: null,
  configStatusError: null,
  configSaveInFlight: false,
  toolTimings: new Map(),
  lifecycleClocks: new Map(),
  pendingUserInput: null,
  pendingSubmitId: null,
  pendingSubmitSessionId: null,
  pendingAttachments: [],
  inputHistory: [],
  inputHistoryIndex: null,
  mobileDrawer: null,
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
  sseTurnStream: null,
  requestSequence: 0,
  attachmentDrafts: loadAttachmentDrafts(),
  attachmentsPreviewOpen: true,
  debugDetailsVisible: false,
  forceScrollToBottom: false,
  rollbackArmedAt: 0,
  composerFocused: false,
  newSessionKind: "conversation",
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
    openDetailDrawerButton.setAttribute("aria-expanded", drawer === "details" ? "true" : "false");
  }
  if (openSettingsDrawerButton) {
    openSettingsDrawerButton.setAttribute("aria-expanded", drawer === "settings" ? "true" : "false");
  }
  if (mobileDrawerScrim) {
    mobileDrawerScrim.setAttribute("aria-hidden", drawer ? "false" : "true");
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
  applyMobileDrawerState();
}

function closeMobileDrawer() {
  setMobileDrawer(null);
}

function syncMobileDrawerForLayout() {
  if (!isMobileDrawerLayout(document.body.dataset.layoutShape || applyLayoutShape())) {
    state.mobileDrawer = null;
  }
  applyMobileDrawerState();
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

function adpCommand(command) {
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
      setBackgroundCommandStatus(`${frame.receipt.dispatch_status} -> ${frame.receipt.target_feature_id}`);
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
      setCommandStatus(`unknown service message: ${frame.kind}`);
  }
}

function setText(id, value) {
  const element = document.getElementById(id);
  if (element) {
    element.textContent = value;
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

function persistAttachmentDrafts() {
  const entries = Array.from(state.attachmentDrafts.entries()).map(([sessionId, attachments]) => ({
    session_id: sessionId,
    attachments: attachments.map(({ file, ...metadata }) => ({
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
      file,
    });
  });
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
      file: null,
    });
  });
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

function attachmentPlaceholderLines(attachments = currentAttachments()) {
  if (attachments.length === 0) {
    return [];
  }
  const lines = ["[attachments: current-send placeholders]"];
  attachments.forEach((attachment) => {
    const availability = attachment.available ? "ready" : "metadata-only";
    lines.push(
      `- ${attachment.kind}: ${attachment.name} (${formatBytes(attachment.size)}, ${attachment.type || "unknown"}, ${availability})`,
    );
  });
  return lines;
}

function textWithAttachmentPlaceholders(text, attachments = currentAttachments()) {
  const placeholders = attachmentPlaceholderLines(attachments);
  if (placeholders.length === 0) {
    return text;
  }
  return `${text}\n\n${placeholders.join("\n")}`;
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
    chip.className = `attachment-chip ${attachment.available ? "ready" : "metadata-only"}`;

    const text = document.createElement("span");
    text.className = "attachment-chip-text";
    text.textContent = `${attachment.kind} · ${attachment.name} · ${formatBytes(attachment.size)}`;
    text.title = attachment.available
      ? "This page still holds the File handle for retry."
      : "Metadata restored from session; reselect the file before sending binary payload.";

    const remove = document.createElement("button");
    remove.className = "attachment-remove";
    remove.type = "button";
    remove.textContent = "Remove";
    remove.addEventListener("click", () => removeAttachment(attachment.id));

    chip.append(text, remove);
    list.appendChild(chip);
  });
  attachmentTray.appendChild(list);
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
  if (turn.terminal_text || isTerminalStatus(turn.terminal_status)) {
    const terminal = `${turn.terminal_status || "success"}`.toLowerCase();
    const phase = terminal === "success" ? "completed" : terminal;
    return {
      phase,
      className: terminal === "failed" || terminal === "cancelled" ? "failed" : "success",
      label: terminal === "failed" ? "failed" : "completed",
      isLive: false,
      elapsed: "",
    };
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
    body: [textWithAttachmentPlaceholders(renderPending.text, renderPending.attachments)],
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
  const userRow = {
    kind: "user",
    title: "User",
    body: [textWithAttachmentPlaceholders(renderPending.text, renderPending.attachments)],
    status: "submitted",
  };
  const assistantRows = [{
    kind: "system",
    title: "Client",
    body: ["Request accepted. Waiting for service dispatch."],
    status: elapsed || "0s",
  }];
  const renderTurn = {
    turnId: "pending-submit",
    lifecycle: {
      className: renderPending.isLive ? "running" : "pending",
      label: elapsed ? `dispatching... ${elapsed}` : "dispatching",
      isLive: renderPending.isLive,
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

  const body = document.createElement("div");
  body.className = "chat-message-body";
  renderTextLines(body, row.body || []);

  article.append(meta, body);
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
  article.appendChild(meta);

  rows.forEach((row) => {
    article.appendChild(chatAssistantSection(row));
  });
  return article;
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
  if (rows.some((row) => row.kind === "final")) {
    return "completed";
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
    return "Final";
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
  section.classList.add(toolStateClass(row.status));
  const display = row.display || null;
  const head = document.createElement("div");
  head.className = "tool-chat-head";
  const title = document.createElement("span");
  title.className = "tool-chat-title";
  title.textContent = row.title || (display && display.action) || "Tool";
  const state = document.createElement("span");
  state.className = `tool-chat-state ${toolStateClass(row.status)}`;
  state.textContent = row.status || "";
  head.append(title, state);

  const body = document.createElement("div");
  body.className = "tool-chat-body";
  const semanticLines = toolSemanticLines(row);
  semanticLines.forEach((line, index) => {
    const item = document.createElement("div");
    item.className = [
      line.kind === "command" ? "tool-command-line" : "tool-chat-line",
      index === 0 ? "tool-chat-line-primary" : "tool-chat-line-secondary",
    ].join(" ");
    item.textContent = line.text;
    item.title = line.fullText || line.text;
    body.appendChild(item);
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
  const display = row.display || null;
  const lines = [];
  const action = `${(display && display.action) || row.title || "Tool"}`.trim();
  const target = display && display.target ? `${display.target}`.trim() : "";
  const parameterSummary = display && display.parameter_summary ? `${display.parameter_summary}`.trim() : "";
  const resultSummary = display && display.result ? `${display.result}`.trim() : "";
  const command = toolDisplayField(display, "command");

  if (target) {
    lines.push({ text: `${action} · ${target}` });
  } else if (parameterSummary) {
    lines.push({ text: `${action} · ${parameterSummary}` });
  } else {
    lines.push({ text: action });
  }

  if (command) {
    lines.push({
      kind: "command",
      text: truncateForChat(command, 180),
      fullText: command,
    });
  }

  if (!command && resultSummary && !isGenericToolResult(resultSummary)) {
    lines.push({ text: resultSummary });
  }

  if (!command && !resultSummary && parameterSummary && target && shouldShowToolParameterSummary(parameterSummary, target)) {
    lines.push({ text: parameterSummary });
  }

  compactToolBodyLines(row, command).forEach((bodyLine) => {
    if (bodyLine && !lines.some((line) => line.text === bodyLine)) {
      lines.push({ text: bodyLine });
    }
  });

  return lines.length > 0 ? lines : [{ text: row.status || "tool activity" }];
}

function compactToolBodyLines(row, command) {
  const rawLines = Array.isArray(row.body) ? row.body : [];
  return rawLines
    .map((bodyLine) => `${bodyLine || ""}`.trim())
    .filter(Boolean)
    .filter((bodyLine) => {
      if (command && (bodyLine.startsWith("command=") || bodyLine === command)) {
        return false;
      }
      if (/^(type|target|path|command)\s*[:=]/i.test(bodyLine)) {
        return false;
      }
      return true;
    })
    .slice(0, 2);
}

function isGenericToolResult(text) {
  const normalized = `${text || ""}`.trim().toLowerCase();
  return (
    normalized === "" ||
    normalized === "success" ||
    normalized === "completed" ||
    normalized === "result returned" ||
    normalized === "succeeded: result returned"
  );
}

function shouldShowToolParameterSummary(summary, target) {
  const normalized = `${summary || ""}`.trim();
  const normalizedTarget = `${target || ""}`.trim();
  if (!normalized || normalized === normalizedTarget) {
    return false;
  }
  if (/^(type|target|path|command)\s*[:=]/i.test(normalized)) {
    return false;
  }
  if (normalizedTarget && normalized.includes(normalizedTarget) && normalized.split(/[,\n;]/).length <= 2) {
    return false;
  }
  return true;
}

function toolDisplayField(display, label) {
  if (!display || !Array.isArray(display.fields)) {
    return "";
  }
  const field = display.fields.find((candidate) => candidate.label === label);
  return field ? `${field.value || ""}` : "";
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
  const normalized = [];
  const toolIndex = new Map();

  items.forEach((item) => {
    if (item.kind !== "ToolSummary" || !item.tool_call_id) {
      normalized.push(item);
      return;
    }

    const existingIndex = toolIndex.get(item.tool_call_id);
    if (existingIndex === undefined) {
      toolIndex.set(item.tool_call_id, normalized.length);
      normalized.push(item);
      return;
    }

    normalized[existingIndex] = item;
  });

  return normalized;
}

function buildConversationRenderModel() {
  const conversationTurns = conversationTurnsForRender();
  const turnsForRender =
    conversationTurns.length === 0 && state.turn
      ? [state.turn]
      : conversationTurns;
  const successorBaseTurns = successorBaseTurnIds(turnsForRender);
  return {
    selectedSessionId: state.selectedSessionId,
    turns: turnsForRender.map((turn) =>
      buildRenderTurn(turn, {
        hideUser: turnOrderKey(turn.turn_id).round > 1,
        hideTerminal: successorBaseTurns.has(turn.turn_id || ""),
      }),
    ),
    pendingSubmit: state.pendingUserInput
      ? {
          text: state.pendingUserInput,
          attachments: state.pendingAttachments,
          isLive: state.submitInFlight,
          elapsed: elapsedSince(state.submitStartedAt),
        }
      : null,
    adpFailure: state.adpFailure ? { message: state.adpFailure } : null,
  };
}

function successorBaseTurnIds(turns) {
  const baseIds = new Set(
    turns
      .filter((turn) => turnOrderKey(turn && turn.turn_id).round > 1)
      .map((turn) => baseTurnId(turn.turn_id))
      .filter(Boolean),
  );
  return new Set(
    turns
      .filter((turn) => baseIds.has(turn && turn.turn_id))
      .map((turn) => turn.turn_id),
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
  const lifecycle = turnLifecycleForRender(turn);
  return {
    turnId: turn.turn_id || "",
    sessionId: turn.session_id || "",
    orderKey: turnOrderKey(turn.turn_id),
    lifecycle,
    rows: buildRenderRows(turn, lifecycle, options),
  };
}

function buildRenderRows(turn, lifecycle, options = {}) {
  const rows = conversationItemsForTurn(turn, { hideUser: !!options.hideUser }).map((item) =>
    buildRenderRowFromConversationItem(turn, item),
  ).filter((row) => !(options.hideTerminal && row.kind === "final"));
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
  if (!lifecycle.isLive || !turnIsWaitingForModelResponse(turn) || !turn.model_request) {
    return null;
  }
  const label = modelRequestLabel(turn);
  return {
    kind: "system",
    title: label === "schema polishing" ? "Schema" : "Model",
    body: [turn.model_request.detail || "Waiting for model response."],
    status: lifecycle.elapsed || "0s",
    identity: { turnId: turn.turn_id },
  };
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
  if (turn.terminal_text || isTerminalStatus(turn.terminal_status)) {
    return "completed";
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
  return !!(turn && turn.model_request && !turn.terminal_text && !isTerminalStatus(turn.terminal_status));
}

function turnIsCurrentLiveTurn(turn) {
  return !!(
    turn &&
    state.turn &&
    turn.turn_id === state.turn.turn_id &&
    turn.session_id === state.turn.session_id &&
    !turn.terminal_text &&
    !isTerminalStatus(turn.terminal_status)
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
  const request = turn.model_request || {};
  return [turn.session_id || "", turn.turn_id || "", modelRequestKind(turn), request.detail || ""].join("|");
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
    lines.push(`diff: ${display.diff.target}`);
    lines.push(`- ${display.diff.before}`);
    lines.push(`+ ${display.diff.after}`);
  } else if (display && display.parameter_summary) {
    pushCompactToolLine(lines, display.parameter_summary, item.title);
  } else if (display && display.summary) {
    pushCompactToolLine(lines, display.summary, item.title);
  } else if (display && Array.isArray(display.fields) && display.fields.length > 0) {
    const compactFields = display.fields
      .slice(0, 4)
      .map((field) => `${field.label}: ${field.value}`)
      .join(" · ");
    pushCompactToolLine(lines, compactFields, item.title);
  } else if (item.body && item.body !== item.status && `${item.status || ""}`.toLowerCase() !== "waiting") {
    pushCompactToolLine(lines, item.body, item.title);
  }
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

function pushCompactToolLine(lines, value, title = "") {
  const line = compactToolResultLine(value, title);
  if (!line) {
    return;
  }
  if (!lines.some((existing) => existing.toLowerCase() === line.toLowerCase())) {
    lines.push(line);
  }
}

function compactToolResultLine(value, title = "") {
  const text = `${value || ""}`.trim();
  if (!text) {
    return "";
  }
  const lower = text.toLowerCase();
  if (lower === "succeeded: result returned" || lower === "succeeded: shell command") {
    return "";
  }
  return text
    .replace(/^result:\s*/i, "")
    .replace(/^succeeded:\s*/i, "")
    .replace(/^failure:\s*/i, "")
    .replace(new RegExp(`^${escapeRegExp(title)}:\\s*`, "i"), "")
    .trim();
}

function escapeRegExp(value) {
  return `${value || ""}`.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
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
      body: turn.user_text,
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
    const status =
      terminalStatus === "failed"
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
      title: "Final",
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

function conversationItemsForTurn(turn, options = {}) {
  const items = normalizePublicConversation(derivePublicConversation(turn));
  if (options.hideUser) {
    return items.filter((item) => item.kind !== "UserText");
  }
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

function clearPendingUserInputIfMaterialized() {
  if (!pendingUserInputIsMaterialized()) {
    return;
  }
  state.pendingUserInput = null;
  state.pendingSubmitId = null;
  state.pendingSubmitSessionId = null;
  state.pendingAttachments = [];
}

function sameRenderableTurn(left, right) {
  if (!left || !right || left.turn_id !== right.turn_id) {
    return false;
  }
  const leftInternal = isInternalRuntimePrompt(left);
  const rightInternal = isInternalRuntimePrompt(right);
  if (leftInternal || rightInternal) {
    return leftInternal && rightInternal;
  }
  return normalizeVisibleText(left.user_text) === normalizeVisibleText(right.user_text);
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
  state.selectedSessionId = sessionId || null;
  if (state.selectedSessionId) {
    window.localStorage.setItem(selectedSessionStorageKey, state.selectedSessionId);
  } else {
    window.localStorage.removeItem(selectedSessionStorageKey);
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
      startNewTask();
    } else {
      startNewConversation();
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
  startNewConversation();
}

function sessionSummaryForSelected() {
  if (!state.selectedSessionId) {
    return null;
  }
  return state.sessions.find((session) => session.session_id === state.selectedSessionId) || null;
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
  const stamp = new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
  return `webui-session-${stamp}-${browserRandomId().slice(0, 8)}`;
}

function resetLocalConversationState(sessionId) {
  state.draftSessionId = sessionId;
  state.sessionTurns = [];
  state.turn = null;
  state.publicConversation = [];
  state.debug = null;
  state.adpFailure = null;
  state.pendingUserInput = null;
  state.pendingSubmitId = null;
  state.pendingSubmitSessionId = null;
  state.pendingAttachments = [];
  state.lifecycleClocks.clear();
  state.toolTimings.clear();
  state.submitStartedAt = null;
  state.submitInFlight = false;
  setSelectedSessionId(sessionId);
  composerInput.value = "";
  composerInput.focus();
  renderAll();
}

function startNewConversation() {
  const sessionId = newDraftSessionId();
  resetLocalConversationState(sessionId);
  setSelectedCwd("");
  closeMobileDrawer();
  setCommandStatus("new conversation ready", { stickyMs: 5000 });
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
  state.sessions = (projection && projection.sessions) || [];
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
    !state.sessions.some((session) => session.session_id === state.selectedSessionId)
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
    if (session && session.session_id && !isDraftSessionId(session.session_id)) {
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
  if (projection && projection.session_id && !sessionTruthAllowsSessionId(projection.session_id)) {
    if (state.selectedSessionId === projection.session_id) {
      clearLocalConversationTruth();
    }
    return;
  }
  state.sessionTurns = logicalSessionTurns((projection && projection.turns) || []);
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
  if (!options.preserveSelectedSession) {
    setSelectedSessionId(null);
  }
  state.sessionTurns = [];
  state.turn = null;
  state.publicConversation = [];
  state.debug = null;
  state.adpFailure = null;
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
  return state.sessions.some((session) => session.session_id === sessionId);
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
}

function applyAdpQueryResult(result) {
  const turn = variantPayload(result, "Turn");
  if (turn !== undefined) {
    if (turn && !sessionTruthAllowsTurn(turn)) {
      renderAll();
      return;
    }
    if (state.selectedSessionId && turn.session_id !== state.selectedSessionId) {
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
    renderSettingsShell();
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
  if (state.submitInFlight && !state.turn) {
    const elapsed = elapsedSince(state.submitStartedAt);
    return elapsed ? `dispatching... ${elapsed}` : "dispatching...";
  }
  const turn = activeTurnForSelectedSession();
  if (!turn) {
    return null;
  }

  const waitingTools = (turn.tool_activities || []).filter(
    (tool) => tool.status === "Waiting" || tool.status === "waiting",
  );
  if (waitingTools.length > 0) {
    return waitingToolStatus(waitingTools);
  }

  if (turnIsWaitingForModelResponse(turn)) {
    const elapsed = elapsedSince(lifecycleClockStartedAt(modelRequestTimingKey(turn)));
    const label = modelRequestLabel(turn);
    return elapsed ? `${label}... ${elapsed}` : `${label}...`;
  }

  if (state.submitInFlight) {
    const elapsed = elapsedSince(state.submitStartedAt);
    return elapsed ? `dispatching... ${elapsed}` : "dispatching...";
  }

  if (turn.terminal_text) {
    return "turn completed";
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
  const shouldStickToBottom = state.forceScrollToBottom || messageListIsNearBottom();
  state.forceScrollToBottom = false;
  messageList.replaceChildren();
  const fragments = [];
  syncToolTimings(conversationTurnsForRender());
  syncRenderLifecycleClocks();
  const renderModel = buildConversationRenderModel();
  const hasSelectedSessionTranscript = renderModel.turns.length > 0;

  if (renderModel.turns.length > 0) {
    renderModel.turns.forEach((renderTurn) => {
      fragments.push(...turnChatCards(renderTurn));
    });
  }

  if (renderModel.pendingSubmit) {
    fragments.push(...pendingChatCards(renderModel.pendingSubmit));
  }

  if (renderModel.adpFailure) {
    fragments.push(failureChatBubble(renderModel.adpFailure.message));
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

  uniqueChatFragments(fragments).forEach((fragment) => messageList.appendChild(fragment));
  if (shouldStickToBottom) {
    scrollMessagesToBottom();
  }
}

function uniqueChatFragments(fragments) {
  const seen = new Set();
  let previousAssistantText = "";
  return fragments.filter((fragment) => {
    if (!fragment || !fragment.classList || !fragment.classList.contains("chat-message")) {
      previousAssistantText = "";
      return true;
    }
    const text = normalizeVisibleText(fragment.innerText);
    const isAssistant = fragment.classList.contains("chat-message-assistant");
    if (isAssistant && text && text === previousAssistantText) {
      return false;
    }
    const key = `${fragment.dataset.turnId || ""}:${text}`;
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    previousAssistantText = isAssistant ? text : "";
    return true;
  });
}

function messageListIsNearBottom() {
  const listRemaining = messageList.scrollHeight - messageList.scrollTop - messageList.clientHeight;
  if (messageList.scrollHeight > messageList.clientHeight + 2 && listRemaining >= 96) {
    return false;
  }
  const pageRemaining =
    document.documentElement.scrollHeight - window.scrollY - window.innerHeight;
  return pageRemaining < 160;
}

function scrollMessagesToBottom() {
  window.requestAnimationFrame(() => {
    const streamStage = document.querySelector(".stream-stage");
    if (streamStage) {
      streamStage.scrollTop = streamStage.scrollHeight;
    }
    messageList.scrollTop = messageList.scrollHeight;
    messageList.lastElementChild?.scrollIntoView({ block: "end" });
  });
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

function normalizeAgentId(value) {
  const agentId = `${value || ""}`.trim();
  return agentId || "master";
}

function sessionAgentId(session) {
  if (!session) {
    return "master";
  }
  const explicit = session.agent_id || session.source_agent_id || session.agent || session.source_agent;
  if (explicit) {
    return normalizeAgentId(explicit);
  }
  if (state.turn && state.turn.session_id === session.session_id && state.turn.source) {
    return normalizeAgentId(state.turn.source.source_agent_id);
  }
  const turn = state.sessionTurns.find((candidate) => candidate.session_id === session.session_id && candidate.source);
  return normalizeAgentId(turn && turn.source && turn.source.source_agent_id);
}

function agentDisplayName(agentId) {
  const normalized = normalizeAgentId(agentId);
  if (normalized === "master") {
    return "Master";
  }
  return normalized;
}

function sessionKindLabel(session) {
  return normalizeCwd(session && session.cwd) ? "task" : "global";
}

function groupedSessionsByAgent(sessions) {
  const groups = new Map();
  (sessions || []).forEach((session) => {
    const agentId = sessionAgentId(session);
    if (!groups.has(agentId)) {
      groups.set(agentId, []);
    }
    groups.get(agentId).push(session);
  });
  return Array.from(groups.entries()).map(([agentId, groupSessions]) => ({
    agentId,
    sessions: groupSessions,
  }));
}

function ensureSessionAgentExpanded(agentId) {
  state.expandedAgentIds.add(normalizeAgentId(agentId));
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
  appendSessionParts(
    button,
    session.active_turn_id ? "active" : `${sessionKindLabel(session)} · ${session.latest_status || "session"}`,
    session.title || session.session_id,
    turnText,
  );

  button.addEventListener("click", () => {
    setSelectedSessionId(session.session_id);
    closeMobileDrawer();
    refreshSelectedSession().catch((error) => {
      setCommandStatus(`session refresh failed: ${error.message}`);
    });
  });
  item.append(selector, button);
  return item;
}

function renderSessionAgentGroup(group) {
  const selectedInGroup = group.sessions.some((session) => session.session_id === state.selectedSessionId);
  if (selectedInGroup) {
    ensureSessionAgentExpanded(group.agentId);
  }
  const section = document.createElement("section");
  section.className = "session-agent-group";
  section.dataset.agentId = group.agentId;
  section.dataset.expanded = state.expandedAgentIds.has(group.agentId) ? "true" : "false";

  const button = document.createElement("button");
  button.className = "session-agent-button";
  button.type = "button";
  button.setAttribute("aria-expanded", section.dataset.expanded);
  const taskCount = group.sessions.filter((session) => normalizeCwd(session.cwd)).length;
  const globalCount = group.sessions.length - taskCount;
  button.innerHTML = `
    <span class="session-agent-main">
      <span class="session-agent-chevron" aria-hidden="true">›</span>
      <span class="session-agent-name"></span>
    </span>
    <span class="session-agent-count"></span>
  `;
  button.querySelector(".session-agent-name").textContent = agentDisplayName(group.agentId);
  button.querySelector(".session-agent-count").textContent =
    `${group.sessions.length} session(s) · ${taskCount} task · ${globalCount} global`;
  button.addEventListener("click", () => {
    if (state.expandedAgentIds.has(group.agentId)) {
      state.expandedAgentIds.delete(group.agentId);
    } else {
      state.expandedAgentIds.add(group.agentId);
    }
    renderSessions();
  });

  const sessionsNode = document.createElement("div");
  sessionsNode.className = "session-agent-sessions";
  group.sessions.forEach((session) => sessionsNode.appendChild(renderSessionItem(session)));

  section.append(button, sessionsNode);
  return section;
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

  groupedSessionsByAgent(state.sessions).forEach((group) => {
    sessionList.appendChild(renderSessionAgentGroup(group));
  });
  renderSessionBulkToolbar();
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
    inspectorEyebrow.textContent = showingSettings ? "settings" : "detail region";
  }
  if (inspectorTitle) {
    inspectorTitle.textContent = showingSettings ? "Provider Settings" : "Selected Turn Debug";
  }
  if (inspectorCopy) {
    inspectorCopy.textContent = showingSettings
      ? "Edit provider endpoint, model, and credential environment variable. Runtime status lives under Status."
      : "右侧只展示当前 turn 的调试摘要，不抢主消息流焦点。";
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
  setText("settings-provider-auth", state.configStatus ? `${settingsAuthTypeLabel(state.configStatus.provider_auth_type)} · ${state.configStatus.provider_auth_source}` : "loading");
  setText("settings-restart-required", state.configStatus?.restart_required_on_change ? "restart required after changes" : "no restart flag");
  setText("settings-config-error", state.configStatusError || "none");
  syncSettingsProviderForm();
  showInspectorPanel(state.inspectorPanel);
}

function settingsAuthTypeLabel(authType) {
  return authType === "apikey" ? "credential" : (authType || "credential");
}

function syncSettingsProviderForm() {
  const status = state.configStatus;
  setInputValueIfNotFocused(settingsProviderIdInput, status?.provider_id || "");
  setInputValueIfNotFocused(settingsProviderTypeInput, status?.provider_type || "openai");
  setInputValueIfNotFocused(settingsProviderProtocolInput, status?.provider_protocol || "responses");
  setInputValueIfNotFocused(settingsProviderModelInput, status?.default_model || "");
  if (settingsProviderSaveButton) {
    settingsProviderSaveButton.disabled = state.configSaveInFlight;
    settingsProviderSaveButton.textContent = state.configSaveInFlight ? "Saving..." : "Save provider config";
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
    api_key_env: settingsProviderEnvInput?.value.trim() || "",
  };
  state.configSaveInFlight = true;
  setText("settings-provider-save-status", "Saving config...");
  renderSettingsShell();
  try {
    const receipt = await adpCommand({ UpdateProviderConfig: { update } });
    setCommandStatus(providerConfigReceiptStatus(receipt), { stickyMs: 5000 });
    await refreshConfigStatus();
    setText("settings-provider-save-status", "Saved. Restart required before active runtime changes.");
  } catch (error) {
    state.configStatusError = error.message;
    setText("settings-provider-save-status", `Save failed: ${error.message}`);
    renderSettingsShell();
  } finally {
    state.configSaveInFlight = false;
    renderSettingsShell();
  }
}

function renderTurnMeta() {
  const turn = activeTurnForSelectedSession();
  if (!turn) {
    setText("session-title", state.selectedSessionId || "waiting for service state");
    setText("session-copy", state.selectedSessionId ? "no turns in selected session" : "no active turn yet");
    setShellDataset("selectedSession", state.selectedSessionId || "");
    setShellDataset("selectedTurn", "");
    setShellDataset("selectedCwd", state.selectedCwd || "");
    setText("turn-status", liveTurnStatus() || "waiting");
    return;
  }

  setText("session-title", turn.session_id);
  setText("session-copy", turn.cwd ? `${turn.turn_id} · ${turn.cwd}` : turn.turn_id);
  setShellDataset("selectedSession", turn.session_id || "");
  setShellDataset("selectedTurn", turn.turn_id || "");
  setShellDataset("selectedCwd", turn.cwd || state.selectedCwd || "");
  const runningTools = (turn.tool_activities || []).filter((tool) => tool.status === "Waiting" || tool.status === "waiting");
  const turnStatus = turn.terminal_text
    ? "completed"
    : runningTools.length > 0
      ? waitingToolStatus(runningTools).replace("tool executing", "tool running")
      : turnIsWaitingForModelResponse(turn)
        ? liveTurnStatus()
        : state.submitInFlight
          ? liveTurnStatus()
          : "waiting";
  setText("turn-status", turnStatus);
}

setInterval(() => {
  if (renderModelHasLiveLifecycle()) {
    renderMessages();
    renderTurnMeta();
    renderCommandStatus();
  }
}, 1000);

function renderModelHasLiveLifecycle() {
  if (state.submitInFlight || !!state.pendingUserInput) {
    return true;
  }
  return buildConversationRenderModel().turns.some((turn) => turn.lifecycle.isLive);
}

function renderAll() {
  setText("workspace-status", state.adpStatus);
  renderDebugDetailsToggle();
  renderSessions();
  renderTurnMeta();
  renderMessages();
  renderAttachmentTray();
  renderDebug();
  renderCheckpoints();
  renderSettingsShell();
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
    setTurnProjection(null, { preserveSessionTurns: true });
    renderAll();
    return;
  }
  const result = await adpQuery({
    QuerySessionTurns: { session_id: state.selectedSessionId },
  });
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
  }
}

async function refreshAllProtocolState() {
  await refreshSessions();
  await refreshSelectedSession();
  if (!state.selectedSessionId && !state.sessionListLoaded) {
    await refreshTurn();
  }
  await refreshCheckpoints();
  await refreshConfigStatus();
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

async function submitUserInput(text) {
  const command = { SubmitUserInput: { text } };
  if (state.selectedSessionId) {
    command.SubmitUserInput.session_id = state.selectedSessionId;
  }
  const cwd = normalizeCwd(state.selectedCwd);
  if (cwd) {
    command.SubmitUserInput.cwd = cwd;
  }
  const payload = await adpCommand(command);
  setCommandStatus(`${payload.dispatch_status} -> ${payload.target_feature_id}`);
  return payload;
}

function activeTurnId() {
  return state.turn && state.turn.turn_id ? state.turn.turn_id : null;
}

async function cancelActiveTurn() {
  const turnId = activeTurnId();
  if (!turnId && !state.submitInFlight && !state.pendingUserInput) {
    composerInput.value = "";
    state.pendingUserInput = null;
    state.pendingSubmitId = null;
    state.pendingSubmitSessionId = null;
    state.pendingAttachments = [];
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
  let payload;
  try {
    payload = await adpCommand(command);
  } catch (error) {
    setCommandStatus(`cancel failed: ${error.message}`);
    return;
  }
  state.pendingUserInput = null;
  state.pendingSubmitId = null;
  state.pendingSubmitSessionId = null;
  state.pendingAttachments = [];
  state.lifecycleClocks.clear();
  state.submitStartedAt = null;
  state.submitInFlight = false;
  composerInput.value = "";
  setCommandStatus(`${payload.dispatch_status} -> ${payload.target_feature_id}`);
  await refreshTurn().catch((error) => {
    setCommandStatus(`${payload.dispatch_status} -> ${payload.target_feature_id} (turn refresh failed: ${error.message})`);
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
  setCommandStatus(`${payload.dispatch_status} -> ${payload.target_feature_id}`);
  await refreshCheckpoints();
}

async function runSlashCommand(rawText) {
  const command = rawText.trim();
  if (command.startsWith("/")) {
    composerInput.value = "";
    state.pendingUserInput = null;
    state.pendingSubmitId = null;
    state.pendingSubmitSessionId = null;
    state.pendingAttachments = [];
    state.lifecycleClocks.clear();
    state.submitStartedAt = null;
  }
  switch (command) {
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
      state.pendingAttachments = [];
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
      if (command.startsWith("/")) {
        setCommandStatus(`unknown slash command: ${command}. ${shortcutHelp}`, { stickyMs: 8000 });
        return true;
      }
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
  if (!text) {
    setCommandStatus("empty input rejected", { stickyMs: 3000 });
    return;
  }
  try {
    if (await runSlashCommand(text)) {
      return;
    }
  } catch (error) {
    setCommandStatus(`slash command failed: ${error.message}`, { stickyMs: 8000 });
    return;
  }
  setCommandStatus("dispatching...");
  const attachments = currentAttachments();
  const commandText = textWithAttachmentPlaceholders(text, attachments);
  rememberInputHistory(text);
  state.pendingUserInput = text;
  state.pendingSubmitId = null;
  state.pendingSubmitSessionId = state.selectedSessionId;
  state.pendingAttachments = attachments;
  state.submitStartedAt = Date.now();
  state.submitInFlight = true;
  composerInput.value = "";
  state.forceScrollToBottom = true;
  renderMessages();
  try {
    const receipt = await submitUserInput(commandText);
    state.pendingSubmitId = receipt && receipt.ingress ? receipt.ingress.submit_id : null;
    clearCurrentAttachments();
    state.submitInFlight = false;
    state.submitStartedAt = null;
    state.pendingAttachments = [];
    try {
      await refreshAllProtocolState();
      renderCommandStatus();
    } catch (error) {
      setCommandStatus(`${receipt.dispatch_status} -> ${receipt.target_feature_id} (turn refresh failed: ${error.message})`);
    }
  } catch (error) {
    state.submitInFlight = false;
    state.pendingUserInput = null;
    state.pendingSubmitId = null;
    state.pendingSubmitSessionId = null;
    state.pendingAttachments = [];
    state.submitStartedAt = null;
    composerInput.value = "";
    renderMessages();
    setCommandStatus(`dispatch failed; use ↑ to recall input. Draft attachments retained: ${error.message}`);
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

function openDebugPanel() {
  showInspectorPanel("debug");
  setMobileDrawer("details");
  renderAll();
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
    setMobileDrawer(state.mobileDrawer === "sessions" ? null : "sessions");
  });
}
if (closeSessionDrawerButton) {
  closeSessionDrawerButton.addEventListener("click", closeMobileDrawer);
}
if (openDetailDrawerButton) {
  openDetailDrawerButton.addEventListener("click", () => {
    if (state.mobileDrawer === "details" && state.inspectorPanel === "debug") {
      closeMobileDrawer();
      return;
    }
    openDebugPanel();
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
if (closeDetailDrawerButton) {
  closeDetailDrawerButton.addEventListener("click", closeMobileDrawer);
}
if (mobileDrawerScrim) {
  mobileDrawerScrim.addEventListener("click", closeMobileDrawer);
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
    .then(() => setCommandStatus("selected session refreshed", { stickyMs: 4000 }))
    .catch((error) => setCommandStatus(`selected session refresh failed: ${error.message}`, { stickyMs: 8000 }));
});
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
syncMobileDrawerForLayout();
installMobileSessionSwipeGesture();
window.addEventListener("resize", () => {
  applyLayoutShape();
  syncMobileDrawerForLayout();
});
window.addEventListener("orientationchange", () => {
  applyLayoutShape();
  syncMobileDrawerForLayout();
});
if (window.visualViewport) {
  window.visualViewport.addEventListener("resize", () => {
    applyLayoutShape();
    syncMobileDrawerForLayout();
  });
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
  if (state.mobileDrawer) {
    closeMobileDrawer();
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

ensureAdpSocket()
  .then(async () => {
    ensureTurnSubscription();
    ensureSseTurnSubscription();
    await refreshAllProtocolState();
  })
  .catch((error) => {
    setCommandStatus(`startup connection failed: ${error.message}`);
    renderAll();
  });
