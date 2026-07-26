import { spawn } from 'node:child_process';
import fsSync from 'node:fs';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const repo = process.cwd();
const chromePath = process.env.FREEHAND_WEBUI_TOOLS_CHROME ||
  defaultBrowserPath();
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_TOOLS_DEBUG_PORT || '9261', 10);
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WEBUI_TOOLS_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_WEBUI_TOOLS_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const runStamp = new Date().toISOString().replace(/[-:.]/g, '').slice(0, 15);
const runId = `webui-tools-registry-${runStamp}-${process.pid}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);
const assetVersion = '20260726-session-select-rename';

await fs.mkdir(artifactDir, { recursive: true });

const chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-webui-tools-'));
let chrome = null;
let cdp = null;
let summary = null;

try {
  await waitHealth();
  await assertProductionPageReachable();
  const beforeSessions = sessionListPayload(await adpQuery('QuerySessionList'));
  const registry = toolRegistryPayload(await adpQuery('QueryToolRegistry'));
  await fs.writeFile(path.join(artifactDir, 'session-list-before.json'), JSON.stringify(beforeSessions, null, 2));
  await fs.writeFile(path.join(artifactDir, 'tool-registry-adp.json'), JSON.stringify(registry, null, 2));
  assertOwnerProjection(registry);

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
      !!document.getElementById('open-tools-dashboard-button') &&
      !!document.getElementById('tools-dashboard-dialog');
  }, 20_000, 'tool-capable WebUI shell ready');

  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
    document.getElementById('open-tools-dashboard-button')?.click();
  });
  await waitForFunction(cdp, () => {
    const dialog = document.getElementById('tools-dashboard-dialog');
    return !!dialog && dialog.open;
  }, 10_000, 'tools dashboard dialog open');

  const expectedNames = registry.tools.map((tool) => tool.name).filter(Boolean);
  const dom = await waitForFunction(cdp, (names) => {
    const rows = Array.from(document.querySelectorAll('.tool-registry-card'));
    const rowNames = rows.map((row) => row.dataset.toolName || '');
    if (names.every((name) => rowNames.includes(name))) {
      const dialog = document.getElementById('tools-dashboard-dialog');
      const bodyWidth = document.body.scrollWidth;
      const docWidth = document.documentElement.scrollWidth;
      return {
        dialogOpen: !!dialog?.open,
        statusText: document.getElementById('tools-dashboard-status')?.innerText || '',
        guidanceText: document.getElementById('tools-dashboard-guidance')?.innerText || '',
        commandStatusText: document.getElementById('command-status')?.innerText || '',
        rows: rows.map((row) => ({
          toolName: row.dataset.toolName || '',
          scope: row.dataset.scope || '',
          implemented: row.dataset.implemented || '',
          readOnly: row.dataset.readOnly || '',
          exposedToMaster: row.dataset.exposedToMaster || '',
          exposedToWorker: row.dataset.exposedToWorker || '',
          text: row.innerText || '',
        })),
        bodyText: document.body.innerText || '',
        noHorizontalOverflow: Math.max(bodyWidth, docWidth) <= window.innerWidth + 2,
      };
    }
    return null;
  }, 30_000, 'tool registry DOM rows', expectedNames);
  await fs.writeFile(path.join(artifactDir, 'tool-registry-dom.json'), JSON.stringify(dom, null, 2));
  await captureScreenshot(cdp, 'tools-registry-dashboard.png');

  const afterSessions = sessionListPayload(await adpQuery('QuerySessionList'));
  await fs.writeFile(path.join(artifactDir, 'session-list-after.json'), JSON.stringify(afterSessions, null, 2));

  const byName = Object.fromEntries(dom.rows.map((row) => [row.toolName, row]));
  summary = {
    ok: true,
    baseUrl,
    adpUrl,
    artifactDir,
    assetVersion,
    sourceAgentId: registry.source_agent_id,
    registryVersion: registry.registry_version,
    toolCount: registry.tools.length,
    screenshots: ['tools-registry-dashboard.png'],
    checks: {
      productionAssetVersion: true,
      dialogOpened: dom.dialogOpen === true,
      domMatchesAdpToolNames: expectedNames.every((name) => !!byName[name]),
      coreToolsVisible: ['task', 'timer', 'web_fetch', 'read_file', 'glob', 'ls'].every((name) => !!byName[name]),
      noLocalWebSearchTool: !expectedNames.includes('web_search') && !byName.web_search && !dom.bodyText.includes('data-tool-name="web_search"'),
      taskMasterOnly: byName.task?.exposedToMaster === 'true' && byName.task?.exposedToWorker === 'false',
      timerMasterOnly: byName.timer?.exposedToMaster === 'true' && byName.timer?.exposedToWorker === 'false',
      webFetchMasterWorker: byName.web_fetch?.exposedToMaster === 'true' && byName.web_fetch?.exposedToWorker === 'true',
      bashHiddenFromMasterWorker: byName.bash?.implemented === 'true' && byName.bash?.exposedToMaster === 'false' && byName.bash?.exposedToWorker === 'false',
      workerToolsVisible: byName.todo_write?.exposedToWorker === 'true' && byName.complete_step?.exposedToWorker === 'true',
      workerOnlyToolsHiddenFromMaster: byName.todo_write?.exposedToMaster === 'false' && byName.complete_step?.exposedToMaster === 'false',
      pathGuidanceVisible: /locked workspace/i.test(byName.glob?.text || '') &&
        /absolute/i.test(byName.glob?.text || '') &&
        /symlink/i.test(byName.glob?.text || '') &&
        /Leading-~|leading `~`/i.test(byName.glob?.text || ''),
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
    throw new Error(`webui_tools_registry_failed checks=${failed.join(',')} artifactDir=${artifactDir}`);
  }
  console.log(`webui_tools_registry_ok url=${baseUrl} adp=${adpUrl} tools=${registry.tools.length} artifactDir=${artifactDir}`);
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
  await adpQuery('QueryToolRegistry')
    .then((value) => fs.writeFile(path.join(failureDir, 'tool-registry.json'), JSON.stringify(value, null, 2)))
    .catch((queryError) => fs.writeFile(path.join(failureDir, 'tool-registry-error.txt'), queryError.stack || queryError.message));
  if (summary) {
    await fs.writeFile(path.join(failureDir, 'summary.partial.json'), JSON.stringify(summary, null, 2));
  }
}

function assertOwnerProjection(registry) {
  const names = registry.tools.map((tool) => tool.name);
  for (const required of ['task', 'timer', 'web_fetch', 'read_file', 'glob', 'ls', 'bash', 'todo_write', 'complete_step']) {
    if (!names.includes(required)) {
      throw new Error(`ADP tool registry missing ${required}`);
    }
  }
  if (names.includes('web_search')) {
    throw new Error('ADP tool registry exposed local web_search tool');
  }
  const task = toolByName(registry, 'task');
  const timer = toolByName(registry, 'timer');
  const webFetch = toolByName(registry, 'web_fetch');
  const bash = toolByName(registry, 'bash');
  if (!task.exposed_to_master || task.exposed_to_worker || task.execution_scope !== 'framework') {
    throw new Error('task projection is not Master-only framework scope');
  }
  if (!timer.exposed_to_master || timer.exposed_to_worker || timer.execution_scope !== 'framework') {
    throw new Error('timer projection is not Master-only framework scope');
  }
  if (!webFetch.exposed_to_master || !webFetch.exposed_to_worker || webFetch.execution_scope !== 'network') {
    throw new Error('web_fetch projection is not Master+Worker network scope');
  }
  if (!bash.implemented || bash.exposed_to_master || bash.exposed_to_worker || bash.execution_scope !== 'shell') {
    throw new Error('bash projection should be implemented but hidden from live Master/Worker');
  }
  const globText = [
    toolByName(registry, 'glob').description,
    ...(toolByName(registry, 'glob').guidance || []),
    ...(toolByName(registry, 'glob').examples || []),
  ].join('\n');
  if (!/locked workspace/i.test(globText) || !/absolute/i.test(globText) || !/symlink/i.test(globText) || !/Leading-~|leading `~`/i.test(globText)) {
    throw new Error('glob projection missing locked workspace absolute/symlink/leading-~ guidance');
  }
}

function toolByName(registry, name) {
  const tool = registry.tools.find((candidate) => candidate.name === name);
  if (!tool) {
    throw new Error(`missing tool ${name}`);
  }
  return tool;
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

async function captureScreenshot(cdpClient, fileName) {
  const screenshot = await cdpClient.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  await fs.writeFile(path.join(artifactDir, fileName), Buffer.from(screenshot.data, 'base64'));
}

function toolRegistryPayload(result) {
  const payload = result?.ToolRegistry || result?.tool_registry || result;
  return {
    source_agent_id: payload?.source_agent_id || '',
    generated_at: payload?.generated_at || 0,
    registry_version: payload?.registry_version || '',
    guidance: Array.isArray(payload?.guidance) ? payload.guidance : [],
    tools: Array.isArray(payload?.tools) ? payload.tools : [],
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
    // Fall through to the system Chrome path when Playwright is not installed.
  }
  return '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
