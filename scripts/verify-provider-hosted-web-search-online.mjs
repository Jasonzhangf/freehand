#!/usr/bin/env node
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import fss from 'node:fs';
import http from 'node:http';
import path from 'node:path';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const configPath =
  process.env.FREEHAND_PROVIDER_HOSTED_SEARCH_CONFIG || path.join(runtimeHome, 'config.toml');
const envPath =
  process.env.FREEHAND_PROVIDER_HOSTED_SEARCH_ENV || path.join(runtimeHome, 'daemonS.env');
const cli =
  process.env.FREEHAND_PROVIDER_HOSTED_SEARCH_CLI || path.join(home, '.local/bin/freehand-cliS');
const baseUrl = normalizedBaseUrl(
  process.env.FREEHAND_PROVIDER_HOSTED_SEARCH_BASE_URL || 'http://127.0.0.1:4042/',
);
const adpUrl = process.env.FREEHAND_PROVIDER_HOSTED_SEARCH_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const fixedSessionId =
  process.env.FREEHAND_PROVIDER_HOSTED_SEARCH_SESSION ||
  'provider-hosted-web-search-online-fixed';
const verifierCwd =
  process.env.FREEHAND_PROVIDER_HOSTED_SEARCH_CWD || runtimeHome;
const fixtureKeyName = 'FREEHAND_PROVIDER_HOSTED_SEARCH_FIXTURE_KEY';
const fixtureProviderId =
  process.env.FREEHAND_PROVIDER_HOSTED_SEARCH_PROVIDER || 'provider-hosted-search-fixture';
const submitReceiptTimeoutMs = positiveIntegerEnv(
  'FREEHAND_PROVIDER_HOSTED_SEARCH_SUBMIT_RECEIPT_TIMEOUT_MS',
  45_000,
);
const onlineRunTimeoutMs = positiveIntegerEnv(
  'FREEHAND_PROVIDER_HOSTED_SEARCH_TIMEOUT_MS',
  600_000,
);
const runId = `provider-hosted-web-search-${new Date()
  .toISOString()
  .replace(/[-:]/g, '')
  .slice(0, 15)}-${process.pid}`;
const runMarker = runId;
const searchQuery = `Freehand provider-hosted search verifier ${runMarker}`;
const finalTextMarker = `provider hosted web_search online verifier complete ${runMarker}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);

let providerServer;
let requestCount = 0;
let firstProviderRequest = null;
let restoreFailure = null;
let submitReceipt = null;
let submitError = null;
let lastObservation = null;

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
  await must(['scripts/install-launchd.sh', 'restartS']);
  await waitHealth();
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
    `${providerBaseUrl()}/openai/v1`,
    '--model',
    'gpt-5.5',
    '--web-search',
    'auto',
    '--api-key-env',
    fixtureKeyName,
  ]);
  await must(['scripts/install-launchd.sh', 'restartS']);
  await waitHealth();
  await ensureFixedSession();
  await rollbackFixedSessionTranscript();

  const beforeAdp = await querySessionTurns();
  await fs.writeFile(
    path.join(artifactDir, 'adp-session-before.json'),
    JSON.stringify(beforeAdp, null, 2),
  );
  const beforeTurns = sessionTurnsFromAdp(beforeAdp);
  const beforeTurnIds = new Set(beforeTurns.map((turn) => turn.turn_id).filter(Boolean));

  const prompt = [
    `Provider-hosted web_search online verifier RUN_MARKER=${runMarker}.`,
    `Use provider-native web_search if declared to search: ${searchQuery}.`,
    'Do not call web_fetch; this is a broad provider-hosted search proof.',
    'Then answer with the required Freehand completion schema and summarize the hosted-search result.',
  ].join(' ');
  const submitAttempt = await adpCommand(
    {
      SubmitUserInput: {
        text: prompt,
        session_id: fixedSessionId,
        cwd: verifierCwd,
      },
    },
    submitReceiptTimeoutMs,
  )
    .then((receipt) => ({ ok: true, receipt }))
    .catch((error) => ({
      ok: false,
      error: error && (error.stack || error.message || String(error)),
    }));
  if (submitAttempt.ok) {
    submitReceipt = submitAttempt.receipt;
  } else {
    submitError = submitAttempt.error;
  }
  await fs.writeFile(
    path.join(artifactDir, 'submit-attempt.json'),
    JSON.stringify(submitAttempt, null, 2),
  );

  const observation = await waitForCurrentRun(beforeTurnIds, onlineRunTimeoutMs);
  const { adp, currentRunTurns, currentRunText, latestTurn } = observation;
  await fs.writeFile(path.join(artifactDir, 'adp-session.json'), JSON.stringify(adp, null, 2));
  const configAfterFixture = await run([cli, 'adp-config-query', '--url', adpUrl]);

  const providerRequests = await readProviderRequestSummaries();
  const firstBodyText = JSON.stringify(firstProviderRequest || {});
  const firstFunctionToolNames = requestFunctionToolNames(firstProviderRequest);
  const firstHostedToolTypes = requestHostedToolTypes(firstProviderRequest);
  const checks = {
    fixedSessionReused: latestTurn.session_id ? latestTurn.session_id === fixedSessionId : true,
    submitMaterialized: Boolean(submitReceipt) || currentRunTurns.length > 0,
    fixtureSawOneProviderRequest: requestCount === 1,
    firstRequestDeclaredHostedWebSearch: firstHostedToolTypes.includes('web_search'),
    firstRequestHostedWebSearchExternalAccess: requestHostedWebSearchExternalAccess(
      firstProviderRequest,
    ),
    firstRequestDidNotDeclareFunctionWebSearch: !firstFunctionToolNames.includes('web_search'),
    firstRequestKeptConcreteUrlWebFetchAvailable: firstFunctionToolNames.includes('web_fetch'),
    firstRequestStillHasMasterFunctionTools:
      firstFunctionToolNames.includes('task') && firstFunctionToolNames.includes('timer'),
    firstRequestContextMentionsHostedSearch: firstBodyText.includes('provider-hosted'),
    adpObservedHostedSearch: currentRunText.includes('provider-hosted web_search'),
    adpObservedSearchQuery: currentRunText.includes(`query=${searchQuery}`),
    adpDidNotUseWebFetchAsSearch:
      !currentRunText.includes('"name":"web_fetch"') &&
      !currentRunText.includes('function_call_output') &&
      !currentRunText.includes('WEB_FETCH'),
    latestTurnSucceeded: latestTurn.terminal_status === 'Success',
    finalMarkerVisible: currentRunText.includes(finalTextMarker),
  };
  const summary = {
    ok: Object.values(checks).every(Boolean),
    runId,
    artifactDir,
    adpUrl,
    baseUrl,
    fixedSessionId,
    searchQuery,
    requestCount,
    providerRequests,
    submitReceipt,
    submitError,
    beforeTurnIds: Array.from(beforeTurnIds),
    currentRunTurnIds: currentRunTurns.map((turn) => turn.turn_id),
    latestTurnId: latestTurn.turn_id || null,
    latestTerminalStatus: latestTurn.terminal_status || null,
    firstFunctionToolNames,
    firstHostedToolTypes,
    firstRequestHostedWebSearchExternalAccess: checks.firstRequestHostedWebSearchExternalAccess,
    adpObservedHostedSearch: checks.adpObservedHostedSearch,
    adpObservedSearchQuery: checks.adpObservedSearchQuery,
    lastObservation,
    configAfterFixture: `${configAfterFixture.stdout}${configAfterFixture.stderr}`.trim(),
    checks,
  };
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  if (!summary.ok) {
    const failed = Object.entries(checks)
      .filter(([, value]) => !value)
      .map(([key]) => key);
    throw new Error(`provider hosted web_search online checks failed: ${failed.join(', ')}`);
  }
  console.log(
    `provider_hosted_web_search_online_ok url=${adpUrl} session=${fixedSessionId} turns=${summary.currentRunTurnIds.join(',')} provider_requests=${requestCount} hosted_tools=${firstHostedToolTypes.join(',')}`,
  );
} catch (error) {
  await captureFailureState(error).catch((captureError) => {
    console.error(`failure capture failed: ${captureError.stack || captureError.message}`);
  });
  throw error;
} finally {
  if (providerServer) {
    await new Promise((resolve) => providerServer.close(resolve));
  }
  restoreFailure = await restoreRuntime();
  if (restoreFailure) {
    console.error(restoreFailure);
    process.exitCode = 1;
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
          hasHostedWebSearch: requestHostedToolTypes(parsed).includes('web_search'),
          hasFunctionWebSearch: requestFunctionToolNames(parsed).includes('web_search'),
          hasWebFetchFunction: requestFunctionToolNames(parsed).includes('web_fetch'),
          bodyLength: body.length,
        };
        fss.appendFileSync(
          path.join(artifactDir, 'provider-requests.jsonl'),
          `${JSON.stringify(requestSummary)}\n`,
        );
        await fs.writeFile(
          path.join(artifactDir, `provider-request-${String(requestCount).padStart(3, '0')}.json`),
          JSON.stringify(parsed, null, 2),
        );
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(finalHostedSearchCompletionBody());
      });
    });
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

function providerBaseUrl() {
  const address = providerServer.address();
  return `http://127.0.0.1:${address.port}`;
}

function finalHostedSearchCompletionBody() {
  const schema = {
    claim: 'complete',
    completion_reason: 'provider-hosted web_search online verifier observed hosted search',
    evidence: `provider response carried web_search_call action search query=${searchQuery}`,
    summary: finalTextMarker,
    learned: 'OpenAI Responses hosted web_search is provider-native and not a Freehand function tool',
  };
  return JSON.stringify({
    id: 'resp-provider-hosted-web-search-online',
    object: 'response',
    status: 'completed',
    error: null,
    output: [
      {
        type: 'web_search_call',
        id: `ws-${runMarker}`,
        status: 'completed',
        action: {
          type: 'search',
          query: searchQuery,
        },
      },
      {
        type: 'message',
        id: 'msg-provider-hosted-web-search-online-final',
        role: 'assistant',
        status: 'completed',
        content: [
          {
            type: 'output_text',
            text: `${finalTextMarker}\nProvider-hosted search query: ${searchQuery}\n<freehand_completion>\n${JSON.stringify(schema)}\n</freehand_completion>`,
            annotations: [],
          },
        ],
      },
    ],
    usage: {
      input_tokens: 20,
      output_tokens: 24,
      total_tokens: 44,
    },
  });
}

async function ensureFixedSession() {
  const activeList = await adpQuery('QuerySessionList');
  const activeSessions = activeList?.SessionList?.sessions || [];
  if (activeSessions.some((session) => session.session_id === fixedSessionId)) {
    return;
  }
  const archivedList = await adpQuery('QueryArchivedSessionList');
  const archivedSessions = archivedList?.SessionList?.sessions || [];
  if (archivedSessions.some((session) => session.session_id === fixedSessionId)) {
    await adpCommand({ RestoreSession: { session_id: fixedSessionId } });
    return;
  }
  await adpCommand({
    CreateSession: {
      session_id: fixedSessionId,
      title: 'provider hosted web_search online verifier fixed session',
      cwd: verifierCwd,
    },
  });
}

async function rollbackFixedSessionTranscript() {
  const evidence = [];
  for (let attempt = 1; attempt <= 20; attempt += 1) {
    const turns = sessionTurnsFromAdp(await querySessionTurns());
    const turnIds = turns.map((turn) => turn.turn_id).filter(Boolean);
    evidence.push({ attempt, turnIds });
    if (turnIds.length === 0) {
      await fs.writeFile(
        path.join(artifactDir, 'fixed-session-reset.json'),
        JSON.stringify({ fixedSessionId, evidence }, null, 2),
      );
      return;
    }
    const rollback = await run([
      cli,
      'adp-session-manage',
      '--url',
      adpUrl,
      '--action',
      'rollback',
      '--session',
      fixedSessionId,
    ]);
    evidence[evidence.length - 1].rollback = {
      code: rollback.code,
      stdout: rollback.stdout.trim(),
      stderr: rollback.stderr.trim(),
    };
    if (rollback.code !== 0) {
      await fs.writeFile(
        path.join(artifactDir, 'fixed-session-reset.json'),
        JSON.stringify({ fixedSessionId, evidence }, null, 2),
      );
      throw new Error(`fixed session rollback failed: ${rollback.stderr || rollback.stdout}`);
    }
  }
  await fs.writeFile(
    path.join(artifactDir, 'fixed-session-reset.json'),
    JSON.stringify({ fixedSessionId, evidence }, null, 2),
  );
  throw new Error(`fixed session reset exceeded rollback limit for ${fixedSessionId}`);
}

async function captureFailureState(error) {
  const failureDir = path.join(artifactDir, 'failure');
  await fs.mkdir(failureDir, { recursive: true });
  await fs.writeFile(
    path.join(failureDir, 'failure.json'),
    JSON.stringify(
      {
        ok: false,
        runId,
        fixedSessionId,
        searchQuery,
        requestCount,
        submitReceipt,
        submitError,
        lastObservation,
        providerRequests: await readProviderRequestSummaries(),
        error: error && (error.stack || error.message || String(error)),
      },
      null,
      2,
    ),
  );
  await Promise.all([
    querySessionTurns()
      .then((value) =>
        fs.writeFile(path.join(failureDir, 'adp-session-turns.json'), JSON.stringify(value, null, 2)),
      )
      .catch((queryError) =>
        fs.writeFile(
          path.join(failureDir, 'adp-session-turns-error.txt'),
          queryError.stack || queryError.message,
        ),
      ),
    run([cli, 'adp-config-query', '--url', adpUrl]).then((value) =>
      fs.writeFile(path.join(failureDir, 'config-after.txt'), `${value.stdout}${value.stderr}`),
    ),
    fs.writeFile(
      path.join(failureDir, 'daemonS.stderr.tail.txt'),
      await tailText(path.join(runtimeHome, 'logs', 'daemonS.stderr.log'), 160),
    ),
    fs.writeFile(
      path.join(failureDir, 'daemonS.stdout.tail.txt'),
      await tailText(path.join(runtimeHome, 'logs', 'daemonS.stdout.log'), 160),
    ),
  ]);
}

async function waitForCurrentRun(beforeTurnIds, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastQueryError = null;
  let lastObservationWriteAt = 0;
  while (Date.now() < deadline) {
    try {
      const adp = await querySessionTurns();
      const turns = sessionTurnsFromAdp(adp);
      const currentRunTurns = turns.filter((turn) => !beforeTurnIds.has(turn.turn_id));
      const currentRunText = JSON.stringify(currentRunTurns);
      const latestTurn =
        currentRunTurns[currentRunTurns.length - 1] || turns[turns.length - 1] || {};
      const providerRequests = await readProviderRequestSummaries();
      lastObservation = {
        at: new Date().toISOString(),
        currentRunTurnIds: currentRunTurns.map((turn) => turn.turn_id),
        latestTurnId: latestTurn.turn_id || null,
        latestTerminalStatus: latestTurn.terminal_status || null,
        latestModelRequest: latestTurn.model_request || null,
        latestToolActivities: latestTurn.tool_activities || [],
        requestCount,
        providerRequests,
        lastQueryError,
      };
      if (Date.now() - lastObservationWriteAt > 2_000) {
        lastObservationWriteAt = Date.now();
        await fs.writeFile(
          path.join(artifactDir, 'live-observation.json'),
          JSON.stringify(lastObservation, null, 2),
        );
      }
      if (latestTurn.terminal_status === 'Success' && requestCount >= 1) {
        return { adp, turns, currentRunTurns, currentRunText, latestTurn };
      }
      if (isTerminalFailure(latestTurn.terminal_status)) {
        const failure = new Error(
          `current run terminalized before provider-hosted web_search proof: status=${latestTurn.terminal_status}`,
        );
        failure.nonRetryable = true;
        throw failure;
      }
    } catch (error) {
      if (error && error.nonRetryable) {
        throw error;
      }
      lastQueryError = error && (error.stack || error.message || String(error));
    }
    await delay(500);
  }
  throw new Error(
    `timeout waiting for provider-hosted web_search online completion after ${timeoutMs}ms; last_observation=${JSON.stringify(lastObservation)}`,
  );
}

async function readProviderRequestSummaries() {
  const text = await fs.readFile(path.join(artifactDir, 'provider-requests.jsonl'), 'utf8').catch(() => '');
  return text
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

async function restoreRuntime() {
  const errors = [];
  await fs.writeFile(configPath, originalConfig).catch((error) => errors.push(error.message));
  await fs.writeFile(envPath, stripFixtureEnv(originalEnv)).catch((error) => errors.push(error.message));
  const restart = await run(['scripts/install-launchd.sh', 'restartS']);
  if (restart.code !== 0) {
    errors.push(`restartS restore failed: ${restart.stderr || restart.stdout}`);
  } else {
    await waitHealth().catch((error) => errors.push(error.message));
  }
  const config = await run([cli, 'adp-config-query', '--url', adpUrl]);
  const envMatches = await fixtureEnvMatches(envPath);
  const restoreSummary = {
    ok: errors.length === 0 && config.code === 0 && envMatches.length === 0,
    errors,
    config: `${config.stdout}${config.stderr}`.trim(),
    fixtureEnvMatches: envMatches,
  };
  await fs
    .writeFile(path.join(artifactDir, 'restore-summary.json'), JSON.stringify(restoreSummary, null, 2))
    .catch(() => null);
  if (!restoreSummary.ok) {
    return JSON.stringify(restoreSummary, null, 2);
  }
  console.log(`provider_hosted_web_search_restore_ok ${restoreSummary.config}`);
  return null;
}

async function adpQuery(query) {
  return await adpRequest('query', 'query', query, 30_000);
}

async function querySessionTurns() {
  return await adpQuery({ QuerySessionTurns: { session_id: fixedSessionId } });
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
    throw new Error(`command failed ${argv.join(' ')}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}`);
  }
  return result;
}

async function waitHealth() {
  await waitFor(async () => {
    const response = await fetch(new URL('/health', baseUrl));
    return response.ok;
  }, 60_000, 'daemon health');
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

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function tailText(file, maxLines) {
  const text = await fs.readFile(file, 'utf8').catch((error) => {
    if (error && error.code === 'ENOENT') {
      return '';
    }
    throw error;
  });
  const lines = text.split(/\r?\n/);
  return lines.slice(Math.max(0, lines.length - maxLines)).join('\n');
}

async function fixtureEnvMatches(file) {
  const text = await fs.readFile(file, 'utf8').catch((error) => {
    if (error && error.code === 'ENOENT') {
      return '';
    }
    throw error;
  });
  return text
    .split(/\r?\n/)
    .map((line, index) => ({ line: index + 1, text: line }))
    .filter(({ text: line }) => line.includes(fixtureKeyName));
}

function sessionTurnsFromAdp(adp) {
  return adp?.SessionTurns?.turns || [];
}

function requestFunctionToolNames(value) {
  const tools = value && Array.isArray(value.tools) ? value.tools : [];
  return tools
    .filter((tool) => tool && (tool.type === 'function' || tool.name || tool.function?.name))
    .map((tool) => tool.name || tool.function?.name)
    .filter(Boolean);
}

function requestHostedToolTypes(value) {
  const tools = value && Array.isArray(value.tools) ? value.tools : [];
  return tools
    .filter((tool) => tool && tool.type && tool.type !== 'function')
    .map((tool) => tool.type)
    .filter(Boolean);
}

function requestHostedWebSearchExternalAccess(value) {
  const tools = value && Array.isArray(value.tools) ? value.tools : [];
  return tools.some(
    (tool) => tool && tool.type === 'web_search' && tool.external_web_access === true,
  );
}

function parseJsonOrNull(value) {
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
}

function isTerminalFailure(status) {
  return Boolean(status) && status !== 'Success' && status !== 'ToolPending';
}

function positiveIntegerEnv(name, defaultValue) {
  const raw = process.env[name];
  if (!raw) {
    return defaultValue;
  }
  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer, got ${raw}`);
  }
  return Math.floor(value);
}

function stripFixtureEnv(value) {
  return value
    .replace(new RegExp(`\\n?${fixtureKeyName}=.*$`, 'gm'), '')
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

function normalizedBaseUrl(value) {
  return value.endsWith('/') ? value : `${value}/`;
}

function adpUrlFromBaseUrl(value) {
  const url = new URL(value);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  url.pathname = path.posix.join(url.pathname, 'adp');
  return url.toString();
}
