import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { adpVerifierRequest, requireSessionListPage } from './lib/adp-verifier-client.mjs';

const repo = process.cwd();
const chromePath = process.env.FREEHAND_WEBUI_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_TIMER_DEBUG_PORT || '9257', 10);
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WEBUI_TIMER_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_WEBUI_TIMER_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const adpAuthToken = process.env.FREEHAND_ADP_AUTH_TOKEN || '';
const runStamp = new Date().toISOString().replace(/[-:.]/g, '').slice(0, 15);
const runId = `webui-timer-dashboard-${runStamp}-${process.pid}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);
const assetVersion = '20260824-session-list-page';
const marker = `timer-dashboard-online-proof-${runStamp}-${process.pid}`;
const wakePrompt = `Inspect current framework truth for ${marker}, then decide the next Master action.`;
const createdTimerIds = new Set();

await fs.mkdir(artifactDir, { recursive: true });

const chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-webui-timer-'));
let chrome = null;
let cdp = null;
let summary = null;

try {
  await waitHealth();
  await assertProductionPageReachable();
  const beforeSessions = await activeSessionListPage();
  const before = timerListPayload(await adpQuery({ QueryTimerList: { include_terminal: true } }));
  await fs.writeFile(path.join(artifactDir, 'session-list-before.json'), JSON.stringify(beforeSessions, null, 2));
  await fs.writeFile(path.join(artifactDir, 'timer-list-before.json'), JSON.stringify(before, null, 2));

  chrome = spawn(
    chromePath,
    [
      '--headless=new',
      `--remote-debugging-port=${debugPort}`,
      '--remote-debugging-address=0.0.0.0',
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
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );
  const chromeLog = [];
  chrome.stdout.on('data', (chunk) => chromeLog.push(`[stdout] ${chunk}`));
  chrome.stderr.on('data', (chunk) => chromeLog.push(`[stderr] ${chunk}`));

  const pageTarget = await waitForPageTarget(baseUrl, 20_000);
  cdp = await createCdpClient(pageTarget.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 2,
    mobile: true,
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(cdp, () => {
    return document.body.dataset.webuiJsReady === 'true' &&
      !!document.querySelector('[data-webui-shell="true"]') &&
      !!document.getElementById('open-timer-dashboard-button');
  }, 20_000, 'Timer-capable WebUI shell 就绪');

  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
    document.getElementById('open-timer-dashboard-button')?.click();
  });
  await waitForFunction(cdp, () => {
    const dialog = document.getElementById('timer-dashboard-dialog');
    return !!dialog && dialog.open;
  }, 10_000, 'timer dashboard dialog open');

  await evalInPage(cdp, (reason, prompt) => {
    function setValue(id, value) {
      const node = document.getElementById(id);
      if (!node) {
        throw new Error(`missing timer form field ${id}`);
      }
      node.value = value;
      node.dispatchEvent(new Event('input', { bubbles: true }));
      node.dispatchEvent(new Event('change', { bubbles: true }));
    }
    setValue('timer-mode-input', 'relative');
    setValue('timer-delay-input', '900');
    setValue('timer-max-runs-input', '1');
    setValue('timer-source-session-input', '');
    setValue('timer-reason-input', reason);
    setValue('timer-prompt-input', prompt);
  }, marker, wakePrompt);
  await evalInPage(cdp, () => {
    document.getElementById('timer-dashboard-create-button')?.click();
  });

  await waitForFunction(cdp, (reason) => {
    const rows = Array.from(document.querySelectorAll('.timer-row'));
    return rows.some((row) => row.innerText.includes(reason) && row.dataset.timerId);
  }, 20_000, 'scheduled timer row in DOM', marker);
  const scheduledDom = await evalInPage(cdp, collectTimerDomState, marker);
  await fs.writeFile(path.join(artifactDir, 'dom-scheduled.json'), JSON.stringify(scheduledDom, null, 2));
  await captureScreenshot(cdp, 'timer-dashboard-scheduled.png');

  const afterSchedule = timerListPayload(await adpQuery({ QueryTimerList: { include_terminal: true } }));
  await fs.writeFile(path.join(artifactDir, 'timer-list-after-schedule.json'), JSON.stringify(afterSchedule, null, 2));
  const scheduledTimer = findTimerByMarker(afterSchedule, marker);
  if (!scheduledTimer) {
    throw new Error(`scheduled timer not visible in owner projection marker=${marker}`);
  }
  createdTimerIds.add(scheduledTimer.timer_id);
  if (scheduledTimer.status !== 'active') {
    throw new Error(`scheduled timer status is ${scheduledTimer.status}, expected active`);
  }

  await evalInPage(cdp, (timerId) => {
    const row = Array.from(document.querySelectorAll('.timer-row'))
      .find((candidate) => candidate.dataset.timerId === timerId);
    const button = row?.querySelector('.timer-cancel-button');
    if (!button) {
      throw new Error(`missing cancel button for ${timerId}`);
    }
    button.click();
  }, scheduledTimer.timer_id);

  await waitFor(async () => {
    const projection = timerListPayload(await adpQuery({ QueryTimerList: { include_terminal: true } }));
    const timer = projection.timers.find((candidate) => candidate.timer_id === scheduledTimer.timer_id);
    return timer?.status === 'cancelled' ? projection : null;
  }, 20_000, 'cancelled timer owner projection');
  await evalInPage(cdp, () => {
    document.getElementById('timer-dashboard-refresh-button')?.click();
  });
  await waitForFunction(cdp, (timerId) => {
    const text = document.getElementById('timer-dashboard-history')?.innerText || '';
    return text.includes('TimerCancelled') || text.includes(timerId) && text.includes('cancelled');
  }, 15_000, 'cancelled timer visible in DOM history', scheduledTimer.timer_id);
  const cancelledDom = await evalInPage(cdp, collectTimerDomState, marker);
  await fs.writeFile(path.join(artifactDir, 'dom-cancelled.json'), JSON.stringify(cancelledDom, null, 2));
  await captureScreenshot(cdp, 'timer-dashboard-cancelled.png');

  const afterCancel = timerListPayload(await adpQuery({ QueryTimerList: { include_terminal: true } }));
  const afterSessions = await activeSessionListPage();
  await fs.writeFile(path.join(artifactDir, 'timer-list-after-cancel.json'), JSON.stringify(afterCancel, null, 2));
  await fs.writeFile(path.join(artifactDir, 'session-list-after.json'), JSON.stringify(afterSessions, null, 2));
  const cancelledTimer = afterCancel.timers.find((candidate) => candidate.timer_id === scheduledTimer.timer_id);
  const cancelEvent = afterCancel.events.find((event) =>
    event.timer_id === scheduledTimer.timer_id && event.event_type === 'TimerCancelled'
  );

  summary = {
    ok: true,
    baseUrl,
    adpUrl,
    artifactDir,
    assetVersion,
    marker,
    scheduledTimerId: scheduledTimer.timer_id,
    beforeTimerCount: before.timers.length,
    afterScheduleTimerCount: afterSchedule.timers.length,
    afterCancelTimerCount: afterCancel.timers.length,
    screenshots: ['timer-dashboard-scheduled.png', 'timer-dashboard-cancelled.png'],
    checks: {
      productionAssetVersion: true,
      dialogOpened: scheduledDom.dialogOpen === true,
      domShowsScheduledTimer: scheduledDom.rows.some((row) =>
        row.timerId === scheduledTimer.timer_id && row.text.includes(marker)
      ),
      adpHasScheduledTimer: scheduledTimer.reason === marker && scheduledTimer.status === 'active',
      scheduleLedgerVisible: afterSchedule.events.some((event) =>
        event.timer_id === scheduledTimer.timer_id && event.event_type === 'TimerScheduled'
      ),
      cancelUpdatedAdpTruth: cancelledTimer?.status === 'cancelled',
      cancelLedgerVisible: !!cancelEvent,
      domShowsCancelHistory: cancelledDom.historyText.includes('TimerCancelled') ||
        cancelledDom.historyText.includes('cancelled'),
      noTopLevelSessionCreated: sessionIds(beforeSessions).join('\n') === sessionIds(afterSessions).join('\n'),
    },
  };
  summary.ok = Object.values(summary.checks).every(Boolean);
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  if (!summary.ok) {
    const failed = Object.entries(summary.checks)
      .filter(([, value]) => value !== true)
      .map(([key]) => key);
    throw new Error(`webui_timer_dashboard_failed checks=${failed.join(',')} artifactDir=${artifactDir}`);
  }
  console.log(`webui_timer_dashboard_ok url=${baseUrl} adp=${adpUrl} timer_id=${scheduledTimer.timer_id} artifactDir=${artifactDir}`);
} catch (error) {
  await writeFailure(error);
  throw error;
} finally {
  await cleanupCreatedTimers();
  if (cdp) {
    await cdp.close().catch(() => null);
  }
  if (chrome && chrome.pid) {
    chrome.kill('SIGTERM');
  }
  await fs.rm(chromeProfileDir, { recursive: true, force: true }).catch(() => null);
}

async function cleanupCreatedTimers() {
  try {
    const projection = timerListPayload(await adpQuery({ QueryTimerList: { include_terminal: true } }));
    for (const timer of projection.timers) {
      if (timer.reason !== marker || timer.status !== 'active') {
        continue;
      }
      createdTimerIds.add(timer.timer_id);
    }
    for (const timerId of createdTimerIds) {
      const timer = projection.timers.find((candidate) => candidate.timer_id === timerId);
      if (timer?.status === 'active') {
        await adpCommand({ CancelTimer: { timer_id: timerId } }, 20_000).catch(() => null);
      }
    }
  } catch (_) {
    // Cleanup is best-effort for verifier-owned timers only; failure is recorded by main proof.
  }
}

async function writeFailure(error) {
  const failureDir = path.join(artifactDir, 'failure');
  await fs.mkdir(failureDir, { recursive: true });
  await fs.writeFile(path.join(failureDir, 'error.txt'), error.stack || error.message);
  await adpQuery({ QueryTimerList: { include_terminal: true } })
    .then((value) => fs.writeFile(path.join(failureDir, 'timer-list.json'), JSON.stringify(value, null, 2)))
    .catch((queryError) => fs.writeFile(path.join(failureDir, 'timer-list-error.txt'), queryError.stack || queryError.message));
  if (summary) {
    await fs.writeFile(path.join(failureDir, 'summary.partial.json'), JSON.stringify(summary, null, 2));
  }
}

async function waitHealth() {
  await waitFor(async () => {
    const response = await fetch(new URL('/health', baseUrl));
    return response.ok && (await response.text()).trim() === 'ok';
  }, 60_000, 'S-profile health');
}

async function assertProductionPageReachable() {
  const response = await fetch(baseUrl, { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`production WebUI not reachable: ${response.status} ${response.statusText}`);
  }
  const html = await response.text();
  if (!html.includes(assetVersion)) {
    throw new Error(`served WebUI asset version mismatch: expected ${assetVersion}`);
  }
}

function collectTimerDomState(markerText) {
  const dialog = document.getElementById('timer-dashboard-dialog');
  const rows = Array.from(document.querySelectorAll('.timer-row')).map((row) => ({
    timerId: row.dataset.timerId || '',
    text: row.innerText || '',
    hasCancel: !!row.querySelector('.timer-cancel-button'),
  }));
  const historyText = document.getElementById('timer-dashboard-history')?.innerText || '';
  const bodyWidth = document.body.scrollWidth;
  const docWidth = document.documentElement.scrollWidth;
  return {
    dialogOpen: !!dialog?.open,
    statusText: document.getElementById('timer-dashboard-status')?.innerText || '',
    commandStatusText: document.getElementById('command-status')?.innerText || '',
    markerVisible: document.body.innerText.includes(markerText),
    historyText,
    rows,
    noHorizontalOverflow: Math.max(bodyWidth, docWidth) <= window.innerWidth + 2,
  };
}

async function captureScreenshot(cdpClient, fileName) {
  const screenshot = await cdpClient.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  await fs.writeFile(path.join(artifactDir, fileName), Buffer.from(screenshot.data, 'base64'));
}

function findTimerByMarker(projection, markerText) {
  return projection.timers.find((timer) => timer.reason === markerText) || null;
}

function timerListPayload(result) {
  const payload = result?.TimerList || result?.timer_list || result;
  return {
    source_agent_id: payload?.source_agent_id || '',
    generated_at: payload?.generated_at || 0,
    include_terminal: !!payload?.include_terminal,
    timers: Array.isArray(payload?.timers) ? payload.timers : [],
    events: Array.isArray(payload?.events) ? payload.events : [],
  };
}

function sessionListPayload(result) {
  return requireSessionListPage(result);
}

function sessionIds(list) {
  return list.sessions
    .map((session) => session.session_id || '')
    .filter(Boolean)
    .sort();
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

async function adpCommand(command, timeoutMs = 30_000) {
  return await adpRequest('command', 'command', command, timeoutMs);
}

function adpRequest(kind, payloadKey, payload, timeoutMs) {
  return adpVerifierRequest({
    url: adpUrl,
    authToken: adpAuthToken,
    kind,
    payloadKey,
    payload,
    timeoutMs,
    clientName: 'freehand-timer-dashboard-verifier',
  });
}

async function waitForPageTarget(urlPrefix, timeoutMs) {
  return await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
    if (!response.ok) {
      return null;
    }
    const targets = await response.json();
    return targets.find((target) =>
      target.type === 'page' && `${target.url || ''}`.startsWith(urlPrefix)
    ) || targets.find((target) => target.type === 'page') || null;
  }, timeoutMs, 'Chrome DevTools page target');
}

async function waitForFunction(cdpClient, fn, timeoutMs, label, ...args) {
  return await waitFor(async () => {
    return await evalInPage(cdpClient, fn, ...args);
  }, timeoutMs, label);
}

async function waitFor(fn, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const result = await fn();
      if (result) {
        return result;
      }
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  const suffix = lastError ? `: ${lastError.message}` : '';
  throw new Error(`timeout waiting for ${label}${suffix}`);
}

async function evalInPage(cdpClient, fn, ...args) {
  const response = await cdpClient.send('Runtime.evaluate', {
    expression: `(${fn.toString()})(...${JSON.stringify(args)})`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    const details = response.exceptionDetails;
    throw new Error(details.exception?.description || details.text || 'Runtime.evaluate failed');
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
        close() {
          socket.close();
          return Promise.resolve();
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
    socket.addEventListener('error', () => reject(new Error('CDP socket error')));
  });
}

function adpUrlFromBaseUrl(value) {
  const url = new URL(value);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  url.pathname = path.posix.join(url.pathname, 'adp');
  url.search = '';
  url.hash = '';
  return url.toString();
}

function normalizedBaseUrl(value) {
  const url = new URL(value);
  if (!url.pathname.endsWith('/')) {
    url.pathname = `${url.pathname}/`;
  }
  return url.toString();
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
