import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import fss from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { adpVerifierRequest, requireSessionListPage } from './lib/adp-verifier-client.mjs';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const configPath = process.env.FREEHAND_LIVE_TOOL_RENDER_CONFIG || path.join(runtimeHome, 'config.toml');
const envPath = process.env.FREEHAND_LIVE_TOOL_RENDER_ENV || path.join(runtimeHome, 'daemonS.env');
const cli = process.env.FREEHAND_LIVE_TOOL_RENDER_CLI || path.join(home, '.local/bin/freehand-cliS');
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_LIVE_TOOL_RENDER_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_LIVE_TOOL_RENDER_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const adpAuthToken = process.env.FREEHAND_ADP_AUTH_TOKEN || '';
const fixedSessionId = process.env.FREEHAND_LIVE_TOOL_RENDER_SESSION || 'webui-live-tool-render-fixed';
const fixturePort = Number.parseInt(process.env.FREEHAND_LIVE_TOOL_RENDER_FIXTURE_PORT || '18139', 10);
const debugPort = Number.parseInt(process.env.FREEHAND_LIVE_TOOL_RENDER_DEBUG_PORT || '9241', 10);
const finalDelayMs = Number.parseInt(process.env.FREEHAND_LIVE_TOOL_RENDER_FINAL_DELAY_MS || '8000', 10);
const chromePath =
  process.env.FREEHAND_LIVE_TOOL_RENDER_CHROME ||
  '/Applications/Google Chrome.app/Contents/MOS/Google Chrome'.replace('/MOS/', '/MacOS/');
const runId = `live-tool-render-${Date.now()}`;
const runMarker = runId;
const runTaskId = `definitely-missing-live-tool-render-${runMarker}`;
const finalTextMarker = `live tool render completed ${runMarker}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);
const fixtureKeyName = 'FREEHAND_LIVE_TOOL_RENDER_FIXTURE_KEY';
const fixtureProviderId = 'live-tool-render-fixture';

let chrome;
let cdp;
let fixtureServer;
let requestCount = 0;
let toolOutputRequestCount = 0;
let toolSchemaIncluded = false;
const restoreErrors = [];

await fs.mkdir(artifactDir, { recursive: true });
const originalConfig = await fs.readFile(configPath, 'utf8');
const originalEnv = await fs.readFile(envPath, 'utf8').catch(() => '');

try {
  fixtureServer = await startFixtureServer();
  await fs.writeFile(path.join(artifactDir, 'config.before.toml'), redactConfig(originalConfig));
  await fs.writeFile(path.join(artifactDir, 'daemonS.before.env'), redactEnv(originalEnv));
  await fs.writeFile(
    envPath,
    `${stripFixtureEnv(originalEnv)}\n${fixtureKeyName}="fixture-key"\nFREEHAND_TEST_DISABLE_MASTER_LIFECYCLE_RUNNER="1"\n`,
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
    `http://127.0.0.1:${fixturePort}/openai/v1`,
    '--model',
    'gpt-5.5',
    '--api-key-env',
    fixtureKeyName,
  ]);
  await must(['scripts/install-launchd.sh', 'restartS']);
  await waitHealth();
  await ensureFixedSession();

  const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-live-tool-render-chrome-'));
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
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: `window.localStorage.setItem("freehand-webui-selected-session", ${JSON.stringify(fixedSessionId)});`,
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(
    cdp,
    (sessionId) => {
      const shell = document.querySelector('[data-webui-shell="true"]');
      return !!shell &&
        !!document.getElementById('composer-input') &&
        (shell.dataset.selectedSession || '') === sessionId &&
        !((document.getElementById('message-list')?.innerText || '').includes('Loading conversation'));
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
    runTaskId,
    finalTextMarker,
  };
  await fs.writeFile(path.join(artifactDir, 'adp-session-before.json'), JSON.stringify(beforeAdp, null, 2));

  const prompt = [
    `Live tool render verifier RUN_MARKER=${runMarker}.`,
    `Call the task tool exactly once with {"op":"query","task_id":"${runTaskId}"}, then continue from the tool result and finish with the required Freehand completion schema. The final answer must include the run marker ${runMarker}.`,
  ].join(' ');
  await submitPrompt(cdp, prompt);
  await waitForFunction(
    cdp,
    (value) =>
      (document.getElementById('composer-input')?.value || '') === '' ||
        (document.getElementById('command-status')?.textContent || '').includes('dispatching') ||
        (document.getElementById('message-list')?.innerText || '').includes(value),
    5_000,
    'local submit projection',
    prompt,
  );
  await capture(cdp, '02-after-submit', scope);

  const serviceTurn = await waitFor(
    async () => {
      const state = await readDomState(cdp, scope);
      if (state.currentRunRealTurnIds.length > 0 || state.currentRunToolCardCount > 0) {
        return state;
      }
      return null;
    },
    120_000,
    'service current-run turn before tool card',
  );
  await capture(cdp, '03-service-current-run-turn-visible', scope);

  const duringTool = await waitFor(
    async () => {
      const state = await readDomState(cdp, scope);
      if (state.currentRunToolCardCount > 0 && !state.currentRunTerminalSuccessPresent) {
        return state;
      }
      return null;
    },
    90_000,
    'live tool card before final response',
  );
  await capture(cdp, '04-live-tool-card-visible', scope);

  await waitFor(
    () => toolOutputRequestCount > 0,
    30_000,
    'second provider request with tool output',
  );
  const duringContinuation = await waitFor(
    async () => {
      const state = await readDomState(cdp, scope);
      if (
        state.currentRunToolCardCount > 0 &&
        state.currentRunModelWaitingPresent &&
        !state.currentRunTerminalSuccessPresent
      ) {
        return state;
      }
      return null;
    },
    Math.max(3_000, finalDelayMs - 500),
    'tool card plus continuation waiting before final response',
  );
  await capture(cdp, '05-tool-result-continuation-waiting', scope);

  const finalState = await waitFor(
    async () => {
      const state = await readDomState(cdp, scope);
      if (
        state.currentRunTerminalSuccessPresent &&
        state.currentRunFinalRows.length > 0 &&
        state.currentRunSelectedCycleTerminal &&
        state.currentRunSelectedCycleFrozen &&
        state.liveCount === 0 &&
        state.currentRunLiveCount === 0
      ) {
        return state;
      }
      return null;
    },
    60_000,
    'terminal success after live tool render',
  );
  await capture(cdp, '06-terminal-success-still-has-tool-card', scope);

  const adp = await querySessionTurns();
  await fs.writeFile(path.join(artifactDir, 'adp-session.json'), JSON.stringify(adp, null, 2));
  const turns = sessionTurnsFromAdp(adp);
  const currentRunTurns = turns.filter((turn) => !beforeTurnIds.has(turn.turn_id));
  const latestTurn = currentRunTurns[currentRunTurns.length - 1] || {};
  const toolActivityTurns = currentRunTurns.filter((turn) => (turn.tool_activities || []).length > 0);
  const toolActivityCount = toolActivityTurns.reduce((sum, turn) => sum + (turn.tool_activities || []).length, 0);
  const configAfter = await run([cli, 'adp-config-query', '--url', adpUrl]);
  const summary = {
    ok: true,
    runId,
    artifactDir,
    baseUrl,
    adpUrl,
    fixedSessionId,
    runMarker,
    runTaskId,
    finalTextMarker,
    beforeTurnIds: Array.from(beforeTurnIds),
    requestCount,
    toolOutputRequestCount,
    finalDelayMs,
    toolSchemaIncluded,
    serviceTurn,
    duringTool,
    duringContinuation,
    finalState,
    adpTurnCount: turns.length,
    adpCurrentRunTurnIds: currentRunTurns.map((turn) => turn.turn_id),
    adpLatestTurnId: latestTurn.turn_id || null,
    adpLatestTerminalStatus: latestTurn.terminal_status || null,
    adpToolActivityTurnIds: toolActivityTurns.map((turn) => turn.turn_id),
    adpToolActivityCount: toolActivityCount,
    configAfter: configAfter.stdout.trim(),
    checks: {
      fixedSessionSelected: finalState.selectedSession === fixedSessionId,
      serviceTurnMaterialized: serviceTurn.currentRunRealTurnIds.length > 0,
      fixtureSawToolSchema: toolSchemaIncluded,
      fixtureSawToolOutputRequest: toolOutputRequestCount > 0,
      liveToolCardVisibleBeforeFinal:
        duringTool.currentRunToolCardCount > 0 &&
        duringTool.currentRunToolCycleIndexes.length > 0 &&
        !duringTool.currentRunTerminalSuccessPresent,
      liveToolCardOrderedAfterUserRequest:
        duringTool.currentRunUserBeforeToolCard &&
        duringTool.currentRunUserCycleBeforeToolCycle &&
        duringTool.previousCyclesBeforeCurrent,
      continuationWaitingVisibleBeforeFinal:
        duringContinuation.currentRunToolCardCount > 0 &&
        duringContinuation.currentRunModelWaitingPresent &&
        duringContinuation.currentRunModelCycleIndexes.length > 0 &&
        !duringContinuation.currentRunTerminalSuccessPresent,
      continuationWaitingOrderedAfterUserRequest:
        duringContinuation.currentRunUserBeforeToolCard &&
        duringContinuation.currentRunUserBeforeModelWaiting &&
        duringContinuation.currentRunUserCycleBeforeToolCycle &&
        duringContinuation.currentRunUserCycleBeforeModelCycle &&
        duringContinuation.previousCyclesBeforeCurrent,
      finalStillShowsToolCard: finalState.currentRunToolCardCount > 0,
      eachToolCardExposesCopyOnly:
        finalState.currentRunToolCards.length > 0 &&
        finalState.currentRunToolCards.every((card) =>
          card.copyButtonCount === 1 &&
          card.memoryButtonCount === 0 &&
          card.copyButtonLabel === '复制工具结果'
        ),
      finalSummaryExposesCopyAndMemory:
        finalState.currentRunFinalRows.length > 0 &&
        finalState.currentRunFinalRows.some((row) =>
          row.copyButtonCount === 1 &&
          row.memoryButtonCount === 1 &&
          row.copyButtonLabel === '复制 summary' &&
          row.memoryButtonLabel === '把 summary 加入记忆'
        ),
      finalCycleCardFrozen:
        finalState.currentRunSelectedCycleTerminal &&
        finalState.currentRunSelectedCycleFrozen &&
        finalState.currentRunTerminalCycleCount > 0 &&
        finalState.currentRunLiveCycleCount === 0 &&
        finalState.previousCyclesBeforeCurrent,
      finalSucceeded:
        finalState.currentRunTerminalSuccessPresent &&
        finalState.currentRunSelectedTurn &&
        finalState.selectedTerminalStatus.toLowerCase() === 'success' &&
        /^(completed|已完成)$/i.test(finalState.turnStatus.trim()) &&
        finalState.liveCount === 0 &&
        finalState.currentRunLiveCount === 0 &&
        !finalState.finalStatusStillDispatching &&
        latestTurn.terminal_status === 'Success',
      adpRetainedToolActivity: toolActivityCount > 0,
    },
  };
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));

  const firstFinalRow = finalState.currentRunFinalRows[0];
  if (firstFinalRow) {
    const clickMemory = await evalInPage(
      cdp,
      (turnId) => {
        const section = document.querySelector(`.chat-section-final[data-turn-id="${turnId}"]`);
        const button = section?.querySelector('.final-summary-memory');
        if (!section || !button) {
          return { clicked: false, reason: 'memory button missing' };
        }
        button.click();
        return { clicked: true };
      },
      firstFinalRow.turnId,
    );
    await fs.writeFile(
      path.join(artifactDir, 'memory-button-click.json'),
      JSON.stringify(clickMemory, null, 2),
    );
    await new Promise((resolve) => setTimeout(resolve, 1500));
    const memoryEntry = await readDomState(cdp, scope);
    await fs.writeFile(
      path.join(artifactDir, 'after-memory-button.json'),
      JSON.stringify(memoryEntry, null, 2),
    );
  }
  console.log(JSON.stringify(summary, null, 2));
  const failed = Object.entries(summary.checks).filter(([, value]) => !value);
  if (failed.length > 0) {
    throw new Error(`live tool render checks failed: ${failed.map(([key]) => key).join(', ')}`);
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
  await fs.writeFile(configPath, originalConfig).catch((error) => restoreErrors.push(error.message));
  await fs.writeFile(envPath, originalEnv).catch((error) => restoreErrors.push(error.message));
  const restart = await run(['scripts/install-launchd.sh', 'restartS']);
  if (restart.code !== 0) {
    restoreErrors.push(`restartS restore failed: ${restart.stderr || restart.stdout}`);
  } else {
    await waitHealth().catch((error) => restoreErrors.push(error.message));
  }
  if (restoreErrors.length > 0) {
    console.error(JSON.stringify({ restoreErrors }, null, 2));
    process.exitCode = 1;
  }
}

async function captureFailureState(error) {
  const failureDir = path.join(artifactDir, 'failure');
  await fs.mkdir(failureDir, { recursive: true });
  const failure = {
    ok: false,
    runId,
    fixedSessionId,
    runMarker,
    runTaskId,
    finalTextMarker,
    error: error && (error.stack || error.message || String(error)),
    requestCount,
    toolOutputRequestCount,
    toolSchemaIncluded,
    fixtureRequestsPresent: fss.existsSync(path.join(artifactDir, 'fixture-requests.jsonl')),
  };
  await fs.writeFile(path.join(failureDir, 'failure.json'), JSON.stringify(failure, null, 2));

  if (cdp) {
    await capture(cdp, 'failure-dom', {
      beforeTurnIds: JSON.parse(
        await fs.readFile(path.join(artifactDir, 'adp-session-before.json'), 'utf8').catch(() => '{"SessionTurns":{"turns":[]}}'),
      ).SessionTurns?.turns?.map((turn) => turn.turn_id).filter(Boolean) || [],
      runTaskId,
      finalTextMarker,
    }).catch((captureError) =>
      fs.writeFile(path.join(failureDir, 'dom-capture-error.txt'), captureError.stack || captureError.message),
    );
  }

  const [sessionTurns, configAfter, cancelReceipt] = await Promise.all([
    querySessionTurns().catch((queryError) => ({ error: queryError.message })),
    run([cli, 'adp-config-query', '--url', adpUrl]).catch((runError) => ({
      code: -1,
      stdout: '',
      stderr: runError.message,
    })),
    adpCommand({ CancelLatestActiveTurn: {} }).catch((cancelError) => ({
      error: cancelError.message,
    })),
  ]);
  await fs.writeFile(path.join(failureDir, 'adp-session-turns.json'), JSON.stringify(sessionTurns, null, 2));
  await fs.writeFile(path.join(failureDir, 'config-after.txt'), `${configAfter.stdout || ''}${configAfter.stderr || ''}`);
  await fs.writeFile(path.join(failureDir, 'cancel-latest-active-turn.json'), JSON.stringify(cancelReceipt, null, 2));

  await fs.writeFile(
    path.join(failureDir, 'active-work-list.json'),
    JSON.stringify(
      await listFiles(path.join(runtimeHome, 'state', 'master-loop')).catch((listError) => ({
        error: listError.message,
      })),
      null,
      2,
    ),
  );
  await copyIfExists(
    path.join(runtimeHome, 'state', 'master-loop', 'master.active-work.json'),
    path.join(failureDir, 'master.active-work.json'),
  );
  await copyIfExists(
    path.join(runtimeHome, 'state', 'master-loop', 'master.json'),
    path.join(failureDir, 'master-loop-master.json'),
  );
  await fs.writeFile(
    path.join(failureDir, 'metadata-tail.jsonl'),
    await tailText(path.join(runtimeHome, 'ledgers', 'metadata', 'master', `${fixedSessionId}.jsonl`), 120),
  );
  await fs.writeFile(
    path.join(failureDir, 'daemonS.stderr.tail.txt'),
    await tailText(path.join(runtimeHome, 'logs', 'daemonS.stderr.log'), 120),
  );
  await fs.writeFile(
    path.join(failureDir, 'daemonS.stdout.tail.txt'),
    await tailText(path.join(runtimeHome, 'logs', 'daemonS.stdout.log'), 120),
  );
  await fs.writeFile(
    path.join(failureDir, 'env-fixture-matches.json'),
    JSON.stringify(await fixtureEnvMatches(envPath), null, 2),
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
      req.on('end', async () => {
        requestCount += 1;
        const parsed = parseJsonOrNull(body);
        const hasToolOutput = requestHasFunctionCallOutput(parsed);
        if (hasToolOutput) {
          toolOutputRequestCount += 1;
        }
        toolSchemaIncluded = toolSchemaIncluded || requestIncludesTool(parsed, 'task');
        fss.appendFileSync(
          path.join(artifactDir, 'fixture-requests.jsonl'),
          JSON.stringify({
            count: requestCount,
            at: new Date().toISOString(),
            method: req.method,
            url: req.url,
            hasToolOutput,
            toolNames: requestToolNames(parsed),
            bodyLength: body.length,
          }) + '\n',
        );
        res.writeHead(200, { 'content-type': 'application/json' });
        if (!hasToolOutput) {
          res.end(firstToolCallBody());
          return;
        }
        await delay(finalDelayMs);
        res.end(finalCompletionBody());
      });
    });
    server.on('error', reject);
    server.listen(fixturePort, '127.0.0.1', () => resolve(server));
  });
}

function firstToolCallBody() {
  return JSON.stringify({
    id: 'resp-live-tool-render-1',
    object: 'response',
    status: 'in_progress',
    output: [
      {
        type: 'reasoning',
        summary: [{ text: 'Need Task Center truth before answering.' }],
      },
      {
        type: 'function_call',
        call_id: `call-${runMarker}`,
        name: 'task',
        arguments: JSON.stringify({
          op: 'query',
          task_id: runTaskId,
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
    completion_reason: 'live tool render verifier observed tool result',
    evidence: `fixture received function_call_output for ${runTaskId} and then returned final completion`,
    summary: `${finalTextMarker} after a visible task tool call`,
    learned: 'tool calls must stay visible before final summary',
  };
  return JSON.stringify({
    id: 'resp-live-tool-render-2',
    object: 'response',
    status: 'completed',
    error: null,
    output: [
      {
        type: 'message',
        id: 'msg-live-tool-render-final',
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

async function ensureFixedSession() {
  const activeList = await adpQuery({
    QuerySessionListPage: {
      archived: false,
      page: { direction: 'Latest', cursor: null, limit: 100 },
    },
  });
  const activeSessions = requireSessionListPage(activeList, 'active session list').sessions;
  if (activeSessions.some((session) => session.session_id === fixedSessionId)) {
    return;
  }
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
      title: 'Live tool render fixed verifier',
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
      input.dispatchEvent(new InputEvent('input', { bubbles: true, data: value, inputType: 'insertText' }));
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
  return await evalInPage(cdpClient, (scopeArg) => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    const messageText = document.getElementById('message-list')?.innerText || '';
    const beforeTurnIds = new Set((scopeArg && scopeArg.beforeTurnIds) || []);
    const runTaskId = (scopeArg && scopeArg.runTaskId) || '';
    const finalTextMarker = (scopeArg && scopeArg.finalTextMarker) || '';
    const syntheticTurnIds = new Set(['pending-submit', 'accepted-submit', 'adp-failure', 'session-refresh-loading']);
    const isRealTurnId = (turnId) => !!turnId && !syntheticTurnIds.has(turnId);
    const chatNodes = Array.from(document.querySelectorAll('.chat-message'));
    const cycleNodes = Array.from(document.querySelectorAll('.turn-cycle-card'));
    const cycleIndexForNode = (node) => cycleNodes.indexOf(node?.closest('.turn-cycle-card'));
    const chatMessages = chatNodes.map((node, index) => ({
      index,
      turnId: node.dataset.turnId || '',
      cycleIndex: cycleIndexForNode(node),
      cycleTurnId: node.closest('.turn-cycle-card')?.dataset.turnId || '',
      cycleKind: node.closest('.turn-cycle-card')?.dataset.cycleKind || '',
      text: node.innerText || '',
      live: node.dataset.live === 'true',
      className: node.className || '',
      assistant: node.classList.contains('chat-message-assistant'),
    }));
    const cycleCards = cycleNodes.map((node, index) => ({
      index,
      kind: node.dataset.cycleKind || '',
      turnId: node.dataset.turnId || '',
      sessionId: node.dataset.sessionId || '',
      submitId: node.dataset.submitId || '',
      createdAt: node.dataset.createdAt || '',
      live: node.dataset.live === 'true',
      terminal: node.dataset.terminal === 'true',
      frozen: node.dataset.frozen === 'true',
      className: node.className || '',
      text: node.innerText || '',
      childMessageIndexes: Array.from(node.querySelectorAll('.chat-message'))
        .map((child) => chatNodes.indexOf(child))
        .filter((index) => index >= 0),
    }));
    const toolCards = Array.from(document.querySelectorAll('.chat-section-tool')).map((node) => ({
      turnId: node.dataset.turnId || '',
      messageTurnId: node.closest('.chat-message')?.dataset.turnId || '',
      cycleTurnId: node.closest('.turn-cycle-card')?.dataset.turnId || '',
      cycleIndex: cycleIndexForNode(node),
      messageIndex: chatNodes.indexOf(node.closest('.chat-message')),
      toolCallId: node.dataset.toolCallId || '',
      text: node.innerText || '',
      className: node.className || '',
      copyButtonCount: node.querySelectorAll('.tool-chat-copy').length,
      memoryButtonCount: node.querySelectorAll('.final-summary-memory').length,
      copyButtonLabel: node.querySelector('.tool-chat-copy')?.getAttribute('aria-label') || '',
    }));
    const finalRows = Array.from(document.querySelectorAll('.chat-section-final')).map((node) => ({
      turnId: node.dataset.turnId || '',
      messageTurnId: node.closest('.chat-message')?.dataset.turnId || '',
      cycleTurnId: node.closest('.turn-cycle-card')?.dataset.turnId || '',
      cycleIndex: cycleIndexForNode(node),
      messageIndex: chatNodes.indexOf(node.closest('.chat-message')),
      text: node.innerText || '',
      copyButtonCount: node.querySelectorAll('.final-summary-copy').length,
      memoryButtonCount: node.querySelectorAll('.final-summary-memory').length,
      copyButtonLabel: node.querySelector('.final-summary-copy')?.getAttribute('aria-label') || '',
      memoryButtonLabel: node.querySelector('.final-summary-memory')?.getAttribute('aria-label') || '',
    }));
    const modelRows = Array.from(document.querySelectorAll('.chat-section-system')).map((node) => ({
      turnId: node.dataset.turnId || '',
      messageTurnId: node.closest('.chat-message')?.dataset.turnId || '',
      cycleTurnId: node.closest('.turn-cycle-card')?.dataset.turnId || '',
      cycleIndex: cycleIndexForNode(node),
      messageIndex: chatNodes.indexOf(node.closest('.chat-message')),
      text: node.innerText || '',
    }));
    const currentRunTurnIds = new Set();
    const includeTurn = (turnId) => {
      if (turnId) {
        currentRunTurnIds.add(turnId);
      }
    };
    cycleCards.forEach((card) => {
      if (isRealTurnId(card.turnId) && !beforeTurnIds.has(card.turnId)) {
        includeTurn(card.turnId);
      }
      if ((card.kind === 'pending' || card.kind === 'accepted') && card.turnId) {
        includeTurn(card.turnId);
      }
      if (runTaskId && card.text.includes(runTaskId)) {
        includeTurn(card.turnId);
      }
      if (finalTextMarker && card.text.includes(finalTextMarker)) {
        includeTurn(card.turnId);
      }
    });
    chatMessages.forEach((message) => {
      if (isRealTurnId(message.turnId) && !beforeTurnIds.has(message.turnId)) {
        includeTurn(message.turnId);
      }
      if (isRealTurnId(message.cycleTurnId) && !beforeTurnIds.has(message.cycleTurnId)) {
        includeTurn(message.cycleTurnId);
      }
      if (runTaskId && message.text.includes(runTaskId)) {
        includeTurn(message.turnId);
        includeTurn(message.cycleTurnId);
      }
      if (finalTextMarker && message.text.includes(finalTextMarker)) {
        includeTurn(message.turnId);
        includeTurn(message.cycleTurnId);
      }
    });
    toolCards.forEach((card) => {
      if (runTaskId && card.text.includes(runTaskId)) {
        includeTurn(card.turnId);
        includeTurn(card.messageTurnId);
        includeTurn(card.cycleTurnId);
      }
    });
    const isCurrentRunTurn = (turnId) => !!turnId && currentRunTurnIds.has(turnId);
    const currentRunCycles = cycleCards.filter((card) =>
      isCurrentRunTurn(card.turnId) ||
        (runTaskId && card.text.includes(runTaskId)) ||
        (finalTextMarker && card.text.includes(finalTextMarker)),
    );
    const currentRunMessages = chatMessages.filter((message) =>
      isCurrentRunTurn(message.turnId) ||
        isCurrentRunTurn(message.cycleTurnId) ||
        (runTaskId && message.text.includes(runTaskId)) ||
        (finalTextMarker && message.text.includes(finalTextMarker)),
    );
    const currentRunToolCards = toolCards.filter((card) =>
      (runTaskId && card.text.includes(runTaskId)) ||
        isCurrentRunTurn(card.turnId) ||
        isCurrentRunTurn(card.messageTurnId) ||
        isCurrentRunTurn(card.cycleTurnId),
    );
    const currentRunFinalRows = finalRows.filter((row) =>
      isCurrentRunTurn(row.turnId) ||
        isCurrentRunTurn(row.messageTurnId) ||
        isCurrentRunTurn(row.cycleTurnId),
    );
    const currentRunModelRows = modelRows.filter((row) =>
      isCurrentRunTurn(row.turnId) ||
        isCurrentRunTurn(row.messageTurnId) ||
        isCurrentRunTurn(row.cycleTurnId),
    );
    const currentRunUserIndexes = currentRunMessages
      .filter((message) => !message.assistant)
      .map((message) => message.index)
      .filter((index) => index >= 0);
    const currentRunAssistantIndexes = currentRunMessages
      .filter((message) => message.assistant)
      .map((message) => message.index)
      .filter((index) => index >= 0);
    const currentRunToolMessageIndexes = currentRunToolCards
      .map((card) => card.messageIndex)
      .filter((index) => index >= 0);
    const currentRunModelMessageIndexes = currentRunModelRows
      .map((row) => row.messageIndex)
      .filter((index) => index >= 0);
    const firstCurrentRunUserIndex = currentRunUserIndexes.length > 0
      ? Math.min(...currentRunUserIndexes)
      : -1;
    const firstCurrentRunAssistantIndex = currentRunAssistantIndexes.length > 0
      ? Math.min(...currentRunAssistantIndexes)
      : -1;
    const firstCurrentRunToolMessageIndex = currentRunToolMessageIndexes.length > 0
      ? Math.min(...currentRunToolMessageIndexes)
      : -1;
    const firstCurrentRunModelMessageIndex = currentRunModelMessageIndexes.length > 0
      ? Math.min(...currentRunModelMessageIndexes)
      : -1;
    const currentRunCycleIndexes = currentRunCycles
      .map((card) => card.index)
      .filter((index) => index >= 0);
    const currentRunUserCycleIndexes = currentRunMessages
      .filter((message) => !message.assistant)
      .map((message) => message.cycleIndex)
      .filter((index) => index >= 0);
    const currentRunToolCycleIndexes = currentRunToolCards
      .map((card) => card.cycleIndex)
      .filter((index) => index >= 0);
    const currentRunModelCycleIndexes = currentRunModelRows
      .map((row) => row.cycleIndex)
      .filter((index) => index >= 0);
    const firstCurrentRunCycleIndex = currentRunCycleIndexes.length > 0
      ? Math.min(...currentRunCycleIndexes)
      : -1;
    const firstCurrentRunUserCycleIndex = currentRunUserCycleIndexes.length > 0
      ? Math.min(...currentRunUserCycleIndexes)
      : -1;
    const firstCurrentRunToolCycleIndex = currentRunToolCycleIndexes.length > 0
      ? Math.min(...currentRunToolCycleIndexes)
      : -1;
    const firstCurrentRunModelCycleIndex = currentRunModelCycleIndexes.length > 0
      ? Math.min(...currentRunModelCycleIndexes)
      : -1;
    const previousTurnCycleIndexes = cycleCards
      .filter((card) => isRealTurnId(card.turnId) && beforeTurnIds.has(card.turnId))
      .map((card) => card.index)
      .filter((index) => index >= 0);
    const previousCyclesBeforeCurrent =
      firstCurrentRunCycleIndex < 0 ||
      previousTurnCycleIndexes.every((index) => index < firstCurrentRunCycleIndex);
    const selectedTurn = shell?.dataset.selectedTurn || '';
    const selectedCycle = cycleCards.find((card) => card.turnId === selectedTurn) || null;
    const selectedTurnText = chatMessages
      .filter((message) => message.turnId === selectedTurn)
      .map((message) => message.text)
      .join('\n');
    const finalStatusText = [
      document.getElementById('command-status')?.textContent || '',
      document.getElementById('turn-status')?.textContent || '',
    ].join('\n');
    return {
      selectedSession: shell?.dataset.selectedSession || '',
      selectedTurn,
      selectedTerminalStatus: shell?.dataset.selectedTerminalStatus || '',
      commandStatus: document.getElementById('command-status')?.textContent || '',
      turnStatus: document.getElementById('turn-status')?.textContent || '',
      messageText,
      currentRunMessageText: currentRunMessages.map((message) => message.text).join('\n---\n'),
      currentRunUserIndexes,
      currentRunAssistantIndexes,
      currentRunToolMessageIndexes,
      currentRunModelMessageIndexes,
      currentRunCycleIndexes,
      currentRunUserCycleIndexes,
      currentRunToolCycleIndexes,
      currentRunModelCycleIndexes,
      firstCurrentRunUserIndex,
      firstCurrentRunAssistantIndex,
      firstCurrentRunToolMessageIndex,
      firstCurrentRunModelMessageIndex,
      firstCurrentRunCycleIndex,
      firstCurrentRunUserCycleIndex,
      firstCurrentRunToolCycleIndex,
      firstCurrentRunModelCycleIndex,
      previousCyclesBeforeCurrent,
      currentRunUserBeforeAssistant:
        firstCurrentRunUserIndex >= 0 &&
        firstCurrentRunAssistantIndex >= 0 &&
        firstCurrentRunUserIndex < firstCurrentRunAssistantIndex,
      currentRunUserBeforeToolCard:
        firstCurrentRunUserIndex >= 0 &&
        firstCurrentRunToolMessageIndex >= 0 &&
        firstCurrentRunUserIndex < firstCurrentRunToolMessageIndex,
      currentRunUserCycleBeforeToolCycle:
        firstCurrentRunUserCycleIndex >= 0 &&
        firstCurrentRunToolCycleIndex >= 0 &&
        firstCurrentRunUserCycleIndex <= firstCurrentRunToolCycleIndex,
      currentRunUserBeforeModelWaiting:
        firstCurrentRunUserIndex >= 0 &&
        firstCurrentRunModelMessageIndex >= 0 &&
        firstCurrentRunUserIndex < firstCurrentRunModelMessageIndex,
      currentRunUserCycleBeforeModelCycle:
        firstCurrentRunUserCycleIndex >= 0 &&
        firstCurrentRunModelCycleIndex >= 0 &&
        firstCurrentRunUserCycleIndex <= firstCurrentRunModelCycleIndex,
      currentRunTurnIds: Array.from(currentRunTurnIds),
      currentRunSelectedTurn: isCurrentRunTurn(selectedTurn),
      currentRunSelectedCycleTerminal:
        !!selectedCycle && isCurrentRunTurn(selectedCycle.turnId) && selectedCycle.terminal,
      currentRunSelectedCycleFrozen:
        !!selectedCycle && isCurrentRunTurn(selectedCycle.turnId) && selectedCycle.frozen,
      selectedTurnText,
      cycleCardCount: cycleCards.length,
      cycleCards,
      currentRunCycleCardCount: currentRunCycles.length,
      currentRunCycleCards: currentRunCycles,
      currentRunLiveCycleCount: currentRunCycles.filter((card) => card.live).length,
      currentRunTerminalCycleCount: currentRunCycles.filter((card) => card.terminal).length,
      currentRunFrozenCycleCount: currentRunCycles.filter((card) => card.frozen).length,
      toolCardCount: toolCards.length,
      failedToolCardCount: toolCards.filter((card) => /failed|执行失败/i.test(`${card.className} ${card.text}`)).length,
      runningToolCardCount: toolCards.filter((card) => /running|等待中|waiting/i.test(`${card.className} ${card.text}`)).length,
      toolCards,
      currentRunToolCardCount: currentRunToolCards.length,
      currentRunFailedToolCardCount: currentRunToolCards.filter((card) => /failed|执行失败/i.test(`${card.className} ${card.text}`)).length,
      currentRunRunningToolCardCount: currentRunToolCards.filter((card) => /running|等待中|waiting/i.test(`${card.className} ${card.text}`)).length,
      currentRunToolCards,
      finalRows,
      currentRunFinalRows,
      modelRows,
      currentRunModelRows,
      modelWaitingPresent: modelRows.some((row) =>
        /thinking after tool result|tool result returned|waiting model|provider retry|waiting for model/i.test(row.text),
      ),
      currentRunModelWaitingPresent: currentRunModelRows.some((row) =>
        /thinking after tool result|tool result returned|waiting model|provider retry|waiting for model/i.test(row.text),
      ),
      terminalSuccessPresent: /live tool render completed/i.test(messageText),
      currentRunTerminalSuccessPresent: finalTextMarker
        ? currentRunMessages.some((message) => message.assistant && message.text.includes(finalTextMarker))
        : false,
      currentRunRealTurnIds: Array.from(currentRunTurnIds).filter((turnId) =>
        turnId && turnId !== 'pending-submit' && turnId !== 'accepted-submit'
      ),
      liveCount: document.querySelectorAll('[data-live="true"]').length,
      currentRunLiveCount:
        currentRunMessages.filter((message) => message.live).length +
        currentRunCycles.filter((card) => card.live).length,
      finalStatusStillDispatching: /dispatching|thinking|tool running|tool executing|waiting model/i.test(finalStatusText),
    };
  }, scope);
}

async function adpQuery(query) {
  return await adpRequest('query', 'query', query);
}

async function querySessionTurns() {
  return await adpQuery({ QuerySessionTurns: { session_id: fixedSessionId } });
}

function sessionTurnsFromAdp(adp) {
  return (adp && adp.SessionTurns && Array.isArray(adp.SessionTurns.turns))
    ? adp.SessionTurns.turns
    : [];
}

async function adpCommand(command) {
  return await adpRequest('command', 'command', command);
}

function adpRequest(kind, payloadKey, payload) {
  return adpVerifierRequest({
    url: adpUrl,
    authToken: adpAuthToken,
    kind,
    payloadKey,
    payload,
    clientName: 'freehand-live-tool-render-verifier',
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

async function copyIfExists(source, target) {
  try {
    await fs.copyFile(source, target);
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return;
    }
    throw error;
  }
}

async function listFiles(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  return entries
    .map((entry) => ({
      name: entry.name,
      type: entry.isDirectory() ? 'directory' : entry.isFile() ? 'file' : 'other',
    }))
    .sort((left, right) => left.name.localeCompare(right.name));
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
    .filter(({ text: line }) =>
      /FREEHAND_LIVE_TOOL_RENDER_FIXTURE_KEY|FREEHAND_PROVIDER_RETRY_BACKOFF_MS|FREEHAND_PROVIDER_RECOVERY_FIXTURE_KEY/.test(
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

async function waitForLoad(cdpClient) {
  await new Promise((resolve) => {
    const listener = (method) => {
      if (method === 'Page.loadEventFired') {
        cdpClient.offEvent(listener);
        resolve();
      }
    };
    cdpClient.onEvent(listener);
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

function requestIncludesTool(value, name) {
  return requestToolNames(value).includes(name);
}

function requestToolNames(value) {
  const tools = value && Array.isArray(value.tools) ? value.tools : [];
  return tools
    .map((tool) => tool && (tool.name || (tool.function && tool.function.name)))
    .filter(Boolean);
}

function parseJsonOrNull(value) {
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
}

function stripFixtureEnv(value) {
  return value
    .replace(/\n?FREEHAND_LIVE_TOOL_RENDER_FIXTURE_KEY=.*$/gm, '')
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
