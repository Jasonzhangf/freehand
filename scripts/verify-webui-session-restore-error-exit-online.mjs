import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const repo = process.cwd();
const home = process.env.HOME;
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_SESSION_UNLOCK_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_SESSION_UNLOCK_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const chromePath = process.env.FREEHAND_SESSION_UNLOCK_CHROME ||
  process.env.FREEHAND_WEBUI_CHROME ||
  defaultBrowserPath();
const debugPort = Number.parseInt(process.env.FREEHAND_SESSION_UNLOCK_DEBUG_PORT || '9284', 10);
const problemSessionId = process.env.FREEHAND_SESSION_UNLOCK_SESSION_ID ||
  'webui-session-20260723001509-bd98e156';
const problemTurnId = process.env.FREEHAND_SESSION_UNLOCK_TURN_ID || 'runtime-turn-541-r3';
const assetVersion = '20260726-stale-lifecycle-reconcile';
const runId = `webui-session-unlock-${Date.now()}`;
const newSessionId = process.env.FREEHAND_SESSION_UNLOCK_NEW_SESSION_ID ||
  `${runId}-new`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);

await fs.mkdir(artifactDir, { recursive: true });

let chrome = null;
let cdp = null;
let chromeProfileDir = null;
let chromeStdout = '';
let chromeStderr = '';
let summary = null;

try {
  await waitHealth();
  await assertProductionPageReachable();

  const sessionQueryResult = await adpQuery({
    QuerySessionTurns: { session_id: problemSessionId },
  });
  const transcript = sessionTurnsPayload(sessionQueryResult);
  await fs.writeFile(path.join(artifactDir, 'session-turns-adp.json'), JSON.stringify(transcript, null, 2));
  const problemTurn = transcript.turns.find((turn) => turn.turn_id === problemTurnId);
  if (!problemTurn) {
    throw new Error(`ADP SessionTurns missing ${problemTurnId}`);
  }
  const turnIsToolPending = normalizeStatus(problemTurn.terminal_status) === 'toolpending';
  const warningVisible = (problemTurn.errors || []).some((line) =>
    `${line || ''}`.includes('历史会话轮次不完整') ||
      `${line || ''}`.includes('reason_persistence_partial_ui_restore')
  );
  if (!turnIsToolPending) {
    throw new Error(`expected ${problemTurnId} terminal_status ToolPending, got ${problemTurn.terminal_status}`);
  }
  if (!warningVisible) {
    throw new Error(`expected ${problemTurnId} to carry partial transcript integrity warning`);
  }

  chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-session-unlock-'));
  chrome = spawn(chromePath, [
    '--headless=new',
    `--remote-debugging-port=${debugPort}`,
    `--user-data-dir=${chromeProfileDir}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-networking',
    '--disable-extensions',
    '--disable-sync',
    '--disable-gpu',
    '--no-sandbox',
    '--window-size=390,844',
    baseUrl,
  ], { stdio: ['ignore', 'pipe', 'pipe'] });
  chrome.stdout.on('data', (chunk) => { chromeStdout += chunk.toString(); });
  chrome.stderr.on('data', (chunk) => { chromeStderr += chunk.toString(); });

  const target = await waitForPageTarget(baseUrl, 20_000);
  cdp = createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: `
      window.__freehandEnableTestHooks = true;
      window.__freehandDraftSessionIdsForTest = ${JSON.stringify([newSessionId])};
      window.localStorage.setItem("freehand-webui-selected-session", ${JSON.stringify(problemSessionId)});
    `,
  });
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 2,
    mobile: true,
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(
    cdp,
    () => document.body.dataset.webuiJsReady === 'true' &&
      !!window.__freehandWebUiTest &&
      !!document.querySelector('[data-webui-shell="true"]'),
    20_000,
    'session-unlock-capable WebUI shell 就绪',
  );

  const lifecycleDom = await waitForFunction(cdp, (turnId) => {
    const card = document.querySelector(`.turn-cycle-card[data-turn-id="${turnId}"]`);
    if (!card) return null;
    const cardText = card.innerText || '';
    if (!cardText.includes('等待用户选择')) return null;
    return {
      selectedSession: document.querySelector('[data-webui-shell="true"]')?.dataset.selectedSession || '',
      selectedTurn: document.querySelector('[data-webui-shell="true"]')?.dataset.selectedTurn || '',
      turnStatus: document.getElementById('turn-status')?.innerText || '',
      commandStatus: document.getElementById('command-status')?.innerText || '',
      relationHeader: document.getElementById('session-relation-header')?.innerText || '',
      cardText,
      cardClass: card.className,
      bodyText: document.body.innerText || '',
      noHorizontalOverflow: Math.max(document.body.scrollWidth, document.documentElement.scrollWidth) <= window.innerWidth + 2,
    };
  }, 45_000, `${problemTurnId} 等待用户选择 DOM`, problemTurnId);
  await fs.writeFile(path.join(artifactDir, 'problem-session-dom.json'), JSON.stringify(lifecycleDom, null, 2));
  await captureScreenshot(cdp, 'problem-session-waiting-user.png');

  const refreshErrorInitial = await evalInPage(cdp, (sessionId) =>
    window.__freehandWebUiTest.simulateSessionRefreshFailureForTest(
      'dispatch port failure: verifier session refresh failure',
      sessionId,
    ),
  problemSessionId);
  await fs.writeFile(path.join(artifactDir, 'refresh-error-initial.json'), JSON.stringify(refreshErrorInitial, null, 2));
  const expectedActions = ['新建会话', '返回会话列表', '忽略错误'];
  const missingActions = expectedActions.filter((label) => !refreshErrorInitial.actionLabels.includes(label));
  if (missingActions.length > 0) {
    throw new Error(`session refresh failure actions missing: ${missingActions.join(',')}`);
  }
  if (refreshErrorInitial.adpFailure !== null) {
    throw new Error(`session refresh failure leaked into global adpFailure: ${refreshErrorInitial.adpFailure}`);
  }
  if (!refreshErrorInitial.messageText.includes('不是全局连接失败')) {
    throw new Error('session refresh failure card does not explain it is session-local');
  }

  const androidBackExit = await evalInPage(cdp, () => {
    document.activeElement?.blur?.();
    const handled = !!window.__freehandHandleAndroidBack?.();
    return { handled, ...window.__freehandWebUiTest.captureSessionRefreshExitState() };
  });
  await fs.writeFile(path.join(artifactDir, 'refresh-error-android-back-exit.json'), JSON.stringify(androidBackExit, null, 2));
  await captureScreenshot(cdp, 'refresh-error-returned-session-list.png');

  const refreshErrorForNew = await evalInPage(cdp, (sessionId) =>
    window.__freehandWebUiTest.simulateSessionRefreshFailureForTest(
      'dispatch port failure: verifier new-session exit',
      sessionId,
    ),
  problemSessionId);
  if (!refreshErrorForNew.actionLabels.includes('新建会话')) {
    throw new Error('new-session action missing after second simulated refresh failure');
  }
  await evalInPage(cdp, () => {
    const button = Array.from(document.querySelectorAll('.session-refresh-action-bar button'))
      .find((candidate) => candidate.textContent.trim() === '新建会话');
    button?.click();
  });
  const newSessionDom = await waitForFunction(cdp, (sessionId) => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    if (shell?.dataset?.selectedSession !== sessionId) return null;
    const commandStatus = document.getElementById('command-status')?.innerText || '';
    const actionLabels = Array.from(document.querySelectorAll('.session-refresh-action-bar button')).map((button) => button.textContent.trim());
    const messageText = document.getElementById('message-list')?.innerText || '';
    if (actionLabels.length > 0 || !messageText.includes('新会话')) return null;
    return {
      selectedSession: shell.dataset.selectedSession || '',
      selectedTurn: shell.dataset.selectedTurn || '',
      messageText,
      commandStatus,
      actionLabels,
    };
  }, 30_000, 'new session selected after refresh-error action', newSessionId);
  let newSessionOwner = null;
  for (let attempt = 0; attempt < 60; attempt += 1) {
    newSessionOwner = findSession(sessionListPayload(await adpQuery('QuerySessionList')), newSessionId);
    if (newSessionOwner) break;
    await delay(500);
  }
  if (!newSessionOwner) {
    throw new Error(`new session did not persist in owner truth: ${newSessionId}`);
  }
  await fs.writeFile(path.join(artifactDir, 'new-session-dom.json'), JSON.stringify(newSessionDom, null, 2));
  await fs.writeFile(path.join(artifactDir, 'new-session-owner.json'), JSON.stringify(newSessionOwner, null, 2));
  await captureScreenshot(cdp, 'refresh-error-new-session.png');

  summary = {
    ok: true,
    baseUrl,
    adpUrl,
    artifactDir,
    assetVersion,
    problemSessionId,
    problemTurnId,
    newSessionId,
    adp: {
      turnCount: transcript.turns.length,
      problemTurnStatus: problemTurn.terminal_status,
      warningVisible,
    },
    checks: {
      assetVersionServed: true,
      adpSessionTurnsReturned: transcript.session_id === problemSessionId && transcript.turns.length > 0,
      adpProblemTurnToolPending: turnIsToolPending,
      adpPartialWarningVisible: warningVisible,
      browserSelectedProblemSession: lifecycleDom.selectedSession === problemSessionId,
      browserProblemTurnRendered: lifecycleDom.cardText.includes(problemTurnId) || lifecycleDom.selectedTurn === problemTurnId,
      browserToolPendingWaitsForUserChoice: lifecycleDom.cardText.includes('等待用户选择'),
      browserProblemTurnNotLifecycle: !lifecycleDom.cardText.includes('等待生命周期') &&
        !lifecycleDom.cardText.includes('等待工具生命周期') &&
        !lifecycleDom.cardText.includes('Waiting for lifecycle'),
      sessionRefreshActionsRendered: missingActions.length === 0,
      sessionRefreshNotGlobalAdpFailure: refreshErrorInitial.adpFailure === null,
      sessionRefreshExplainsSessionLocal: refreshErrorInitial.messageText.includes('不是全局连接失败'),
      androidBackHandledRefreshError: androidBackExit.handled === true,
      androidBackClearsSelectedSession: androidBackExit.selectedSession === '',
      androidBackOpensSessionDrawer: androidBackExit.mobileDrawer === 'sessions',
      newConversationActionCreatesOwnerSession: !!newSessionOwner && newSessionOwner.session_id === newSessionId,
      newConversationActionSelectsCleanSession: newSessionDom.selectedSession === newSessionId &&
        newSessionDom.actionLabels.length === 0 &&
        newSessionDom.messageText.includes('新会话'),
      noHorizontalOverflow: lifecycleDom.noHorizontalOverflow === true,
    },
  };
  summary.ok = Object.values(summary.checks).every(Boolean);
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  if (!summary.ok) {
    const failed = Object.entries(summary.checks)
      .filter(([, value]) => value !== true)
      .map(([key]) => key);
    throw new Error(`webui_session_unlock_failed checks=${failed.join(',')} artifactDir=${artifactDir}`);
  }
  console.log(`webui_session_unlock_ok url=${baseUrl} adp=${adpUrl} session=${problemSessionId} turn=${problemTurnId} newSession=${newSessionId} artifactDir=${artifactDir}`);
} catch (error) {
  await writeFailure(error);
  throw error;
} finally {
  if (cdp) await cdp.close().catch(() => null);
  if (chrome?.pid) chrome.kill('SIGTERM');
  if (chromeProfileDir) await fs.rm(chromeProfileDir, { recursive: true, force: true }).catch(() => null);
}

function sessionTurnsPayload(result) {
  return result?.SessionTurns || result?.session_turns || result;
}

function sessionListPayload(result) {
  return result?.SessionList || result?.session_list || result;
}

function allSessionRows(list) {
  if (Array.isArray(list?.active)) return list.active;
  if (Array.isArray(list?.sessions)) return list.sessions;
  return [];
}

function findSession(list, sessionId) {
  return allSessionRows(list).find((row) => row.session_id === sessionId) || null;
}

async function adpQuery(query, timeoutMs = 30_000) {
  return await adpRequest('query', 'query', query, timeoutMs);
}

function adpRequest(kind, payloadKey, payload, timeoutMs) {
  const socket = new WebSocket(adpUrl);
  const requestId = `${kind}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      socket.close();
      reject(new Error(`ADP ${kind} timeout`));
    }, timeoutMs);
    socket.addEventListener('open', () => {
      socket.send(JSON.stringify({ kind, request_id: requestId, [payloadKey]: payload }));
    });
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data);
      if (message.request_id !== requestId) return;
      clearTimeout(timer);
      socket.close();
      if (message.kind === 'failure') {
        return reject(new Error(message.failure?.message || message.failure?.code || 'ADP failure'));
      }
      if (message.kind === 'query_result') return resolve(message.result);
      reject(new Error(`unexpected ADP ${kind} response: ${message.kind}`));
    });
    socket.addEventListener('error', () => {
      clearTimeout(timer);
      reject(new Error(`ADP ${kind} socket error`));
    });
  });
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
  await fs.writeFile(path.join(artifactDir, 'served-root.html'), html);
  if (!html.includes(assetVersion)) {
    throw new Error(`served WebUI asset version mismatch: expected ${assetVersion}`);
  }
}

async function waitForPageTarget(urlPrefix, timeoutMs) {
  return await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
    if (!response.ok) return null;
    const targets = await response.json();
    return targets.find((target) => target.type === 'page' && `${target.url || ''}`.startsWith(urlPrefix)) ||
      targets.find((target) => target.type === 'page') ||
      null;
  }, timeoutMs, 'Chrome DevTools page target');
}

async function waitForFunction(cdpClient, fn, timeoutMs, label, ...args) {
  return await waitFor(async () => await evalInPage(cdpClient, fn, ...args), timeoutMs, label);
}

async function evalInPage(cdpClient, fn, ...args) {
  const response = await cdpClient.send('Runtime.evaluate', {
    expression: `(${fn.toString()})(...${JSON.stringify(args)})`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    throw new Error(response.exceptionDetails.exception?.description || response.exceptionDetails.text || 'Runtime.evaluate failed');
  }
  return response.result.value;
}

async function waitForLoad(cdpClient, timeoutMs = 15_000) {
  await new Promise((resolve) => {
    const timer = setTimeout(resolve, timeoutMs);
    const listener = (method) => {
      if (method === 'Page.loadEventFired') {
        clearTimeout(timer);
        cdpClient.offEvent(listener);
        resolve();
      }
    };
    cdpClient.onEvent(listener);
  });
}

async function captureScreenshot(cdpClient, fileName) {
  const screenshot = await cdpClient.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  await fs.writeFile(path.join(artifactDir, fileName), Buffer.from(screenshot.data, 'base64'));
}

async function writeFailure(error) {
  const failureDir = path.join(artifactDir, 'failure');
  await fs.mkdir(failureDir, { recursive: true });
  await fs.writeFile(path.join(failureDir, 'error.txt'), error.stack || error.message);
  await fs.writeFile(path.join(failureDir, 'chrome-stdout.txt'), chromeStdout);
  await fs.writeFile(path.join(failureDir, 'chrome-stderr.txt'), chromeStderr);
  await adpQuery('QuerySessionList')
    .then((value) => fs.writeFile(path.join(failureDir, 'session-list.json'), JSON.stringify(value, null, 2)))
    .catch((queryError) => fs.writeFile(path.join(failureDir, 'session-list-error.txt'), queryError.stack || queryError.message));
  if (cdp) {
    await evalInPage(cdp, () => ({
      bodyText: document.body.innerText || '',
      selectedSession: document.querySelector('[data-webui-shell="true"]')?.dataset.selectedSession || '',
      selectedTurn: document.querySelector('[data-webui-shell="true"]')?.dataset.selectedTurn || '',
      messageText: document.getElementById('message-list')?.innerText || '',
      commandStatus: document.getElementById('command-status')?.innerText || '',
    }))
      .then((value) => fs.writeFile(path.join(failureDir, 'dom-state.json'), JSON.stringify(value, null, 2)))
      .catch(() => null);
  }
}

async function waitFor(fn, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw new Error(`timeout waiting for ${label}${lastError ? `: ${lastError.message}` : ''}`);
}

function normalizeStatus(status) {
  return `${status || ''}`.toLowerCase().replace(/[_-]/g, '');
}

function delay(ms) { return new Promise((resolve) => setTimeout(resolve, ms)); }
function normalizedBaseUrl(value) { return value.endsWith('/') ? value : `${value}/`; }
function adpUrlFromBaseUrl(value) {
  const url = new URL(value);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  url.pathname = '/adp';
  url.search = '';
  return url.toString();
}
function defaultBrowserPath() {
  return path.join(home, 'Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell');
}

function createCdpClient(wsUrl) {
  const ws = new WebSocket(wsUrl);
  let nextId = 1;
  const pending = new Map();
  const listeners = new Set();
  ws.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      const { resolve, reject } = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) reject(new Error(message.error.message || JSON.stringify(message.error)));
      else resolve(message.result || {});
      return;
    }
    if (message.method) listeners.forEach((listener) => listener(message.method, message.params || {}));
  });
  const open = new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve, { once: true });
    ws.addEventListener('error', () => reject(new Error('CDP socket error')), { once: true });
  });
  return {
    async send(method, params = {}) {
      await open;
      const id = nextId++;
      ws.send(JSON.stringify({ id, method, params }));
      return await new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
    },
    onEvent(listener) { listeners.add(listener); },
    offEvent(listener) { listeners.delete(listener); },
    async close() { ws.close(); },
  };
}
