import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const chromePath =
  process.env.FREEHAND_WEBUI_CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_DEBUG_PORT || '9238', 10);
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WEBUI_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_WEBUI_AMBIGUOUS_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const fixedSessionId =
  process.env.FREEHAND_WEBUI_AMBIGUOUS_SESSION || 'webui-ambiguous-submit-recovery-fixed';
const fixedAttachmentSessionId =
  process.env.FREEHAND_WEBUI_ATTACHMENT_FAILURE_SESSION || 'webui-attachment-failure-retain-fixed';
const fixedPrompt =
  process.env.FREEHAND_WEBUI_AMBIGUOUS_PROMPT || 'fixed ambiguous submit recovery prompt';
const attachmentFailurePrompt =
  process.env.FREEHAND_WEBUI_ATTACHMENT_FAILURE_PROMPT || 'fixed attachment failure retain proof prompt';
const taskCwd = process.env.FREEHAND_WEBUI_ATTACHMENT_FAILURE_CWD || process.cwd();
const assetVersion = '20260725-settings-layer-ui';
const artifactDir =
  process.env.FREEHAND_WEBUI_AMBIGUOUS_ARTIFACT_DIR ||
  path.join(process.cwd(), 'artifacts', 'webui-online', 'ambiguous-submit-recovery-fixed');

await fs.mkdir(artifactDir, { recursive: true });

const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-webui-ambiguous-'));
let chrome;

try {
  await waitHealth();
  await assertProductionPageReachable();
  const imagePath = path.join(artifactDir, 'attachment-failure-proof.png');
  await fs.writeFile(
    imagePath,
    Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl2n1cAAAAASUVORK5CYII=', 'base64'),
  );
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
      '--window-size=390,844',
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
  await cdp.send('DOM.enable');
  await cdp.send('Network.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: `window.__freehandEnableTestHooks = true; window.__freehandDraftSessionIdsForTest = ${JSON.stringify([fixedAttachmentSessionId])};`,
  });
  await cdp.send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
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

  const attachmentFailure = await runAttachmentFailureRetentionProof(
    cdp,
    fixedAttachmentSessionId,
    attachmentFailurePrompt,
    taskCwd,
    imagePath,
  );
  await cdp.send('Network.emulateNetworkConditions', {
    offline: false,
    latency: 0,
    downloadThroughput: -1,
    uploadThroughput: -1,
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitFor(
    () => evalPage(cdp, () => !!document.querySelector('[data-webui-shell="true"]') && !!window.__freehandWebUiTest),
    20_000,
    'WebUI shell after attachment failure proof',
  );
  const ambiguous = await evalPage(cdp, runAmbiguousSubmitRecoveryProof, fixedSessionId, fixedPrompt);
  const result = {
    ok: true,
    baseUrl,
    assetVersion,
    fixedSessionId,
    fixedAttachmentSessionId,
    taskCwd,
    attachmentFailure,
    ambiguous,
    checks: {
      ...attachmentFailure.checks,
      ...ambiguous.checks,
    },
  };
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(result, null, 2));
  if (Object.values(result.checks).some((passed) => !passed)) {
    throw new Error(`ambiguous submit recovery proof failed: ${JSON.stringify(result.checks)}`);
  }
  console.log(
    `webui_ambiguous_submit_recovery_ok session=${fixedSessionId} attachment_session=${fixedAttachmentSessionId} artifact=${path.join(artifactDir, 'summary.json')}`,
  );
  await cdp.close();
} finally {
  if (chrome?.pid && chrome.exitCode === null) {
    chrome.kill('SIGTERM');
    await new Promise((resolve) => chrome.once('exit', resolve));
  }
  await fs.rm(profileDir, { recursive: true, force: true }).catch(() => {});
}

async function captureFailureState(cdp, name) {
  if (!cdp) return;
  const state = await evalPage(cdp, () => {
    const hook = window.__freehandWebUiTest;
    return {
      attachmentState: hook?.captureAttachmentState?.() || null,
      bodyText: document.body.innerText || '',
    };
  }).catch((error) => ({ captureError: error.message }));
  await fs.writeFile(path.join(artifactDir, `${name}.json`), JSON.stringify(state, null, 2));
}

async function runAttachmentFailureRetentionProof(cdp, sessionId, prompt, cwd, imagePath) {
  await evalPage(cdp, (targetCwd) => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
    document.getElementById('mobile-new-entry-button')?.click();
    const taskRadio = document.querySelector('input[name="new-session-kind"][value="task"]');
    taskRadio.checked = true;
    taskRadio.dispatchEvent(new Event('change', { bubbles: true }));
    const input = document.getElementById('new-session-cwd-input');
    input.value = targetCwd;
    input.dispatchEvent(new Event('input', { bubbles: true }));
  }, cwd);
  await waitFor(
    () => evalPage(cdp, () => {
      const dialog = document.getElementById('new-session-dialog');
      return dialog?.open && dialog.dataset.kind === 'task';
    }),
    10_000,
    'task New dialog for attachment failure proof',
  );
  await evalPage(cdp, () => document.getElementById('new-session-form')?.requestSubmit());
  const ownerSession = await waitForOwnerSession(sessionId, (row) => row && row.cwd === cwd, 30_000);
  const selectedAfterCreate = await waitForSelectedSession(cdp, sessionId, cwd, 30_000);

  const { root } = await cdp.send('DOM.getDocument', {});
  const { nodeId } = await cdp.send('DOM.querySelector', {
    nodeId: root.nodeId,
    selector: '#attachment-image-input',
  });
  if (!nodeId) {
    throw new Error('attachment image input not found');
  }
  await cdp.send('DOM.setFileInputFiles', { nodeId, files: [imagePath] });
  await evalPage(cdp, () => {
    const input = document.getElementById('attachment-image-input');
    input?.dispatchEvent(new Event('change', { bubbles: true }));
  });
  const selectedAttachment = await waitFor(
    () => evalPage(cdp, () => window.__freehandWebUiTest.captureAttachmentState()),
    10_000,
    'selected image attachment',
  );
  await cdp.send('Network.emulateNetworkConditions', {
    offline: true,
    latency: 0,
    downloadThroughput: 0,
    uploadThroughput: 0,
  });
  await evalPage(cdp, () => window.__freehandWebUiTest.closeAdpSocketForTest());
  await evalPage(cdp, (text) => {
    document.getElementById('composer-input').value = text;
    document.getElementById('composer-form').requestSubmit();
  }, prompt);
  let retainedAfterFailure;
  try {
    retainedAfterFailure = await waitFor(
      () => evalPage(cdp, (expectedPrompt, expectedSession, expectedCwd) => {
        const state = window.__freehandWebUiTest.captureAttachmentState();
        if (
          state.selectedSession === expectedSession &&
          state.selectedCwd === expectedCwd &&
          state.pendingUserInput === expectedPrompt &&
          state.pendingSubmitSessionId === expectedSession &&
          state.attachmentCount === 1 &&
          state.pendingAttachments === 1 &&
          state.messageText.includes(expectedPrompt) &&
          state.messageText.includes('Attachments') &&
          state.messageText.includes('ready') &&
          state.turnStatus.includes('checking service truth') &&
          state.messageText.includes('Draft attachments retained')
        ) {
          return state;
        }
        return null;
      }, prompt, sessionId, cwd),
      30_000,
      'attachment failure retention state',
    );
  } catch (error) {
    await captureFailureState(cdp, 'attachment-failure-retention-timeout');
    throw error;
  }
  const ownerAfterFailure = await waitForOwnerSession(sessionId, (row) => row && row.cwd === cwd, 30_000);
  return {
    sessionId,
    prompt,
    cwd,
    ownerSession,
    selectedAfterCreate,
    selectedAttachment,
    retainedAfterFailure,
    ownerAfterFailure,
    checks: {
      attachmentSessionCreatedThroughOwnerTruth:
        !!ownerSession && ownerSession.session_id === sessionId && ownerSession.cwd === cwd,
      attachmentTaskSelectedWithCwd:
        selectedAfterCreate.selectedSession === sessionId && selectedAfterCreate.selectedCwd === cwd,
      imageSelectedThroughInput:
        selectedAttachment.attachmentCount === 1 &&
        selectedAttachment.thumbCount === 1 &&
        selectedAttachment.removeCount === 1 &&
        selectedAttachment.trayText.includes('attachment-failure-proof.png') &&
        selectedAttachment.commandStatus.includes('1 attachment draft'),
      failureKeepsSessionCwdAndPendingCard:
        retainedAfterFailure.selectedSession === sessionId &&
        retainedAfterFailure.selectedCwd === cwd &&
        retainedAfterFailure.pendingUserInput === prompt &&
        retainedAfterFailure.messageText.includes(prompt) &&
        retainedAfterFailure.turnStatus.includes('checking service truth'),
      failureKeepsAttachmentDraft:
        retainedAfterFailure.attachmentCount === 1 &&
        retainedAfterFailure.pendingAttachments === 1 &&
        retainedAfterFailure.messageText.includes('Attachments') &&
        retainedAfterFailure.messageText.includes('ready') &&
        retainedAfterFailure.messageText.includes('Draft attachments retained'),
      ownerSessionStillCwdBoundAfterFailure:
        !!ownerAfterFailure && ownerAfterFailure.session_id === sessionId && ownerAfterFailure.cwd === cwd,
    },
  };
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

async function waitHealth() {
  await waitFor(async () => {
    const response = await fetch(new URL('/health', baseUrl));
    return response.ok && (await response.text()).trim() === 'ok';
  }, 60_000, 'S-profile health');
}

async function assertProductionPageReachable() {
  const response = await fetch(baseUrl, { cache: 'no-store' });
  if (!response.ok) throw new Error(`production WebUI not reachable: ${response.status} ${response.statusText}`);
  const html = await response.text();
  if (!html.includes(assetVersion)) throw new Error(`served WebUI asset version mismatch: expected ${assetVersion}`);
}

async function waitForSelectedSession(cdp, sessionId, cwd, timeoutMs) {
  return await waitFor(
    () => evalPage(cdp, (expectedSession, expectedCwd) => {
      const shell = document.querySelector('[data-webui-shell="true"]');
      if (shell?.dataset?.selectedSession !== expectedSession) return null;
      if (expectedCwd && shell?.dataset?.selectedCwd !== expectedCwd) return null;
      return {
        selectedSession: shell.dataset.selectedSession || '',
        selectedCwd: shell.dataset.selectedCwd || '',
        messageText: document.getElementById('message-list')?.innerText || '',
        commandStatus: document.getElementById('command-status')?.innerText || '',
      };
    }, sessionId, cwd),
    timeoutMs,
    `selected session ${sessionId}`,
  );
}

async function waitForOwnerSession(sessionId, predicate, timeoutMs) {
  return await waitFor(async () => {
    const list = sessionListPayload(await adpQuery('QuerySessionList'));
    const row = allSessionRows(list).find((session) => session.session_id === sessionId) || null;
    if (predicate(row)) return row;
    return null;
  }, timeoutMs, `owner session ${sessionId}`);
}

function sessionListPayload(result) {
  return result?.SessionList || result?.session_list || result;
}

function allSessionRows(list) {
  if (Array.isArray(list?.active)) return list.active;
  if (Array.isArray(list?.sessions)) return list.sessions;
  return [];
}

async function adpQuery(query) {
  return await adpRequest('query', 'query', query, 30_000);
}

function adpRequest(kind, payloadKey, payload, timeoutMs) {
  const socket = new WebSocket(adpUrl);
  const requestId = `${kind}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      socket.close();
      reject(new Error(`ADP ${kind} timeout`));
    }, timeoutMs);
    socket.addEventListener('open', () => socket.send(JSON.stringify({ kind, request_id: requestId, [payloadKey]: payload })));
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data);
      if (message.request_id !== requestId) return;
      clearTimeout(timer);
      socket.close();
      if (message.kind === 'failure') return reject(new Error(message.failure?.message || message.failure?.code || 'ADP failure'));
      if (message.kind === 'query_result') return resolve(message.result);
      reject(new Error(`unexpected ADP ${kind} response: ${message.kind}`));
    });
    socket.addEventListener('error', () => {
      clearTimeout(timer);
      reject(new Error(`ADP ${kind} socket error`));
    });
  });
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

function adpUrlFromBaseUrl(value) {
  const url = new URL(value);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  url.pathname = '/adp';
  url.search = '';
  return url.toString();
}
