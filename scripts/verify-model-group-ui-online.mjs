import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const configPath = process.env.FREEHAND_MODEL_GROUP_UI_CONFIG || path.join(runtimeHome, 'config.toml');
const envPath = process.env.FREEHAND_MODEL_GROUP_UI_ENV || path.join(runtimeHome, 'daemonS.env');
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_MODEL_GROUP_UI_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_MODEL_GROUP_UI_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const cli = process.env.FREEHAND_MODEL_GROUP_UI_CLI || path.join(home, '.local/bin/freehand-cliS');
const debugPort = Number.parseInt(process.env.FREEHAND_MODEL_GROUP_UI_DEBUG_PORT || '9259', 10);
const chromePath =
  process.env.FREEHAND_MODEL_GROUP_UI_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const assetVersion = '20260725-diagnostics-ui';
const runId = `model-group-ui-${Date.now()}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);
const testGroupId = process.env.FREEHAND_MODEL_GROUP_UI_GROUP_ID || `ui.verify.${Date.now()}`;
const primaryProviderId = process.env.FREEHAND_MODEL_GROUP_UI_PRIMARY_PROVIDER || 'cc';
const fallbackProviderId = process.env.FREEHAND_MODEL_GROUP_UI_FALLBACK_PROVIDER || 'minimax';
const primaryModel = process.env.FREEHAND_MODEL_GROUP_UI_PRIMARY_MODEL || 'gpt-5.5-model-group-ui';
const subModel = process.env.FREEHAND_MODEL_GROUP_UI_SUB_MODEL || 'gpt-5.5-model-group-sub';
const searchModel = process.env.FREEHAND_MODEL_GROUP_UI_SEARCH_MODEL || 'gpt-5.5-model-group-search';
const titleModel = process.env.FREEHAND_MODEL_GROUP_UI_TITLE_MODEL || 'gpt-5.5-model-group-title';
const fallbackModel = process.env.FREEHAND_MODEL_GROUP_UI_FALLBACK_MODEL || 'MiniMax-M3-model-group-fallback';
const loadBalanceText =
  process.env.FREEHAND_MODEL_GROUP_UI_LOAD_BALANCE ||
  `${primaryProviderId}:gpt-5.5-model-group-balanced:2, ${fallbackProviderId}:MiniMax-M3-model-group-balanced:1`;

let chrome;
let cdp;
let chromeProfileDir = null;
let restored = false;
const restoreErrors = [];
const pageEvents = [];
let chromeStdout = '';
let chromeStderr = '';
let originalStatus = null;

await fs.mkdir(artifactDir, { recursive: true });
const originalConfig = await fs.readFile(configPath, 'utf8');
const originalEnv = await fs.readFile(envPath, 'utf8').catch(() => '');

try {
  await writeFile('config.before.toml', redactConfig(originalConfig));
  await writeFile('daemonS.before.env', redactEnv(originalEnv));

  await waitHealth();
  await assertProductionPageReachable();
  originalStatus = unwrapConfigStatus(await adpQuery('QueryConfigStatus'));
  const initialCli = await must([cli, 'adp-config-query', '--url', adpUrl]);
  assertNoSecrets(initialCli.stdout, 'initial CLI config output');
  assertEnabledProvider(originalStatus, primaryProviderId);
  assertEnabledProvider(originalStatus, fallbackProviderId);

  chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-model-group-ui-'));
  chrome = spawn(
    chromePath,
    [
      '--headless=new',
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${chromeProfileDir}`,
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-background-networking',
      '--disable-extensions',
      '--disable-sync',
      '--window-size=1500,1000',
      baseUrl,
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );
  chrome.stdout.on('data', (chunk) => {
    chromeStdout += chunk.toString();
  });
  chrome.stderr.on('data', (chunk) => {
    chromeStderr += chunk.toString();
  });

  const target = await waitPageTarget();
  cdp = await createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Log.enable');
  cdp.onEvent(recordPageEvent);
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(
    cdp,
    () => !!document.querySelector('[data-webui-shell="true"]') &&
      document.body.dataset.webuiJsReady === 'true' &&
      !!document.getElementById('settings-model-group-form'),
    30_000,
    'model group capable WebUI shell ready',
  );

  await openSettings(cdp);
  const initialDom = await waitForFunction(
    cdp,
    () => {
      const state = readModelGroupSettingsDom();
      return state.providerIds.includes('cc') &&
        state.providerIds.includes('minimax') &&
        state.modelGroupSummary !== 'loading'
        ? state
        : null;
    },
    30_000,
    'model group settings loaded',
  );
  await writeJson('01-initial-dom.json', initialDom);

  await evalInPage(cdp, (input) => {
    const setValue = (id, value) => {
      const element = document.getElementById(id);
      if (!element) {
        throw new Error(`missing ${id}`);
      }
      element.value = value;
      element.dispatchEvent(new Event('input', { bubbles: true }));
      element.dispatchEvent(new Event('change', { bubbles: true }));
    };
    const enabled = document.getElementById('settings-model-group-enabled-input');
    if (enabled) {
      enabled.checked = true;
      enabled.dispatchEvent(new Event('change', { bubbles: true }));
    }
    setValue('settings-model-group-id-input', input.groupId);
    setValue('settings-model-group-label-input', 'UI verifier model group');
    setValue('settings-model-group-primary-provider-input', input.primaryProviderId);
    setValue('settings-model-group-primary-model-input', input.primaryModel);
    setValue('settings-model-group-sub-provider-input', input.primaryProviderId);
    setValue('settings-model-group-sub-model-input', input.subModel);
    setValue('settings-model-group-search-provider-input', input.primaryProviderId);
    setValue('settings-model-group-search-model-input', input.searchModel);
    setValue('settings-model-group-title-provider-input', input.primaryProviderId);
    setValue('settings-model-group-title-model-input', input.titleModel);
    setValue('settings-model-group-fallback-provider-input', input.fallbackProviderId);
    setValue('settings-model-group-fallback-model-input', input.fallbackModel);
    setValue('settings-model-group-load-balance-input', input.loadBalanceText);
    document.getElementById('settings-model-group-form')?.requestSubmit();
  }, {
    groupId: testGroupId,
    primaryProviderId,
    fallbackProviderId,
    primaryModel,
    subModel,
    searchModel,
    titleModel,
    fallbackModel,
    loadBalanceText,
  });

  const afterUpsertDom = await waitForFunction(
    cdp,
    (groupId) => {
      const state = readModelGroupSettingsDom();
      return state.modelGroupIds.includes(groupId) &&
        /model group saved/i.test(state.saveStatus + ' ' + state.commandStatus)
        ? state
        : null;
    },
    30_000,
    'model group upsert reflected in DOM',
    testGroupId,
  );
  await writeJson('02-after-upsert-dom.json', afterUpsertDom);
  const afterUpsertAdp = unwrapConfigStatus(await adpQuery('QueryConfigStatus'));
  await writeJson('03-after-upsert-adp.json', afterUpsertAdp);
  const upsertedGroup = findModelGroup(afterUpsertAdp, testGroupId);
  if (!upsertedGroup) {
    throw new Error(`ADP config status missing model group ${testGroupId}`);
  }
  assertRoute(upsertedGroup.primary, primaryProviderId, primaryModel, 'upsert primary');
  assertRoute(upsertedGroup.sub, primaryProviderId, subModel, 'upsert sub');
  assertRoute(upsertedGroup.search, primaryProviderId, searchModel, 'upsert search');
  assertRoute(upsertedGroup.title, primaryProviderId, titleModel, 'upsert title');
  assertRoute(upsertedGroup.fallback, fallbackProviderId, fallbackModel, 'upsert fallback');
  if (afterUpsertAdp.provider_id !== originalStatus.provider_id) {
    throw new Error(`model group upsert changed active provider from ${originalStatus.provider_id} to ${afterUpsertAdp.provider_id}`);
  }

  await evalInPage(cdp, (groupId) => {
    const select = document.getElementById('settings-model-group-current-select');
    const button = document.getElementById('settings-model-group-switch-button');
    if (!select || !button) {
      throw new Error('missing model group switch controls');
    }
    select.value = groupId;
    select.dispatchEvent(new Event('change', { bubbles: true }));
    button.click();
  }, testGroupId);

  const afterSwitchDom = await waitForFunction(
    cdp,
    (groupId) => {
      const state = readModelGroupSettingsDom();
      return state.currentModelGroup === groupId &&
        /model group selection saved/i.test(state.switchStatus + ' ' + state.commandStatus)
        ? state
        : null;
    },
    30_000,
    'model group switch reflected in DOM',
    testGroupId,
  );
  await writeJson('04-after-switch-dom.json', afterSwitchDom);
  const afterSwitchAdp = unwrapConfigStatus(await adpQuery('QueryConfigStatus'));
  await writeJson('05-after-switch-adp.json', afterSwitchAdp);
  if (afterSwitchAdp.model_group_id !== testGroupId) {
    throw new Error(`active model group ${afterSwitchAdp.model_group_id || 'none'} != ${testGroupId}`);
  }
  if (afterSwitchAdp.provider_id !== primaryProviderId) {
    throw new Error(`model group did not project primary provider ${primaryProviderId}: ${afterSwitchAdp.provider_id}`);
  }
  if (afterSwitchAdp.default_model !== primaryModel) {
    throw new Error(`model group did not project primary model ${primaryModel}: ${afterSwitchAdp.default_model}`);
  }
  if ((afterSwitchAdp.fallback_provider_id || null) !== fallbackProviderId) {
    throw new Error(`model group did not project fallback provider ${fallbackProviderId}: ${afterSwitchAdp.fallback_provider_id || 'none'}`);
  }

  await writeJson('summary.before-restore.json', {
    runId,
    url: baseUrl,
    adpUrl,
    testGroupId,
    initial: configSummary(originalStatus),
    afterUpsert: configSummary(afterUpsertAdp),
    afterSwitch: configSummary(afterSwitchAdp),
    domAfterUpsert: afterUpsertDom,
    domAfterSwitch: afterSwitchDom,
  });
} catch (error) {
  await writeFailure(error).catch(() => {});
  throw error;
} finally {
  if (cdp) {
    await captureBrowserDebug('zz-finally-browser-debug.json').catch(() => {});
    cdp.offEvent(recordPageEvent);
    await cdp.close().catch(() => {});
  }
  if (chrome && chrome.pid) {
    chrome.kill('SIGTERM');
  }
  if (chromeProfileDir) {
    await fs.rm(chromeProfileDir, { recursive: true, force: true }).catch(() => null);
  }
  try {
    await fs.writeFile(configPath, originalConfig);
    await fs.writeFile(envPath, originalEnv);
    await must(['scripts/install-launchd.sh', 'restartS']);
    await waitHealth();
    restored = true;
  } catch (error) {
    restoreErrors.push(error.message);
  }
}

const finalAdp = unwrapConfigStatus(await adpQuery('QueryConfigStatus'));
const finalCli = await must([cli, 'adp-config-query', '--url', adpUrl]);
assertNoSecrets(finalCli.stdout, 'final CLI config output');
const finalEnvGrep = await grepFixtureEnv();
const finalSummary = {
  runId,
  restored,
  restoreErrors,
  url: baseUrl,
  adpUrl,
  testGroupId,
  final: configSummary(finalAdp),
  finalCli: finalCli.stdout.trim(),
  finalEnvGrep,
};
await writeJson('summary.json', finalSummary);

if (!restored || restoreErrors.length > 0) {
  throw new Error(`restore failed: ${restoreErrors.join('; ')}`);
}
if (findModelGroup(finalAdp, testGroupId)) {
  throw new Error(`test model group remained after restore: ${testGroupId}`);
}
if (finalEnvGrep.matchCount !== 0) {
  throw new Error(`fixture env remained in daemonS.env: ${finalEnvGrep.output}`);
}
if (!sameConfigSelection(finalAdp, originalStatus)) {
  throw new Error(`final config selection differs from original: final=${JSON.stringify(configSummary(finalAdp))} original=${JSON.stringify(configSummary(originalStatus))}`);
}

console.log(
  [
    'model_group_ui_online_ok',
    `url=${adpUrl}`,
    `run_id=${runId}`,
    `group=${testGroupId}`,
    `projected_provider=${primaryProviderId}`,
    `projected_model=${primaryModel}`,
    `projected_fallback=${fallbackProviderId}`,
    `final_provider=${finalAdp.provider_id}`,
    `final_model=${finalAdp.default_model}`,
    `final_group=${finalAdp.model_group_id || 'none'}`,
    `artifact=${artifactDir}`,
  ].join(' '),
);

function normalizedBaseUrl(value) {
  return value.endsWith('/') ? value : `${value}/`;
}

function adpUrlFromBaseUrl(value) {
  const url = new URL(value);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  url.pathname = path.posix.join(url.pathname, 'adp');
  return url.toString();
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

async function openSettings(cdpClient) {
  await evalInPage(cdpClient, () => {
    document.getElementById('open-settings-drawer-button')?.click();
    if (document.getElementById('settings-shell')?.hidden !== false) {
      document.getElementById('settings-shell-toggle')?.click();
    }
  });
  await waitForFunction(
    cdpClient,
    () => document.getElementById('settings-shell')?.hidden === false,
    10_000,
    'settings shell visible',
  );
}

function unwrapConfigStatus(result) {
  if (!result || !result.ConfigStatus) {
    throw new Error(`missing ConfigStatus result: ${JSON.stringify(result)}`);
  }
  return result.ConfigStatus;
}

function providerIds(status) {
  return (status.provider_registry || []).map((provider) => provider.provider_id).filter(Boolean);
}

function assertEnabledProvider(status, providerId) {
  const provider = (status.provider_registry || []).find((candidate) => candidate.provider_id === providerId);
  if (!provider) {
    throw new Error(`provider ${providerId} missing from registry: ${providerIds(status).join(',')}`);
  }
  if (provider.enabled === false) {
    throw new Error(`provider ${providerId} is disabled`);
  }
}

function findModelGroup(status, groupId) {
  return (status.model_group_registry || []).find((group) => group.group_id === groupId) || null;
}

function assertRoute(route, providerId, model, label) {
  if (!route) {
    throw new Error(`${label} route missing`);
  }
  if (route.provider_id !== providerId || route.model !== model) {
    throw new Error(`${label} route ${route.provider_id}:${route.model} != ${providerId}:${model}`);
  }
}

function configSummary(status) {
  return {
    provider_id: status?.provider_id || '',
    fallback_provider_id: status?.fallback_provider_id || null,
    model_group_id: status?.model_group_id || null,
    provider_protocol: status?.provider_protocol || '',
    provider_base_url_host: status?.provider_base_url_host || '',
    default_model: status?.default_model || '',
    provider_web_search: status?.provider_web_search || '',
    provider_web_search_effective: status?.provider_web_search_effective || '',
    provider_auth_source: status?.provider_auth_source || '',
    groups: (status?.model_group_registry || []).map((group) => group.group_id).sort(),
  };
}

function sameConfigSelection(a, b) {
  const left = configSummary(a);
  const right = configSummary(b);
  return left.provider_id === right.provider_id &&
    left.fallback_provider_id === right.fallback_provider_id &&
    left.model_group_id === right.model_group_id &&
    left.provider_protocol === right.provider_protocol &&
    left.provider_base_url_host === right.provider_base_url_host &&
    left.default_model === right.default_model &&
    left.provider_web_search === right.provider_web_search &&
    left.provider_web_search_effective === right.provider_web_search_effective &&
    left.provider_auth_source === right.provider_auth_source;
}

function assertNoSecrets(value, label) {
  if (/api_key|pair_token|sk-|secret/i.test(value)) {
    throw new Error(`${label} contains secret-looking text`);
  }
}

async function adpQuery(query) {
  return await adpRequest('query', 'query', query, 20_000);
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
        reject(new Error((message.failure && (message.failure.message || message.failure.code)) || 'ADP failure'));
        return;
      }
      if (message.kind === 'query_result') {
        resolve(message.result);
        return;
      }
      if (message.kind === 'command_receipt') {
        resolve(message.receipt);
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

async function waitHealth() {
  await waitFor(async () => {
    const response = await fetch(new URL('/health', baseUrl));
    return response.ok && (await response.text()).trim() === 'ok';
  }, 60_000, 'S-profile health');
}

async function waitPageTarget() {
  return await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
    if (!response.ok) {
      return null;
    }
    const targets = await response.json();
    return targets.find((target) => target.type === 'page');
  }, 20_000, 'Chrome DevTools page target');
}

async function waitFor(fn, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const value = await Promise.resolve(fn());
      if (value) {
        return value;
      }
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  const suffix = lastError ? `: ${lastError.message}` : '';
  throw new Error(`timeout waiting for ${label}${suffix}`);
}

async function waitForFunction(cdpClient, fn, timeoutMs, label, ...args) {
  return await waitFor(() => evalInPage(cdpClient, fn, ...args), timeoutMs, label);
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

async function waitForLoad(cdpClient) {
  await new Promise((resolve) => {
    const timer = setTimeout(resolve, 10_000);
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

async function evalInPage(cdpClient, fn, ...args) {
  const response = await cdpClient.send('Runtime.evaluate', {
    expression: `(() => { ${browserHelperSource()}; return (${fn})(...${JSON.stringify(args)}); })()`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    const details = response.exceptionDetails;
    const description = details.exception?.description || details.text || 'Runtime.evaluate failed';
    throw new Error(description);
  }
  return response.result.value;
}

function browserHelperSource() {
  return readModelGroupSettingsDom.toString();
}

function readModelGroupSettingsDom() {
  const text = (id) => document.getElementById(id)?.textContent?.trim() || '';
  return {
    modelGroupSummary: text('settings-model-group-summary'),
    currentModelGroup: document.getElementById('settings-model-group-current-select')?.value || '',
    modelGroupIds: Array.from(document.querySelectorAll('.settings-model-group-card'))
      .map((card) => card.dataset.modelGroupId)
      .filter(Boolean),
    modelGroupRegistryText: text('settings-model-group-registry-list'),
    saveStatus: text('settings-model-group-save-status'),
    switchStatus: text('settings-model-group-switch-status'),
    commandStatus: text('command-status'),
    providerIds: Array.from(document.querySelectorAll('.settings-provider-card'))
      .map((card) => card.dataset.providerId)
      .filter(Boolean),
  };
}

function recordPageEvent(method, params) {
  if (!['Runtime.consoleAPICalled', 'Runtime.exceptionThrown', 'Log.entryAdded'].includes(method)) {
    return;
  }
  pageEvents.push({
    method,
    params: redactPageEvent(params),
    ts: new Date().toISOString(),
  });
  if (pageEvents.length > 200) {
    pageEvents.shift();
  }
}

function redactPageEvent(value) {
  const json = JSON.stringify(value || {});
  return JSON.parse(
    json
      .replace(/api_key/gi, 'api_key')
      .replace(/sk-[A-Za-z0-9_-]+/g, '<redacted-token>')
      .replace(/Bearer\\s+[A-Za-z0-9._~-]+/g, 'Bearer <redacted>'),
  );
}

async function captureBrowserDebug(fileName) {
  if (!cdp) {
    return null;
  }
  const dom = await evalInPage(cdp, () => {
    const byId = (id) => {
      const element = document.getElementById(id);
      if (!element) {
        return { exists: false };
      }
      return {
        exists: true,
        hidden: element.hidden === true,
        value: 'value' in element ? element.value : '',
        className: `${element.className || ''}`,
        text: `${element.textContent || ''}`.trim().slice(0, 3000),
      };
    };
    return {
      href: location.href,
      readyState: document.readyState,
      webuiJsReady: document.body.dataset.webuiJsReady === 'true',
      shellPresent: !!document.querySelector('[data-webui-shell="true"]'),
      bodyText: `${document.body?.innerText || ''}`.trim().slice(0, 8000),
      settingsShell: byId('settings-shell'),
      modelGroupSummary: byId('settings-model-group-summary'),
      modelGroupRegistry: byId('settings-model-group-registry-list'),
      modelGroupCurrentSelect: byId('settings-model-group-current-select'),
      modelGroupSaveStatus: byId('settings-model-group-save-status'),
      modelGroupSwitchStatus: byId('settings-model-group-switch-status'),
      commandStatus: byId('command-status'),
      modelGroupCards: Array.from(document.querySelectorAll('.settings-model-group-card')).map((card) => ({
        modelGroupId: card.dataset.modelGroupId || '',
        text: `${card.textContent || ''}`.trim(),
      })),
    };
  });
  const debug = {
    dom,
    modelGroupSettings: dom.shellPresent
      ? await evalInPage(cdp, () => readModelGroupSettingsDom()).catch((error) => ({ error: error.message }))
      : null,
    pageEvents,
    chromeStdout: chromeStdout.slice(-4000),
    chromeStderr: chromeStderr.slice(-4000),
  };
  await writeJson(fileName, debug);
  return debug;
}

async function writeFailure(error) {
  const failureDir = path.join(artifactDir, 'failure');
  await fs.mkdir(failureDir, { recursive: true });
  await fs.writeFile(path.join(failureDir, 'error.txt'), error.stack || error.message);
  await adpQuery('QueryConfigStatus')
    .then((value) => fs.writeFile(path.join(failureDir, 'config-status.json'), JSON.stringify(value, null, 2)))
    .catch((queryError) => fs.writeFile(path.join(failureDir, 'config-status-error.txt'), queryError.stack || queryError.message));
  await captureBrowserDebug(path.join('failure', 'browser-debug.json')).catch(() => {});
}

async function must(argv, opts = {}) {
  return await new Promise((resolve, reject) => {
    const child = spawn(argv[0], argv.slice(1), {
      cwd: repo,
      env: { ...process.env, ...(opts.env || {}) },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    child.on('error', reject);
    child.on('close', (code) => {
      if (code === 0) {
        resolve({ stdout, stderr });
      } else {
        reject(new Error(`${argv.join(' ')} exited ${code}\nstdout=${stdout}\nstderr=${stderr}`));
      }
    });
  });
}

async function grepFixtureEnv() {
  const content = await fs.readFile(envPath, 'utf8').catch(() => '');
  const lines = content
    .split(/\r?\n/)
    .filter((line) =>
      /FREEHAND_MODEL_GROUP_UI|FREEHAND_PROVIDER_RETRY_FIXTURE_KEY|FREEHAND_MASTER_AUTONOMY_FIXTURE_KEY/.test(line)
    );
  return { matchCount: lines.length, output: lines.join('\n') };
}

function redactConfig(value) {
  return value
    .replace(/api_key\s*=\s*"[^"]*"/g, 'api_key = "<redacted>"')
    .replace(/pair_token\s*=\s*"[^"]*"/g, 'pair_token = "<redacted>"');
}

function redactEnv(value) {
  return value.replace(/=.*/g, '=<redacted>');
}

async function writeJson(fileName, value) {
  await fs.writeFile(path.join(artifactDir, fileName), JSON.stringify(value, null, 2));
}

async function writeFile(fileName, value) {
  await fs.writeFile(path.join(artifactDir, fileName), value);
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
