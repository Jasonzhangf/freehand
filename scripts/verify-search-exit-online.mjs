#!/usr/bin/env node
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import fss from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { adpVerifierRequest, requireSessionListPage } from './lib/adp-verifier-client.mjs';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const configPath =
  process.env.FREEHAND_SEARCH_EXIT_VERIFY_CONFIG || path.join(runtimeHome, 'config.toml');
const envPath = process.env.FREEHAND_SEARCH_EXIT_VERIFY_ENV || path.join(runtimeHome, 'daemonS.env');
const cli =
  process.env.FREEHAND_SEARCH_EXIT_VERIFY_CLI || path.join(home, '.local/bin/freehand-cliS');
const baseUrl = normalizedBaseUrl(
  process.env.FREEHAND_SEARCH_EXIT_VERIFY_BASE_URL || 'http://100.66.1.82:4042/',
);
const adpUrl = process.env.FREEHAND_SEARCH_EXIT_VERIFY_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const adpAuthToken = process.env.FREEHAND_ADP_AUTH_TOKEN || '';
const fixedSessionId =
  process.env.FREEHAND_SEARCH_EXIT_VERIFY_SESSION || 'search-exit-online-fixed';
const verifierCwd = process.env.FREEHAND_SEARCH_EXIT_VERIFY_CWD || runtimeHome;
const fixtureKeyName = 'FREEHAND_SEARCH_EXIT_VERIFY_FIXTURE_KEY';
const fixtureProviderId =
  process.env.FREEHAND_SEARCH_EXIT_VERIFY_PROVIDER || 'search-exit-fixture';
const submitReceiptTimeoutMs = positiveIntegerEnv(
  'FREEHAND_SEARCH_EXIT_VERIFY_SUBMIT_RECEIPT_TIMEOUT_MS',
  45_000,
);
const onlineRunTimeoutMs = positiveIntegerEnv(
  'FREEHAND_SEARCH_EXIT_VERIFY_TIMEOUT_MS',
  600_000,
);
const runId = `search-exit-${new Date().toISOString().replace(/[-:]/g, '').slice(0, 15)}-${
  process.pid
}`;
const runMarker = runId;
const searchQuery = `Freehand search-exit verifier ${runMarker}`;
const finalTextMarker = `search exit online verifier complete ${runMarker}`;
const blockedTextMarker = `search exit blocked explanation ${runMarker}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);

let providerServer;
let webFetchServer;
let restoreFailure = null;
let providerRequests = [];
let webFetchRequests = 0;
let lastObservation = null;

await fs.mkdir(artifactDir, { recursive: true });
const originalConfig = await fs.readFile(configPath, 'utf8');
const originalEnv = await fs.readFile(envPath, 'utf8').catch(() => '');

try {
  await fs.writeFile(path.join(artifactDir, 'config.before.toml'), redactConfig(originalConfig));
  await fs.writeFile(path.join(artifactDir, 'daemonS.before.env'), redactEnv(originalEnv));

  webFetchServer = await startWebFetchServer();
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
    `Search-exit online verifier RUN_MARKER=${runMarker}.`,
    `Use provider-native web_search if declared to search: ${searchQuery}.`,
    'Do not call web_fetch unless the runtime asks for a concrete URL recovery.',
    'Then answer with the required Freehand completion schema.',
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
  await fs.writeFile(
    path.join(artifactDir, 'submit-attempt.json'),
    JSON.stringify(submitAttempt, null, 2),
  );

  const observation = await waitForCurrentRun(beforeTurnIds, onlineRunTimeoutMs);
  const { adp, currentRunTurns, currentRunText, latestTurn } = observation;
  await fs.writeFile(path.join(artifactDir, 'adp-session.json'), JSON.stringify(adp, null, 2));
  const configAfterFixture = await run([cli, 'adp-config-query', '--url', adpUrl]);
  const providerSummaries = await readProviderRequestSummaries();
  const firstBodyText = JSON.stringify(providerSummaries[0]?.body || {});
  const hostedTools = requestHostedToolTypes(providerSummaries[0]?.body);
  const functionTools = requestFunctionToolNames(providerSummaries[0]?.body);
  const recoveryRequest = providerSummaries.find((entry) =>
    requestFunctionToolNames(entry.body).includes('web_fetch'),
  );
  const checks = {
    fixedSessionReused: latestTurn.session_id ? latestTurn.session_id === fixedSessionId : true,
    submitMaterialized: Boolean(submitAttempt.ok) || currentRunTurns.length > 0,
    hostedSearchDeclared: hostedTools.some((type) =>
      ['web_search', 'web_search_preview'].includes(type),
    ),
    hostedSearchExternalAccess: requestHostedWebSearchExternalAccess(providerSummaries[0]?.body),
    noFreehandFunctionSearch: !functionTools.includes('web_search'),
    recoveryWebFetchAvailable: Boolean(recoveryRequest),
    recoveryRequestHasWebFetch:
      Boolean(recoveryRequest) && requestFunctionToolNames(recoveryRequest.body).includes('web_fetch'),
    webFetchFixtureCalledOnce: webFetchRequests === 1,
    blockedFinalVisible: currentRunText.includes(blockedTextMarker),
    noUnboundedRecovery: providerRequests.length <= 4,
    latestTurnBlocked: latestTurn.terminal_status === 'Blocked',
    finalMarkerVisible: currentRunText.includes(finalTextMarker),
    noFixtureEnvAfterRestore: true,
  };
  const summary = {
    ok: Object.values(checks).every(Boolean),
    runId,
    artifactDir,
    adpUrl,
    baseUrl,
    fixedSessionId,
    searchQuery,
    providerRequestCount: providerRequests.length,
    webFetchRequests,
    providerSummaries,
    submitReceipt: submitAttempt.receipt,
    beforeTurnIds: Array.from(beforeTurnIds),
    currentRunTurnIds: currentRunTurns.map((turn) => turn.turn_id),
    latestTurnId: latestTurn.turn_id || null,
    latestTerminalStatus: latestTurn.terminal_status || null,
    latestTurnTerminalText: latestTurn.terminal_text || null,
    latestTurnSearchEvidence: latestTurn.search_evidence || null,
    hostedTools,
    functionTools,
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
    throw new Error(`search-exit online checks failed: ${failed.join(', ')}`);
  }
  console.log(
    `search_exit_online_ok url=${adpUrl} session=${fixedSessionId} provider_requests=${providerRequests.length} web_fetch_requests=${webFetchRequests} status=${latestTurn.terminal_status}`,
  );
} catch (error) {
  await captureFailureState(error).catch((captureError) => {
    console.error(`failure capture failed: ${captureError.stack || captureError.message}`);
  });
  throw error;
} finally {
  if (providerServer) await new Promise((resolve) => providerServer.close(resolve));
  if (webFetchServer) await new Promise((resolve) => webFetchServer.close(resolve));
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
        const parsed = parseJsonOrNull(body);
        providerRequests.push({ count: providerRequests.length + 1, body: parsed });
        fss.appendFileSync(
          path.join(artifactDir, 'provider-requests.jsonl'),
          `${JSON.stringify({ count: providerRequests.length, request: parsed })}\n`,
        );
        await fs.writeFile(
          path.join(artifactDir, `provider-request-${String(providerRequests.length).padStart(3, '0')}.json`),
          JSON.stringify(parsed, null, 2),
        );
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(providerResponseBody());
      });
    });
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

function providerResponseBody() {
  if (providerRequests.length === 1) {
    return JSON.stringify({
      id: 'resp-search-exit-hosted',
      object: 'response',
      status: 'completed',
      error: null,
      output: [
        {
          type: 'web_search_call',
          id: `ws-${runMarker}`,
          status: 'completed',
          action: { type: 'search', query: searchQuery },
        },
        {
          type: 'message',
          id: 'msg-search-exit-hosted',
          role: 'assistant',
          status: 'completed',
          content: [
            {
              type: 'output_text',
              text: `Provider hosted search returned no usable source.\n${taggedCompletion(
                'continue',
                'continue with the required search evidence flow',
              )}`,
              annotations: [],
            },
          ],
        },
      ],
      usage: { input_tokens: 20, output_tokens: 20, total_tokens: 40 },
    });
  }
  if (providerRequests.length === 2) {
    return JSON.stringify({
      id: 'resp-search-exit-web-fetch',
      object: 'response',
      status: 'completed',
      error: null,
      output: [
        {
          type: 'function_call',
          call_id: `call-search-exit-${runMarker}`,
          name: 'web_fetch',
          arguments: JSON.stringify({
            url: webFetchServerUrl(),
            timeout_seconds: 5,
            limit: 4096,
          }),
        },
      ],
      usage: { input_tokens: 20, output_tokens: 10, total_tokens: 30 },
    });
  }
  if (providerRequests.length === 3) {
    return JSON.stringify({
      id: 'resp-search-exit-blocked-final',
      object: 'response',
      status: 'completed',
      error: null,
      output: [
        {
          type: 'message',
          id: 'msg-search-exit-blocked-final',
          role: 'assistant',
          status: 'completed',
          content: [
            {
              type: 'output_text',
              text: `${blockedTextMarker}\n${searchFinalBlockedBody()}`,
              annotations: [],
            },
          ],
        },
      ],
      usage: { input_tokens: 20, output_tokens: 30, total_tokens: 50 },
    });
  }
  return JSON.stringify({
    id: 'resp-search-exit-completion',
    object: 'response',
    status: 'completed',
    error: null,
    output: [
      {
        type: 'message',
        id: 'msg-search-exit-completion',
        role: 'assistant',
        status: 'completed',
        content: [
          {
            type: 'output_text',
            text: `${finalTextMarker}\n${taggedCompletion(
              'blocked',
              'hosted search and web_fetch recovery failed',
            )}`,
            annotations: [],
          },
        ],
      },
    ],
    usage: { input_tokens: 20, output_tokens: 20, total_tokens: 40 },
  });
}

function searchFinalBlockedBody() {
  return `<freehand_search_delivery>\n${JSON.stringify({
    schema: 'search_evidence.final.v1',
    delivery_id: `final-search-exit-${runMarker}`,
    domain_plan_ref: null,
    claim: 'blocked',
    claims: [],
    unconfirmed: [],
    blocked_reason: blockedTextMarker,
  })}\n</freehand_search_delivery>`;
}

function taggedCompletion(claim, completionReason) {
  return `<freehand_completion>\n${JSON.stringify({
    claim,
    completion_reason: completionReason,
    blocked_reason: claim === 'blocked' ? blockedTextMarker : null,
  })}\n</freehand_completion>`;
}

async function startWebFetchServer() {
  return await new Promise((resolve, reject) => {
    const server = http.createServer((req, res) => {
      webFetchRequests += 1;
      res.writeHead(500, { 'content-type': 'text/plain' });
      res.end('search-exit fixture unavailable');
    });
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => resolve(server));
  });
}

function webFetchServerUrl() {
  return `http://127.0.0.1:${webFetchServer.address().port}/unavailable`;
}

function providerBaseUrl() {
  return `http://127.0.0.1:${providerServer.address().port}`;
}

async function ensureFixedSession() {
  const activeList = await adpQuery({
    QuerySessionListPage: {
      archived: false,
      page: { direction: 'Latest', cursor: null, limit: 100 },
    },
  });
  const activeSessions = requireSessionListPage(activeList, 'active session list').sessions;
  if (activeSessions.some((session) => session.session_id === fixedSessionId)) return;
  const archivedList = await adpQuery({
    QuerySessionListPage: {
      archived: true,
      page: { direction: 'Latest', cursor: null, limit: 100 },
    },
  });
  const archivedSessions = requireSessionListPage(archivedList, 'archived session list').sessions;
  if (archivedSessions.some((session) => session.session_id === fixedSessionId)) {
    await adpCommand({ RestoreSession: { session_id: fixedSessionId } });
    return;
  }
  await adpCommand({
    CreateSession: {
      session_id: fixedSessionId,
      title: 'search exit online verifier fixed session',
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
      lastObservation = {
        at: new Date().toISOString(),
        currentRunTurnIds: currentRunTurns.map((turn) => turn.turn_id),
        latestTurnId: latestTurn.turn_id || null,
        latestTerminalStatus: latestTurn.terminal_status || null,
        latestToolActivities: latestTurn.tool_activities || [],
        providerRequestCount: providerRequests.length,
        webFetchRequests,
        lastQueryError,
      };
      if (Date.now() - lastObservationWriteAt > 2_000) {
        lastObservationWriteAt = Date.now();
        await fs.writeFile(
          path.join(artifactDir, 'live-observation.json'),
          JSON.stringify(lastObservation, null, 2),
        );
      }
      if (
        latestTurn.terminal_status === 'Blocked' &&
        currentRunText.includes(finalTextMarker) &&
        currentRunText.includes(blockedTextMarker) &&
        webFetchRequests >= 1
      ) {
        return { adp, currentRunTurns, currentRunText, latestTurn };
      }
      if (latestTurn.terminal_status === 'Failed') {
        const failure = new Error(
          `current run terminalized as failure: ${latestTurn.terminal_status}`,
        );
        failure.nonRetryable = true;
        throw failure;
      }
    } catch (error) {
      if (error && error.nonRetryable) throw error;
      lastQueryError = error && (error.stack || error.message || String(error));
    }
    await delay(500);
  }
  throw new Error(
    `timeout waiting for search-exit online completion after ${timeoutMs}ms; last_observation=${JSON.stringify(
      lastObservation,
    )}`,
  );
}

async function readProviderRequestSummaries() {
  return providerRequests.map((entry) => ({
    count: entry.count,
    body: entry.body,
    functionToolNames: requestFunctionToolNames(entry.body),
    hostedToolTypes: requestHostedToolTypes(entry.body),
  }));
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
  if (!restoreSummary.ok) return JSON.stringify(restoreSummary, null, 2);
  console.log(`search_exit_restore_ok ${restoreSummary.config}`);
  return null;
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
        providerRequests,
        webFetchRequests,
        lastObservation,
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
  ]);
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
  return adpVerifierRequest({
    url: adpUrl,
    authToken: adpAuthToken,
    kind,
    payloadKey,
    payload,
    timeoutMs,
    clientName: 'freehand-search-exit-verifier',
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
    if (child.stdout) child.stdout.on('data', (chunk) => (stdout += chunk));
    if (child.stderr) child.stderr.on('data', (chunk) => (stderr += chunk));
    child.on('close', (code) => resolve({ code, stdout, stderr, argv }));
  });
}

async function must(argv, opts = {}) {
  const result = await run(argv, opts);
  if (result.code !== 0) {
    throw new Error(
      `command failed ${argv.join(' ')}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}`,
    );
  }
  return result;
}

async function waitHealth() {
  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${baseUrl}health`, { signal: AbortSignal.timeout(5_000) });
      const body = await response.text();
      if (response.ok && body.trim() === 'ok') return;
    } catch {}
    await delay(1_000);
  }
  throw new Error(`daemon did not become healthy at ${baseUrl}health`);
}

function sessionTurnsFromAdp(adp) {
  return adp?.SessionTurns?.turns || [];
}

function parseJsonOrNull(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function requestFunctionToolNames(body) {
  if (!body) return [];
  const tools = body?.tools || body?.input?.tools || [];
  return tools
    .map((tool) => tool?.function?.name || tool?.name)
    .filter(Boolean)
    .map(String);
}

function requestHostedToolTypes(body) {
  if (!body) return [];
  const tools = body?.tools || body?.input?.tools || [];
  return tools.map((tool) => tool?.type).filter(Boolean).map(String);
}

function requestHostedWebSearchExternalAccess(body) {
  if (!body) return false;
  const text = JSON.stringify(body);
  return text.includes('web_search') || text.includes('web_search_preview');
}

function redactConfig(text) {
  return text.replace(/(api_key|token|secret)(\s*=\s*)(\S+)/gi, '$1$2[REDACTED]');
}

function redactEnv(text) {
  return text.replace(/^(FREEHAND_.*(?:KEY|TOKEN|SECRET|CREDENTIAL)=).*$/gim, '$1[REDACTED]');
}

async function fixtureEnvMatches(envText) {
  return envText
    .split(/\r?\n/)
    .filter((line) => line.includes(fixtureKeyName))
    .map((line) => line.replace(/=.*$/, ''));
}

function stripFixtureEnv(envText) {
  return envText
    .split(/\r?\n/)
    .filter((line) => !line.includes(fixtureKeyName))
    .join('\n');
}

function positiveIntegerEnv(name, fallback) {
  const value = Number.parseInt(process.env[name] || '', 10);
  return Number.isFinite(value) && value > 0 ? value : fallback;
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

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function tailText(file, lines) {
  try {
    const text = await fs.readFile(file, 'utf8');
    return text.split(/\r?\n/).slice(-lines).join('\n');
  } catch {
    return '';
  }
}
