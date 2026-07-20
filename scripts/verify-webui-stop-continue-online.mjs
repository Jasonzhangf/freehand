import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import fss from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const configPath =
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_CONFIG || path.join(runtimeHome, 'config.toml');
const envPath =
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_ENV || path.join(runtimeHome, 'daemonS.env');
const cli =
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_CLI || path.join(home, '.local/bin/freehand-cliS');
const baseUrl = normalizedBaseUrl(
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_BASE_URL || 'http://127.0.0.1:4042/',
);
const adpUrl =
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const fixedSessionId =
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_SESSION || 'webui-stop-continue-fixed';
const fixturePort = Number.parseInt(
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_FIXTURE_PORT || '18141',
  10,
);
const debugPort = Number.parseInt(
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_DEBUG_PORT || '9242',
  10,
);
const retryBackoffMs = Number.parseInt(
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_RETRY_BACKOFF_MS || '5000',
  10,
);
const cancelLatencyLimitMs = Number.parseInt(
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_CANCEL_LATENCY_LIMIT_MS || '3000',
  10,
);
const chromePath =
  process.env.FREEHAND_WEBUI_STOP_CONTINUE_CHROME ||
  '/Applications/Google Chrome.app/Contents/MOS/Google Chrome'.replace('/MOS/', '/MacOS/');
const runId = `stop-continue-${Date.now()}`;
const runMarker = runId;
const cancelledPromptMarker = `cancel request ${runMarker}`;
const continuedPromptMarker = `continue request ${runMarker}`;
const finalTextMarker = `stop continue completed ${runMarker}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);
const fixtureKeyName = 'FREEHAND_WEBUI_STOP_CONTINUE_FIXTURE_KEY';

let chrome;
let cdp;
let fixtureServer;
let requestCount = 0;
let preContinueRetryRequestCount = 0;
let cancelIssued = false;
let continueSubmitted = false;
let chromeProfileDir = null;
const restoreErrors = [];
const browserErrors = [];

await fs.mkdir(artifactDir, { recursive: true });
const originalConfig = await fs.readFile(configPath, 'utf8');
const originalEnv = await fs.readFile(envPath, 'utf8').catch(() => '');

try {
  fixtureServer = await startFixtureServer();
  await fs.writeFile(path.join(artifactDir, 'config.before.toml'), redactConfig(originalConfig));
  await fs.writeFile(path.join(artifactDir, 'daemonS.before.env'), redactEnv(originalEnv));
  await writeFixtureEnv();

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
    'cc',
    '--type',
    'openai',
    '--protocol',
    'responses',
    '--base-url',
    `http://127.0.0.1:${fixturePort}/openai/v1`,
    '--model',
    'gpt-5.5',
    '--api-key-env',
    fixtureKeyName,
  ]);
  await must(['scripts/install-launchd.sh', 'restartS']);
  await waitHealth();
  await ensureFixedSession();

  const sessionListBefore = await adpQuery('QuerySessionList');
  const sessionIdsBefore = sessionIdsFromList(sessionListBefore);
  await fs.writeFile(
    path.join(artifactDir, 'adp-session-list-before.json'),
    JSON.stringify(sessionListBefore, null, 2),
  );

  chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-stop-continue-chrome-'));
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
      'about:blank',
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );
  chrome.stdout.on('data', () => {});
  chrome.stderr.on('data', () => {});

  const target = await waitPageTarget();
  cdp = await createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Log.enable');
  cdp.onEvent((method, params) => {
    if (method === 'Runtime.exceptionThrown') {
      browserErrors.push({
        method,
        text: params.exceptionDetails?.text || 'runtime exception',
      });
    }
    if (
      method === 'Log.entryAdded' &&
      params.entry?.level === 'error' &&
      !isIgnoredBrowserLogError(params.entry)
    ) {
      browserErrors.push({
        method,
        text: params.entry.text || 'browser log error',
      });
    }
  });
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: `window.localStorage.setItem("freehand-webui-selected-session", ${JSON.stringify(fixedSessionId)});`,
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForFunction(
    cdp,
    (sessionId) => {
      const shell = document.querySelector('[data-webui-shell="true"]');
      return (
        !!shell &&
        !!document.getElementById('composer-input') &&
        (shell.dataset.selectedSession || '') === sessionId &&
        !((document.getElementById('message-list')?.innerText || '').includes(
          'Loading conversation',
        ))
      );
    },
    30_000,
    'fixed selected session loaded',
    fixedSessionId,
  );
  await capture(cdp, '01-fixed-session-ready');

  const beforeAdp = await querySessionTurns();
  const beforeTurns = sessionTurnsFromAdp(beforeAdp);
  const beforeTurnIds = new Set(beforeTurns.map((turn) => turn.turn_id).filter(Boolean));
  const scope = {
    beforeTurnIds: Array.from(beforeTurnIds),
    runMarker,
    finalTextMarker,
  };
  await fs.writeFile(
    path.join(artifactDir, 'adp-session-before.json'),
    JSON.stringify(beforeAdp, null, 2),
  );

  const cancelPrompt = [
    `${cancelledPromptMarker}.`,
    'Begin this request normally and return the required Freehand completion schema when the provider responds.',
  ].join(' ');
  await submitPrompt(cdp, cancelPrompt);
  await waitForFunction(
    cdp,
    (value) =>
      (document.getElementById('composer-input')?.value || '') === '' ||
      (document.getElementById('command-status')?.textContent || '').includes('dispatching') ||
      (document.getElementById('message-list')?.innerText || '').includes(value),
    5_000,
    'cancel-run local submit projection',
    cancelPrompt,
  );
  await capture(cdp, '02-cancel-run-submitted', scope);

  const providerActivityState = await waitFor(
    async () => {
      const state = await readDomState(cdp, scope);
      if (
        requestCount === 1 &&
        state.currentRunRealTurnIds.length > 0 &&
        state.currentRunProviderActivityVisible &&
        !state.currentRunTerminalCancelledPresent
      ) {
        return state;
      }
      return null;
    },
    120_000,
    'visible provider activity before cancellation',
  );
  await capture(cdp, '03-provider-activity-visible', scope);

  const cancelStartedAt = Date.now();
  cancelIssued = true;
  await clickCancel(cdp);
  const cancelledProof = await waitFor(
    async () => {
      const [dom, adp] = await Promise.all([readDomState(cdp, scope), querySessionTurns()]);
      const turns = sessionTurnsFromAdp(adp);
      const currentRunTurns = turns.filter((turn) => !beforeTurnIds.has(turn.turn_id));
      const cancelledTurn = [...currentRunTurns]
        .reverse()
        .find((turn) => `${turn.terminal_status || ''}`.toLowerCase() === 'cancelled');
      if (
        cancelledTurn &&
        dom.selectedSession === fixedSessionId &&
        dom.selectedTurn === cancelledTurn.turn_id &&
        dom.selectedTerminalStatus.toLowerCase() === 'cancelled' &&
        dom.turnStatus.toLowerCase() === 'cancelled' &&
        dom.liveCount === 0 &&
        !dom.liveStatusPresent
      ) {
        return {
          dom,
          adp,
          cancelledTurnId: cancelledTurn.turn_id,
          cancelLatencyMs: Date.now() - cancelStartedAt,
        };
      }
      return null;
    },
    20_000,
    'cancelled owner truth and cleared browser live state',
  );
  await fs.writeFile(
    path.join(artifactDir, 'adp-session-cancelled.json'),
    JSON.stringify(cancelledProof.adp, null, 2),
  );
  await capture(cdp, '04-cancelled-terminal-visible', scope);

  const requestCountAtCancel = requestCount;
  await delay(retryBackoffMs + 750);
  const afterBackoffState = await readDomState(cdp, scope);
  await capture(cdp, '05-cancelled-after-full-backoff-window', scope);

  const continuePrompt = [
    `${continuedPromptMarker}.`,
    'Start a new turn in this same session and return the required Freehand completion schema.',
  ].join(' ');
  continueSubmitted = true;
  await submitPrompt(cdp, continuePrompt);
  await waitForFunction(
    cdp,
    (value) =>
      (document.getElementById('composer-input')?.value || '') === '' ||
      (document.getElementById('command-status')?.textContent || '').includes('dispatching') ||
      (document.getElementById('message-list')?.innerText || '').includes(value),
    5_000,
    'continue-run local submit projection',
    continuePrompt,
  );
  await capture(cdp, '06-continue-run-submitted', scope);

  const finalProof = await waitFor(
    async () => {
      const [dom, adp] = await Promise.all([readDomState(cdp, scope), querySessionTurns()]);
      const turns = sessionTurnsFromAdp(adp);
      const currentRunTurns = turns.filter((turn) => !beforeTurnIds.has(turn.turn_id));
      const successTurn = [...currentRunTurns]
        .reverse()
        .find((turn) => `${turn.terminal_status || ''}`.toLowerCase() === 'success');
      if (
        successTurn &&
        dom.selectedSession === fixedSessionId &&
        dom.selectedTurn === successTurn.turn_id &&
        dom.selectedTerminalStatus.toLowerCase() === 'success' &&
        dom.turnStatus.toLowerCase() === 'completed' &&
        dom.finalTextVisible &&
        dom.liveCount === 0 &&
        !dom.liveStatusPresent
      ) {
        return {
          dom,
          adp,
          successTurnId: successTurn.turn_id,
        };
      }
      return null;
    },
    60_000,
    'same-session successful turn after cancellation',
  );
  await fs.writeFile(
    path.join(artifactDir, 'adp-session-final.json'),
    JSON.stringify(finalProof.adp, null, 2),
  );
  await capture(cdp, '07-same-session-continued-success', scope);

  const sessionListAfter = await adpQuery('QuerySessionList');
  const sessionIdsAfter = sessionIdsFromList(sessionListAfter);
  await fs.writeFile(
    path.join(artifactDir, 'adp-session-list-after.json'),
    JSON.stringify(sessionListAfter, null, 2),
  );

  const finalTurns = sessionTurnsFromAdp(finalProof.adp);
  const cancelledTurnIndex = finalTurns.findIndex(
    (turn) => turn.turn_id === cancelledProof.cancelledTurnId,
  );
  const successTurnIndex = finalTurns.findIndex((turn) => turn.turn_id === finalProof.successTurnId);
  const newSessionIds = sessionIdsAfter.filter((sessionId) => !sessionIdsBefore.includes(sessionId));
  const configDuringFixture = await run([cli, 'adp-config-query', '--url', adpUrl]);
  const summary = {
    ok: true,
    runId,
    artifactDir,
    baseUrl,
    adpUrl,
    fixedSessionId,
    runMarker,
    cancelledPromptMarker,
    continuedPromptMarker,
    finalTextMarker,
    beforeTurnIds: Array.from(beforeTurnIds),
    cancelledTurnId: cancelledProof.cancelledTurnId,
    successTurnId: finalProof.successTurnId,
    cancelLatencyMs: cancelledProof.cancelLatencyMs,
    cancelLatencyLimitMs,
    retryBackoffMs,
    requestCount,
    requestCountAtCancel,
    preContinueRetryRequestCount,
    providerActivityState,
    cancelledState: cancelledProof.dom,
    afterBackoffState,
    finalState: finalProof.dom,
    sessionIdsBefore,
    sessionIdsAfter,
    newSessionIds,
    configDuringFixture: configDuringFixture.stdout.trim(),
    browserErrors,
    checks: {
      fixedSessionSelectedBeforeCancel: providerActivityState.selectedSession === fixedSessionId,
      providerActivityVisibleBeforeCancel: providerActivityState.currentRunProviderActivityVisible,
      cancelMaterializedInAdp: !!cancelledProof.cancelledTurnId,
      cancelVisibleInDom:
        cancelledProof.dom.selectedTerminalStatus.toLowerCase() === 'cancelled' &&
        cancelledProof.dom.turnStatus.toLowerCase() === 'cancelled' &&
        cancelledProof.dom.currentRunTerminalCancelledPresent,
      cancelPromptlyInterruptedBackoff:
        cancelledProof.cancelLatencyMs < cancelLatencyLimitMs &&
        cancelledProof.cancelLatencyMs < retryBackoffMs,
      cancelClearedLiveState:
        cancelledProof.dom.liveCount === 0 &&
        !cancelledProof.dom.liveStatusPresent &&
        afterBackoffState.liveCount === 0 &&
        !afterBackoffState.liveStatusPresent,
      cancelPreventedProviderRetry:
        requestCountAtCancel === 1 &&
        preContinueRetryRequestCount === 0 &&
        requestCount === 2,
      sameSessionSelectedAfterCancel: cancelledProof.dom.selectedSession === fixedSessionId,
      sameSessionSelectedAfterContinue: finalProof.dom.selectedSession === fixedSessionId,
      continuedTurnAppendedAfterCancelledTurn:
        cancelledTurnIndex >= 0 &&
        successTurnIndex > cancelledTurnIndex &&
        finalProof.successTurnId !== cancelledProof.cancelledTurnId,
      cancelledCardPreservedAfterContinue:
        finalProof.dom.cancelledTurnVisible &&
        finalProof.dom.visibleTurnOrder.indexOf(cancelledProof.cancelledTurnId) <
          finalProof.dom.visibleTurnOrder.indexOf(finalProof.successTurnId),
      continuedTurnSucceeded:
        finalProof.dom.finalTextVisible &&
        finalProof.dom.selectedTerminalStatus.toLowerCase() === 'success' &&
        finalProof.dom.turnStatus.toLowerCase() === 'completed' &&
        finalProof.dom.liveCount === 0 &&
        !finalProof.dom.liveStatusPresent,
      noRandomSessionCreated: newSessionIds.length === 0,
      noBrowserErrors: browserErrors.length === 0,
    },
  };
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));

  const failed = Object.entries(summary.checks).filter(([, value]) => !value);
  if (failed.length > 0) {
    throw new Error(`stop/continue checks failed: ${failed.map(([key]) => key).join(', ')}`);
  }
} catch (error) {
  await captureFailureState(error).catch((captureError) => {
    restoreErrors.push(`failure capture failed: ${captureError.message}`);
  });
  throw error;
} finally {
  if (cdp) {
    await cdp.close().catch(() => null);
  }
  if (chrome?.pid && chrome.exitCode === null) {
    chrome.kill('SIGTERM');
    await new Promise((resolve) => chrome.once('exit', resolve));
  }
  if (fixtureServer) {
    await new Promise((resolve) => fixtureServer.close(resolve));
  }
  await restoreSProfile();
  if (restoreErrors.length > 0) {
    console.error(JSON.stringify({ restoreErrors }, null, 2));
    process.exitCode = 1;
  }
}

async function writeFixtureEnv() {
  const base = stripFixtureEnv(originalEnv);
  const prefix = base ? `${base}\n` : '';
  await fs.writeFile(
    envPath,
    [
      prefix,
      `${fixtureKeyName}="fixture-key"\n`,
      `FREEHAND_PROVIDER_RETRY_BACKOFF_MS="${retryBackoffMs}"\n`,
    ].join(''),
  );
}

async function restoreSProfile() {
  await fs.writeFile(configPath, originalConfig).catch((error) => restoreErrors.push(error.message));
  await fs.writeFile(envPath, originalEnv).catch((error) => restoreErrors.push(error.message));

  const restart = await run(['scripts/install-launchd.sh', 'restartS']);
  if (restart.code !== 0) {
    restoreErrors.push(`restartS restore failed: ${restart.stderr || restart.stdout}`);
  } else {
    await waitHealth().catch((error) => restoreErrors.push(error.message));
  }

  const configAfter = await run([cli, 'adp-config-query', '--url', adpUrl]).catch((error) => ({
    code: -1,
    stdout: '',
    stderr: error.message,
  }));
  const envMatches = await fixtureEnvMatches(envPath).catch((error) => [
    { line: 0, text: `env check failed: ${error.message}` },
  ]);
  const restoration = {
    restartCode: restart.code,
    healthOk: restoreErrors.length === 0,
    configQueryCode: configAfter.code,
    configAfter: `${configAfter.stdout || ''}${configAfter.stderr || ''}`.trim(),
    fixtureEnvMatches: envMatches,
    chromeProfileDir,
  };
  await fs
    .writeFile(path.join(artifactDir, 'restoration.json'), JSON.stringify(restoration, null, 2))
    .catch((error) => restoreErrors.push(error.message));
  if (configAfter.code !== 0) {
    restoreErrors.push(`config query after restore failed: ${restoration.configAfter}`);
  }
  if (envMatches.length > 0) {
    restoreErrors.push('fixture env remains after restore');
  }
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
        runMarker,
        error: error && (error.stack || error.message || String(error)),
        requestCount,
        preContinueRetryRequestCount,
        cancelIssued,
        continueSubmitted,
        browserErrors,
      },
      null,
      2,
    ),
  );

  if (cdp) {
    const beforeTurnIds = sessionTurnsFromAdp(
      JSON.parse(
        await fs
          .readFile(path.join(artifactDir, 'adp-session-before.json'), 'utf8')
          .catch(() => '{"SessionTurns":{"turns":[]}}'),
      ),
    )
      .map((turn) => turn.turn_id)
      .filter(Boolean);
    await capture(cdp, 'failure-dom', {
      beforeTurnIds,
      runMarker,
      finalTextMarker,
    }).catch((captureError) =>
      fs.writeFile(
        path.join(failureDir, 'dom-capture-error.txt'),
        captureError.stack || captureError.message,
      ),
    );
  }

  const [sessionTurns, sessionList, configAfter] = await Promise.all([
    querySessionTurns().catch((queryError) => ({ error: queryError.message })),
    adpQuery('QuerySessionList').catch((queryError) => ({ error: queryError.message })),
    run([cli, 'adp-config-query', '--url', adpUrl]).catch((runError) => ({
      code: -1,
      stdout: '',
      stderr: runError.message,
    })),
  ]);
  await fs.writeFile(
    path.join(failureDir, 'adp-session-turns.json'),
    JSON.stringify(sessionTurns, null, 2),
  );
  await fs.writeFile(
    path.join(failureDir, 'adp-session-list.json'),
    JSON.stringify(sessionList, null, 2),
  );
  await fs.writeFile(
    path.join(failureDir, 'config-after.txt'),
    `${configAfter.stdout || ''}${configAfter.stderr || ''}`,
  );
  await fs.writeFile(
    path.join(failureDir, 'daemonS.stderr.tail.txt'),
    await tailText(path.join(runtimeHome, 'logs', 'daemonS.stderr.log'), 160),
  );
  await fs.writeFile(
    path.join(failureDir, 'daemonS.stdout.tail.txt'),
    await tailText(path.join(runtimeHome, 'logs', 'daemonS.stdout.log'), 160),
  );
  await copyIfExists(
    path.join(artifactDir, 'fixture-requests.jsonl'),
    path.join(failureDir, 'fixture-requests.jsonl'),
  );
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
        const phase =
          requestCount === 1
            ? 'initial-retry'
            : continueSubmitted
              ? 'continued-success'
              : 'unexpected-pre-continue-retry';
        if (requestCount > 1 && !continueSubmitted) {
          preContinueRetryRequestCount += 1;
        }
        fss.appendFileSync(
          path.join(artifactDir, 'fixture-requests.jsonl'),
          `${JSON.stringify({
            count: requestCount,
            at: new Date().toISOString(),
            method: req.method,
            url: req.url,
            phase,
            cancelIssued,
            continueSubmitted,
            bodyLength: body.length,
          })}\n`,
        );

        if (requestCount === 1 || !continueSubmitted) {
          res.writeHead(500, { 'content-type': 'application/json' });
          res.end(
            JSON.stringify({
              error: {
                type: 'server_error',
                message: 'stop continue fixture retry',
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
    server.listen(fixturePort, '127.0.0.1', () => resolve(server));
  });
}

function finalCompletionBody() {
  const schema = {
    claim: 'complete',
    completion_reason: 'same persisted session continued after explicit cancellation',
    evidence: `fixture accepted a new provider request after cancelled turn for ${runMarker}`,
    summary: `${finalTextMarker} in the same persisted session`,
    learned: 'cancelled turns remain visible and do not make the selected session unusable',
  };
  return JSON.stringify({
    id: 'resp-stop-continue-final',
    object: 'response',
    status: 'completed',
    error: null,
    output: [
      {
        type: 'message',
        id: 'msg-stop-continue-final',
        role: 'assistant',
        status: 'completed',
        content: [
          {
            type: 'output_text',
            text: `${finalTextMarker}\n<freehand_completion>\n${JSON.stringify(schema)}\n</freehand_completion>`,
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

function isIgnoredBrowserLogError(entry) {
  const text = `${entry?.text || ''}`;
  return /Failed to load resource: the server responded with a status of 404 \(Not Found\)/i.test(text);
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
      title: 'WebUI stop continue fixed verifier',
      cwd: repo,
    },
  });
}

async function submitPrompt(cdpClient, prompt) {
  await evalInPage(
    cdpClient,
    (value) => {
      const input = document.getElementById('composer-input');
      input.focus();
      input.value = value;
      input.dispatchEvent(
        new InputEvent('input', { bubbles: true, data: value, inputType: 'insertText' }),
      );
      input.dispatchEvent(new Event('change', { bubbles: true }));
      const button = document.getElementById('send-button');
      if (button) {
        button.click();
        return;
      }
      const form = document.getElementById('composer-form');
      if (form && typeof form.requestSubmit === 'function') {
        form.requestSubmit();
      } else {
        document.getElementById('send-button')?.click();
      }
    },
    prompt,
  );
}

async function clickCancel(cdpClient) {
  await evalInPage(cdpClient, () => {
    const button = document.getElementById('cancel-button');
    if (!button) {
      throw new Error('cancel button missing');
    }
    button.click();
  });
}

async function capture(cdpClient, name, scope = {}) {
  const state = await readDomState(cdpClient, scope);
  const screenshot = await cdpClient.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  await fs.writeFile(path.join(artifactDir, `${name}.png`), Buffer.from(screenshot.data, 'base64'));
  await fs.writeFile(path.join(artifactDir, `${name}.json`), JSON.stringify(state, null, 2));
  return state;
}

async function readDomState(cdpClient, scope = {}) {
  return await evalInPage(
    cdpClient,
    (scopeArg) => {
      const shell = document.querySelector('[data-webui-shell="true"]');
      const beforeTurnIds = new Set(scopeArg?.beforeTurnIds || []);
      const runMarkerValue = scopeArg?.runMarker || '';
      const finalMarkerValue = scopeArg?.finalTextMarker || '';
      const chatMessages = Array.from(document.querySelectorAll('.chat-message')).map((node) => ({
        turnId: node.dataset.turnId || '',
        text: node.innerText || '',
        live: node.dataset.live === 'true',
        className: node.className || '',
        assistant: node.classList.contains('chat-message-assistant'),
      }));
      const modelRows = Array.from(document.querySelectorAll('.chat-section-system')).map((node) => ({
        turnId: node.dataset.turnId || '',
        messageTurnId: node.closest('.chat-message')?.dataset.turnId || '',
        text: node.innerText || '',
      }));
      const currentRunTurnIds = new Set();
      chatMessages.forEach((message) => {
        if (message.turnId && !beforeTurnIds.has(message.turnId)) {
          currentRunTurnIds.add(message.turnId);
        }
        if (runMarkerValue && message.text.includes(runMarkerValue) && message.turnId) {
          currentRunTurnIds.add(message.turnId);
        }
      });
      const isCurrentRunTurn = (turnId) => !!turnId && currentRunTurnIds.has(turnId);
      const currentRunMessages = chatMessages.filter(
        (message) =>
          isCurrentRunTurn(message.turnId) ||
          (runMarkerValue && message.text.includes(runMarkerValue)),
      );
      const currentRunModelRows = modelRows.filter(
        (row) => isCurrentRunTurn(row.turnId) || isCurrentRunTurn(row.messageTurnId),
      );
      const selectedTurn = shell?.dataset.selectedTurn || '';
      const statusText = [
        document.getElementById('command-status')?.textContent || '',
        document.getElementById('turn-status')?.textContent || '',
      ].join('\n');
      const liveStatusPattern =
        /provider retry|dispatching|thinking|tool executing|tool running|waiting model|waiting for model|cancelling/i;
      const visibleTurnOrder = [];
      chatMessages.forEach((message) => {
        if (message.turnId && !visibleTurnOrder.includes(message.turnId)) {
          visibleTurnOrder.push(message.turnId);
        }
      });
      const selectedTerminalStatus = shell?.dataset.selectedTerminalStatus || '';
      return {
        selectedSession: shell?.dataset.selectedSession || '',
        selectedTurn,
        selectedTerminalStatus,
        commandStatus: document.getElementById('command-status')?.textContent || '',
        turnStatus: document.getElementById('turn-status')?.textContent || '',
        currentRunTurnIds: Array.from(currentRunTurnIds),
        currentRunRealTurnIds: Array.from(currentRunTurnIds).filter(
          (turnId) => turnId !== 'pending-submit' && turnId !== 'accepted-submit',
        ),
        currentRunSelectedTurn: isCurrentRunTurn(selectedTurn),
        currentRunMessageText: currentRunMessages.map((message) => message.text).join('\n---\n'),
        modelRows,
        currentRunModelRows,
        currentRunProviderRetryVisible: currentRunModelRows.some((row) =>
          /provider retry/i.test(row.text),
        ),
        currentRunProviderActivityVisible:
          currentRunModelRows.some((row) =>
            /provider request built|provider retry|thinking|waiting model/i.test(row.text),
          ) ||
          currentRunMessages.some((message) => /provider retry|thinking/i.test(message.text)),
        currentRunTerminalCancelledPresent:
          selectedTerminalStatus.toLowerCase() === 'cancelled' ||
          currentRunMessages.some((message) => /cancelled|request cancelled/i.test(message.text)),
        finalTextVisible: currentRunMessages.some(
          (message) =>
            message.assistant &&
            !!finalMarkerValue &&
            message.text.includes(finalMarkerValue),
        ),
        liveCount: document.querySelectorAll('[data-live="true"]').length,
        liveStatusPresent: liveStatusPattern.test(statusText),
        visibleTurnOrder,
        cancelledTurnVisible: currentRunMessages.some(
          (message) =>
            message.turnId &&
            `${message.turnId}` !== selectedTurn &&
            /cancelled|request cancelled/i.test(message.text),
        ),
      };
    },
    scope,
  );
}

async function adpQuery(query) {
  return await adpRequest('query', 'query', query);
}

async function querySessionTurns() {
  return await adpQuery({ QuerySessionTurns: { session_id: fixedSessionId } });
}

function sessionTurnsFromAdp(adp) {
  return adp?.SessionTurns && Array.isArray(adp.SessionTurns.turns) ? adp.SessionTurns.turns : [];
}

function sessionIdsFromList(adp) {
  return (adp?.SessionList?.sessions || [])
    .map((session) => session.session_id)
    .filter(Boolean)
    .sort();
}

async function adpCommand(command) {
  return await adpRequest('command', 'command', command);
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
        reject(
          new Error(
            (message.failure && (message.failure.message || message.failure.code)) ||
              'ADP failure',
          ),
        );
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
    return response.ok;
  }, 60_000, 'daemon health');
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
    child.stdout?.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr?.on('data', (chunk) => {
      stderr += chunk;
    });
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

async function copyIfExists(source, target) {
  try {
    await fs.copyFile(source, target);
  } catch (error) {
    if (error?.code === 'ENOENT') {
      return;
    }
    throw error;
  }
}

async function tailText(file, maxLines) {
  const text = await fs.readFile(file, 'utf8').catch((error) => {
    if (error?.code === 'ENOENT') {
      return '';
    }
    throw error;
  });
  const lines = text.split(/\r?\n/);
  return lines.slice(Math.max(0, lines.length - maxLines)).join('\n');
}

async function fixtureEnvMatches(file) {
  const text = await fs.readFile(file, 'utf8').catch((error) => {
    if (error?.code === 'ENOENT') {
      return '';
    }
    throw error;
  });
  return text
    .split(/\r?\n/)
    .map((line, index) => ({ line: index + 1, text: line }))
    .filter(({ text: line }) =>
      /FREEHAND_WEBUI_STOP_CONTINUE_FIXTURE_KEY|FREEHAND_PROVIDER_RETRY_BACKOFF_MS/.test(
        line,
      ),
    );
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

async function evalInPage(cdpClient, fn, ...args) {
  const response = await cdpClient.send('Runtime.evaluate', {
    expression: `(${fn})(...${JSON.stringify(args)})`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    throw new Error(response.exceptionDetails.text || 'Runtime.evaluate failed');
  }
  return response.result.value;
}

function stripFixtureEnv(value) {
  return value
    .replace(/\n?FREEHAND_WEBUI_STOP_CONTINUE_FIXTURE_KEY=.*$/gm, '')
    .replace(/\n?FREEHAND_PROVIDER_RETRY_BACKOFF_MS=.*$/gm, '')
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

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
