import { initializeThemeToggle } from "/assets/theme.js";

initializeThemeToggle(document);

const shell = document.querySelector("[data-webui-shell]");
const messageList = document.getElementById("message-list");
const composerForm = document.getElementById("composer-form");
const composerInput = document.getElementById("composer-input");
const cancelButton = document.getElementById("cancel-button");

const state = {
  turn: null,
  publicConversation: [],
  debug: null,
  checkpoints: [],
  pendingUserInput: null,
  submitInFlight: false,
  commandStatusMessage: "ready",
  debugEventSource: null,
};

function shellConfig() {
  return {
    turnQuery: shell.dataset.turnQuery,
    turnSubscribe: shell.dataset.turnSubscribe,
    debugQueryBase: shell.dataset.debugQueryBase,
    debugSubscribeBase: shell.dataset.debugSubscribeBase,
    checkpointQuery: shell.dataset.checkpointQuery,
    commandEndpoint: shell.dataset.commandEndpoint,
  };
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
  setText("workspace-status", state.turn ? "connected" : "booting");
  renderTurnMeta();
  renderMessages();
  renderDebug();
  renderCheckpoints();
  renderCommandStatus();
}

async function fetchJson(url) {
  const response = await fetch(url);
  if (!response.ok) {
    const error = new Error(`${url} -> ${response.status}`);
    error.status = response.status;
    throw error;
  }
  return response.json();
}

async function refreshTurn() {
  const config = shellConfig();
  let payload;
  try {
    payload = await fetchJson(config.turnQuery);
  } catch (error) {
    if (error.status === 404) {
      state.turn = null;
      state.publicConversation = [];
      renderAll();
      await refreshCheckpoints();
      return;
    }
    throw error;
  }
  state.turn = payload.turn;
  state.publicConversation = payload.public_conversation || [];
  if (state.pendingUserInput) {
    state.pendingUserInput = null;
  }
  renderAll();
  await refreshDebug();
  await refreshCheckpoints();
  ensureDebugSubscription();
}

async function refreshDebug() {
  if (!state.turn) {
    state.debug = null;
    renderDebug();
    return;
  }
  const config = shellConfig();
  try {
    state.debug = await fetchJson(`${config.debugQueryBase}${state.turn.turn_id}`);
  } catch (error) {
    if (error.status === 404) {
      state.debug = {
        status_text: "debug pending",
        detail_lines: ["waiting for debug snapshot"],
      };
      renderDebug();
      return;
    }
    throw error;
  }
  renderDebug();
}

async function refreshCheckpoints() {
  const payload = await fetchJson(shellConfig().checkpointQuery);
  state.checkpoints = payload.checkpoints || [];
  renderCheckpoints();
}

function ensureTurnSubscription() {
  const source = new EventSource(shellConfig().turnSubscribe);
  source.addEventListener("turn", (event) => {
    const payload = JSON.parse(event.data);
    state.turn = payload.turn;
    state.publicConversation = payload.public_conversation || [];
    state.pendingUserInput = null;
    state.commandStatusMessage = "reasoning stream active...";
    renderAll();
    refreshDebug().catch((error) => {
      state.commandStatusMessage = `debug refresh failed: ${error.message}`;
      renderCommandStatus();
    });
    ensureDebugSubscription();
  });
}

function ensureDebugSubscription() {
  if (!state.turn) {
    return;
  }
  if (state.debugEventSource) {
    state.debugEventSource.close();
  }
  const source = new EventSource(`${shellConfig().debugSubscribeBase}${state.turn.turn_id}`);
  source.addEventListener("debug", (event) => {
    state.debug = JSON.parse(event.data);
    renderDebug();
  });
  source.onerror = () => {
    const status = state.debug?.status_text || "";
    if (!state.debug || status === "debug pending" || status === "debug stream waiting") {
      state.debug = {
        status_text: "debug stream reconnecting",
        detail_lines: ["waiting for debug SSE reconnect"],
      };
      renderDebug();
    }
  };
  state.debugEventSource = source;
}

async function submitUserInput(text) {
  const response = await fetch(shellConfig().commandEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ SubmitUserInput: { text } }),
  });
  const payload = await response.json();
  if (!response.ok) {
    throw new Error(payload.message || payload.code || "command failed");
  }
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
  const response = await fetch(shellConfig().commandEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(command),
  });
  const payload = await response.json();
  if (!response.ok) {
    state.commandStatusMessage = `cancel failed: ${payload.message || payload.code || "command failed"}`;
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
  const response = await fetch(shellConfig().commandEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ RewindCheckpoint: { checkpoint_id: checkpointId } }),
  });
  const payload = await response.json();
  if (!response.ok) {
    state.commandStatusMessage = `rewind failed: ${payload.message || payload.code || "command failed"}`;
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

refreshTurn().catch((error) => {
  state.commandStatusMessage = `bootstrap failed: ${error.message}`;
  renderAll();
});
refreshCheckpoints().catch(() => {
  state.checkpoints = [];
  renderCheckpoints();
});
ensureTurnSubscription();
