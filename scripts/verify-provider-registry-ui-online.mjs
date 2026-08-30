import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const configPath = process.env.FREEHAND_PROVIDER_REGISTRY_UI_CONFIG || path.join(runtimeHome, 'config.toml');
const envPath = process.env.FREEHAND_PROVIDER_REGISTRY_UI_ENV || path.join(runtimeHome, 'daemonS.env');
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_PROVIDER_REGISTRY_UI_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_PROVIDER_REGISTRY_UI_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const cli = process.env.FREEHAND_PROVIDER_REGISTRY_UI_CLI || path.join(home, '.local/bin/freehand-cliS');
const debugPort = Number.parseInt(process.env.FREEHAND_PROVIDER_REGISTRY_UI_DEBUG_PORT || '9251', 10);
const chromePath =
  process.env.FREEHAND_PROVIDER_REGISTRY_UI_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const testProviderId = process.env.FREEHAND_PROVIDER_REGISTRY_UI_PROVIDER_ID || 'ui-verify-provider-registry';
const testProviderEnv = process.env.FREEHAND_PROVIDER_REGISTRY_UI_PROVIDER_ENV || 'FREEHAND_PROVIDER_REGISTRY_UI_VERIFY_KEY';
const testProviderBaseUrl =
  process.env.FREEHAND_PROVIDER_REGISTRY_UI_PROVIDER_BASE_URL || 'https://provider-registry-ui.example.test/openai/v1';
const testProviderModel = process.env.FREEHAND_PROVIDER_REGISTRY_UI_PROVIDER_MODEL || 'provider-registry-ui-model';
const runId = `provider-registry-ui-${Date.now()}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);

let chrome;
let cdp;
let restored = false;
let proofOriginalPrimary = null;
let proofOriginalFallback = null;
let proofSwitchTarget = null;
const restoreErrors = [];
const pageEvents = [];
let chromeStdout = '';
let chromeStderr = '';

await fs.mkdir(artifactDir, { recursive: true });
const originalConfig = await fs.readFile(configPath, 'utf8');
const originalEnv = await fs.readFile(envPath, 'utf8').catch(() => '');

try {
  await fs.writeFile(path.join(artifactDir, 'config.before.toml'), redactConfig(originalConfig));
  await fs.writeFile(path.join(artifactDir, 'daemonS.before.env'), redactEnv(originalEnv));

  await waitHealth();
  const initialAdp = unwrapConfigStatus(await adpQuery('QueryConfigStatus'));
  const initialCli = await must([cli, 'adp-config-query', '--url', adpUrl]);
  assertNoSecrets(initialCli.stdout, 'initial CLI config output');
  assertProviderPresent(initialAdp, 'cc');
  assertProviderPresent(initialAdp, 'minimax');

  const originalPrimary = initialAdp.provider_id;
  const originalFallback = initialAdp.fallback_provider_id || null;
  const switchTarget = chooseSwitchTarget(initialAdp, originalPrimary);
  if (!switchTarget) {
    throw new Error('need at least one alternate enabled provider for UI switch proof');
  }
  proofOriginalPrimary = originalPrimary;
  proofOriginalFallback = originalFallback;
  proofSwitchTarget = switchTarget;

  const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-provider-registry-ui-'));
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
    () => !!document.querySelector('[data-webui-shell="true"]') && !!document.getElementById('settings-shell-toggle'),
    30_000,
    'WebUI shell ready',
  );

  await openSettings(cdp);
  await captureBrowserDebug(cdp, '00-settings-open-dom.json');
  let initialDom;
  try {
    initialDom = await waitForFunction(
      cdp,
      () => {
        const state = readProviderSettingsDom();
        return state.providerIds.includes('cc') &&
          state.providerIds.includes('minimax') &&
          state.currentProvider !== 'loading'
          ? state
          : null;
      },
      30_000,
      'provider settings loaded',
    );
  } catch (error) {
    await captureBrowserDebug(cdp, 'failure-provider-settings-dom.json');
    throw error;
  }
  assertDomProviderPresent(initialDom, 'cc');
  assertDomProviderPresent(initialDom, 'minimax');
  if (initialDom.currentProvider !== originalPrimary) {
    throw new Error(`DOM current provider ${initialDom.currentProvider} != ADP ${originalPrimary}`);
  }
  if ((initialDom.fallbackSelectValue || null) !== originalFallback) {
    throw new Error(
      `DOM fallback provider ${initialDom.fallbackSelectValue || 'none'} != ADP ${originalFallback || 'none'}`,
    );
  }
  await writeJson('01-initial-dom.json', initialDom);

  await evalInPage(cdp, ({ providerId, baseUrl, model, envName }) => {
    const setValue = (id, value) => {
      const element = document.getElementById(id);
      if (!element) {
        throw new Error(`missing ${id}`);
      }
      element.value = value;
      element.dispatchEvent(new Event('input', { bubbles: true }));
      element.dispatchEvent(new Event('change', { bubbles: true }));
    };
    setValue('settings-provider-id-input', providerId);
    setValue('settings-provider-type-input', 'openai');
    setValue('settings-provider-protocol-input', 'responses');
    setValue('settings-provider-url-input', baseUrl);
    setValue('settings-provider-model-input', model);
    setValue('settings-provider-env-input', envName);
    document.getElementById('settings-provider-form')?.requestSubmit();
  }, {
    providerId: testProviderId,
    baseUrl: testProviderBaseUrl,
    model: testProviderModel,
    envName: testProviderEnv,
  });
  const afterUpsertDom = await waitForFunction(
    cdp,
    (providerId) => {
      const state = readProviderSettingsDom();
      return state.providerIds.includes(providerId) &&
        /provider definition saved/i.test(state.saveStatus + ' ' + state.commandStatus)
        ? state
        : null;
    },
    30_000,
    'provider upsert reflected in DOM',
    testProviderId,
  );
  if (afterUpsertDom.currentProvider !== originalPrimary) {
    throw new Error(`upsert changed current provider from ${originalPrimary} to ${afterUpsertDom.currentProvider}`);
  }
  if ((afterUpsertDom.fallbackSelectValue || null) !== originalFallback) {
    throw new Error(
      `upsert changed fallback provider from ${originalFallback || 'none'} to ${afterUpsertDom.fallbackSelectValue || 'none'}`,
    );
  }
  await writeJson('02-after-upsert-dom.json', afterUpsertDom);
  const afterUpsertAdp = unwrapConfigStatus(await adpQuery('QueryConfigStatus'));
  assertProviderPresent(afterUpsertAdp, testProviderId);
  if (afterUpsertAdp.provider_id !== originalPrimary) {
    throw new Error(`ADP upsert changed provider from ${originalPrimary} to ${afterUpsertAdp.provider_id}`);
  }

  await evalInPage(cdp, ({ providerId }) => {
    const primary = document.getElementById('settings-provider-current-select');
    const fallback = document.getElementById('settings-provider-fallback-select');
    const button = document.getElementById('settings-provider-switch-button');
    if (!primary || !fallback || !button) {
      throw new Error('missing provider switch controls');
    }
    primary.value = providerId;
    primary.dispatchEvent(new Event('change', { bubbles: true }));
    fallback.value = '';
    fallback.dispatchEvent(new Event('change', { bubbles: true }));
    button.click();
  }, { providerId: switchTarget });
  const afterSwitchDom = await waitForFunction(
    cdp,
    (providerId) => {
      const state = readProviderSettingsDom();
      return state.currentProvider === providerId &&
        /provider selection saved/i.test(state.switchStatus + ' ' + state.commandStatus)
        ? state
        : null;
    },
    30_000,
    'provider switch reflected in DOM',
    switchTarget,
  );
  await writeJson('03-after-switch-dom.json', afterSwitchDom);
  const afterSwitchAdp = unwrapConfigStatus(await adpQuery('QueryConfigStatus'));
  if (afterSwitchAdp.provider_id !== switchTarget) {
    throw new Error(`ADP switch provider ${afterSwitchAdp.provider_id} != ${switchTarget}`);
  }
  if (afterSwitchAdp.fallback_provider_id) {
    throw new Error(`fallback should be cleared during switch proof, got ${afterSwitchAdp.fallback_provider_id}`);
  }
  assertProviderPresent(afterSwitchAdp, originalPrimary);
  assertProviderPresent(afterSwitchAdp, testProviderId);

  const summaryBeforeRestore = {
    runId,
    url: baseUrl,
    adpUrl,
    originalPrimary,
    originalFallback,
    switchTarget,
    testProviderId,
    initialProviderIds: providerIds(initialAdp),
    afterUpsertProviderIds: providerIds(afterUpsertAdp),
    afterSwitchProviderIds: providerIds(afterSwitchAdp),
    domAfterUpsert: afterUpsertDom,
    domAfterSwitch: afterSwitchDom,
  };
  await writeJson('summary.before-restore.json', summaryBeforeRestore);
} finally {
  if (cdp) {
    await captureBrowserDebug(cdp, 'zz-finally-browser-debug.json').catch(() => {});
    cdp.offEvent(recordPageEvent);
  }
  if (cdp) {
    await cdp.close().catch(() => {});
  }
  if (chrome && chrome.pid) {
    chrome.kill('SIGTERM');
  }
  try {
    await fs.writeFile(configPath, originalConfig);
    await fs.writeFile(envPath, originalEnv);
    await restartSProfile();
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
if (finalEnvGrep.matchCount !== 0) {
  throw new Error(`fixture env remained in daemonS.env: ${finalEnvGrep.output}`);
}
const finalSummary = {
  runId,
  restored,
  restoreErrors,
  url: baseUrl,
  adpUrl,
  originalPrimary: proofOriginalPrimary,
  originalFallback: proofOriginalFallback,
  switchedProvider: proofSwitchTarget,
  finalProvider: finalAdp.provider_id,
  finalFallback: finalAdp.fallback_provider_id || null,
  finalProviderIds: providerIds(finalAdp),
  finalCli: finalCli.stdout.trim(),
  finalEnvGrep,
};
await writeJson('summary.json', finalSummary);

if (!restored || restoreErrors.length > 0) {
  throw new Error(`restore failed: ${restoreErrors.join('; ')}`);
}
if (providerIds(finalAdp).includes(testProviderId)) {
  throw new Error(`test provider remained after restore: ${testProviderId}`);
}

console.log(
  [
    'provider_registry_ui_online_ok',
    `url=${adpUrl}`,
    `run_id=${runId}`,
    `added_provider=${testProviderId}`,
    `switched_provider=${proofSwitchTarget || 'unknown'}`,
    `final_provider=${finalAdp.provider_id}`,
    `final_fallback=${finalAdp.fallback_provider_id || 'none'}`,
    `final_registry=${providerIds(finalAdp).join(',')}`,
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

function assertProviderPresent(status, providerId) {
  if (!providerIds(status).includes(providerId)) {
    throw new Error(`provider ${providerId} missing from registry: ${providerIds(status).join(',')}`);
  }
}

function assertDomProviderPresent(dom, providerId) {
  if (!dom.providerIds.includes(providerId)) {
    throw new Error(`DOM provider ${providerId} missing from registry: ${dom.providerIds.join(',')}`);
  }
}

function chooseSwitchTarget(status, originalPrimary) {
  const candidates = (status.provider_registry || [])
    .filter((provider) => provider.enabled !== false && provider.provider_id !== originalPrimary)
    .map((provider) => provider.provider_id);
  if (candidates.includes('minimax')) {
    return 'minimax';
  }
  if (candidates.includes('cc')) {
    return 'cc';
  }
  return candidates[0] || null;
}

function assertNoSecrets(value, label) {
  if (/api_key|pair_token|sk-|secret/i.test(value)) {
    throw new Error(`${label} contains secret-looking text`);
  }
}

async function adpQuery(query) {
  return await adpRequest('query', 'query', query);
}

function adpRequest(kind, payloadKey, payload) {
  const socket = new WebSocket(adpUrl);
  const requestId = `${kind}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      socket.close();
      reject(new Error(`ADP ${kind} timeout`));
    }, 20_000);
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
  while (Date.now() < deadline) {
    const value = await Promise.resolve(fn()).catch(() => null);
    if (value) {
      return value;
    }
    await delay(250);
  }
  throw new Error(`timeout waiting for ${label}`);
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
  return readProviderSettingsDom.toString();
}

function readProviderSettingsDom() {
  const text = (id) => document.getElementById(id)?.textContent?.trim() || '';
  return {
    currentProvider: text('settings-provider-id'),
    fallbackSelectValue: document.getElementById('settings-provider-fallback-select')?.value || '',
    currentSelectValue: document.getElementById('settings-provider-current-select')?.value || '',
    providerIds: Array.from(document.querySelectorAll('.settings-provider-card'))
      .map((card) => card.dataset.providerId)
      .filter(Boolean),
    registryText: text('settings-provider-registry-list'),
    saveStatus: text('settings-provider-save-status'),
    switchStatus: text('settings-provider-switch-status'),
    commandStatus: text('command-status'),
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

async function captureBrowserDebug(cdpClient, fileName) {
  const dom = await evalInPage(cdpClient, () => {
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
      webuiJsReady: window.webuiJsReady === true,
      shellPresent: !!document.querySelector('[data-webui-shell="true"]'),
      bodyText: `${document.body?.innerText || ''}`.trim().slice(0, 8000),
      scriptSrcs: Array.from(document.scripts).map((script) => script.src).filter(Boolean),
      stylesheetHrefs: Array.from(document.querySelectorAll('link[rel="stylesheet"]'))
        .map((link) => link.href)
        .filter(Boolean),
      settingsShell: byId('settings-shell'),
      settingsToggle: byId('settings-shell-toggle'),
      settingsOpenButton: byId('open-settings-drawer-button'),
      providerId: byId('settings-provider-id'),
      providerRegistry: byId('settings-provider-registry-list'),
      providerCurrentSelect: byId('settings-provider-current-select'),
      providerFallbackSelect: byId('settings-provider-fallback-select'),
      configError: byId('settings-config-error'),
      commandStatus: byId('command-status'),
      workspaceStatus: byId('workspace-status'),
      providerCards: Array.from(document.querySelectorAll('.settings-provider-card')).map((card) => ({
        providerId: card.dataset.providerId || '',
        text: `${card.textContent || ''}`.trim(),
      })),
    };
  });
  const debug = {
    dom,
    providerSettings: dom.shellPresent ? await evalInPage(cdpClient, () => readProviderSettingsDom()).catch((error) => ({
      error: error.message,
    })) : null,
    pageEvents,
    chromeStdout: chromeStdout.slice(-4000),
    chromeStderr: chromeStderr.slice(-4000),
  };
  await writeJson(fileName, debug);
  return debug;
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

async function restartSProfile() {
  const uid = typeof process.getuid === 'function' ? process.getuid() : null;
  if (!Number.isInteger(uid)) {
    throw new Error('cannot resolve current uid for service-scoped S-profile restart');
  }
  await must(['launchctl', 'kickstart', '-k', `gui/${uid}/com.freehand.daemonS`]);
}

async function grepFixtureEnv() {
  const content = await fs.readFile(envPath, 'utf8').catch(() => '');
  const lines = content
    .split(/\r?\n/)
    .filter((line) => line.includes(testProviderEnv) || line.includes('FREEHAND_PROVIDER_REGISTRY_UI_VERIFY_KEY'));
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

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
