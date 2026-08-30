#!/usr/bin/env node
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import fss from 'node:fs';
import http from 'node:http';
import path from 'node:path';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const configPath = process.env.FREEHAND_WEB_FETCH_VERIFY_CONFIG || path.join(runtimeHome, 'config.toml');
const envPath = process.env.FREEHAND_WEB_FETCH_VERIFY_ENV || path.join(runtimeHome, 'daemonS.env');
const cli = process.env.FREEHAND_WEB_FETCH_VERIFY_CLI || path.join(home, '.local/bin/freehand-cliS');
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WEB_FETCH_VERIFY_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_WEB_FETCH_VERIFY_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const fixedSessionId =
  process.env.FREEHAND_WEB_FETCH_VERIFY_SESSION || 'web-fetch-tool-online-fixed';
const fixtureKeyName = 'FREEHAND_WEB_FETCH_VERIFY_FIXTURE_KEY';
const fixtureProviderId = process.env.FREEHAND_WEB_FETCH_VERIFY_PROVIDER || 'web-fetch-fixture';
const submitReceiptTimeoutMs = positiveIntegerEnv(
  'FREEHAND_WEB_FETCH_VERIFY_SUBMIT_RECEIPT_TIMEOUT_MS',
  45_000,
);
const onlineRunTimeoutMs = positiveIntegerEnv('FREEHAND_WEB_FETCH_VERIFY_TIMEOUT_MS', 600_000);
const runId = `web-fetch-tool-${new Date().toISOString().replace(/[-:]/g, '').slice(0, 15)}-${process.pid}`;
const runMarker = runId;
const pageBody = `WEB_FETCH_TOOL_ONLINE_BODY ${runMarker} fetched-content`;
const finalTextMarker = `web_fetch online verifier complete ${runMarker}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);

let pageServer;
let providerServer;
let pageUrl;
let requestCount = 0;
let pageRequestCount = 0;
let firstProviderRequest = null;
let secondProviderRequest = null;
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

  pageServer = await startPageServer();
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
    '--api-key-env',
    fixtureKeyName,
  ]);
  await must(['scripts/install-launchd.sh', 'restartS']);
  await waitHealth();
  await ensureFixedSession();

  const beforeAdp = await querySessionTurns();
  await fs.writeFile(path.join(artifactDir, 'adp-session-before.json'), JSON.stringify(beforeAdp, null, 2));
  const beforeTurns = sessionTurnsFromAdp(beforeAdp);
  const beforeTurnIds = new Set(beforeTurns.map((turn) => turn.turn_id).filter(Boolean));

  const prompt = [
    `web_fetch online verifier RUN_MARKER=${runMarker}.`,
    `Fetch this exact URL with the web_fetch tool: ${pageUrl}`,
    'Then answer with the required Freehand completion schema and include the fetched marker.',
  ].join(' ');
  const submitAttempt = await adpCommand(
    {
      SubmitUserInput: {
        text: prompt,
        session_id: fixedSessionId,
        cwd: repo,
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
  await fs.writeFile(path.join(artifactDir, 'submit-attempt.json'), JSON.stringify(submitAttempt, null, 2));

  const observation = await waitForCurrentRun(beforeTurnIds, onlineRunTimeoutMs);
  const { adp, turns, currentRunTurns, currentRunText, latestTurn } = observation;
  await fs.writeFile(path.join(artifactDir, 'adp-session.json'), JSON.stringify(adp, null, 2));
  const configAfterFixture = await run([cli, 'adp-config-query', '--url', adpUrl]);

  const providerRequests = await readProviderRequestSummaries();
  const firstToolNames = requestToolNames(firstProviderRequest);
  const secondBodyText = JSON.stringify(secondProviderRequest || {});
  const firstBodyText = JSON.stringify(firstProviderRequest || {});
  const checks = {
    fixedSessionReused: latestTurn.session_id ? latestTurn.session_id === fixedSessionId : true,
    submitMaterialized: Boolean(submitReceipt) || currentRunTurns.length > 0,
    fixtureSawTwoProviderRequests: requestCount === 2,
    firstRequestAdvertisedWebFetch: firstToolNames.includes('web_fetch'),
    firstRequestAdvertisedTaskAndTimer: firstToolNames.includes('task') && firstToolNames.includes('timer'),
    firstRequestContextNamesMasterNetworkSurface: firstBodyText.includes('Master network tool surface'),
    firstRequestContextNamesWorkerCapabilities:
      firstBodyText.includes('Configured Worker capability surface') &&
      firstBodyText.includes('configured_worker_capabilities') &&
      firstBodyText.includes('network_tools'),
    pageFixtureFetchedExactlyOnce: pageRequestCount === 1,
    secondRequestContainsFunctionCallOutput: requestHasFunctionCallOutput(secondProviderRequest),
    secondRequestContainsFetchedBody: secondBodyText.includes(pageBody),
    adpRetainedWebFetchTool: currentRunText.includes('web_fetch'),
    adpRetainedFetchedBody: currentRunText.includes(pageBody),
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
    pageUrl,
    pageBody,
    requestCount,
    pageRequestCount,
    providerRequests,
    submitReceipt,
    submitError,
    beforeTurnIds: Array.from(beforeTurnIds),
    currentRunTurnIds: currentRunTurns.map((turn) => turn.turn_id),
    latestTurnId: latestTurn.turn_id || null,
    latestTerminalStatus: latestTurn.terminal_status || null,
    firstRequestToolNames: firstToolNames,
    secondRequestHasFunctionCallOutput: requestHasFunctionCallOutput(secondProviderRequest),
    secondRequestContainsFetchedBody: checks.secondRequestContainsFetchedBody,
    adpRetainedWebFetchTool: checks.adpRetainedWebFetchTool,
    adpRetainedFetchedBody: checks.adpRetainedFetchedBody,
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
    throw new Error(`web_fetch online checks failed: ${failed.join(', ')}`);
  }
  console.log(
    `web_fetch_tool_online_ok url=${adpUrl} session=${fixedSessionId} turns=${summary.currentRunTurnIds.join(',')} provider_requests=${requestCount} page_requests=${pageRequestCount}`,
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
  if (pageServer) {
    await new Promise((resolve) => pageServer.close(resolve));
  }
  restoreFailure = await restoreRuntime();
  if (restoreFailure) {
    console.error(restoreFailure);
    process.exitCode = 1;
  }
}

async function startPageServer() {
  return await new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      pageRequestCount += 1;
      fss.appendFileSync(
        path.join(artifactDir, 'page-requests.jsonl'),
        JSON.stringify({
          count: pageRequestCount,
          at: new Date().toISOString(),
          method: req.method,
          url: req.url,
        }) + '\n',
      );
      res.writeHead(200, {
        'content-type': 'text/plain; charset=utf-8',
        'cache-control': 'no-store, max-age=0',
      });
      res.end(`${pageBody}\n`);
    });
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      pageUrl = `http://127.0.0.1:${address.port}/fixture`;
      resolve(server);
    });
  });
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
        const hasToolOutput = requestHasFunctionCallOutput(parsed);
        if (requestCount === 1) {
          firstProviderRequest = parsed;
        }
        if (hasToolOutput) {
          secondProviderRequest = parsed;
        }
        const requestSummary = {
          count: requestCount,
          at: new Date().toISOString(),
          method: req.method,
          url: req.url,
          hasToolOutput,
          containsFetchedBody: body.includes(pageBody),
          toolNames: requestToolNames(parsed),
          bodyLength: body.length,
        };
        fss.appendFileSync(
          path.join(artifactDir, 'provider-requests.jsonl'),
          JSON.stringify(requestSummary) + '\n',
        );
        await fs.writeFile(
          path.join(artifactDir, `provider-request-${String(requestCount).padStart(3, '0')}.json`),
          JSON.stringify(parsed, null, 2),
        );
        if (!hasToolOutput) {
          res.writeHead(200, { 'content-type': 'application/json' });
          res.end(firstWebFetchToolCallBody());
          return;
        }
        if (!body.includes(pageBody)) {
          res.writeHead(500, { 'content-type': 'application/json' });
          res.end(
            JSON.stringify({
              error: {
                type: 'fixture_error',
                message: 'second provider request did not include fetched page body',
              },
            }),
          );
          return;
        }
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(finalCompletionBody());
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

function firstWebFetchToolCallBody() {
  return JSON.stringify({
    id: 'resp-web-fetch-online-1',
    object: 'response',
    status: 'in_progress',
    output: [
      {
        type: 'reasoning',
        summary: [{ text: 'Need to fetch the concrete URL with web_fetch.' }],
      },
      {
        type: 'function_call',
        call_id: `call-web-fetch-${runMarker}`,
        name: 'web_fetch',
        arguments: JSON.stringify({
          url: pageUrl,
          timeout_seconds: 5,
          limit: 4096,
        }),
      },
    ],
    usage: {
      input_tokens: 10,
      output_tokens: 5,
      total_tokens: 15,
    },
  });
}

function finalCompletionBody() {
  const schema = {
    claim: 'complete',
    completion_reason: 'web_fetch online verifier observed tool output re-entry',
    evidence: `second provider request contained function_call_output with ${pageBody}`,
    summary: finalTextMarker,
    learned: 'Master can call web_fetch for concrete HTTP URLs and continue with the fetched result',
  };
  return JSON.stringify({
    id: 'resp-web-fetch-online-2',
    object: 'response',
    status: 'completed',
    error: null,
    output: [
      {
        type: 'message',
        id: 'msg-web-fetch-online-final',
        role: 'assistant',
        status: 'completed',
        content: [
          {
            type: 'output_text',
            text: `${finalTextMarker}\nFetched marker: ${pageBody}\n<freehand_completion>\n${JSON.stringify(schema)}\n</freehand_completion>`,
            annotations: [],
          },
        ],
      },
    ],
    usage: {
      input_tokens: 20,
      output_tokens: 20,
      total_tokens: 40,
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
      title: 'web_fetch online verifier fixed session',
      cwd: repo,
    },
  });
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
        pageUrl,
        pageBody,
        requestCount,
        pageRequestCount,
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
      .then((value) => fs.writeFile(path.join(failureDir, 'adp-session-turns.json'), JSON.stringify(value, null, 2)))
      .catch((queryError) =>
        fs.writeFile(path.join(failureDir, 'adp-session-turns-error.txt'), queryError.stack || queryError.message),
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
      const latestTurn = currentRunTurns[currentRunTurns.length - 1] || turns[turns.length - 1] || {};
      const providerRequests = await readProviderRequestSummaries();
      lastObservation = {
        at: new Date().toISOString(),
        currentRunTurnIds: currentRunTurns.map((turn) => turn.turn_id),
        latestTurnId: latestTurn.turn_id || null,
        latestTerminalStatus: latestTurn.terminal_status || null,
        latestModelRequest: latestTurn.model_request || null,
        latestToolActivities: latestTurn.tool_activities || [],
        requestCount,
        pageRequestCount,
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
      if (latestTurn.terminal_status === 'Success' && requestCount >= 2) {
        return { adp, turns, currentRunTurns, currentRunText, latestTurn };
      }
      if (isTerminalFailure(latestTurn.terminal_status)) {
        const error = new Error(
          `current run terminalized before web_fetch proof: status=${latestTurn.terminal_status}`,
        );
        error.nonRetryable = true;
        throw error;
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
    `timeout waiting for web_fetch online completion after ${timeoutMs}ms; last_observation=${JSON.stringify(
      lastObservation,
    )}`,
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
  await fs.writeFile(path.join(artifactDir, 'restore-summary.json'), JSON.stringify(restoreSummary, null, 2)).catch(
    () => null,
  );
  if (!restoreSummary.ok) {
    return JSON.stringify(restoreSummary, null, 2);
  }
  console.log(`web_fetch_tool_restore_ok ${restoreSummary.config}`);
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

function requestHasFunctionCallOutput(value) {
  if (!value) {
    return false;
  }
  const input = value.input;
  if (Array.isArray(input) && input.some((item) => item && item.type === 'function_call_output')) {
    return true;
  }
  return JSON.stringify(value).includes('function_call_output');
}

function requestToolNames(value) {
  const tools = value && Array.isArray(value.tools) ? value.tools : [];
  return tools
    .map((tool) => tool && (tool.name || tool.function?.name))
    .filter(Boolean);
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
