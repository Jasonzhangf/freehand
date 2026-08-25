import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { adpVerifierRequest, requireSessionListPage } from './lib/adp-verifier-client.mjs';

const repo = process.cwd();
const home = process.env.HOME;
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_NEW_SESSION_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_NEW_SESSION_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const adpAuthToken = process.env.FREEHAND_ADP_AUTH_TOKEN || '';
const adpProtocolVersion = 4;
const chromePath = process.env.FREEHAND_NEW_SESSION_CHROME || defaultBrowserPath();
const debugPort = Number.parseInt(process.env.FREEHAND_NEW_SESSION_DEBUG_PORT || '9278', 10);
const conversationSessionId = process.env.FREEHAND_NEW_CONVERSATION_SESSION_ID || 'webui-new-conversation-fixed';
const taskSessionId = process.env.FREEHAND_NEW_TASK_SESSION_ID || 'webui-new-task-fixed';
const taskCwd = process.env.FREEHAND_NEW_TASK_CWD || repo;
const assetVersion = '20260825-session-terminal-cursor';
const runId = `webui-new-session-${Date.now()}`;
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
  const beforeSessions = await activeSessionListPage();
  await fs.writeFile(path.join(artifactDir, 'session-list-before.json'), JSON.stringify(beforeSessions, null, 2));

  chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-new-session-'));
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
    source: `window.__freehandEnableTestHooks = true; window.requestIdleCallback = () => 0; window.__freehandDraftSessionIdsForTest = ${JSON.stringify([conversationSessionId, taskSessionId])};`,
  });
  await cdp.send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(
    cdp,
    () => document.body.dataset.webuiJsReady === 'true' && !!document.getElementById('mobile-new-entry-button') && !!document.getElementById('new-session-dialog'),
    20_000,
    'New-session-capable WebUI shell 就绪',
  );

  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
    document.getElementById('mobile-new-entry-button')?.click();
  });
  const conversationDialog = await waitForFunction(cdp, () => {
    const dialog = document.getElementById('new-session-dialog');
    if (!dialog?.open || dialog.dataset.kind !== 'conversation') return null;
    return {
      open: dialog.open,
      kind: dialog.dataset.kind,
      confirmText: document.getElementById('new-session-confirm-button')?.innerText || '',
    };
  }, 10_000, 'conversation New dialog');
  await evalInPage(cdp, () => document.getElementById('new-session-form')?.requestSubmit());
  const conversationOwnerRow = await waitForOwnerSession(conversationSessionId, (row) => row && !row.cwd, 30_000);
  const conversationDom = await waitForSelectedSession(conversationSessionId, 30_000);
  await captureScreenshot(cdp, 'new-conversation-selected.png');
  const afterConversation = await activeSessionListPage();
  await fs.writeFile(path.join(artifactDir, 'session-list-after-conversation.json'), JSON.stringify(afterConversation, null, 2));

  await evalInPage(cdp, (cwd) => {
    document.getElementById('mobile-new-entry-button')?.click();
    const taskRadio = document.querySelector('input[name="new-session-kind"][value="task"]');
    taskRadio.checked = true;
    taskRadio.dispatchEvent(new Event('change', { bubbles: true }));
    const input = document.getElementById('new-session-cwd-input');
    input.value = cwd;
    input.dispatchEvent(new Event('input', { bubbles: true }));
  }, taskCwd);
  const taskDialog = await waitForFunction(cdp, () => {
    const dialog = document.getElementById('new-session-dialog');
    if (!dialog?.open || dialog.dataset.kind !== 'task') return null;
    return {
      open: dialog.open,
      kind: dialog.dataset.kind,
      cwd: document.getElementById('new-session-cwd-input')?.value || '',
      confirmText: document.getElementById('new-session-confirm-button')?.innerText || '',
    };
  }, 10_000, 'task New dialog');
  await evalInPage(cdp, () => document.getElementById('new-session-form')?.requestSubmit());
  const taskOwnerRow = await waitForOwnerSession(taskSessionId, (row) => row && row.cwd === taskCwd, 30_000);
  const taskDom = await waitForSelectedSession(taskSessionId, 30_000);
  await captureScreenshot(cdp, 'new-task-selected.png');
  const afterTask = await activeSessionListPage();
  await fs.writeFile(path.join(artifactDir, 'session-list-after-task.json'), JSON.stringify(afterTask, null, 2));

  const beforeOlderLoad = await evalInPage(cdp, () => ({
    sessionIds: Array.from(document.querySelectorAll('.session-item.session-row'))
      .map((row) => row.dataset.sessionId || ''),
    buttonText: document.querySelector('button.session-list-older')?.innerText || '',
    buttonDisabled: document.querySelector('button.session-list-older')?.disabled ?? true,
    buttonPresent: !!document.querySelector('button.session-list-older'),
  }));
  let afterOlderLoad;
  let ownerTerminalPage = null;
  if (beforeOlderLoad.buttonPresent && !beforeOlderLoad.buttonDisabled) {
    await evalInPage(cdp, () => document.querySelector('button.session-list-older')?.click());
    afterOlderLoad = await waitForFunction(cdp, (beforeCount) => {
      const sessionIds = Array.from(document.querySelectorAll('.session-item.session-row'))
        .map((row) => row.dataset.sessionId || '');
      if (sessionIds.length <= beforeCount) return null;
      return {
        appended: true,
        sessionIds,
        buttonText: document.querySelector('button.session-list-older')?.innerText || '',
        buttonDisabled: document.querySelector('button.session-list-older')?.disabled ?? true,
      };
    }, 30_000, 'older session page appended', beforeOlderLoad.sessionIds.length);
  } else {
    ownerTerminalPage = await activeSessionListPage();
    if (ownerTerminalPage.page.has_older !== false) {
      throw new Error(`owner reports has_older but WebUI omitted the button: ${JSON.stringify(ownerTerminalPage.page)}`);
    }
    afterOlderLoad = { appended: false, ...beforeOlderLoad };
  }
  await fs.writeFile(
    path.join(artifactDir, 'session-list-older-dom.json'),
    JSON.stringify({ before: beforeOlderLoad, after: afterOlderLoad }, null, 2),
  );
  await captureScreenshot(cdp, 'session-list-after-load-older.png');

  const conversationRow = findSession(afterTask, conversationSessionId);
  const taskRow = findSession(afterTask, taskSessionId);
  const topLevelWorkerRows = allSessionRows(afterTask).filter((row) => `${row.session_id || ''}`.startsWith('worker-task-'));
  const bodyState = await evalInPage(cdp, () => ({
    bodyText: document.body.innerText || '',
    noHorizontalOverflow: Math.max(document.body.scrollWidth, document.documentElement.scrollWidth) <= window.innerWidth + 2,
    selectedSession: document.querySelector('[data-webui-shell="true"]')?.dataset.selectedSession || '',
    selectedCwd: document.querySelector('[data-webui-shell="true"]')?.dataset.selectedCwd || '',
    messageText: document.getElementById('message-list')?.innerText || '',
    commandStatus: document.getElementById('command-status')?.innerText || '',
  }));

  summary = {
    ok: true,
    baseUrl,
    adpUrl,
    artifactDir,
    assetVersion,
    conversationSessionId,
    taskSessionId,
    taskCwd,
    conversationDialog,
    taskDialog,
    conversationOwnerRow,
    taskOwnerRow,
    conversationDom,
    taskDom,
    beforeOlderLoad,
    afterOlderLoad,
    sessionPagingMode: afterOlderLoad.appended ? 'older-page' : 'terminal-single-page',
    bodyState,
    checks: {
      assetVersionServed: true,
      conversationDialogOpenedFromMobileEntry: conversationDialog.open === true && conversationDialog.kind === 'conversation',
      conversationCreatedThroughOwnerTruth: !!conversationOwnerRow && conversationOwnerRow.archived !== true && !conversationOwnerRow.cwd,
      conversationSelectedInUi: conversationDom.selectedSession === conversationSessionId,
      conversationEmptyStateClean: conversationDom.messageText.includes('新会话') && (conversationDom.messageText.includes('发送消息开始这个会话。') || conversationDom.messageText.includes('发送 a message to start this session.')),
      taskDialogOpenedAndCwdEntered: taskDialog.open === true && taskDialog.kind === 'task' && taskDialog.cwd === taskCwd,
      taskCreatedThroughOwnerTruth: !!taskOwnerRow && taskOwnerRow.archived !== true && taskOwnerRow.cwd === taskCwd,
      taskSelectedInUi: taskDom.selectedSession === taskSessionId,
      taskCwdProjectedInUi: taskDom.selectedCwd === taskCwd || bodyState.selectedCwd === taskCwd,
      olderPageHandledInUi: afterOlderLoad.appended === true
        || (afterOlderLoad.appended === false
          && beforeOlderLoad.buttonPresent === false
          && ownerTerminalPage.page.has_older === false),
      noTopLevelWorkerSessions: topLevelWorkerRows.length === 0,
      noHorizontalOverflow: bodyState.noHorizontalOverflow === true,
    },
  };
  summary.ok = Object.values(summary.checks).every(Boolean);
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  if (!summary.ok) {
    const failed = Object.entries(summary.checks).filter(([, value]) => value !== true).map(([key]) => key);
    throw new Error(`webui_new_session_failed checks=${failed.join(',')} artifactDir=${artifactDir}`);
  }
  console.log(`webui_new_session_ok url=${baseUrl} adp=${adpUrl} conversation=${conversationSessionId} task=${taskSessionId} artifactDir=${artifactDir}`);
} catch (error) {
  await writeFailure(error);
  throw error;
} finally {
  if (cdp) await cdp.close().catch(() => null);
  if (chrome?.pid) chrome.kill('SIGTERM');
  if (chromeProfileDir) await fs.rm(chromeProfileDir, { recursive: true, force: true }).catch(() => null);
}

async function waitForSelectedSession(sessionId, timeoutMs) {
  return await waitForFunction(cdp, (expected) => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    if (shell?.dataset?.selectedSession !== expected) return null;
    return {
      selectedSession: shell.dataset.selectedSession || '',
      selectedCwd: shell.dataset.selectedCwd || '',
      sessionTitle: document.getElementById('session-title')?.innerText || '',
      messageText: document.getElementById('message-list')?.innerText || '',
      commandStatus: document.getElementById('command-status')?.innerText || '',
    };
  }, timeoutMs, `selected session ${sessionId}`, sessionId);
}

async function waitForOwnerSession(sessionId, predicate, timeoutMs) {
  return await waitFor(async () => {
    const list = await activeSessionListPage();
    const row = findSession(list, sessionId);
    if (predicate(row)) return row;
    return null;
  }, timeoutMs, `owner session ${sessionId}`);
}

function findSession(list, sessionId) {
  return allSessionRows(list).find((row) => row.session_id === sessionId) || null;
}

function allSessionRows(list) {
  return list.sessions;
}

function sessionListPayload(result) {
  return requireSessionListPage(result);
}

async function adpQuery(query) {
  return await adpRequest('query', 'query', query, 30_000);
}

async function activeSessionListPage() {
  const result = await adpRequest('query', 'query', {
    QuerySessionListPage: {
      archived: false,
      page: { direction: 'Latest', cursor: null, limit: 100 },
    },
  }, 30_000);
  return sessionListPayload(result);
}

function adpRequest(kind, payloadKey, payload, timeoutMs) {
  return adpVerifierRequest({
    url: adpUrl,
    authToken: adpAuthToken,
    kind,
    payloadKey,
    payload,
    timeoutMs,
    clientName: 'freehand-new-session-verifier',
    capabilities: ['query', 'command'],
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
  if (!html.includes(assetVersion)) throw new Error(`served WebUI asset version mismatch: expected ${assetVersion}`);
}

async function waitForPageTarget(urlPrefix, timeoutMs) {
  return await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
    if (!response.ok) return null;
    const targets = await response.json();
    return targets.find((target) => target.type === 'page' && `${target.url || ''}`.startsWith(urlPrefix)) || targets.find((target) => target.type === 'page') || null;
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
  const screenshot = await cdpClient.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true });
  await fs.writeFile(path.join(artifactDir, fileName), Buffer.from(screenshot.data, 'base64'));
}

async function writeFailure(error) {
  const failureDir = path.join(artifactDir, 'failure');
  await fs.mkdir(failureDir, { recursive: true });
  await fs.writeFile(path.join(failureDir, 'error.txt'), error.stack || error.message);
  await fs.writeFile(path.join(failureDir, 'chrome-stdout.txt'), chromeStdout);
  await fs.writeFile(path.join(failureDir, 'chrome-stderr.txt'), chromeStderr);
  await activeSessionListPage()
    .then((value) => fs.writeFile(path.join(failureDir, 'session-list.json'), JSON.stringify(value, null, 2)))
    .catch((queryError) => fs.writeFile(path.join(failureDir, 'session-list-error.txt'), queryError.stack || queryError.message));
  if (cdp) {
    await evalInPage(cdp, () => ({
      bodyText: document.body.innerText || '',
      selectedSession: document.querySelector('[data-webui-shell="true"]')?.dataset.selectedSession || '',
      selectedCwd: document.querySelector('[data-webui-shell="true"]')?.dataset.selectedCwd || '',
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

function delay(ms) { return new Promise((resolve) => setTimeout(resolve, ms)); }
function normalizedBaseUrl(value) { return value.endsWith('/') ? value : `${value}/`; }
function adpUrlFromBaseUrl(value) { const url = new URL(value); url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'; url.pathname = '/adp'; url.search = ''; return url.toString(); }
function defaultBrowserPath() {
  return '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
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
