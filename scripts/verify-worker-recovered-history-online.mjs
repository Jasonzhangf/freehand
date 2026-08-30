import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { adpVerifierRequest } from './lib/adp-verifier-client.mjs';

const repo = process.cwd();
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WORKER_RECOVERY_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_WORKER_RECOVERY_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const chromePath = process.env.FREEHAND_WORKER_RECOVERY_CHROME ||
  process.env.FREEHAND_WEBUI_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WORKER_RECOVERY_DEBUG_PORT || '9291', 10);
const parentSessionId = process.env.FREEHAND_WORKER_RECOVERY_PARENT_SESSION_ID ||
  'webui-session-20260727151900-16c3255e';
const taskId = process.env.FREEHAND_WORKER_RECOVERY_TASK_ID || 'task-1785166804';
const workerSessionId = process.env.FREEHAND_WORKER_RECOVERY_WORKER_SESSION_ID ||
  'worker-task-task-1785166804';
const oldFailedTurnId = process.env.FREEHAND_WORKER_RECOVERY_FAILED_TURN_ID ||
  'worker-turn-exec-worker-worker-1785166810762910000-115919';
const laterSuccessTurnId = process.env.FREEHAND_WORKER_RECOVERY_SUCCESS_TURN_ID ||
  'worker-turn-exec-worker-worker-1785167783036348000-229-r11';
const assetSource = await fs.readFile(path.join(repo, 'apps/freehand-server/src/assets.rs'), 'utf8');
const assetVersion = assetSource.match(/WEBUI_ASSET_VERSION:\s*&str\s*=\s*"([^"]+)"/)?.[1];
if (!assetVersion) throw new Error('unable to read WEBUI_ASSET_VERSION');
const runId = `worker-recovered-history-${new Date().toISOString().replace(/[-:.]/g, '').slice(0, 15)}-${process.pid}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);
await fs.mkdir(artifactDir, { recursive: true });

let chrome = null;
let cdp = null;
let profileDir = null;
let chromeStderr = '';

try {
  await assertProductionPageReachable();
  profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-worker-recovery-'));
  chrome = spawn(chromePath, [
    '--headless=new',
    `--remote-debugging-port=${debugPort}`,
    '--remote-debugging-address=127.0.0.1',
    `--user-data-dir=${profileDir}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-networking',
    '--disable-extensions',
    '--disable-sync',
    '--disable-gpu',
    '--no-sandbox',
    '--window-size=390,844',
    baseUrl,
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
  chrome.stderr.on('data', (chunk) => { chromeStderr += chunk.toString(); });

  const target = await waitForPageTarget(20_000);
  cdp = createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: 'window.localStorage.removeItem("freehand-webui-selected-session");',
  });
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 2,
    mobile: true,
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(20_000);
  await waitForFunction(() => document.body.dataset.webuiJsReady === 'true', 30_000, 'production WebUI shell');

  await waitForFunction((targetSessionId) => {
    return !!document.querySelector(`#mobile-home-dashboard [data-session-id="${targetSessionId}"] .mobile-home-session-open`);
  }, 30_000, 'parent Home session row', parentSessionId);
  await evalInPage((targetSessionId) => {
    document.querySelector(`#mobile-home-dashboard [data-session-id="${targetSessionId}"] .mobile-home-session-open`)?.click();
  }, parentSessionId);
  await waitForFunction((targetSessionId) => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    return document.body.dataset.webuiRoute === 'session_detail' &&
      shell?.dataset.routeSession === targetSessionId;
  }, 30_000, 'selected parent SessionDetail', parentSessionId);

  await waitForFunction((targetTaskId) => {
    return !!document.querySelector(`#session-worker-rail .session-worker-row[data-task-id="${targetTaskId}"] .session-worker-pill`);
  }, 30_000, 'TaskBoard Worker rail row', taskId);
  await evalInPage((targetTaskId) => {
    document.querySelector(`#session-worker-rail .session-worker-row[data-task-id="${targetTaskId}"] .session-worker-pill`)?.click();
  }, taskId);
  await waitForFunction((targetTaskId) => {
    return !!document.querySelector(`#session-worker-rail .session-worker-row[data-task-id="${targetTaskId}"] .session-worker-open-button`);
  }, 10_000, 'expanded Worker detail action', taskId);
  await evalInPage((targetTaskId) => {
    document.querySelector(`#session-worker-rail .session-worker-row[data-task-id="${targetTaskId}"] .session-worker-open-button`)?.click();
  }, taskId);
  await waitForFunction((targetSessionId) => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    return document.body.dataset.webuiRoute === 'session_detail' &&
      shell?.dataset.routeSession === targetSessionId;
  }, 30_000, 'selected Worker SessionDetail', workerSessionId);

  const taskBoardResult = await pageAdpQuery({ QueryTaskBoard: { include_terminal: true } });
  const taskBoard = variant(taskBoardResult, 'TaskBoard');
  const task = (taskBoard.tasks || []).find((row) => row.task_id === taskId);
  if (!task) throw new Error(`TaskBoard missing ${taskId}`);
  if (task.parent_session_id !== parentSessionId) {
    throw new Error(`task parent mismatch: ${task.parent_session_id}`);
  }
  if (task.worker_session_id !== workerSessionId) {
    throw new Error(`task Worker session mismatch: ${task.worker_session_id}`);
  }
  if (`${task.status || ''}`.toLowerCase() !== 'closed') {
    throw new Error(`task is not closed: ${task.status}`);
  }

  const turnsResult = await pageAdpQuery({ QuerySessionTurns: { session_id: workerSessionId } });
  const transcript = variant(turnsResult, 'SessionTurns');
  const turns = transcript.turns || [];
  const failedIndex = turns.findIndex((turn) => turn.turn_id === oldFailedTurnId);
  const successIndex = turns.findIndex((turn) => turn.turn_id === laterSuccessTurnId);
  if (failedIndex < 0 || normalizeStatus(turns[failedIndex]?.terminal_status) !== 'failed') {
    throw new Error(`missing historical Failed turn ${oldFailedTurnId}`);
  }
  if (successIndex <= failedIndex || normalizeStatus(turns[successIndex]?.terminal_status) !== 'success') {
    throw new Error(`missing later Success turn ${laterSuccessTurnId}`);
  }

  const normal = await waitForFunction((failedTurnId, successTurnId) => {
    const oldCard = document.querySelector(`.turn-cycle-card[data-turn-id="${failedTurnId}"]`);
    const successCard = document.querySelector(`.turn-cycle-card[data-turn-id="${successTurnId}"]`);
    if (!oldCard || !successCard) return null;
    if (oldCard.dataset.recoveryState !== 'historical_failure_recovered') return null;
    const text = oldCard.innerText || '';
    if (!text.includes('历史失败 · 后续已恢复') || !text.includes('后续已恢复')) return null;
    return {
      route: document.body.dataset.webuiRoute || '',
      selectedSession: document.querySelector('[data-webui-shell="true"]')?.dataset.routeSession || '',
      relationKicker: document.getElementById('session-relation-kicker')?.innerText || '',
      relationMetrics: document.getElementById('session-relation-metrics')?.innerText || '',
      workerNavigation: document.getElementById('worker-session-nav')?.innerText || '',
      oldCard: {
        turnId: oldCard.dataset.turnId || '',
        recoveryState: oldCard.dataset.recoveryState || '',
        recoveryDebugDetails: oldCard.dataset.recoveryDebugDetails || '',
        text,
      },
      successCard: {
        turnId: successCard.dataset.turnId || '',
        lifecycleClass: successCard.dataset.lifecycleClass || '',
        text: successCard.innerText || '',
      },
    };
  }, 30_000, 'historical failure recovery projection', oldFailedTurnId, laterSuccessTurnId);

  if (/invalid api key|anthropic_http_status_401|openai_http_status_401/i.test(normal.oldCard.text)) {
    throw new Error('normal recovered cycle still exposes raw 401 text');
  }
  if (!normal.workerNavigation.includes('工作器会话')) {
    throw new Error(`Worker SessionDetail navigation missing: ${normal.workerNavigation}`);
  }
  if (!/关闭/.test(normal.relationMetrics)) {
    throw new Error(`SessionDetail Header does not project closed lifecycle: ${normal.relationMetrics}`);
  }

  const normalScreenshot = await cdp.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true });
  await fs.writeFile(
    path.join(artifactDir, 'worker-recovered-history-normal.png'),
    Buffer.from(normalScreenshot.data, 'base64'),
  );

  await evalInPage(() => document.getElementById('debug-details-toggle')?.click());
  const debug = await waitForFunction((failedTurnId) => {
    const card = document.querySelector(`.turn-cycle-card[data-turn-id="${failedTurnId}"]`);
    if (!card || card.dataset.recoveryDebugDetails !== 'true') return null;
    const text = card.innerText || '';
    if (!/invalid api key|anthropic_http_status_401|openai_http_status_401/i.test(text)) return null;
    return {
      recoveryState: card.dataset.recoveryState || '',
      recoveryDebugDetails: card.dataset.recoveryDebugDetails || '',
      text,
    };
  }, 15_000, 'debug details retain original failed turn', oldFailedTurnId);

  const screenshot = await cdp.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true });
  await fs.writeFile(path.join(artifactDir, 'worker-recovered-history-debug.png'), Buffer.from(screenshot.data, 'base64'));
  const summary = {
    baseUrl,
    assetVersion,
    fixture: { parentSessionId, taskId, workerSessionId, oldFailedTurnId, laterSuccessTurnId },
    ownerTruth: {
      taskStatus: task.status,
      failedIndex,
      failedStatus: turns[failedIndex].terminal_status,
      successIndex,
      successStatus: turns[successIndex].terminal_status,
    },
    normal,
    debug,
    screenshots: {
      normal: 'worker-recovered-history-normal.png',
      debug: 'worker-recovered-history-debug.png',
    },
    checks: {
      taskClosed: `${task.status || ''}`.toLowerCase() === 'closed',
      failedBeforeLaterSuccess: failedIndex >= 0 && successIndex > failedIndex,
      recoveryStateProjected: normal.oldCard.recoveryState === 'historical_failure_recovered',
      normalSummaryProjected: normal.oldCard.text.includes('历史失败 · 后续已恢复'),
      normalRaw401Suppressed: !/invalid api key|anthropic_http_status_401|openai_http_status_401/i.test(normal.oldCard.text),
      headerClosed: /关闭/.test(normal.relationMetrics),
      debugRawFailureRetained: /invalid api key|anthropic_http_status_401|openai_http_status_401/i.test(debug.text),
    },
  };
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  const failed = Object.entries(summary.checks).filter(([, ok]) => ok !== true).map(([name]) => name);
  if (failed.length > 0) throw new Error(`worker_recovered_history_failed checks=${failed.join(',')}`);
  console.log(`worker_recovered_history_ok task=${taskId} session=${workerSessionId} artifactDir=${artifactDir}`);
} catch (error) {
  let pageState = null;
  if (cdp) {
    pageState = await evalInPage(() => ({
      jsReady: document.body.dataset.webuiJsReady || '',
      route: document.body.dataset.webuiRoute || '',
      routeSession: document.querySelector('[data-webui-shell="true"]')?.dataset.routeSession || '',
      relationKicker: document.getElementById('session-relation-kicker')?.innerText || '',
      relationMetrics: document.getElementById('session-relation-metrics')?.innerText || '',
      workerNavigation: document.getElementById('worker-session-nav')?.innerText || '',
      cycles: Array.from(document.querySelectorAll('.turn-cycle-card')).map((card) => ({
        turnId: card.dataset.turnId || '',
        lifecycleClass: card.dataset.lifecycleClass || '',
        lifecyclePhase: card.dataset.lifecyclePhase || '',
        recoveryState: card.dataset.recoveryState || '',
        recoveryDebugDetails: card.dataset.recoveryDebugDetails || '',
        text: (card.innerText || '').slice(0, 1600),
      })),
      commandStatus: document.getElementById('command-status')?.innerText || '',
    })).catch((captureError) => ({ captureError: captureError.message }));
  }
  await fs.writeFile(path.join(artifactDir, 'failure.json'), JSON.stringify({
    error: error.message,
    stack: error.stack,
    pageState,
    chromeStderr: chromeStderr.slice(-12000),
  }, null, 2)).catch(() => null);
  throw error;
} finally {
  if (cdp) await cdp.close().catch(() => null);
  if (chrome?.pid) chrome.kill('SIGTERM');
  if (profileDir) await fs.rm(profileDir, { recursive: true, force: true }).catch(() => null);
}

async function assertProductionPageReachable() {
  const response = await fetch(baseUrl, { cache: 'no-store' });
  if (!response.ok) throw new Error(`production WebUI not reachable: ${response.status}`);
  const html = await response.text();
  await fs.writeFile(path.join(artifactDir, 'served-root.html'), html);
  if (!html.includes(assetVersion)) {
    throw new Error(`served WebUI asset version mismatch: expected ${assetVersion}`);
  }
}

async function pageAdpQuery(query, timeoutMs = 30_000) {
  return await adpVerifierRequest({
    url: adpUrl,
    kind: 'query',
    payloadKey: 'query',
    payload: query,
    timeoutMs,
    clientName: 'worker-recovered-history-verifier',
  });
}

function variant(result, name) {
  if (!result || typeof result !== 'object' || !Object.prototype.hasOwnProperty.call(result, name)) {
    throw new Error(`expected ${name}, got ${JSON.stringify(result)}`);
  }
  return result[name];
}

async function waitForPageTarget(timeoutMs) {
  return await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
    if (!response.ok) return null;
    const targets = await response.json();
    return targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl) || null;
  }, timeoutMs, 'Chrome DevTools page');
}

async function waitForLoad(timeoutMs) {
  await new Promise((resolve) => {
    const timer = setTimeout(() => {
      cdp.offEvent(listener);
      resolve();
    }, timeoutMs);
    const listener = (method) => {
      if (method !== 'Page.loadEventFired') return;
      clearTimeout(timer);
      cdp.offEvent(listener);
      resolve();
    };
    cdp.onEvent(listener);
  });
}

async function waitForFunction(fn, timeoutMs, label, ...args) {
  return await waitFor(async () => await evalInPage(fn, ...args), timeoutMs, label);
}

async function evalInPage(fn, ...args) {
  const response = await cdp.send('Runtime.evaluate', {
    expression: `(${fn.toString()})(...${JSON.stringify(args)})`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    throw new Error(response.exceptionDetails.exception?.description || response.exceptionDetails.text || 'Runtime.evaluate failed');
  }
  return response.result.value;
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

function createCdpClient(wsUrl) {
  const socket = new WebSocket(wsUrl);
  const pending = new Map();
  const listeners = new Set();
  let nextId = 0;
  const opened = new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', () => reject(new Error('CDP socket error')), { once: true });
  });
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      const waiter = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) waiter.reject(new Error(message.error.message || JSON.stringify(message.error)));
      else waiter.resolve(message.result || {});
      return;
    }
    if (message.method) listeners.forEach((listener) => listener(message.method, message.params || {}));
  });
  return {
    async send(method, params = {}) {
      await opened;
      const id = ++nextId;
      socket.send(JSON.stringify({ id, method, params }));
      return await new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
    },
    onEvent(listener) { listeners.add(listener); },
    offEvent(listener) { listeners.delete(listener); },
    async close() { socket.close(); },
  };
}

function normalizeStatus(value) { return `${value || ''}`.trim().toLowerCase(); }
function normalizedBaseUrl(value) { return value.endsWith('/') ? value : `${value}/`; }

function adpUrlFromBaseUrl(value) {
  const url = new URL(value);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  url.pathname = '/adp';
  url.search = '';
  return url.toString();
}
function delay(ms) { return new Promise((resolve) => setTimeout(resolve, ms)); }
