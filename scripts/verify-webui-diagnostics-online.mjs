import { spawn } from 'node:child_process';
import fsSync from 'node:fs';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const repo = process.cwd();
const chromePath = process.env.FREEHAND_WEBUI_DIAGNOSTICS_CHROME || defaultBrowserPath();
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_DIAGNOSTICS_DEBUG_PORT || '9279', 10);
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WEBUI_DIAGNOSTICS_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_WEBUI_DIAGNOSTICS_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const runId = `webui-diagnostics-${Date.now()}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);
const assetVersion = '20260725-settings-layer-ui';
const forbiddenPattern = /\/Users\/|\/Volumes\/|authorization|api_key|apikey|x-api-key|bearer |pair_token|secret|provider request|provider payload/i;

await fs.mkdir(artifactDir, { recursive: true });
const chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-webui-diagnostics-'));
let chrome = null;
let cdp = null;
let summary = null;

try {
  await waitHealth();
  await assertProductionPageReachable();
  const beforeSessions = sessionListPayload(await adpQuery('QuerySessionList'));
  const diagnostics = diagnosticsPayload(await adpQuery('QueryDiagnostics'));
  await fs.writeFile(path.join(artifactDir, 'session-list-before.json'), JSON.stringify(beforeSessions, null, 2));
  await fs.writeFile(path.join(artifactDir, 'diagnostics-adp.json'), JSON.stringify(diagnostics, null, 2));
  assertDiagnosticsProjection(diagnostics);

  chrome = spawn(chromePath, [
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
  ], { stdio: ['ignore', 'pipe', 'pipe'] });

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
      !!document.getElementById('open-settings-drawer-button') &&
      !!document.getElementById('settings-diagnostics-refresh-button');
  }, 20_000, 'diagnostics-capable WebUI shell ready');

  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
    document.getElementById('open-settings-drawer-button')?.click();
  });
  await waitForFunction(cdp, () => {
    return !document.getElementById('settings-shell')?.hidden &&
      !!document.querySelector('.settings-diagnostics-page');
  }, 10_000, 'diagnostics top-level entry visible');

  await evalInPage(cdp, () => {
    const diagnostics = document.querySelector('.settings-diagnostics-page');
    if (diagnostics && !diagnostics.open) {
      diagnostics.querySelector('summary')?.click();
    }
    document.getElementById('settings-diagnostics-refresh-button')?.click();
  });
  const expectedNames = diagnostics.files.map((file) => file.name).filter(Boolean).slice(0, 8);
  const dom = await waitForFunction(cdp, (names) => {
    const rows = Array.from(document.querySelectorAll('.diagnostic-log-row'));
    const rowNames = rows.map((row) => row.dataset.logName || '');
    const summaryText = document.getElementById('settings-diagnostics-summary')?.innerText || '';
    if (names.length === 0 || names.every((name) => rowNames.includes(name))) {
      return {
        summaryText,
        runtimeHomeText: document.getElementById('settings-diagnostics-runtime-home')?.innerText || '',
        statusText: document.getElementById('settings-diagnostics-status')?.innerText || '',
        diagnosticsPageOpen: document.querySelector('.settings-diagnostics-page')?.open === true,
        diagnosticsTopLevelText: document.querySelector('.settings-diagnostics-page > summary')?.innerText || '',
        rows: rows.map((row) => ({
          logName: row.dataset.logName || '',
          relativePath: row.dataset.relativePath || '',
          text: row.innerText || '',
        })),
        cardText: document.getElementById('settings-diagnostics-list')?.innerText || '',
        bodyText: document.body.innerText || '',
        noHorizontalOverflow: Math.max(document.body.scrollWidth, document.documentElement.scrollWidth) <= window.innerWidth + 2,
      };
    }
    return null;
  }, 30_000, 'diagnostics DOM rows', expectedNames);
  await fs.writeFile(path.join(artifactDir, 'diagnostics-dom.json'), JSON.stringify(dom, null, 2));
  await captureScreenshot(cdp, 'diagnostics-settings.png');

  const afterSessions = sessionListPayload(await adpQuery('QuerySessionList'));
  await fs.writeFile(path.join(artifactDir, 'session-list-after.json'), JSON.stringify(afterSessions, null, 2));

  summary = {
    ok: true,
    baseUrl,
    adpUrl,
    artifactDir,
    assetVersion,
    sourceAgentId: diagnostics.source_agent_id,
    files: diagnostics.files.length,
    screenshots: ['diagnostics-settings.png'],
    checks: {
      productionAssetVersion: true,
      adpProjectionSafe: diagnosticsProjectionSafe(diagnostics),
      diagnosticsOpenedAsSeparateEntry: dom.diagnosticsPageOpen === true && /Diagnostics/.test(dom.diagnosticsTopLevelText),
      runtimeHomeRedacted: diagnostics.runtime_home === '~/.freehand' && dom.runtimeHomeText.includes('~/.freehand'),
      logsDirRelative: diagnostics.logs_dir === 'logs' && diagnostics.files.every((file) => `${file.relative_path || ''}`.startsWith('logs/')),
      domRowsMatchAdp: expectedNames.every((name) => dom.rows.some((row) => row.logName === name)),
      domNoSecretsOrAbsolutePaths: !forbiddenPattern.test(dom.cardText),
      noTopLevelSessionCreated: sessionIds(beforeSessions).join('\n') === sessionIds(afterSessions).join('\n'),
      noHorizontalOverflow: dom.noHorizontalOverflow === true,
    },
  };
  summary.ok = Object.values(summary.checks).every(Boolean);
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  if (!summary.ok) {
    const failed = Object.entries(summary.checks)
      .filter(([, value]) => value !== true)
      .map(([key]) => key);
    throw new Error(`webui_diagnostics_failed checks=${failed.join(',')} artifactDir=${artifactDir}`);
  }
  console.log(`webui_diagnostics_ok url=${baseUrl} adp=${adpUrl} files=${diagnostics.files.length} artifactDir=${artifactDir}`);
} catch (error) {
  await writeFailure(error);
  throw error;
} finally {
  if (cdp) {
    await cdp.close().catch(() => null);
  }
  if (chrome && chrome.pid) {
    chrome.kill('SIGTERM');
  }
  await fs.rm(chromeProfileDir, { recursive: true, force: true }).catch(() => null);
}

async function writeFailure(error) {
  const failureDir = path.join(artifactDir, 'failure');
  await fs.mkdir(failureDir, { recursive: true });
  await fs.writeFile(path.join(failureDir, 'error.txt'), error.stack || error.message);
  await adpQuery('QueryDiagnostics')
    .then((value) => fs.writeFile(path.join(failureDir, 'diagnostics.json'), JSON.stringify(value, null, 2)))
    .catch((queryError) => fs.writeFile(path.join(failureDir, 'diagnostics-error.txt'), queryError.stack || queryError.message));
  if (summary) {
    await fs.writeFile(path.join(failureDir, 'summary.partial.json'), JSON.stringify(summary, null, 2));
  }
}

function assertDiagnosticsProjection(diagnostics) {
  if (diagnostics.runtime_home !== '~/.freehand') {
    throw new Error(`diagnostics runtime home leaked or changed: ${diagnostics.runtime_home}`);
  }
  if (diagnostics.logs_dir !== 'logs') {
    throw new Error(`diagnostics logs_dir is not relative logs: ${diagnostics.logs_dir}`);
  }
  if (!Array.isArray(diagnostics.files)) {
    throw new Error('diagnostics files missing');
  }
  if (!diagnosticsProjectionSafe(diagnostics)) {
    throw new Error('diagnostics projection contains forbidden absolute path or sensitive marker');
  }
}

function diagnosticsProjectionSafe(diagnostics) {
  const raw = JSON.stringify(diagnostics);
  return !forbiddenPattern.test(raw) &&
    diagnostics.files.every((file) => `${file.relative_path || ''}`.startsWith('logs/') && `${file.name || ''}`.endsWith('.log'));
}

function diagnosticsPayload(result) {
  const payload = result?.Diagnostics || result?.diagnostics || result;
  return {
    source_agent_id: payload?.source_agent_id || '',
    generated_at: payload?.generated_at || 0,
    runtime_home: payload?.runtime_home || '',
    logs_dir: payload?.logs_dir || '',
    files: Array.isArray(payload?.files) ? payload.files : [],
  };
}

function sessionListPayload(result) {
  const payload = result?.SessionList || result?.session_list || result;
  return {
    sessions: Array.isArray(payload?.sessions) ? payload.sessions : [],
    archived: Array.isArray(payload?.archived) ? payload.archived : [],
  };
}

function sessionIds(list) {
  return (list.sessions || [])
    .map((session) => session.session_id || '')
    .filter(Boolean)
    .sort();
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
    socket.addEventListener('open', () => {
      socket.send(JSON.stringify({ kind, request_id: requestId, [payloadKey]: payload }));
    });
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data);
      if (message.request_id !== requestId) {
        return;
      }
      clearTimeout(timer);
      socket.close();
      if (message.kind === 'failure') {
        reject(new Error(message.failure?.message || message.failure?.code || 'ADP failure'));
        return;
      }
      if (message.kind === 'query_result') {
        resolve(message.result);
        return;
      }
      reject(new Error(`unexpected ADP ${kind} response: ${message.kind}`));
    });
    socket.addEventListener('error', () => {
      clearTimeout(timer);
      reject(new Error(`ADP ${kind} socket error`));
    });
  });
}

async function captureScreenshot(cdpClient, fileName) {
  const screenshot = await cdpClient.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  await fs.writeFile(path.join(artifactDir, fileName), Buffer.from(screenshot.data, 'base64'));
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
  return await waitFor(async () => await evalInPage(cdpClient, fn, ...args), timeoutMs, label);
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

function defaultBrowserPath() {
  const playwrightCache = path.join(os.homedir(), 'Library', 'Caches', 'ms-playwright');
  try {
    const shellPath = fsSync.readdirSync(playwrightCache)
      .filter((entry) => /^chromium_headless_shell-\d+$/.test(entry))
      .sort((left, right) => Number(right.split('-').at(-1)) - Number(left.split('-').at(-1)))
      .map((entry) => path.join(playwrightCache, entry, 'chrome-mac', 'headless_shell'))
      .find((candidate) => fsSync.existsSync(candidate));
    if (shellPath) {
      return shellPath;
    }
  } catch (_) {
    // Fall through to system Chrome when Playwright is not installed.
  }
  return '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
