import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const repo = process.cwd();
const home = process.env.HOME;
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_SESSION_SEARCH_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_SESSION_SEARCH_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const chromePath = process.env.FREEHAND_SESSION_SEARCH_CHROME || defaultBrowserPath();
const debugPort = Number.parseInt(process.env.FREEHAND_SESSION_SEARCH_DEBUG_PORT || '9277', 10);
const fixedSessionId = process.env.FREEHAND_SESSION_SEARCH_SESSION_ID || 'webui-session-search-fixed';
const queryToken = process.env.FREEHAND_SESSION_SEARCH_QUERY || `session-search-proof-${fixedSessionId}`;
const fixedTitle = `Session 搜索 Proof ${queryToken}`;
const assetVersion = '20260726-mobile-route-one-row';
const runId = `webui-session-search-${Date.now()}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);

await fs.mkdir(artifactDir, { recursive: true });

let chrome = null;
let cdp = null;
let chromeProfileDir = null;
let summary = null;
let chromeStdout = '';
let chromeStderr = '';

try {
  await waitHealth();
  await assertProductionPageReachable();

  const beforeSessions = sessionListPayload(await adpQuery('QuerySessionList'));
  await fs.writeFile(path.join(artifactDir, 'session-list-before.json'), JSON.stringify(beforeSessions, null, 2));

  await ensureFixedSession();
  const ownerSearch = searchPayload(await adpQuery({ QuerySessionSearch: { query: queryToken, limit: 20 } }));
  await fs.writeFile(path.join(artifactDir, 'session-search-adp.json'), JSON.stringify(ownerSearch, null, 2));
  assertOwnerSearch(ownerSearch);

  chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-session-search-'));
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
  cdp = await createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(cdp, () => document.body.dataset.webuiJsReady === 'true' && !!document.getElementById('open-session-drawer-button') && !!document.getElementById('session-search-dialog'), 20_000, '搜索-capable WebUI shell 就绪');

  await evalInPage(cdp, (query) => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
    document.getElementById('open-session-drawer-button')?.click();
    const input = document.getElementById('session-search-input');
    input.value = query;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    document.getElementById('session-search-form')?.dispatchEvent(new SubmitEvent('submit', { bubbles: true, cancelable: true }));
  }, queryToken);

  const dom = await waitForFunction(cdp, (sessionId) => {
    const dialog = document.getElementById('session-search-dialog');
    const cards = Array.from(document.querySelectorAll('.session-search-card'));
    const targetCard = cards.find((card) => card.dataset.sessionId === sessionId);
    if (!dialog?.open || !targetCard) return null;
    const allCardSessionIds = cards.map((card) => card.dataset.sessionId || '');
    return {
      dialogOpen: dialog.open,
      statusText: document.getElementById('session-search-status')?.innerText || '',
      allCardSessionIds,
      targetText: targetCard.innerText || '',
      childRows: Array.from(targetCard.querySelectorAll('.session-search-child')).map((row) => ({
        parentSessionId: row.dataset.parentSessionId || '',
        childSessionId: row.dataset.childSessionId || '',
        text: row.innerText || '',
      })),
      bodyText: document.body.innerText || '',
      noHorizontalOverflow: Math.max(document.body.scrollWidth, document.documentElement.scrollWidth) <= window.innerWidth + 2,
    };
  }, 30_000, '搜索 DOM result', fixedSessionId);
  await fs.writeFile(path.join(artifactDir, 'session-search-dom.json'), JSON.stringify(dom, null, 2));
  await captureScreenshot(cdp, 'session-search-results.png');

  await evalInPage(cdp, (sessionId) => {
    document.querySelector(`.session-search-card[data-session-id="${sessionId}"] .session-search-card-head`)?.click();
  }, fixedSessionId);
  const selected = await waitForFunction(cdp, (sessionId) => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    if (shell?.dataset?.selectedSession === sessionId) {
      return {
        selectedSession: shell.dataset.selectedSession,
        sessionTitle: document.getElementById('session-title')?.innerText || '',
        dialogOpen: !!document.getElementById('session-search-dialog')?.open,
      };
    }
    return null;
  }, 20_000, '搜索 result opens selected session', fixedSessionId);
  await fs.writeFile(path.join(artifactDir, 'selected-session-dom.json'), JSON.stringify(selected, null, 2));
  await captureScreenshot(cdp, 'session-search-selected-session.png');

  const afterSessions = sessionListPayload(await adpQuery('QuerySessionList'));
  await fs.writeFile(path.join(artifactDir, 'session-list-after.json'), JSON.stringify(afterSessions, null, 2));

  const beforeIds = sessionIds(beforeSessions);
  const afterIds = sessionIds(afterSessions);
  const allowedNew = new Set([fixedSessionId]);
  const unexpectedNew = afterIds.filter((id) => !beforeIds.includes(id) && !allowedNew.has(id));
  const workerTopLevelCards = dom.allCardSessionIds.filter((id) => id.startsWith('worker-task-'));
  const workerTopLevelSessions = afterIds.filter((id) => id.startsWith('worker-task-'));

  summary = {
    ok: true,
    baseUrl,
    adpUrl,
    artifactDir,
    fixedSessionId,
    queryToken,
    assetVersion,
    screenshots: ['session-search-results.png', 'session-search-selected-session.png'],
    checks: {
      ownerProjectionContainsFixedSession: ownerSearch.results.some((row) => row.session_id === fixedSessionId),
      browserDialogOpened: dom.dialogOpen === true,
      browserRowsMatchOwnerProjection: dom.allCardSessionIds.includes(fixedSessionId),
      noTopLevelWorkerResultCards: workerTopLevelCards.length === 0,
      selectedSessionOpened: selected.selectedSession === fixedSessionId,
      dialogClosedAfterOpen: selected.dialogOpen === false,
      noUnexpectedTopLevelSessionCreated: unexpectedNew.length === 0,
      noTopLevelWorkerSessionsAfter: workerTopLevelSessions.length === 0,
      noHorizontalOverflow: dom.noHorizontalOverflow === true,
      assetVersionServed: true,
    },
    unexpectedNew,
    workerTopLevelCards,
    workerTopLevelSessions,
  };
  summary.ok = Object.values(summary.checks).every(Boolean);
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  if (!summary.ok) {
    const failed = Object.entries(summary.checks).filter(([, value]) => value !== true).map(([key]) => key);
    throw new Error(`webui_session_search_failed checks=${failed.join(',')} artifactDir=${artifactDir}`);
  }
  console.log(`webui_session_search_ok url=${baseUrl} adp=${adpUrl} session=${fixedSessionId} artifactDir=${artifactDir}`);
} catch (error) {
  await writeFailure(error);
  throw error;
} finally {
  if (cdp) await cdp.close().catch(() => null);
  if (chrome?.pid) chrome.kill('SIGTERM');
  if (chromeProfileDir) await fs.rm(chromeProfileDir, { recursive: true, force: true }).catch(() => null);
}

async function ensureFixedSession() {
  const create = { CreateSession: { session_id: fixedSessionId, title: fixedTitle, cwd: repo } };
  try {
    await adpCommand(create, 20_000);
  } catch (error) {
    await adpCommand({ RenameSession: { session_id: fixedSessionId, title: fixedTitle } }, 20_000);
  }
}

function assertOwnerSearch(search) {
  if (search.query !== queryToken) throw new Error(`unexpected search query echo ${search.query}`);
  const row = search.results.find((candidate) => candidate.session_id === fixedSessionId);
  if (!row) throw new Error(`owner search missing fixed session ${fixedSessionId}`);
  if ((search.results || []).some((candidate) => `${candidate.session_id || ''}`.startsWith('worker-task-'))) {
    throw new Error('owner search returned worker session as top-level result');
  }
}

function searchPayload(result) {
  return result?.SessionSearch || result?.session_search || result;
}

function sessionListPayload(result) {
  return result?.SessionList || result?.session_list || result;
}

function sessionIds(list) {
  const active = Array.isArray(list?.active) ? list.active : [];
  return active.map((row) => row.session_id).filter(Boolean).sort();
}

async function adpQuery(query) {
  return await adpRequest('query', 'query', query, 30_000);
}

async function adpCommand(command, timeoutMs = 30_000) {
  return await adpRequest('command', 'command', command, timeoutMs);
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
      if (message.kind === 'command_receipt') return resolve(message.receipt);
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
  if (summary) await fs.writeFile(path.join(failureDir, 'summary.partial.json'), JSON.stringify(summary, null, 2));
  await adpQuery({ QuerySessionSearch: { query: queryToken, limit: 20 } })
    .then((value) => fs.writeFile(path.join(failureDir, 'session-search.json'), JSON.stringify(value, null, 2)))
    .catch((queryError) => fs.writeFile(path.join(failureDir, 'session-search-error.txt'), queryError.stack || queryError.message));
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
  const cached = path.join(home, 'Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell');
  return process.env.FREEHAND_SESSION_SEARCH_CHROME || cached;
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
