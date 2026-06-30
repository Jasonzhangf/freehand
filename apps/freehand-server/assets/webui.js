import { initializeThemeToggle } from "/assets/theme.js";

initializeThemeToggle(document);

const shell = document.querySelector("[data-webui-shell]");
const messageList = document.getElementById("message-list");
const sessionList = document.getElementById("session-list");
const newSessionButton = document.getElementById("new-session-button");
const sessionCwdInput = document.getElementById("session-cwd-input");
const useCwdButton = document.getElementById("use-cwd-button");
const composerForm = document.getElementById("composer-form");
const composerInput = document.getElementById("composer-input");
const cancelButton = document.getElementById("cancel-button");
const successSampleButton = document.getElementById("success-sample-button");
const failureSampleButton = document.getElementById("failure-sample-button");
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
    "ADP success sample: answer with one short sentence and a valid Freehand completion schema. Do not call tools.",
  failure:
    "ADP failure sample: call the read_file tool exactly once with path definitely-missing-freehand-file.txt, then use the failed tool result to continue and report success through the required Freehand completion schema.",
};

const selectedSessionStorageKey = "freehand-webui-selected-session";
const selectedCwdStorageKey = "freehand-webui-selected-cwd";
const attachmentDraftStorageKey = "freehand-webui-attachment-drafts-v1";
const adpRequestTimeoutMs = 8000;
const shortcutHelp =
  "Shortcuts: Cmd/Ctrl+Enter send · Esc cancel · Cmd/Ctrl+R refresh · Cmd/Ctrl+K focus · Cmd/Ctrl+1 success sample · Cmd/Ctrl+2 failure sample. Slash: /help /new /cwd /sessions /reload /success /failure /cancel /clear /attachments /model";
const initialSelectedSessionId = window.localStorage.getItem(selectedSessionStorageKey) || null;
const initialSelectedCwd = window.localStorage.getItem(selectedCwdStorageKey) || "";

const state = {
  turn: null,
  sessions: [],
  selectedSessionId: initialSelectedSessionId,
  selectedCwd: initialSelectedCwd,
  draftSessionId: initialSelectedSessionId && initialSelectedSessionId.startsWith("webui-session-")
    ? initialSelectedSessionId
    : null,
  sessionTurns: [],
  publicConversation: [],
  debug: null,
  checkpoints: [],
  toolTimings: new Map(),
  modelRequestStartedAt: null,
  modelWaitStartedAt: null,
  activeTurnId: null,
  pendingUserInput: null,
  pendingAttachments: [],
  submitStartedAt: null,
  submitInFlight: false,
  commandStatusMessage: "connecting to ADP...",
  commandStatusStickyUntil: 0,
  adpFailure: null,
  adpStatus: "connecting",
  adpSocket: null,
  adpOpened: null,
  adpRequests: new Map(),
  adpSubscriptions: new Set(),
  requestSequence: 0,
  attachmentDrafts: loadAttachmentDrafts(),
  attachmentsPreviewOpen: true,
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
  setCommandStatus("ADP connecting...");

  state.adpOpened = new Promise((resolve, reject) => {
    socket.addEventListener("open", () => {
      state.adpStatus = "connected";
      setCommandStatus("ADP connected; waiting for subscription...");
      renderAll();
      resolve(socket);
    });
    socket.addEventListener("message", (event) => {
      try {
        handleAdpFrame(JSON.parse(event.data));
      } catch (error) {
        state.adpFailure = `ADP decode failed: ${error.message}`;
        setCommandStatus(state.adpFailure);
        renderAll();
      }
    });
    socket.addEventListener("error", () => {
      state.adpStatus = "error";
      setCommandStatus("ADP transport error");
      renderAll();
      reject(new Error("ADP transport error"));
    });
    socket.addEventListener("close", () => {
      state.adpStatus = "closed";
      setCommandStatus("ADP closed");
      state.adpSocket = null;
      state.adpOpened = null;
      state.adpSubscriptions.clear();
      for (const { reject: rejectRequest } of state.adpRequests.values()) {
        rejectRequest(new Error("ADP closed"));
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
      reject(new Error(`ADP ${kind} request timed out after ${formatDuration(adpRequestTimeoutMs)}`));
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
      setBackgroundCommandStatus(`ADP subscription accepted: ${frame.selector.stream_kind}`);
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
      setCommandStatus(`ADP failure: ${frame.failure.code}`);
      return;
    default:
      setCommandStatus(`unknown ADP frame: ${frame.kind}`);
  }
}

function setText(id, value) {
  const element = document.getElementById(id);
  if (element) {
    element.textContent = value;
  }
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
      id: crypto.randomUUID(),
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

function renderToolBody(container, body) {
  const lines = `${body || ""}`.split("\n").filter((line) => line.length > 0);
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

function pendingSubmitBody(text, attachments = []) {
  const elapsed = elapsedSince(state.submitStartedAt);
  const body = textWithAttachmentPlaceholders(text, attachments);
  return elapsed ? `${body}\n等待调度 ${elapsed}` : body;
}

function turnIsWaitingForModelResponse(turn) {
  return !!(turn && turn.model_request && !turn.terminal_text && !isTerminalStatus(turn.terminal_status));
}

function syncModelRequestTiming(turn) {
  if (!turnIsWaitingForModelResponse(turn)) {
    state.modelRequestStartedAt = null;
    return;
  }
  if (!state.modelRequestStartedAt) {
    state.modelRequestStartedAt = Date.now();
  }
}

function modelRequestBody(turn) {
  const elapsed = elapsedSince(state.modelRequestStartedAt);
  const detail = turn && turn.model_request && turn.model_request.detail
    ? turn.model_request.detail
    : "provider request sent";
  return elapsed ? `${detail}\n等待模型响应 ${elapsed}` : `${detail}\n等待模型响应`;
}

function syncToolTimings(items) {
  const now = Date.now();
  const seen = new Set();

  items.forEach((item) => {
    if (item.kind !== "ToolSummary" || !item.tool_call_id) {
      return;
    }
    seen.add(item.tool_call_id);
    const previous = state.toolTimings.get(item.tool_call_id);
    if (!previous) {
      state.toolTimings.set(item.tool_call_id, {
        startedAt: now,
        finishedAt: null,
        status: item.status,
      });
      return;
    }
    const next = { ...previous, status: item.status };
    if (previous.status !== item.status && (item.status === "completed" || item.status === "failed")) {
      next.finishedAt = now;
    }
    state.toolTimings.set(item.tool_call_id, next);
  });

  for (const toolCallId of Array.from(state.toolTimings.keys())) {
    if (!seen.has(toolCallId)) {
      state.toolTimings.delete(toolCallId);
    }
  }
}

function toolSummaryBody(item) {
  const timing = item.tool_call_id ? state.toolTimings.get(item.tool_call_id) : null;
  const endAt = timing && timing.finishedAt ? timing.finishedAt : Date.now();
  const elapsed = timing ? formatDuration(endAt - timing.startedAt) : "";
  const display = item.display || null;
  const status = `${item.status || ""}`.toLowerCase();
  const lines = [];
  if (status === "waiting") {
    const statusLabel = toolStatusLabel(item.status);
    lines.push(elapsed ? `${statusLabel} · ${elapsed}` : statusLabel);
  }
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
  } else if (item.body && item.body !== item.status) {
    pushCompactToolLine(lines, item.body, item.title);
  }
  if (display && display.result_summary && !display.parameter_summary && !display.diff) {
    const result = compactToolResultLine(display.result_summary, item.title);
    if (result && !lines.some((line) => line.toLowerCase() === result.toLowerCase())) {
      lines.push(result);
    }
  }
  return lines.filter(Boolean).join("\n");
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

function waitingToolStatus(tools) {
  const names = tools.map((tool) => tool.tool_name).join(", ");
  const elapsedValues = tools
    .map((tool) => {
      const timing = tool.tool_call_id ? state.toolTimings.get(tool.tool_call_id) : null;
      return timing ? Date.now() - timing.startedAt : null;
    })
    .filter((elapsed) => Number.isFinite(elapsed));
  const longestElapsed = elapsedValues.length > 0 ? Math.max(...elapsedValues) : null;
  const elapsed = longestElapsed === null ? "" : formatDuration(longestElapsed);
  return elapsed ? `tool executing: ${names} · ${elapsed}` : `tool executing: ${names}`;
}

function turnIsWaitingForModel(turn) {
  if (!turn || turn.terminal_text || isTerminalStatus(turn.terminal_status)) {
    return false;
  }
  const tools = turn.tool_activities || [];
  const hasFinishedTool = tools.some((tool) => {
    const status = `${tool.status || ""}`.toLowerCase();
    return status === "completed" || status === "failed";
  });
  const hasWaitingTool = tools.some((tool) => {
    const status = `${tool.status || ""}`.toLowerCase();
    return status === "waiting";
  });
  return hasFinishedTool && !hasWaitingTool;
}

function syncModelWaitTiming(turn) {
  if (!turnIsWaitingForModel(turn)) {
    state.modelWaitStartedAt = null;
    return;
  }
  if (!state.modelWaitStartedAt) {
    state.modelWaitStartedAt = Date.now();
  }
}

function modelWaitBody() {
  const elapsed = elapsedSince(state.modelWaitStartedAt);
  return elapsed ? `工具结果已返回，等待模型继续推理 ${elapsed}` : "工具结果已返回，等待模型继续推理";
}

function shouldRenderLiveWaitForTurn(turn) {
  if (!turn || !state.turn) {
    return false;
  }
  if (turn.turn_id !== state.turn.turn_id) {
    return false;
  }
  return !turn.terminal_text && !isTerminalStatus(turn.terminal_status);
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
  if (turn.user_text) {
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
      body: turn.terminal_text,
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

function stripFreehandCompletionBlock(text) {
  const stripped = `${text || ""}`
    .replace(/<freehand_completion>[\s\S]*?<\/freehand_completion>/g, "")
    .trim();
  return stripped;
}

function conversationItemsForTurn(turn) {
  return normalizePublicConversation(derivePublicConversation(turn));
}

function logicalTurnKey(turnId) {
  const raw = `${turnId || ""}`;
  const runtimeMatch = raw.match(/^(runtime-turn-\d+)(?:-r\d+)?$/);
  return runtimeMatch ? runtimeMatch[1] : raw;
}

function mergeLogicalTurnGroup(turns) {
  const group = turns.filter(Boolean);
  if (group.length === 0) {
    return null;
  }
  const latest = group[group.length - 1];
  const first = group[0];
  const merged = {
    ...latest,
    turn_id: latest.turn_id,
    user_text: first.user_text || latest.user_text,
    text: [],
    tool_activities: [],
    errors: [],
  };
  const toolById = new Map();
  const assistantBodies = [];
  group.forEach((turn) => {
    (turn.text || []).forEach((text) => {
      const visibleText = stripFreehandCompletionBlock(text);
      if (visibleText) {
        assistantBodies.push(visibleText);
      }
    });
    (turn.tool_activities || []).forEach((tool) => {
      const key = tool.tool_call_id || `${tool.tool_name}:${tool.status}`;
      toolById.set(key, tool);
    });
    (turn.errors || []).forEach((error) => {
      if (error && !merged.errors.includes(error)) {
        merged.errors.push(error);
      }
    });
  });
  merged.text = assistantBodies.length > 0 ? [assistantBodies.join("\n")] : [];
  merged.tool_activities = Array.from(toolById.values());
  return merged;
}

function logicalSessionTurns(turns) {
  const groups = new Map();
  const order = [];
  turns.filter(Boolean).forEach((turn) => {
    const key = logicalTurnKey(turn.turn_id);
    if (!groups.has(key)) {
      groups.set(key, []);
      order.push(key);
    }
    groups.get(key).push(turn);
  });
  return order.map((key) => mergeLogicalTurnGroup(groups.get(key))).filter(Boolean);
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
  if (sessionCwdInput && sessionCwdInput.value !== state.selectedCwd) {
    sessionCwdInput.value = state.selectedCwd;
  }
  if (state.selectedCwd) {
    window.localStorage.setItem(selectedCwdStorageKey, state.selectedCwd);
  } else {
    window.localStorage.removeItem(selectedCwdStorageKey);
  }
}

function selectedWorkspaceCwd() {
  const sidebarCwd = sessionCwdInput ? normalizeCwd(sessionCwdInput.value) : "";
  const composerCwd = cwdInput ? normalizeCwd(cwdInput.value) : "";
  return sidebarCwd || composerCwd || normalizeCwd(state.selectedCwd);
}

function requireWorkspaceCwd(action) {
  const cwd = selectedWorkspaceCwd();
  if (cwd) {
    setSelectedCwd(cwd);
    return cwd;
  }
  setCommandStatus(`${action} requires a workspace directory`, { stickyMs: 6000 });
  (sessionCwdInput || cwdInput || composerInput).focus();
  return "";
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

function newDraftSessionId() {
  const stamp = new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
  return `webui-session-${stamp}-${crypto.randomUUID().slice(0, 8)}`;
}

function startNewSession() {
  const cwd = requireWorkspaceCwd("new session");
  if (!cwd) {
    return;
  }
  const sessionId = newDraftSessionId();
  state.draftSessionId = sessionId;
  state.sessionTurns = [];
  state.turn = null;
  state.publicConversation = [];
  state.pendingUserInput = null;
  state.pendingAttachments = [];
  state.modelRequestStartedAt = null;
  state.modelWaitStartedAt = null;
  state.submitStartedAt = null;
  state.submitInFlight = false;
  setSelectedSessionId(sessionId);
  setSelectedCwd(cwd);
  composerInput.value = "";
  composerInput.focus();
  setCommandStatus(`new session ready · cwd ${cwd}`, { stickyMs: 5000 });
  renderAll();
}

function setSessionList(projection) {
  state.sessions = (projection && projection.sessions) || [];
  if (
    state.selectedSessionId &&
    !isDraftSessionId(state.selectedSessionId) &&
    !state.sessions.some((session) => session.session_id === state.selectedSessionId)
  ) {
    setSelectedSessionId(null);
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

function setSessionTranscript(projection) {
  state.sessionTurns = (projection && projection.turns) || [];
  syncSelectedCwdFromProjection(projection);
  if (projection && projection.session_id && state.sessionTurns.length > 0) {
    setSelectedSessionId(projection.session_id);
    if (state.draftSessionId === projection.session_id) {
      state.draftSessionId = null;
    }
  }
  const latestTurn = state.sessionTurns[state.sessionTurns.length - 1] || null;
  setTurnProjection(latestTurn, { preserveSessionTurns: true });
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

function compareTurnIds(leftTurnId, rightTurnId) {
  const left = turnOrderKey(leftTurnId);
  const right = turnOrderKey(rightTurnId);
  if (left.prefix !== right.prefix) {
    return left.prefix.localeCompare(right.prefix);
  }
  if (left.ordinal !== right.ordinal) {
    return left.ordinal - right.ordinal;
  }
  if (left.round !== right.round) {
    return left.round - right.round;
  }
  return left.raw.localeCompare(right.raw);
}

function setTurnProjection(turn, options = {}) {
  const nextTurnId = turn && turn.turn_id ? turn.turn_id : null;
  if (state.activeTurnId !== nextTurnId) {
    state.toolTimings.clear();
  }
  state.activeTurnId = nextTurnId;
  state.turn = turn || null;
  if (state.turn && !state.selectedSessionId) {
    setSelectedSessionId(state.turn.session_id);
  }
  syncSelectedCwdFromProjection(state.turn);
  if (state.turn && !options.preserveSessionTurns) {
    const existingIndex = state.sessionTurns.findIndex(
      (existing) => existing.turn_id === state.turn.turn_id,
    );
    if (existingIndex >= 0) {
      state.sessionTurns[existingIndex] = state.turn;
    } else if (!state.selectedSessionId || state.turn.session_id === state.selectedSessionId) {
      state.sessionTurns.push(state.turn);
    }
    state.sessionTurns.sort((left, right) => compareTurnIds(left.turn_id, right.turn_id));
  }
  state.publicConversation = derivePublicConversation(state.turn);
  syncToolTimings(state.publicConversation);
  syncModelRequestTiming(state.turn);
  syncModelWaitTiming(state.turn);
  if (state.turn && state.pendingUserInput) {
    state.pendingUserInput = null;
    state.pendingAttachments = [];
  }
}

function applyAdpQueryResult(result) {
  const turn = variantPayload(result, "Turn");
  if (turn !== undefined) {
    if (state.selectedSessionId && turn.session_id !== state.selectedSessionId) {
      renderAll();
      return;
    }
    setTurnProjection(turn);
    renderAll();
    if (state.turn) {
      refreshDebug().catch((error) => {
        setCommandStatus(`debug ADP query failed: ${error.message}`);
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
  }
}

function applyAdpSubscriptionEvent(event) {
  const projection = event.projection || {};
  const turn = variantPayload(projection, "Turn");
  if (turn !== undefined) {
    if (state.selectedSessionId && turn.session_id !== state.selectedSessionId) {
      renderCommandStatus();
      return;
    }
    setTurnProjection(turn);
    setBackgroundCommandStatus("ADP turn update received");
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
  if (!state.turn) {
    return null;
  }

  const waitingTools = (state.turn.tool_activities || []).filter(
    (tool) => tool.status === "Waiting" || tool.status === "waiting",
  );
  if (waitingTools.length > 0) {
    return waitingToolStatus(waitingTools);
  }

  if (turnIsWaitingForModelResponse(state.turn)) {
    const elapsed = elapsedSince(state.modelRequestStartedAt);
    return elapsed ? `waiting for model response... ${elapsed}` : "waiting for model response...";
  }

  if (turnIsWaitingForModel(state.turn)) {
    const elapsed = elapsedSince(state.modelWaitStartedAt);
    return elapsed ? `waiting for model... ${elapsed}` : "waiting for model...";
  }

  if (state.submitInFlight) {
    const elapsed = elapsedSince(state.submitStartedAt);
    return elapsed ? `dispatching... ${elapsed}` : "dispatching...";
  }

  if (state.turn.terminal_text) {
    return "turn completed";
  }

  return null;
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
  messageList.replaceChildren();
  const fragments = [];
  const hasSelectedSessionTranscript = state.sessionTurns.length > 0;

  if (state.pendingUserInput) {
    fragments.push(
      card(
        "User",
        {
          className: state.submitInFlight ? "running" : "pending",
          label: state.submitInFlight ? "dispatching" : "pending",
        },
        state.submitInFlight ? "正在提交输入" : "待写入输入",
        pendingSubmitBody(state.pendingUserInput, state.pendingAttachments),
        "user",
      ),
    );
  }

  if (state.selectedSessionId && !hasSelectedSessionTranscript) {
    // A newly selected draft session should stay visually clean until the user sends.
  } else if (state.sessionTurns.length === 0 && !state.turn) {
    fragments.push(
      card(
        "Assistant",
        { className: "pending", label: "idle" },
        "等待数据",
        "WebUI 正在查询最新 turn。",
        "assistant",
      ),
    );
  } else {
    const turns = state.sessionTurns.length > 0
      ? logicalSessionTurns(state.sessionTurns)
      : state.selectedSessionId
        ? []
        : [state.turn];
    turns.filter(Boolean).forEach((turn) => {
      conversationItemsForTurn(turn).forEach((item) => {
      const variant =
        item.kind === "UserText"
          ? "user"
          : item.kind === "ToolSummary"
            ? "tool"
            : item.kind === "Error"
              ? "failure"
              : "assistant";
      const statusClass =
        item.kind === "Error" || item.status === "failed" || item.status === "cancelled"
          ? "failed"
          : item.kind === "ToolSummary" && item.status === "completed"
            ? "success"
          : item.kind === "Terminal"
            ? "success"
          : item.kind === "ToolSummary"
              ? "running"
              : "success";
      const identity = item.tool_call_id ? `tool:${item.tool_call_id}` : null;
      const body = item.kind === "ToolSummary" ? toolSummaryBody(item) : item.body;
      const role = item.kind === "ToolSummary" ? "Tool" : item.title;
      fragments.push(card(role, { className: statusClass, label: item.status }, item.title, body, variant, identity));
      });
      if (shouldRenderLiveWaitForTurn(turn) && turnIsWaitingForModelResponse(turn)) {
        fragments.push(
          card(
            "Assistant",
            { className: "running", label: "waiting" },
            "等待模型响应",
            modelRequestBody(turn),
            "assistant",
          ),
        );
      }
      if (shouldRenderLiveWaitForTurn(turn) && turnIsWaitingForModel(turn)) {
        fragments.push(
          card(
            "Assistant",
            { className: "running", label: "thinking" },
            "等待模型继续",
            modelWaitBody(),
            "assistant",
          ),
        );
      }
    });
  }

  if (state.adpFailure) {
    fragments.push(
      card(
        "ADP",
        { className: "failed", label: "failed" },
        "ADP failure",
        state.adpFailure,
        "failure",
      ),
    );
  }

  if (fragments.length === 0) {
    const empty = document.createElement("div");
    empty.className = "chat-empty-state";
    empty.textContent = state.selectedSessionId
      ? "New session. Send a message to start."
      : "Waiting for protocol state.";
    fragments.push(empty);
  }

  fragments.forEach((fragment) => messageList.appendChild(fragment));
  scrollMessagesToBottom();
}

function scrollMessagesToBottom() {
  window.requestAnimationFrame(() => {
    messageList.scrollTop = messageList.scrollHeight;
    messageList.lastElementChild?.scrollIntoView({ block: "end" });
    window.scrollTo({ top: document.documentElement.scrollHeight, behavior: "auto" });
  });
}

function isDraftSessionId(sessionId) {
  return !!sessionId && sessionId.startsWith("webui-session-");
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

function renderSessions() {
  if (!sessionList) {
    return;
  }
  sessionList.replaceChildren();
  if (state.sessions.length === 0) {
    const empty = document.createElement("section");
    empty.className = "session-item active";
    appendSessionParts(empty, "empty", "no sessions", "waiting for first turn");
    sessionList.appendChild(empty);
    if (state.draftSessionId && state.selectedSessionId === state.draftSessionId) {
      renderDraftSessionItem();
    }
    return;
  }

  if (state.draftSessionId && !state.sessions.some((session) => session.session_id === state.draftSessionId)) {
    renderDraftSessionItem();
  }

  state.sessions.forEach((session) => {
    const item = document.createElement("button");
    item.className = `session-item session-button${session.session_id === state.selectedSessionId ? " active" : ""}`;
    item.type = "button";
    item.dataset.sessionId = session.session_id;

    const cwd = normalizeCwd(session.cwd);
    const cwdTail = cwd ? ` · ${cwd.split("/").filter(Boolean).slice(-2).join("/") || cwd}` : "";
    const turnText = session.latest_turn_id ? `${session.latest_turn_id} · ${session.turn_count} turn(s)${cwdTail}` : `${session.turn_count} turn(s)${cwdTail}`;
    appendSessionParts(
      item,
      session.active_turn_id ? "active" : session.latest_status || "session",
      session.session_id,
      turnText,
    );

    item.addEventListener("click", () => {
      setSelectedSessionId(session.session_id);
      refreshSelectedSession().catch((error) => {
        setCommandStatus(`session refresh failed: ${error.message}`);
      });
    });
    sessionList.appendChild(item);
  });
}

function renderDraftSessionItem() {
  const item = document.createElement("button");
  item.className = `session-item session-button${state.draftSessionId === state.selectedSessionId ? " active" : ""}`;
  item.type = "button";
  item.dataset.sessionId = state.draftSessionId;
  appendSessionParts(item, "draft", state.draftSessionId, state.selectedCwd ? `cwd ${state.selectedCwd}` : "first send creates session");
  item.addEventListener("click", () => {
    setSelectedSessionId(state.draftSessionId);
    state.sessionTurns = [];
    setTurnProjection(null, { preserveSessionTurns: true });
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

function renderTurnMeta() {
  if (!state.turn) {
    setText("session-title", state.selectedSessionId || "waiting for protocol state");
    setText("session-copy", state.selectedSessionId ? "no turns in selected session" : "no active turn yet");
    setText("strip-session", state.selectedSessionId || "-");
    setText("strip-turn", "-");
    setText("strip-cwd", state.selectedCwd || "-");
    setText("conversation-turn", state.selectedSessionId || "latest active turn");
    setText("turn-status", liveTurnStatus() || "waiting");
    setText("strip-slave", "idle");
    setText("slave-chip", "waiting");
    setText("slave-title", "no slave card yet");
    setText("slave-copy", "当前 turn 还没有 slave 子流。");
    return;
  }

  setText("session-title", state.turn.session_id);
  setText("session-copy", state.turn.cwd ? `${state.turn.turn_id} · ${state.turn.cwd}` : state.turn.turn_id);
  setText("strip-session", state.turn.session_id);
  setText("strip-turn", state.turn.turn_id);
  setText("strip-cwd", state.turn.cwd || state.selectedCwd || "-");
  setText("conversation-turn", state.turn.turn_id);
  const runningTools = (state.turn.tool_activities || []).filter((tool) => tool.status === "Waiting" || tool.status === "waiting");
  const turnStatus = state.turn.terminal_text
    ? "completed"
    : runningTools.length > 0
      ? waitingToolStatus(runningTools).replace("tool executing", "tool running")
      : turnIsWaitingForModelResponse(state.turn)
        ? liveTurnStatus()
      : turnIsWaitingForModel(state.turn)
        ? liveTurnStatus()
      : state.submitInFlight
        ? liveTurnStatus()
        : "streaming";
  setText("turn-status", turnStatus);

  if (state.turn.slave_substream_card) {
    setText("strip-slave", "substream active");
    setText("slave-chip", "active");
    setText("slave-title", "slave substream available");
    setText("slave-copy", "当前 turn 启用了 slave 子流卡片，可继续扩展独立子流显示。");
  } else {
    setText("strip-slave", "idle");
    setText("slave-chip", "idle");
    setText("slave-title", "no slave substream");
    setText("slave-copy", "当前 turn 没有 slave 子流卡片。");
  }
}

setInterval(() => {
  const hasPendingSubmit = state.submitInFlight || !!state.pendingUserInput;
  const hasWaitingTool = (state.publicConversation || []).some(
    (item) => item.kind === "ToolSummary" && item.status === "waiting",
  );
  const hasModelRequestWait = turnIsWaitingForModelResponse(state.turn);
  const hasModelWait = turnIsWaitingForModel(state.turn);
  if (hasPendingSubmit || hasWaitingTool || hasModelRequestWait || hasModelWait) {
    renderMessages();
    renderTurnMeta();
    renderCommandStatus();
  }
}, 1000);

function renderAll() {
  setText("workspace-status", state.adpStatus);
  renderSessions();
  renderTurnMeta();
  renderMessages();
  renderAttachmentTray();
  renderDebug();
  renderCheckpoints();
  renderCommandStatus();
}

async function refreshTurn() {
  const result = await adpQuery("QueryLatestActiveTurn");
  applyAdpQueryResult(result);
  await refreshCheckpoints();
}

async function refreshSessions() {
  const result = await adpQuery("QuerySessionList");
  applyAdpQueryResult(result);
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
      setCommandStatus(`debug ADP query failed: ${error.message}`);
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

async function refreshAllProtocolState() {
  await refreshSessions();
  await refreshSelectedSession();
  await refreshTurn();
  await refreshCheckpoints();
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
    setCommandStatus(`ADP turn subscribe failed: ${error.message}`);
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
        detail_lines: ["waiting for ADP debug subscription"],
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
    state.pendingAttachments = [];
    state.modelRequestStartedAt = null;
    state.modelWaitStartedAt = null;
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
  state.pendingAttachments = [];
  state.modelRequestStartedAt = null;
  state.modelWaitStartedAt = null;
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
    state.pendingAttachments = [];
    state.modelRequestStartedAt = null;
    state.modelWaitStartedAt = null;
    state.submitStartedAt = null;
  }
  switch (command) {
    case "/help":
      setCommandStatus(shortcutHelp, { stickyMs: 10000 });
      return true;
    case "/new":
      startNewSession();
      return true;
    case "/cwd": {
      const cwd = requireWorkspaceCwd("cwd selection");
      if (cwd) {
        setCommandStatus(`workspace cwd selected: ${cwd}`, { stickyMs: 5000 });
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
      setCommandStatus("refreshing protocol state...", { stickyMs: 3000 });
      await refreshAllProtocolState();
      setCommandStatus("protocol state refreshed", { stickyMs: 5000 });
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
      state.pendingAttachments = [];
      state.modelRequestStartedAt = null;
      state.modelWaitStartedAt = null;
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
      setCommandStatus("model selection is controlled by runtime config for this ADP surface", { stickyMs: 6000 });
      return true;
    default:
      if (command.startsWith("/")) {
        setCommandStatus(`unknown slash command: ${command}. ${shortcutHelp}`, { stickyMs: 8000 });
        return true;
      }
      return false;
  }
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
  if (isDraftSessionId(state.selectedSessionId) && !requireWorkspaceCwd("draft session submit")) {
    return;
  }
  const attachments = currentAttachments();
  const commandText = textWithAttachmentPlaceholders(text, attachments);
  state.pendingUserInput = text;
  state.pendingAttachments = attachments;
  state.submitStartedAt = Date.now();
  state.submitInFlight = true;
  composerInput.value = "";
  renderMessages();
  try {
    const receipt = await submitUserInput(commandText);
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
    state.pendingAttachments = [];
    state.submitStartedAt = null;
    composerInput.value = text;
    renderMessages();
    setCommandStatus(`dispatch failed; draft attachments retained for retry: ${error.message}`);
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
  setCommandStatus(`${kind} sample loaded; press Send to run through ADP`, { stickyMs: 5000 });
}

newSessionButton.addEventListener("click", startNewSession);
useCwdButton.addEventListener("click", () => {
  const cwd = requireWorkspaceCwd("workspace selection");
  if (!cwd) {
    return;
  }
  setCommandStatus(`workspace cwd selected for next session: ${cwd}`, { stickyMs: 5000 });
  renderAll();
});
successSampleButton.addEventListener("click", () => loadSamplePrompt("success"));
failureSampleButton.addEventListener("click", () => loadSamplePrompt("failure"));
attachFileButton.addEventListener("click", () => attachmentFileInput.click());
attachImageButton.addEventListener("click", () => attachmentImageInput.click());
attachVideoButton.addEventListener("click", () => attachmentVideoInput.click());
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
sessionCwdInput.value = state.selectedCwd;
cwdInput.addEventListener("change", () => {
  setSelectedCwd(cwdInput.value);
  setCommandStatus(state.selectedCwd ? `session cwd selected: ${state.selectedCwd}` : "session cwd cleared; runtime default will be used", { stickyMs: 5000 });
  renderAll();
});
sessionCwdInput.addEventListener("change", () => {
  setSelectedCwd(sessionCwdInput.value);
  setCommandStatus(state.selectedCwd ? `workspace cwd selected for new session: ${state.selectedCwd}` : "workspace cwd cleared", { stickyMs: 5000 });
  renderAll();
});

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") {
    const usesModifier = event.metaKey || event.ctrlKey;
    if (usesModifier && event.key === "Enter") {
      event.preventDefault();
      composerForm.requestSubmit();
      return;
    }
    if (usesModifier && event.key.toLowerCase() === "r") {
      event.preventDefault();
      setCommandStatus("refreshing protocol state...", { stickyMs: 3000 });
      refreshAllProtocolState()
        .then(() => {
          setCommandStatus("protocol state refreshed", { stickyMs: 5000 });
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
  cancelActiveTurn().catch((error) => {
    setCommandStatus(`cancel failed: ${error.message}`);
  });
});

ensureAdpSocket()
  .then(async () => {
    ensureTurnSubscription();
    await refreshAllProtocolState();
  })
  .catch((error) => {
    setCommandStatus(`ADP bootstrap failed: ${error.message}`);
    renderAll();
  });
