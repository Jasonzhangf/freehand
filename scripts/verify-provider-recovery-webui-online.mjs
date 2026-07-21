import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import fss from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const configPath = process.env.FREEHAND_PROVIDER_RECOVERY_CONFIG || path.join(runtimeHome, 'config.toml');
const envPath = process.env.FREEHAND_PROVIDER_RECOVERY_ENV || path.join(runtimeHome, 'daemonS.env');
const cli = process.env.FREEHAND_PROVIDER_RECOVERY_CLI || path.join(home, '.local/bin/freehand-cliS');
const baseUrl = process.env.FREEHAND_PROVIDER_RECOVERY_BASE_URL || 'http://127.0.0.1:4042/';
const adpUrl = process.env.FREEHAND_PROVIDER_RECOVERY_ADP_URL || 'ws://127.0.0.1:4042/adp';
const fixturePort = Number.parseInt(process.env.FREEHAND_PROVIDER_RECOVERY_FIXTURE_PORT || '18137', 10);
const debugPort = Number.parseInt(process.env.FREEHAND_PROVIDER_RECOVERY_DEBUG_PORT || '9237', 10);
const retryBackoffMs = process.env.FREEHAND_PROVIDER_RECOVERY_BACKOFF_MS || '5000';
const fixedSessionId =
  process.env.FREEHAND_PROVIDER_RECOVERY_SESSION_ID || 'webui-provider-recovery-fixed';
const chromePath =
  process.env.FREEHAND_PROVIDER_RECOVERY_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const runId =
  'provider-recovery-' +
  new Date().toISOString().replace(/[-:]/g, '').slice(0, 15) +
  '-' +
  process.pid;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);
const retryBackoffEnv = {
  ...process.env,
  FREEHAND_PROVIDER_RETRY_BACKOFF_MS: retryBackoffMs,
};

let fixtureServer;
let chrome;
let requestCount = 0;

await fs.mkdir(artifactDir, { recursive: true });

const originalConfig = await fs.readFile(configPath, 'utf8');
const originalEnv = await fs.readFile(envPath, 'utf8').catch(() => '');

try {
  fixtureServer = await startFixtureServer();
  await fs.writeFile(path.join(artifactDir, 'config.before.toml'), redactConfig(originalConfig));
  await fs.writeFile(path.join(artifactDir, 'daemonS.before.env'), redactEnv(originalEnv));

  await fs.writeFile(
    envPath,
    stripFixtureEnv(originalEnv) +
      '\nFREEHAND_PROVIDER_RECOVERY_FIXTURE_KEY="fixture-key"\nFREEHAND_PROVIDER_RETRY_BACKOFF_MS="' +
      retryBackoffMs +
      '"\nFREEHAND_TEST_DISABLE_MASTER_LIFECYCLE_RUNNER="1"\n',
  );
  await must(['scripts/install-launchd.sh', 'restartS'], { env: retryBackoffEnv });
  await waitHealth();
  const configBeforeFixture = await must([cli, 'adp-config-query', '--url', adpUrl]);
  await fs.writeFile(path.join(artifactDir, 'adp-config.before.txt'), configBeforeFixture.stdout);
  const fixtureProviderId = parseAdpConfigValue(configBeforeFixture.stdout, 'provider');
  if (!fixtureProviderId) {
    throw new Error('unable to read active provider from adp-config-query output');
  }
  await must([
    cli,
    'adp-config-update',
    '--url',
    adpUrl,
    '--agent',
    'master',
    '--provider',
    fixtureProviderId,
    '--type',
    'openai',
    '--protocol',
    'responses',
    '--base-url',
    'http://127.0.0.1:' + fixturePort + '/openai/v1',
    '--model',
    'gpt-5.5',
    '--api-key-env',
    'FREEHAND_PROVIDER_RECOVERY_FIXTURE_KEY',
  ]);
  await must(['scripts/install-launchd.sh', 'restartS'], { env: retryBackoffEnv });
  await waitHealth();
  await ensureFixedSession();

  const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-provider-recovery-chrome-'));
  chrome = spawn(
    chromePath,
    [
      '--headless=new',
      '--remote-debugging-port=' + debugPort,
      '--user-data-dir=' + profileDir,
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
  chrome.stdout.on('data', () => {});
  chrome.stderr.on('data', () => {});

  const target = await waitPageTarget();
  const cdp = await createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitFor(
    () =>
      evalPage(
        cdp,
        () => !!document.querySelector('[data-webui-shell="true"]') && !!document.getElementById('composer-input'),
      ),
    20000,
    'WebUI shell',
  );
  await selectFixedSession(cdp);
  await capture(cdp, '01-fixed-session');

  const prompt = 'Provider recovery UI fixture ' + Date.now() + '. Answer normally with completion schema.';
  await evalPage(
    cdp,
    (value) => {
      const input = document.getElementById('composer-input');
      input.value = value;
      input.dispatchEvent(new Event('input', { bubbles: true }));
      document.getElementById('send-button')?.click();
    },
    prompt,
  );

  let retryObserved = false;
  const retryStates = [];
  await waitFor(
    async () => {
      const state = await capture(cdp, '02-during-retry');
      retryObserved = retryObserved || state.providerRetryPresent;
      if (state.providerRetryPresent) {
        retryStates.push(state);
      }
      return state.providerRetryPresent;
    },
    20000,
    'provider retry visibility',
  );
  if (!retryObserved) {
    const state = await capture(cdp, '02-after-retry-window');
    retryObserved = state.providerRetryPresent;
    if (state.providerRetryPresent) {
      retryStates.push(state);
    }
  }

  const finalState = await waitFor(
    async () => {
      const state = await capture(cdp, '03-final-poll');
      retryObserved = retryObserved || state.providerRetryPresent;
      const sessionId = state.selectedSession;
      if (!sessionId) {
        return null;
      }
      const query = await adpQuerySession(sessionId).catch(() => null);
      const turns = (query && query.result && query.result.SessionTurns && query.result.SessionTurns.turns) || [];
      const current = turns.find((turn) => turn.turn_id === state.selectedTurn) || turns[turns.length - 1] || {};
      if (
        current.terminal_status === 'Success' &&
        `${state.selectedTerminalStatus || ''}`.toLowerCase() === 'success' &&
        state.liveCount === 0
      ) {
        return state;
      }
      return null;
    },
    90000,
    'ADP and DOM terminal success',
  );
  const selectedSession = finalState.selectedSession;
  retryObserved = retryObserved || finalState.providerRetryPresent;
  const adp = await adpQuerySession(selectedSession);
  await fs.writeFile(path.join(artifactDir, 'adp-session.json'), JSON.stringify(adp, null, 2));
  const turns = (adp.result && adp.result.SessionTurns && adp.result.SessionTurns.turns) || [];
  const lastTurn = turns.find((turn) => turn.turn_id === finalState.selectedTurn) || turns[turns.length - 1] || {};
  const summary = {
    ok: true,
    runId,
    artifactDir,
    requestCount,
    retryObserved,
    fixedSessionId,
    selectedSession,
    selectedTurn: finalState.selectedTurn,
    retryStates: retryStates.map((state) => ({
      selectedTurn: state.selectedTurn,
      selectedTurnCycleCount: state.selectedTurnCycleCount,
      selectedTurnUserCardCount: state.selectedTurnUserCardCount,
      providerRetryDetailText: state.providerRetryDetailText,
      providerRetryFlowLabelPresent: state.providerRetryFlowLabelPresent,
      duplicateCycleKeys: state.duplicateCycleKeys,
    })),
    finalState,
    adpTerminalStatus: lastTurn.terminal_status || null,
    adpErrors: lastTurn.errors || [],
    adpTurnIds: turns.map((turn) => turn.turn_id),
    checks: {
      fixtureRetriedThenRecovered: requestCount === 3,
      providerRetryVisible: retryObserved,
      providerRetryDetailVisible: retryStates.some((state) => state.providerRetryDetailVisible),
      retryNotRenderedAsProviderFlow: retryStates.every((state) => !state.providerRetryFlowLabelPresent),
      retryStayedOnSingleTurnCard: retryStates.every((state) => state.selectedTurnCycleCount <= 1),
      noDuplicateCycleKeys: finalState.duplicateCycleKeys.length === 0,
      finalSelectedTurnSingleCycle: finalState.selectedTurnCycleCount === 1,
      finalSelectedTurnSingleUserCard: finalState.selectedTurnUserCardCount === 1,
      fixedSessionReused: selectedSession === fixedSessionId,
      finalNoOpenAiRequestFailed: !finalState.errorTextPresent,
      finalNoProviderRetryText: !finalState.providerRetryPresent,
      adpSuccess: lastTurn.terminal_status === 'Success',
      adpNoErrors: Array.isArray(lastTurn.errors) && lastTurn.errors.length === 0,
      domTerminalSuccess: `${finalState.selectedTerminalStatus || ''}`.toLowerCase() === 'success',
      domNoLiveRows: finalState.liveCount === 0,
    },
  };
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  if (
    !summary.checks.fixtureRetriedThenRecovered ||
    !summary.checks.providerRetryVisible ||
    !summary.checks.providerRetryDetailVisible ||
    !summary.checks.retryNotRenderedAsProviderFlow ||
    !summary.checks.retryStayedOnSingleTurnCard ||
    !summary.checks.noDuplicateCycleKeys ||
    !summary.checks.finalSelectedTurnSingleCycle ||
    !summary.checks.finalSelectedTurnSingleUserCard ||
    !summary.checks.fixedSessionReused ||
    !summary.checks.finalNoOpenAiRequestFailed ||
    !summary.checks.finalNoProviderRetryText ||
    !summary.checks.adpSuccess ||
    !summary.checks.adpNoErrors ||
    !summary.checks.domTerminalSuccess ||
    !summary.checks.domNoLiveRows
  ) {
    process.exitCode = 1;
  }
  await cdp.close();
} finally {
  if (chrome?.pid && chrome.exitCode === null) {
    chrome.kill('SIGTERM');
    await new Promise((resolve) => chrome.once('exit', resolve));
  }
  if (fixtureServer) {
    await new Promise((resolve) => fixtureServer.close(resolve));
  }
  await fs.writeFile(configPath, originalConfig);
  await fs.writeFile(envPath, originalEnv);
  await run(['scripts/install-launchd.sh', 'restartS']);
}

function completionText() {
  const schema = {
    claim: 'complete',
    completion_reason: 'fixture provider recovered',
    evidence: 'fixture returned OpenAI Responses completed with error null after retry',
    summary: 'provider recovered without persistent error card',
    learned: 'transient provider errors update provider status only',
  };
  return 'fixture provider recovered\n<freehand_completion>\n' + JSON.stringify(schema) + '\n</freehand_completion>';
}

function successBody() {
  return JSON.stringify({
    id: 'resp-provider-recovery',
    object: 'response',
    status: 'completed',
    error: null,
    output: [
      {
        type: 'message',
        id: 'msg-provider-recovery',
        role: 'assistant',
        status: 'completed',
        content: [
          {
            type: 'output_text',
            text: completionText(),
            annotations: [],
          },
        ],
      },
    ],
    usage: {
      input_tokens: 10,
      output_tokens: 20,
      total_tokens: 30,
    },
  });
}

function startFixtureServer() {
  return new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      let body = '';
      req.on('data', (chunk) => {
        body += chunk;
      });
      req.on('end', () => {
        requestCount += 1;
        fss.appendFileSync(
          path.join(artifactDir, 'fixture-requests.jsonl'),
          JSON.stringify({
            count: requestCount,
            at: new Date().toISOString(),
            method: req.method,
            url: req.url,
            bodyLength: body.length,
          }) + '\n',
        );
        if (requestCount <= 2) {
          res.writeHead(500, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ error: { message: 'fixture transient ' + requestCount } }));
          return;
        }
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(successBody());
      });
    });
    server.on('error', reject);
    server.listen(fixturePort, '127.0.0.1', () => resolve(server));
  });
}

function stripFixtureEnv(value) {
  return value
    .replace(/\n?FREEHAND_PROVIDER_RECOVERY_FIXTURE_KEY=.*$/gm, '')
    .replace(/\n?FREEHAND_PROVIDER_RETRY_BACKOFF_MS=.*$/gm, '')
    .replace(/\n?FREEHAND_TEST_DISABLE_MASTER_LIFECYCLE_RUNNER=.*$/gm, '')
    .replace(/\n+$/g, '');
}

function redactEnv(value) {
  return value.replace(/(KEY|TOKEN|SECRET|PASSWORD|API)[A-Z0-9_]*=.*/gi, '$1=<redacted>');
}

function redactConfig(value) {
  return value
    .replace(/((?:api_key|token|secret|password)\s*=\s*)"[^"]*"/gi, '$1"<redacted>"')
    .replace(/((?:api_key|token|secret|password)\s*=\s*)'[^']*'/gi, "$1'<redacted>'");
}

async function waitHealth() {
  await waitFor(async () => {
    const res = await fetch('http://127.0.0.1:4042/health').catch(() => null);
    return res && res.ok;
  }, 60000, 'daemon health');
}

function run(argv, opts = {}) {
  return new Promise((resolve) => {
    const child = spawn(argv[0], argv.slice(1), {
      cwd: repo,
      stdio: ['ignore', 'pipe', 'pipe'],
      ...opts,
    });
    let stdout = '';
    let stderr = '';
    if (child.stdout) {
      child.stdout.on('data', (chunk) => {
        stdout += chunk;
      });
    }
    if (child.stderr) {
      child.stderr.on('data', (chunk) => {
        stderr += chunk;
      });
    }
    child.on('close', (code) => resolve({ code, stdout, stderr, argv }));
  });
}

async function must(argv, opts = {}) {
  const result = await run(argv, opts);
  if (result.code !== 0) {
    throw new Error('command failed ' + argv.join(' ') + '\nSTDOUT:\n' + result.stdout + '\nSTDERR:\n' + result.stderr);
  }
  return result;
}

function parseAdpConfigValue(stdout, key) {
  const pattern = new RegExp('(?:^|\\s)' + key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '=([^\\s]+)');
  const match = `${stdout || ''}`.match(pattern);
  return match ? match[1] : '';
}

async function waitFor(fn, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await fn().catch(() => null);
    if (value) {
      return value;
    }
    await delay(250);
  }
  throw new Error('timeout waiting for ' + label);
}

async function waitPageTarget() {
  return await waitFor(async () => {
    const response = await fetch('http://127.0.0.1:' + debugPort + '/json/list');
    if (!response.ok) {
      return null;
    }
    const targets = await response.json();
    return targets.find((target) => target.type === 'page' && String(target.url || '').startsWith(baseUrl));
  }, 20000, 'Chrome DevTools page target');
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
    socket.addEventListener('error', () => {
      reject(new Error('CDP socket error'));
    });
  });
}

async function waitForLoad(cdp) {
  await new Promise((resolve) => {
    const listener = (method) => {
      if (method === 'Page.loadEventFired') {
        cdp.offEvent(listener);
        resolve();
      }
    };
    cdp.onEvent(listener);
  });
}

async function ensureFixedSession() {
  const activeList = await adpRawRequest('query', 'query', 'QuerySessionList');
  const activeSessions =
    (activeList.result && activeList.result.SessionList && activeList.result.SessionList.sessions) || [];
  if (activeSessions.some((session) => session.session_id === fixedSessionId)) {
    return;
  }
  const archivedList = await adpRawRequest('query', 'query', 'QueryArchivedSessionList');
  const archivedSessions =
    (archivedList.result && archivedList.result.SessionList && archivedList.result.SessionList.sessions) || [];
  if (archivedSessions.some((session) => session.session_id === fixedSessionId)) {
    await adpRawRequest('command', 'command', {
      RestoreSession: { session_id: fixedSessionId },
    });
    return;
  }
  await adpRawRequest('command', 'command', {
    CreateSession: {
      session_id: fixedSessionId,
      title: 'Provider recovery fixed verifier',
      cwd: repo,
    },
  });
}

async function selectFixedSession(cdp) {
  await waitFor(
    () =>
      evalPage(
        cdp,
        (sessionId) => {
          const buttons = Array.from(document.querySelectorAll('.session-button[data-session-id]'));
          const button = buttons.find((candidate) => candidate.dataset.sessionId === sessionId);
          if (!button) {
            document.getElementById('refresh-session-button')?.click();
            return false;
          }
          button.click();
          return true;
        },
        fixedSessionId,
      ),
    20000,
    'fixed session visible in WebUI',
  );
  await waitFor(
    () =>
      evalPage(
        cdp,
        (sessionId) => {
          const shell = document.querySelector('[data-webui-shell="true"]');
          return (shell && shell.dataset.selectedSession) === sessionId;
        },
        fixedSessionId,
      ),
    20000,
    'fixed session selected',
  );
}

async function evalPage(cdp, fn, ...args) {
  const response = await cdp.send('Runtime.evaluate', {
    expression: '(' + fn + ')(...' + JSON.stringify(args) + ')',
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    throw new Error(response.exceptionDetails.text || 'Runtime.evaluate failed');
  }
  return response.result.value;
}

async function capture(cdp, name) {
  const state = await evalPage(cdp, () => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    const messageList = document.getElementById('message-list');
    const text = (messageList && messageList.innerText) || '';
    const selectedTurn = (shell && shell.dataset.selectedTurn) || '';
    const cycleCards = Array.from(document.querySelectorAll('.turn-cycle-card'));
    const cycleKeys = cycleCards.map((node) => node.dataset.cycleKey || '').filter(Boolean);
    const duplicateCycleKeys = cycleKeys.filter((key, index) => cycleKeys.indexOf(key) !== index);
    const selectedTurnCycleCards = cycleCards.filter((node) => node.dataset.turnId === selectedTurn);
    const selectedTurnText = selectedTurnCycleCards.map((node) => node.innerText || '').join('\n');
    const providerRetryDetailText = selectedTurnCycleCards
      .flatMap((node) => Array.from(node.querySelectorAll('.execution-row, .chat-message')))
      .map((node) => node.innerText || '')
      .find((value) => /transport retry|provider retry/i.test(value)) || '';
    const providerRetryFlowLabelPresent = selectedTurnCycleCards
      .flatMap((node) => Array.from(node.querySelectorAll('.execution-row-label')))
      .map((node) => (node.textContent || '').trim())
      .some((value) => value === 'Provider');
    return {
      selectedSession: (shell && shell.dataset.selectedSession) || '',
      selectedTurn,
      selectedTerminalStatus: (shell && shell.dataset.selectedTerminalStatus) || '',
      messageText: text,
      latestTurnText: selectedTurnText,
      errorTextPresent: /openai request failed|Error\\s+openai request failed/i.test(selectedTurnText),
      providerRetryPresent: /transport retry|provider retry|provider error retry scheduled/i.test(text),
      providerRetryDetailVisible:
        /transport retry/i.test(providerRetryDetailText) &&
        /provider retry\s+\d+\/\d+/i.test(providerRetryDetailText) &&
        /wait\s+\d+(?:ms|s)|before internal resend/i.test(providerRetryDetailText) &&
        /fixture transient|http_status_500|raw_hash/i.test(providerRetryDetailText),
      providerRetryDetailText,
      providerRetryFlowLabelPresent,
      providerFailoverPresent: /provider failover|provider route switched/i.test(text),
      terminalSuccessPresent: /provider recovered without persistent error card|fixture provider recovered/i.test(text),
      liveCount: document.querySelectorAll('[data-live="true"]').length,
      cycleCardCount: cycleCards.length,
      cycleKeys,
      duplicateCycleKeys: Array.from(new Set(duplicateCycleKeys)),
      selectedTurnCycleCount: selectedTurnCycleCards.length,
      selectedTurnUserCardCount: selectedTurnCycleCards.reduce(
        (count, node) => count + node.querySelectorAll('.chat-message-user').length,
        0,
      ),
    };
  });
  const screenshot = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  await fs.writeFile(path.join(artifactDir, name + '.png'), Buffer.from(screenshot.data, 'base64'));
  await fs.writeFile(path.join(artifactDir, name + '.json'), JSON.stringify(state, null, 2));
  return state;
}

async function adpQuerySession(sessionId) {
  return await adpRawRequest('query', 'query', {
    QuerySessionTurns: {
      session_id: sessionId,
    },
  });
}

function adpRawRequest(kind, payloadKey, payload) {
  const socket = new WebSocket(adpUrl);
  const requestId = 'provider-recovery-' + kind + '-' + Date.now() + '-' + Math.random().toString(36).slice(2);
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('ADP ' + kind + ' timeout')), 15000);
    socket.addEventListener('open', () => {
      socket.send(
        JSON.stringify({
          kind,
          request_id: requestId,
          [payloadKey]: payload,
        }),
      );
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
      resolve(message);
    });
    socket.addEventListener('error', () => {
      clearTimeout(timer);
      reject(new Error('ADP socket error'));
    });
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
