#!/usr/bin/env node
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import fss from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { adpVerifierRequest } from './lib/adp-verifier-client.mjs';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const configPath =
  process.env.FREEHAND_PROVIDER_WEB_SEARCH_UI_CONFIG || path.join(runtimeHome, 'config.toml');
const envPath =
  process.env.FREEHAND_PROVIDER_WEB_SEARCH_UI_ENV || path.join(runtimeHome, 'daemonS.env');
const baseUrl = normalizedBaseUrl(
  process.env.FREEHAND_PROVIDER_WEB_SEARCH_UI_BASE_URL || 'http://127.0.0.1:4042/',
);
const adpUrl = process.env.FREEHAND_PROVIDER_WEB_SEARCH_UI_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const adpAuthToken = process.env.FREEHAND_ADP_AUTH_TOKEN || '';
const cli = process.env.FREEHAND_PROVIDER_WEB_SEARCH_UI_CLI || path.join(home, '.local/bin/freehand-cliS');
const chromePath =
  process.env.FREEHAND_PROVIDER_WEB_SEARCH_UI_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_PROVIDER_WEB_SEARCH_UI_DEBUG_PORT || '9281', 10);
const fixtureProviderId =
  process.env.FREEHAND_PROVIDER_WEB_SEARCH_UI_FIXTURE_PROVIDER || 'ui-web-search-openai-responses';
const fixtureKeyName = 'FREEHAND_PROVIDER_WEB_SEARCH_UI_FIXTURE_KEY';
const fixtureModel = 'gpt-5.5-web-search-ui';
const runId = `provider-web-search-settings-ui-${Date.now()}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);
const assetVersion = '20260824-session-list-page';

let chrome = null;
let cdp = null;
let providerServer = null;
let requestCount = 0;
let firstProviderRequest = null;
let restored = false;
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
  providerServer = await startProviderServer();
  await fs.writeFile(
    envPath,
    `${stripFixtureEnv(originalEnv)}\n${fixtureKeyName}="fixture-key"\n`,
  );
  await restartSProfile();
  await waitHealth();
  await assertPageReachable();

  const initialConfig = unwrapConfigStatus(await adpQuery('QueryConfigStatus'));
  if (initialConfig.provider_id !== 'minimax') {
    throw new Error(`expected S-profile current provider minimax, got ${initialConfig.provider_id}`);
  }
  assertProviderEffectiveHosted(initialConfig, 'minimax');

  const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-provider-web-search-ui-'));
  chrome = spawn(chromePath, [
    '--headless=new',
    `--remote-debugging-port=${debugPort}`,
    `--user-data-dir=${profileDir}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-networking',
    '--disable-extensions',
    '--disable-sync',
    '--window-size=430,844',
    baseUrl,
  ], { stdio: ['ignore', 'pipe', 'pipe'] });
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
    () => document.body.dataset.webuiJsReady === 'true' &&
      !!document.querySelector('[data-webui-shell="true"]') &&
      !!document.getElementById('settings-provider-web-search-test-button'),
    30_000,
    'WebUI provider settings 就绪',
  );
  await openSettings(cdp);

  const initialDom = await waitForProviderDom((state, providerId) =>
    state.providerIds.includes(providerId) &&
    state.providerTexts.some((text) => text.includes(providerId) && text.includes('hosted_declared')),
    'minimax',
  );
  await writeJson('01-initial-dom.json', initialDom);

  const minimaxStatus = await clickProviderCardTestAndWait('minimax', {
    allowExplicitNoObservationFailure: true,
  });
  await writeJson('02-minimax-test-dom.json', minimaxStatus);
  const minimaxUiTestPassed =
    /模型服务联网搜索测试通过：provider_web_search_test_passed:provider=minimax:protocol=messages:model=MiniMax-M3:hosted_tool=web_search:hosted_observed=true/.test(minimaxStatus.testStatus);
  const minimaxUiTestExplicitNoObservationFailure =
    /模型服务 minimax 联网搜索测试失败: dispatch port failure: provider web_search test did not observe provider-hosted web_search for `minimax`/.test(minimaxStatus.testStatus);
  if (!minimaxUiTestPassed && !minimaxUiTestExplicitNoObservationFailure) {
    throw new Error(`Minimax settings web_search test did not return a pass or exact owner no-observation failure in DOM: ${minimaxStatus.testStatus}`);
  }

  await upsertFixtureProviderThroughUi();
  const afterUpsertConfig = unwrapConfigStatus(await adpQuery('QueryConfigStatus'));
  assertProviderEffectiveHosted(afterUpsertConfig, fixtureProviderId);
  const afterUpsertDom = await waitForProviderDom((state, providerId) =>
    state.providerIds.includes(providerId) &&
    state.providerTexts.some((text) =>
      text.includes(providerId) && text.includes('openai/responses') && text.includes('hosted_declared')),
    fixtureProviderId,
  );
  await writeJson('03-after-fixture-upsert-dom.json', afterUpsertDom);

  const fixtureStatus = await clickProviderCardTestAndWait(fixtureProviderId);
  await writeJson('04-openai-responses-test-dom.json', fixtureStatus);
  if (!new RegExp(`模型服务联网搜索测试通过：provider_web_search_test_passed:provider=${fixtureProviderId}:protocol=responses:model=${fixtureModel}:hosted_tool=web_search:hosted_observed=true`).test(fixtureStatus.testStatus)) {
    throw new Error(`OpenAI/Responses settings web_search test did not pass in DOM: ${fixtureStatus.testStatus}`);
  }
  if (requestCount !== 1) {
    throw new Error(`OpenAI/Responses fixture expected one request, got ${requestCount}`);
  }
  const hostedTools = requestHostedToolTypes(firstProviderRequest);
  const functionTools = requestFunctionToolNames(firstProviderRequest);
  const summaryBeforeRestore = {
    ok: true,
    runId,
    artifactDir,
    baseUrl,
    adpUrl,
    assetVersion,
    minimaxStatus: minimaxStatus.testStatus,
    minimaxTestOutcome: minimaxUiTestPassed ? 'passed' : 'explicit_no_observation_failure',
    fixtureStatus: fixtureStatus.testStatus,
    fixtureRequestCount: requestCount,
    fixtureHostedTools: hostedTools,
    fixtureFunctionTools: functionTools,
    checks: {
      minimaxVisibleHosted: true,
      minimaxUiTestReturnedOwnerStatus: minimaxUiTestPassed || minimaxUiTestExplicitNoObservationFailure,
      openaiResponsesVisibleHosted: true,
      openaiResponsesUiTestPassed: true,
      fixtureDeclaredHostedWebSearch: hostedTools.includes('web_search'),
      fixtureDidNotDeclareFunctionWebSearch: !functionTools.includes('web_search'),
    },
  };
  summaryBeforeRestore.ok = Object.values(summaryBeforeRestore.checks).every(Boolean);
  await writeJson('summary.before-restore.json', summaryBeforeRestore);
  if (!summaryBeforeRestore.ok) {
    throw new Error(`provider web_search UI checks failed: ${JSON.stringify(summaryBeforeRestore.checks)}`);
  }
} catch (error) {
  await captureFailureState(error).catch(() => null);
  throw error;
} finally {
  if (cdp) {
    await captureBrowserDebug('zz-finally-browser-debug.json').catch(() => null);
    cdp.offEvent(recordPageEvent);
    await cdp.close().catch(() => null);
  }
  if (chrome && chrome.pid) {
    chrome.kill('SIGTERM');
  }
  if (providerServer) {
    await new Promise((resolve) => providerServer.close(resolve));
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

const finalConfig = unwrapConfigStatus(await adpQuery('QueryConfigStatus'));
const finalCli = await must([cli, 'adp-config-query', '--url', adpUrl]);
const finalEnvGrep = await grepFixtureEnv();
const summary = {
  ok: restored && restoreErrors.length === 0 && finalConfig.provider_id === 'minimax' && finalEnvGrep.matchCount === 0,
  runId,
  artifactDir,
  restored,
  restoreErrors,
  finalProvider: finalConfig.provider_id,
  finalModel: finalConfig.default_model,
  finalCli: finalCli.stdout.trim(),
  finalEnvGrep,
};
await writeJson('summary.json', summary);
if (!summary.ok) {
  throw new Error(`provider web_search UI restore failed: ${JSON.stringify(summary)}`);
}
console.log(
  `provider_web_search_settings_ui_ok url=${baseUrl} adp=${adpUrl} minimax=owner_status openai_responses=passed fixture_requests=${requestCount} artifactDir=${artifactDir}`,
);

async function upsertFixtureProviderThroughUi() {
  await evalInPage(cdp, ({ providerId, baseUrlValue, model, envName }) => {
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
    setValue('settings-provider-url-input', baseUrlValue);
    setValue('settings-provider-model-input', model);
    setValue('settings-provider-web-search-input', 'auto');
    setValue('settings-provider-env-input', envName);
    document.getElementById('settings-provider-form')?.requestSubmit();
  }, {
    providerId: fixtureProviderId,
    baseUrlValue: `${providerBaseUrl()}/openai/v1`,
    model: fixtureModel,
    envName: fixtureKeyName,
  });
  await waitForFunction(
    cdp,
    (providerId) => {
      const state = readProviderSettingsDom();
      return state.providerIds.includes(providerId) &&
        /provider definition saved|模型服务定义已保存|已保存/i.test(`${state.saveStatus} ${state.commandStatus}`)
        ? state
        : null;
    },
    30_000,
    'fixture provider saved through settings UI',
    fixtureProviderId,
  );
}

async function clickProviderCardTestAndWait(providerId, { allowExplicitNoObservationFailure = false } = {}) {
  await evalInPage(cdp, (targetProviderId) => {
    const card = Array.from(document.querySelectorAll('.settings-provider-card'))
      .find((candidate) => candidate.dataset.providerId === targetProviderId);
    if (!card) {
      throw new Error(`provider card not found: ${targetProviderId}`);
    }
    const button = Array.from(card.querySelectorAll('button'))
      .find((candidate) => /测试联网搜索/i.test(candidate.textContent || ''));
    if (!button) {
      throw new Error(`provider test button not found: ${targetProviderId}`);
    }
    button.click();
  }, providerId);
  return await waitForFunction(
    cdp,
    (targetProviderId, allowNoObservationFailure) => {
      const state = readProviderSettingsDom();
      if (state.testStatus.includes(`provider=${targetProviderId}:`) &&
          /模型服务联网搜索测试通过/.test(state.testStatus)) {
        return state;
      }
      const exactNoObservationFailure =
        state.testStatus.includes(`模型服务 ${targetProviderId} 联网搜索测试失败`) &&
        state.testStatus.includes(`did not observe provider-hosted web_search for \`${targetProviderId}\``);
      if (exactNoObservationFailure && allowNoObservationFailure) {
        return state;
      }
      if (/模型服务.*联网搜索测试失败/.test(state.testStatus)) {
        throw new Error(state.testStatus);
      }
      return null;
    },
    providerId === 'minimax' ? 180_000 : 45_000,
    `provider web_search settings test ${providerId}`,
    providerId,
    allowExplicitNoObservationFailure,
  );
}

async function waitForProviderDom(predicate, ...predicateArgs) {
  return await waitForFunction(
    cdp,
    (predicateSource, args) => {
      const state = readProviderSettingsDom();
      const predicateFn = eval(`(${predicateSource})`);
      return predicateFn(state, ...args) ? state : null;
    },
    30_000,
    'provider settings DOM projection',
    predicate.toString(),
    predicateArgs,
  );
}

function assertProviderEffectiveHosted(status, providerId) {
  const provider = (status.provider_registry || []).find((entry) => entry.provider_id === providerId);
  if (!provider) {
    throw new Error(`provider ${providerId} missing from registry`);
  }
  if (provider.provider_web_search_effective !== 'hosted_declared') {
    throw new Error(`provider ${providerId} web_search effective is ${provider.provider_web_search_effective}`);
  }
}

async function waitHealth() {
  await waitFor(async () => {
    const response = await fetch(new URL('/health', baseUrl));
    return response.ok && (await response.text()).trim() === 'ok';
  }, 90_000, 'S-profile health');
}

async function assertPageReachable() {
  const response = await fetch(baseUrl, { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`WebUI not reachable: ${response.status} ${response.statusText}`);
  }
  const html = await response.text();
  if (!html.includes(assetVersion)) {
    throw new Error(`served WebUI asset version mismatch: expected ${assetVersion}`);
  }
}

async function startProviderServer() {
  return await new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      let body = '';
      req.on('data', (chunk) => {
        body += chunk;
      });
      req.on('end', async () => {
        requestCount += 1;
        const parsed = parseJsonOrNull(body);
        if (requestCount === 1) {
          firstProviderRequest = parsed;
        }
        const requestSummary = {
          count: requestCount,
          at: new Date().toISOString(),
          method: req.method,
          url: req.url,
          functionToolNames: requestFunctionToolNames(parsed),
          hostedToolTypes: requestHostedToolTypes(parsed),
          bodyLength: body.length,
        };
        fss.appendFileSync(path.join(artifactDir, 'provider-requests.jsonl'), `${JSON.stringify(requestSummary)}\n`);
        await fs.writeFile(
          path.join(artifactDir, `provider-request-${String(requestCount).padStart(3, '0')}.json`),
          JSON.stringify(parsed, null, 2),
        );
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(openaiResponsesHostedSearchBody());
      });
    });
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

function openaiResponsesHostedSearchBody() {
  return JSON.stringify({
    id: 'resp-provider-web-search-settings-ui',
    object: 'response',
    status: 'completed',
    error: null,
    output: [
      {
        type: 'web_search_call',
        id: `ws-${runId}`,
        status: 'completed',
        action: {
          type: 'search',
          query: 'provider web_search settings UI fixture',
        },
      },
      {
        type: 'message',
        id: 'msg-provider-web-search-settings-ui',
        role: 'assistant',
        status: 'completed',
        content: [
          {
            type: 'output_text',
            text: '设置 UI provider-hosted search fixture completed.',
            annotations: [],
          },
        ],
      },
    ],
    usage: {
      input_tokens: 8,
      output_tokens: 6,
      total_tokens: 14,
    },
  });
}

function providerBaseUrl() {
  const address = providerServer.address();
  return `http://127.0.0.1:${address.port}`;
}

function requestHostedToolTypes(value) {
  return (value?.tools || [])
    .filter((tool) => tool && tool.type && !tool.name)
    .map((tool) => tool.type);
}

function requestFunctionToolNames(value) {
  return (value?.tools || [])
    .filter((tool) => tool && (tool.type === 'function' || tool.name))
    .map((tool) => tool.name || tool.function?.name || '')
    .filter(Boolean);
}

function parseJsonOrNull(value) {
  try {
    return JSON.parse(value || '{}');
  } catch {
    return null;
  }
}

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
    '设置 shell visible',
  );
}

async function adpQuery(query) {
  return await adpRequest('query', 'query', query, 30_000);
}

function adpRequest(kind, payloadKey, payload, timeoutMs) {
  return adpVerifierRequest({
    url: adpUrl,
    authToken: adpAuthToken,
    kind,
    payloadKey,
    payload,
    timeoutMs,
    clientName: 'freehand-provider-search-verifier',
  });
}

function unwrapConfigStatus(result) {
  if (!result || !result.ConfigStatus) {
    throw new Error(`missing ConfigStatus result: ${JSON.stringify(result)}`);
  }
  return result.ConfigStatus;
}

async function waitPageTarget() {
  return await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${debugPort}/json`);
    if (!response.ok) {
      return null;
    }
    const targets = await response.json();
    return targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl) || null;
  }, 20_000, 'Chrome page target');
}

async function waitFor(predicate, timeoutMs, label) {
  const started = Date.now();
  let lastError = null;
  while (Date.now() - started < timeoutMs) {
    try {
      const result = await predicate();
      if (result) {
        return result;
      }
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw new Error(`timeout waiting for ${label}${lastError ? `; last_error=${lastError.message}` : ''}`);
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
    providerIds: Array.from(document.querySelectorAll('.settings-provider-card'))
      .map((card) => card.dataset.providerId)
      .filter(Boolean),
    providerTexts: Array.from(document.querySelectorAll('.settings-provider-card'))
      .map((card) => card.textContent || ''),
    saveStatus: text('settings-provider-save-status'),
    testStatus: text('settings-provider-web-search-test-status'),
    commandStatus: text('command-status'),
  };
}

function recordPageEvent(method, params) {
  if (!['Runtime.consoleAPICalled', 'Runtime.exceptionThrown', 'Log.entryAdded'].includes(method)) {
    return;
  }
  pageEvents.push({ method, params: redactPageEvent(params), ts: new Date().toISOString() });
  if (pageEvents.length > 200) {
    pageEvents.shift();
  }
}

function redactPageEvent(value) {
  const json = JSON.stringify(value || {});
  return JSON.parse(
    json
      .replace(/sk-[A-Za-z0-9_-]+/g, '<redacted-token>')
      .replace(/Bearer\\s+[A-Za-z0-9._~-]+/g, 'Bearer <redacted>'),
  );
}

async function captureBrowserDebug(fileName) {
  if (!cdp) {
    return null;
  }
  const dom = await evalInPage(cdp, () => ({
    href: location.href,
    readyState: document.readyState,
    bodyText: `${document.body?.innerText || ''}`.trim().slice(0, 8000),
    providerSettings: readProviderSettingsDom(),
  }));
  const debug = {
    dom,
    pageEvents,
    chromeStdout: chromeStdout.slice(-4000),
    chromeStderr: chromeStderr.slice(-4000),
  };
  await writeJson(fileName, debug);
  return debug;
}

async function captureFailureState(error) {
  const failureDir = path.join(artifactDir, 'failure');
  await fs.mkdir(failureDir, { recursive: true });
  await fs.writeFile(path.join(failureDir, 'error.txt'), error.stack || error.message);
  await captureBrowserDebug(path.join('failure', 'browser-debug.json')).catch(() => null);
  await adpQuery('QueryConfigStatus')
    .then((value) => fs.writeFile(path.join(failureDir, 'config-status.json'), JSON.stringify(value, null, 2)))
    .catch((queryError) => fs.writeFile(path.join(failureDir, 'config-status-error.txt'), queryError.stack || queryError.message));
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
    .filter((line) => line.includes(fixtureKeyName) || line.includes(fixtureProviderId));
  return { matchCount: lines.length, output: lines.join('\n') };
}

function stripFixtureEnv(value) {
  return value
    .split(/\r?\n/)
    .filter((line) => !line.includes(fixtureKeyName) && !line.includes(fixtureProviderId))
    .join('\n');
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
  await fs.mkdir(path.dirname(path.join(artifactDir, fileName)), { recursive: true });
  await fs.writeFile(path.join(artifactDir, fileName), JSON.stringify(value, null, 2));
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
