import { initializeThemeToggle } from "/assets/theme.js";

initializeThemeToggle(document);

const shell = document.querySelector("[data-webui-shell]");
const messageList = document.getElementById("message-list");
const composerForm = document.getElementById("composer-form");
const composerInput = document.getElementById("composer-input");
const cancelButton = document.getElementById("cancel-button");
const successSampleButton = document.getElementById("success-sample-button");
const failureSampleButton = document.getElementById("failure-sample-button");

const samplePrompts = {
  success:
    "ADP success sample: answer with one short sentence and a valid Freehand completion schema. Do not call tools.",
  failure:
    "ADP failure sample: call the ls tool exactly once with path ~/code/codex, then report through the required Freehand completion schema.",
};

const state = {
  turn: null,
  publicConversation: [],
  debug: null,
  checkpoints: [],
  pendingUserInput: null,
  submitInFlight: false,
  commandStatusMessage: "connecting to ADP...",
  adpFailure: null,
  adpStatus: "connecting",
  adpSocket: null,
  adpOpened: null,
  adpRequests: new Map(),
  adpSubscriptions: new Set(),
  requestSequence: 0,
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
  state.commandStatusMessage = "ADP connecting...";
  renderCommandStatus();

  state.adpOpened = new Promise((resolve, reject) => {
    socket.addEventListener("open", () => {
      state.adpStatus = "connected";
      state.commandStatusMessage = "ADP connected; waiting for subscription...";
      renderAll();
      resolve(socket);
    });
    socket.addEventListener("message", (event) => {
      try {
        handleAdpFrame(JSON.parse(event.data));
      } catch (error) {
        state.adpFailure = `ADP decode failed: ${error.message}`;
        state.commandStatusMessage = state.adpFailure;
        renderAll();
      }
    });
    socket.addEventListener("error", () => {
      state.adpStatus = "error";
      state.commandStatusMessage = "ADP transport error";
      renderAll();
      reject(new Error("ADP transport error"));
    });
    socket.addEventListener("close", () => {
      state.adpStatus = "closed";
      state.commandStatusMessage = "ADP closed";
      state.adpSocket = null;
      state.adpOpened = null;
      state.adpSubscriptions.clear();
      for (const { reject: rejectRequest } of state.adpRequests.values()) {
        rejectRequest(new Error("ADP closed"));
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
    state.adpRequests.set(requestId, { resolve, reject, kind });
  });
  sendAdpFrame(frame).catch((error) => {
    const request = state.adpRequests.get(requestId);
    state.adpRequests.delete(requestId);
    if (request) {
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

function handleAdpFrame(frame) {
  const request = state.adpRequests.get(frame.request_id);
  switch (frame.kind) {
    case "query_result":
      state.adpFailure = null;
      if (request) {
        state.adpRequests.delete(frame.request_id);
        request.resolve(frame.result);
      }
      return;
    case "command_receipt":
      state.adpFailure = null;
      if (request) {
        state.adpRequests.delete(frame.request_id);
        request.resolve(frame.receipt);
      }
      state.commandStatusMessage = `${frame.receipt.dispatch_status} -> ${frame.receipt.target_feature_id}`;
      renderCommandStatus();
      return;
    case "subscription_accepted":
      state.adpFailure = null;
      if (request) {
        state.adpRequests.delete(frame.request_id);
        request.resolve(frame.selector);
      }
      state.adpSubscriptions.add(frame.request_id);
      state.commandStatusMessage = `ADP subscription accepted: ${frame.selector.stream_kind}`;
      renderCommandStatus();
      return;
    case "subscription_event":
      state.adpFailure = null;
      applyAdpSubscriptionEvent(frame.event);
      return;
    case "failure":
      state.adpFailure = frame.failure.message || frame.failure.code;
      if (request) {
        state.adpRequests.delete(frame.request_id);
        request.reject(new Error(frame.failure.message || frame.failure.code));
      }
      state.commandStatusMessage = `ADP failure: ${frame.failure.code}`;
      renderCommandStatus();
      return;
    default:
      state.commandStatusMessage = `unknown ADP frame: ${frame.kind}`;
      renderCommandStatus();
  }
}

function setText(id, value) {
  const element = document.getElementById(id);
  if (element) {
    element.textContent = value;
  }
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
  stateBadge.textContent = status.label;

  head.append(roleBadge, stateBadge);

  const content = document.createElement("div");
  content.className = `${variant === "tool" || variant === "failure" ? "tool-kind" : "text-kind"} block-body`;

  const titleNode = document.createElement("div");
  titleNode.className = "block-title";
  titleNode.textContent = title;
  content.appendChild(titleNode);

  const bodyNode = document.createElement("div");
  bodyNode.textContent = body;
  content.appendChild(bodyNode);

  article.append(head, content);
  return article;
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
  (turn.text || []).forEach((text) => {
    if (text && text.trim()) {
      items.push({
        kind: "AssistantText",
        title: "Assistant",
        body: text,
        status: "streaming",
      });
    }
  });
  (turn.tool_activities || []).forEach((tool) => {
    const status = `${tool.status || "waiting"}`.toLowerCase();
    const failedDetail = tool.detail ? `: ${tool.detail}` : "";
    const body =
      status === "completed"
        ? `Tool result returned for ${tool.tool_name}`
        : status === "failed"
          ? `Tool execution failed for ${tool.tool_name}${failedDetail}`
          : `Tool call requested: ${tool.tool_name} (waiting for execution)`;
    items.push({
      kind: "ToolSummary",
      title: "Tool",
      body,
      status,
      tool_call_id: tool.tool_call_id,
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

function setTurnProjection(turn) {
  state.turn = turn || null;
  state.publicConversation = derivePublicConversation(state.turn);
  if (state.turn && state.pendingUserInput) {
    state.pendingUserInput = null;
  }
}

function applyAdpQueryResult(result) {
  const turn = variantPayload(result, "Turn");
  if (turn !== undefined) {
    setTurnProjection(turn);
    renderAll();
    if (state.turn) {
      refreshDebug().catch((error) => {
        state.commandStatusMessage = `debug ADP query failed: ${error.message}`;
        renderCommandStatus();
      });
    }
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
    setTurnProjection(turn);
    state.commandStatusMessage = "ADP turn update received";
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
  if (!state.turn) {
    return null;
  }

  if (state.submitInFlight) {
    return "dispatching...";
  }

  const waitingTools = (state.turn.tool_activities || []).filter(
    (tool) => tool.status === "Waiting" || tool.status === "waiting",
  );
  if (waitingTools.length > 0) {
    return `tool executing: ${waitingTools.map((tool) => tool.tool_name).join(", ")}`;
  }

  if (state.turn.terminal_text) {
    return "turn completed";
  }

  return null;
}

function renderCommandStatus() {
  const liveStatus = liveTurnStatus();
  setText("command-status", liveStatus || state.commandStatusMessage);
}

function renderMessages() {
  messageList.replaceChildren();
  const fragments = [];

  if (state.pendingUserInput) {
    fragments.push(
      card(
        "User",
        { className: "pending", label: "pending" },
        "待写入输入",
        state.pendingUserInput,
        "user",
      ),
    );
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

  if (!state.turn) {
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
    normalizePublicConversation(state.publicConversation).forEach((item) => {
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
      fragments.push(card(item.title, { className: statusClass, label: item.status }, item.title, item.body, variant, identity));
    });
  }

  if (fragments.length === 0) {
    fragments.push(
      card(
        "Assistant",
        { className: "pending", label: "idle" },
        "等待内容",
        "当前 turn 暂无可显示语义内容。",
        "assistant",
      ),
    );
  }

  fragments.forEach((fragment) => messageList.appendChild(fragment));
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
    setText("session-title", "waiting for protocol state");
    setText("session-copy", "no active turn yet");
    setText("strip-session", "-");
    setText("strip-turn", "-");
    setText("conversation-turn", "latest active turn");
    setText("turn-status", "waiting");
    setText("strip-slave", "idle");
    setText("slave-chip", "waiting");
    setText("slave-title", "no slave card yet");
    setText("slave-copy", "当前 turn 还没有 slave 子流。");
    return;
  }

  setText("session-title", state.turn.session_id);
  setText("session-copy", state.turn.turn_id);
  setText("strip-session", state.turn.session_id);
  setText("strip-turn", state.turn.turn_id);
  setText("conversation-turn", state.turn.turn_id);
  const runningTools = (state.turn.tool_activities || []).filter((tool) => tool.status === "Waiting" || tool.status === "waiting");
  const turnStatus = state.turn.terminal_text
    ? "completed"
    : runningTools.length > 0
      ? `tool running: ${runningTools.map((tool) => tool.tool_name).join(", ")}`
      : state.submitInFlight
        ? "dispatching"
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

function renderAll() {
  setText("workspace-status", state.adpStatus);
  renderTurnMeta();
  renderMessages();
  renderDebug();
  renderCheckpoints();
  renderCommandStatus();
}

async function refreshTurn() {
  const result = await adpQuery("QueryLatestActiveTurn");
  applyAdpQueryResult(result);
  await refreshCheckpoints();
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

function ensureTurnSubscription() {
  if (state.adpSubscriptions.has("latest-turn")) {
    return;
  }
  state.adpSubscriptions.add("latest-turn");
  adpSubscribe(
    { SubscribeLatestActiveTurn: { client: adpClientKind() } },
    "sub-turn",
  ).catch((error) => {
    state.commandStatusMessage = `ADP turn subscribe failed: ${error.message}`;
    renderCommandStatus();
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
  const payload = await adpCommand({ SubmitUserInput: { text } });
  state.commandStatusMessage = `${payload.dispatch_status} -> ${payload.target_feature_id}`;
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
    state.commandStatusMessage = "no active turn; input cleared";
    renderMessages();
    renderCommandStatus();
    return;
  }
  const command = turnId
    ? { CancelTurn: { turn_id: turnId } }
    : { CancelLatestActiveTurn: {} };
  state.commandStatusMessage = `cancelling ${turnId || "latest active turn"}...`;
  renderCommandStatus();
  let payload;
  try {
    payload = await adpCommand(command);
  } catch (error) {
    state.commandStatusMessage = `cancel failed: ${error.message}`;
    renderCommandStatus();
    return;
  }
  state.pendingUserInput = null;
  composerInput.value = "";
  state.commandStatusMessage = `${payload.dispatch_status} -> ${payload.target_feature_id}`;
  await refreshTurn().catch((error) => {
    state.commandStatusMessage =
      `${payload.dispatch_status} -> ${payload.target_feature_id} (turn refresh failed: ${error.message})`;
    renderCommandStatus();
  });
}

async function rewindCheckpoint(checkpointId) {
  state.commandStatusMessage = `rewinding ${checkpointId}...`;
  renderCommandStatus();
  let payload;
  try {
    payload = await adpCommand({ RewindCheckpoint: { checkpoint_id: checkpointId } });
  } catch (error) {
    state.commandStatusMessage = `rewind failed: ${error.message}`;
    renderCommandStatus();
    return;
  }
  state.commandStatusMessage = `${payload.dispatch_status} -> ${payload.target_feature_id}`;
  await refreshCheckpoints();
  renderCommandStatus();
}

composerForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const text = composerInput.value.trim();
  if (!text) {
    state.commandStatusMessage = "empty input rejected";
    renderCommandStatus();
    return;
  }
  state.commandStatusMessage = "dispatching...";
  state.pendingUserInput = text;
  state.submitInFlight = true;
  composerInput.value = "";
  renderMessages();
  renderCommandStatus();
  try {
    const receipt = await submitUserInput(text);
    state.submitInFlight = false;
    try {
      await refreshTurn();
      await refreshCheckpoints();
      renderCommandStatus();
    } catch (error) {
      state.commandStatusMessage =
        `${receipt.dispatch_status} -> ${receipt.target_feature_id} (turn refresh failed: ${error.message})`;
      renderCommandStatus();
    }
  } catch (error) {
    state.submitInFlight = false;
    state.pendingUserInput = null;
    renderMessages();
    state.commandStatusMessage = `dispatch failed: ${error.message}`;
    renderCommandStatus();
  }
});

cancelButton.addEventListener("click", () => {
  cancelActiveTurn().catch((error) => {
    state.commandStatusMessage = `cancel failed: ${error.message}`;
    renderCommandStatus();
  });
});

function loadSamplePrompt(kind) {
  const prompt = samplePrompts[kind];
  if (!prompt) {
    return;
  }
  composerInput.value = prompt;
  composerInput.focus();
  state.commandStatusMessage = `${kind} sample loaded; press Send to run through ADP`;
  renderCommandStatus();
}

successSampleButton.addEventListener("click", () => loadSamplePrompt("success"));
failureSampleButton.addEventListener("click", () => loadSamplePrompt("failure"));

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") {
    return;
  }
  event.preventDefault();
  cancelActiveTurn().catch((error) => {
    state.commandStatusMessage = `cancel failed: ${error.message}`;
    renderCommandStatus();
  });
});

ensureAdpSocket()
  .then(async () => {
    ensureTurnSubscription();
    await refreshTurn();
    await refreshCheckpoints();
  })
  .catch((error) => {
    state.commandStatusMessage = `ADP bootstrap failed: ${error.message}`;
    renderAll();
  });
