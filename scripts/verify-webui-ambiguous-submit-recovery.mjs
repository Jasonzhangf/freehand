import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const chromePath =
  process.env.FREEHAND_WEBUI_CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_DEBUG_PORT || '9238', 10);
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WEBUI_BASE_URL || 'http://127.0.0.1:4042/');
const fixedSessionId =
  process.env.FREEHAND_WEBUI_AMBIGUOUS_SESSION || 'webui-ambiguous-submit-recovery-fixed';
const fixedPrompt =
  process.env.FREEHAND_WEBUI_AMBIGUOUS_PROMPT || 'fixed ambiguous submit recovery prompt';
const artifactDir =
  process.env.FREEHAND_WEBUI_AMBIGUOUS_ARTIFACT_DIR ||
  path.join(process.cwd(), 'artifacts', 'webui-online', 'ambiguous-submit-recovery-fixed');

await fs.mkdir(artifactDir, { recursive: true });

const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-webui-ambiguous-'));
let chrome;

try {
  chrome = spawn(
    chromePath,
    [
      '--headless=new',
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${profileDir}`,
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-background-networking',
      '--disable-extensions',
      '--disable-sync',
      '--window-size=1400,1000',
      baseUrl,
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );
  chrome.stdout.on('data', () => {});
  chrome.stderr.on('data', () => {});

  const target = await waitPageTarget();
  const cdp = await createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: 'window.__freehandEnableTestHooks = true;',
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitFor(
    () =>
      evalPage(
        cdp,
        () => !!document.querySelector('[data-webui-shell="true"]') && !!document.getElementById('composer-input'),
      ),
    20_000,
    'WebUI shell',
  );

  const result = await evalPage(cdp, runAmbiguousSubmitRecoveryProof, fixedSessionId, fixedPrompt);
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(result, null, 2));
  if (
    !result.checks.materializedClearsPending ||
    !result.checks.taskTruthClearsPending ||
    !result.checks.unverifiedKeepsPendingSession
  ) {
    throw new Error(`ambiguous submit recovery proof failed: ${JSON.stringify(result.checks)}`);
  }
  console.log(
    `webui_ambiguous_submit_recovery_ok session=${fixedSessionId} artifact=${path.join(artifactDir, 'summary.json')}`,
  );
  await cdp.close();
} finally {
  if (chrome?.pid && chrome.exitCode === null) {
    chrome.kill('SIGTERM');
    await new Promise((resolve) => chrome.once('exit', resolve));
  }
  await fs.rm(profileDir, { recursive: true, force: true }).catch(() => {});
}

function runAmbiguousSubmitRecoveryProof(sessionId, prompt) {
  const makeSession = () => ({
    session_id: sessionId,
    title: 'Fixed ambiguous submit recovery',
    active_turn_id: 'runtime-turn-ambiguous-fixed',
    archived: false,
  });
  const makeTurn = () => ({
    session_id: sessionId,
    turn_id: 'runtime-turn-ambiguous-fixed',
    user_text: prompt,
    text: ['owner truth materialized after ambiguous submit failure'],
    tool_activities: [],
    terminal_text: 'owner truth materialized after service refresh',
    terminal_status: 'Success',
  });
  const makeAcceptedTask = () => ({
    task_id: 'task-ambiguous-submit-accepted',
    status: 'closed',
    title: 'Accepted through TaskBoard truth',
    goal: 'prove same-parent task truth clears ambiguous submit',
    priority: 50,
    target_cwd: '/tmp/freehand-webui-ambiguous',
    parent_session_id: sessionId,
    attached_session_ids: [],
    worker_session_id: 'worker-task-task-ambiguous-submit-accepted',
    assignee_agent_id: 'worker',
    active_execution_id: null,
    created_at: Math.floor(Date.now() / 1000),
    updated_at: Math.floor(Date.now() / 1000),
    last_progress_at: Math.floor(Date.now() / 1000),
    last_event_seq: 9,
  });
  const phase2Empty = (queryName) => {
    switch (queryName) {
      case 'QueryTaskBoard':
        return { TaskBoard: { tasks: [] } };
      case 'QueryAgentBoard':
        return { AgentBoard: { agents: [] } };
      case 'QueryEventInbox':
        return { EventInbox: { events: [], cursor: null } };
      case 'QueryTaskHistory':
        return { TaskHistory: { task_id: '', events: [] } };
      case 'QueryWorkerControl':
        return { WorkerControl: { task_id: '', events: [] } };
      default:
        return null;
    }
  };
  const queryName = (query) => (typeof query === 'string' ? query : Object.keys(query || {})[0] || '');
  const hook = window.__freehandWebUiTest;
  const resetPendingState = () => hook.resetAmbiguousSubmitState(sessionId, prompt);
  const capture = () => hook.captureAmbiguousSubmitState();
  const runWithQuery = async (mode) => {
    const calls = [];
    hook.setAdpQueryForTest(async (query) => {
      const name = queryName(query);
      calls.push(name);
      if (name === 'QuerySessionList') {
        return { SessionList: { sessions: [makeSession()] } };
      }
      if (name === 'QuerySessionTurns') {
        return { SessionTurns: { session_id: sessionId, turns: mode === 'turn' ? [makeTurn()] : [] } };
      }
      if (name === 'QueryLatestActiveTurn') {
        return { Turn: mode === 'turn' ? makeTurn() : null };
      }
      if (name === 'QueryTaskBoard') {
        return { TaskBoard: { tasks: mode === 'task' ? [makeAcceptedTask()] : [] } };
      }
      return phase2Empty(name) || {};
    });
    return {
      recovery: await hook.refreshAfterAmbiguousSubmitFailure('simulated receipt timeout'),
      calls,
      state: capture(),
    };
  };

  return (async () => {
    if (!hook) {
      throw new Error('WebUI test hook is not available');
    }

    resetPendingState();
    const materialized = await runWithQuery('turn');
    hook.renderAll();
    const materializedAfterRender = capture();

    resetPendingState();
    const taskTruth = await runWithQuery('task');
    hook.renderAll();
    const taskTruthAfterRender = capture();

    resetPendingState();
    const unverified = await runWithQuery('none');
    if (!unverified.recovery.materialized) {
      hook.markPendingSubmitError(unverified.recovery.message);
    }
    const unverifiedAfterRender = capture();

    return {
      sessionId,
      materialized: {
        recovery: materialized.recovery,
        calls: materialized.calls,
        afterRender: materializedAfterRender,
      },
      taskTruth: {
        recovery: taskTruth.recovery,
        calls: taskTruth.calls,
        afterRender: taskTruthAfterRender,
      },
      unverified: {
        recovery: unverified.recovery,
        calls: unverified.calls,
        afterRender: unverifiedAfterRender,
      },
      checks: {
        materializedClearsPending:
          materialized.recovery.materialized === true &&
          materializedAfterRender.pendingUserInput === null &&
          materializedAfterRender.pendingSubmitCardCount === 0 &&
          materializedAfterRender.selectedSession === sessionId &&
          materializedAfterRender.messageText.includes(prompt),
        taskTruthClearsPending:
          taskTruth.recovery.materialized === true &&
          taskTruthAfterRender.pendingUserInput === null &&
          taskTruthAfterRender.pendingSubmitCardCount === 0 &&
          taskTruthAfterRender.selectedSession === sessionId &&
          taskTruthAfterRender.pendingSubmitAcceptedByTaskTruth === false &&
          taskTruthAfterRender.acceptedSubmitReceipt?.taskId === 'task-ambiguous-submit-accepted' &&
          taskTruthAfterRender.messageText.includes('Service accepted this request through TaskBoard truth') &&
          taskTruthAfterRender.messageText.includes('task-ambiguous-submit-accepted') &&
          !taskTruthAfterRender.messageText.includes('New conversation') &&
          !taskTruthAfterRender.turnStatus.includes('unknown') &&
          !taskTruthAfterRender.messageText.includes('unknown'),
        unverifiedKeepsPendingSession:
          unverified.recovery.materialized === false &&
          unverifiedAfterRender.pendingUserInput === prompt &&
          unverifiedAfterRender.pendingSubmitCardCount >= 1 &&
          unverifiedAfterRender.selectedSession === sessionId &&
          unverifiedAfterRender.turnStatus.includes('checking service truth') &&
          unverifiedAfterRender.messageText.includes(prompt) &&
          unverifiedAfterRender.messageText.includes('Submit receipt is being verified') &&
          !unverifiedAfterRender.messageText.includes('unknown') &&
          !unverifiedAfterRender.messageText.includes('New conversation'),
      },
    };
  })();
}

async function waitPageTarget() {
  return await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
    if (!response.ok) {
      return null;
    }
    const targets = await response.json();
    return targets.find((target) => target.type === 'page' && String(target.url || '').startsWith(baseUrl));
  }, 20_000, 'Chrome DevTools page target');
}

function createCdpClient(webSocketUrl) {
  const socket = new WebSocket(webSocketUrl);
  const pending = new Map();
  const listeners = new Set();
  let nextId = 0;
  return new Promise((resolve, reject) => {
    socket.addEventListener('open', () => {
      resolve({
        send(method, params = {}) {
          const id = ++nextId;
          socket.send(JSON.stringify({ id, method, params }));
          return new Promise((resolveSend, rejectSend) => {
            pending.set(id, { resolve: resolveSend, reject: rejectSend });
          });
        },
        onEvent(listener) {
          listeners.add(listener);
        },
        offEvent(listener) {
          listeners.delete(listener);
        },
        async close() {
          socket.close();
        },
      });
    });
    socket.addEventListener('message', (event) => {
      const payload = JSON.parse(event.data);
      if (payload.id) {
        const entry = pending.get(payload.id);
        if (!entry) {
          return;
        }
        pending.delete(payload.id);
        if (payload.error) {
          entry.reject(new Error(payload.error.message || 'CDP error'));
        } else {
          entry.resolve(payload.result || {});
        }
        return;
      }
      if (payload.method) {
        listeners.forEach((listener) => listener(payload.method, payload.params || {}));
      }
    });
    socket.addEventListener('error', () => {
      reject(new Error('CDP socket error'));
    });
  });
}

async function waitForLoad(cdp) {
  await new Promise((resolve) => {
    const listener = (method) => {
      if (method === 'Page.loadEventFired') {
        cdp.offEvent(listener);
        resolve();
      }
    };
    cdp.onEvent(listener);
  });
}

async function waitFor(fn, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const value = await fn();
      if (value) {
        return value;
      }
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw new Error(`timeout waiting for ${label}${lastError ? `: ${lastError.message}` : ''}`);
}

async function evalPage(cdp, fn, ...args) {
  const response = await cdp.send('Runtime.evaluate', {
    expression: `(${fn})(...${JSON.stringify(args)})`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    throw new Error(response.exceptionDetails.text || 'Runtime.evaluate failed');
  }
  return response.result.value;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function normalizedBaseUrl(value) {
  const parsed = new URL(value);
  if (!parsed.pathname.endsWith('/')) {
    parsed.pathname = `${parsed.pathname}/`;
  }
  return parsed.toString();
}
