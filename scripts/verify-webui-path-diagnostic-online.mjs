import { spawn } from 'node:child_process';
import fsSync from 'node:fs';
import fs from 'node:fs/promises';
import http from 'node:http';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const configPath = process.env.FREEHAND_WEBUI_PATH_CONFIG || path.join(runtimeHome, 'config.toml');
const daemonEnvPath = process.env.FREEHAND_WEBUI_PATH_DAEMON_ENV || path.join(runtimeHome, 'daemonS.env');
const workerEnvPath = process.env.FREEHAND_WEBUI_PATH_WORKER_ENV || path.join(runtimeHome, 'workerS.worker.env');
const cli = process.env.FREEHAND_WEBUI_PATH_CLI || path.join(home, '.local/bin/freehand-cliS');
const adpUrl = process.env.FREEHAND_WEBUI_PATH_ADP_URL || 'ws://127.0.0.1:4042/adp';
const healthUrl = process.env.FREEHAND_WEBUI_PATH_HEALTH_URL || 'http://127.0.0.1:4042/health';
const baseUrl = process.env.FREEHAND_WEBUI_PATH_BASE_URL || 'http://127.0.0.1:4042/?verify=webui-path-diagnostic';
const chromePath =
  process.env.FREEHAND_WEBUI_PATH_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const parentSessionId = process.env.FREEHAND_WEBUI_PATH_PARENT_SESSION || 'webui-path-diagnostic-fixed-v2';
const targetCwd = process.env.FREEHAND_WEBUI_PATH_TARGET_CWD || '/Users/fanzhang/github';
const canonicalTargetCwd = process.env.FREEHAND_WEBUI_PATH_CANONICAL_CWD || '/Users/fanzhang/Documents/github';
const requestedPath = process.env.FREEHAND_WEBUI_PATH_REQUESTED || '/Users/fanzhang/github/codex';
const missingSuffix = process.env.FREEHAND_WEBUI_PATH_MISSING_SUFFIX || 'codex';
const fixtureKeyName = 'FREEHAND_WEBUI_PATH_DIAGNOSTIC_FIXTURE_KEY';
const fixtureKeyValue = 'webui-path-diagnostic-fixture-key';
const runStamp = process.env.FREEHAND_WEBUI_PATH_RUN_STAMP || `${Date.now()}`;
const taskId = process.env.FREEHAND_WEBUI_PATH_TASK_ID || `task-webui-path-diagnostic-${runStamp}`;
const masterModel = 'webui-path-master-model';
const workerModel = 'webui-path-worker-model';
const masterProviderId = 'webui-path-master-fixture';
const workerProviderId = 'webui-path-worker-fixture';
const artifactDir =
  process.env.FREEHAND_WEBUI_PATH_ARTIFACT_DIR ||
  path.join(repo, 'artifacts', 'webui-online', `path-diagnostic-${runStamp}`);

const prompt = [
  'Master/Worker path diagnostic verification.',
  `Check requested path: ${requestedPath}`,
  `Worker target_cwd must be: ${targetCwd}`,
  'Master must dispatch the work to a Worker and wait for Task Center truth; dispatch alone is not completion.',
  'Worker must use the built-in path tool to inspect the requested path. If the leaf is missing, report the tool-owned path_diagnostic instead of guessing that the symlink was not expanded.',
  `Acceptance: WebUI must show the TaskBoard-projected Worker, and the Worker session result must mention requested=${requestedPath}, nearest_existing_canonical=${canonicalTargetCwd}, and missing_suffix=${missingSuffix}.`,
].join('\n');

let fixtureServer = null;
let chrome = null;
let cdp = null;
let originalConfig = null;
let originalDaemonEnv = null;
let originalWorkerEnv = null;
let fixturePort = null;
let debugPort = null;
const fixtureState = {
  masterRequests: [],
  workerRequests: [],
  masterStep: 0,
  workerStep: 0,
  secondHadToolResult: false,
  secondHadDiagnostic: false,
  secondBodyLength: 0,
  diagnosticChecks: {},
  masterLifecycleAppendRequested: false,
  masterLifecycleAppendChecks: {},
};
const webuiEvidence = {
  headerTree: null,
  workerDom: null,
  returnedParentDom: null,
};

await fs.mkdir(artifactDir, { recursive: true });

try {
  assertPathFixturePreconditions();
  await assertFileExists(cli, 'CLI');
  originalConfig = await fs.readFile(configPath, 'utf8');
  originalDaemonEnv = await fs.readFile(daemonEnvPath, 'utf8');
  originalWorkerEnv = await fs.readFile(workerEnvPath, 'utf8').catch(() => null);
  await fs.writeFile(path.join(artifactDir, 'prompt.txt'), prompt);
  await fs.writeFile(path.join(artifactDir, 'config.before.toml'), redactConfig(originalConfig));
  await fs.writeFile(path.join(artifactDir, 'daemonS.before.env'), redactEnv(originalDaemonEnv));
  if (originalWorkerEnv !== null) {
    await fs.writeFile(path.join(artifactDir, 'workerS.worker.before.env'), redactEnv(originalWorkerEnv));
  }

  fixturePort = await getFreePort();
  fixtureServer = await startFixtureServer(fixturePort);
  await configureFixtureProvider();
  await prepareFixedSession();
  await runWebUiSubmitAndInspect();

  const finalTruth = await collectTruth();
  const summary = buildSummary(finalTruth);
  await fs.writeFile(path.join(artifactDir, 'task-board.json'), JSON.stringify(finalTruth.taskBoard, null, 2));
  await fs.writeFile(path.join(artifactDir, 'task-history.json'), JSON.stringify(finalTruth.taskHistory, null, 2));
  await fs.writeFile(path.join(artifactDir, 'parent-session-turns.json'), JSON.stringify(finalTruth.parentTurns, null, 2));
  await fs.writeFile(path.join(artifactDir, 'worker-session-turns.json'), JSON.stringify(finalTruth.workerTurns, null, 2));
  await fs.writeFile(path.join(artifactDir, 'fixture-state.json'), JSON.stringify(fixtureState, null, 2));
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  const failedChecks = Object.entries(summary.checks || {}).filter(([, value]) => value !== true);
  if (failedChecks.length > 0) {
    throw new Error(`path diagnostic WebUI checks failed: ${JSON.stringify(failedChecks)}`);
  }
} catch (error) {
  const failure = {
    ok: false,
    error: error instanceof Error ? error.message : String(error),
    artifactDir,
    fixtureState,
  };
  await fs.writeFile(path.join(artifactDir, 'failure.json'), JSON.stringify(failure, null, 2));
  console.error(JSON.stringify(failure, null, 2));
  process.exitCode = 1;
} finally {
  if (cdp) {
    await cdp.close().catch(() => null);
  }
  if (chrome && chrome.exitCode === null) {
    chrome.kill('SIGTERM');
    await waitForProcessExit(chrome, 5000).catch(() => null);
  }
  if (fixtureServer) {
    await new Promise((resolve) => fixtureServer.close(resolve));
  }
  await restoreRuntime().catch(async (error) => {
    const restoreFailure = {
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    };
    await fs.writeFile(path.join(artifactDir, 'restore-failure.json'), JSON.stringify(restoreFailure, null, 2));
    console.error(JSON.stringify(restoreFailure, null, 2));
    process.exitCode = 1;
  });
}

function assertPathFixturePreconditions() {
  const target = fsSyncStats(targetCwd);
  if (!target || !target.isDirectory()) {
    throw new Error(`target cwd must exist and be a directory: ${targetCwd}`);
  }
  const targetLink = fsSync.lstatSync(targetCwd);
  if (!targetLink.isSymbolicLink()) {
    throw new Error(`target cwd must be the user-facing symlink alias for this sample: ${targetCwd}`);
  }
  const realTarget = fsSync.realpathSync(targetCwd);
  if (realTarget !== canonicalTargetCwd) {
    throw new Error(`target cwd canonical mismatch: expected ${canonicalTargetCwd}, got ${realTarget}`);
  }
  const requested = fsSyncStats(requestedPath);
  if (requested) {
    throw new Error(`requested missing-leaf sample unexpectedly exists: ${requestedPath}`);
  }
}

function fsSyncStats(value) {
  try {
    return fsSync.statSync(value);
  } catch (_) {
    return null;
  }
}

async function configureFixtureProvider() {
  await fs.writeFile(
    daemonEnvPath,
    stripFixtureEnv(originalDaemonEnv) + `\n${fixtureKeyName}="${fixtureKeyValue}"\n`,
  );
  await must(['scripts/install-launchd.sh', 'restartS']);
  await waitHealth('after fixture env restart');
  await must([
    cli,
    'adp-config-update',
    '--url',
    adpUrl,
    '--agent',
    'master',
    '--provider',
    masterProviderId,
    '--type',
    'anthropic',
    '--protocol',
    'messages',
    '--base-url',
    `http://127.0.0.1:${fixturePort}`,
    '--model',
    masterModel,
    '--api-key-env',
    fixtureKeyName,
  ]);
  await must([
    cli,
    'adp-config-update',
    '--url',
    adpUrl,
    '--agent',
    'worker',
    '--provider',
    workerProviderId,
    '--type',
    'anthropic',
    '--protocol',
    'messages',
    '--base-url',
    `http://127.0.0.1:${fixturePort}`,
    '--model',
    workerModel,
    '--api-key-env',
    fixtureKeyName,
  ]);
  await must(['scripts/install-launchd.sh', 'restartS']);
  await waitHealth('after fixture config restart');
  await must(['scripts/install-launchd.sh', 'restartWorkerS']);
}

async function prepareFixedSession() {
  await run([cli, 'adp-session-manage', '--url', adpUrl, '--action', 'delete', '--session', parentSessionId]);
  await must([
    cli,
    'adp-session-manage',
    '--url',
    adpUrl,
    '--action',
    'create',
    '--session',
    parentSessionId,
    '--title',
    'WebUI path diagnostic fixed',
    '--cwd',
    targetCwd,
  ]);
}

async function runWebUiSubmitAndInspect() {
  debugPort = await getFreePort();
  const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-webui-path-diagnostic-'));
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
      '--window-size=390,844',
      baseUrl,
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );
  const chromeLog = [];
  chrome.stdout.on('data', (chunk) => chromeLog.push(`[stdout] ${chunk}`));
  chrome.stderr.on('data', (chunk) => chromeLog.push(`[stderr] ${chunk}`));

  const target = await waitForPageTarget(baseUrl, 15000);
  cdp = await createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: `try { window.localStorage.setItem("freehand-webui-selected-session", ${JSON.stringify(parentSessionId)}); } catch (_) {}`,
  });
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 2,
    mobile: true,
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(
    cdp,
    () => {
      const shell = document.querySelector('[data-webui-shell="true"]');
      return !!shell && !!document.getElementById('composer-input') && !!document.getElementById('send-button');
    },
    20000,
    'WebUI shell ready',
  );
  await waitForFunction(
    cdp,
    (sessionId) => document.querySelector('[data-webui-shell="true"]')?.dataset.selectedSession === sessionId,
    20000,
    'fixed parent session selected',
    parentSessionId,
  );
  await screenshot(cdp, path.join(artifactDir, '01-parent-before-submit.png'));

  await evalInPage(
    cdp,
    (value) => {
      const input = document.getElementById('composer-input');
      input.value = value;
      input.dispatchEvent(new Event('input', { bubbles: true }));
      document.getElementById('send-button')?.click();
    },
    prompt,
  );
  await screenshot(cdp, path.join(artifactDir, '02-parent-after-submit.png'));
  await snapshotWebUiDom('02-parent-after-submit');
  await waitForFixtureState(
    () => fixtureState.masterRequests.length > 0,
    30000,
    'Master provider request after WebUI submit',
  );

  await waitForTaskStatus('blocked', 180000);
  await waitForFixtureState(
    () => fixtureState.masterRequests.length >= 3,
    60000,
    'Master provider waiting completion after task assignment',
  );
  await waitForParentCurrentTaskWaiting(180000);
  await waitForFixtureState(
    () => fixtureState.masterLifecycleAppendRequested,
    60000,
    'Master lifecycle append after Worker blocked',
  );
  await waitForTaskHistoryEvent('TaskProgressed', 60000);
  await refreshWebUiState();
  await screenshot(cdp, path.join(artifactDir, '03-parent-after-worker-blocked.png'));
  const parentDom = await readLifecycleDom();
  await fs.writeFile(path.join(artifactDir, 'webui-parent-dom.json'), JSON.stringify(parentDom, null, 2));
  if (parentDom.selectedSession !== parentSessionId || parentDom.selectedTerminalStatus !== 'toolpending') {
    throw new Error(`parent DOM did not select current ToolPending parent turn: ${JSON.stringify(parentDom)}`);
  }
  if (
    !parentDom.turnStatus.toLowerCase().includes('waiting') ||
    !parentDom.assistantStatus.toLowerCase().includes('waiting') ||
    !parentDom.finalStatus.toLowerCase().includes('running') ||
    !parentDom.messageText.includes(`Waiting for lifecycle: Inspect TaskBoard/TaskHistory for ${taskId}`)
  ) {
    throw new Error(`parent DOM misrepresented dispatched-but-waiting lifecycle as completed: ${JSON.stringify(parentDom)}`);
  }

  await waitForFunction(
    cdp,
    (expectedTaskId) => {
      const title = document.getElementById('mobile-agent-summary-title')?.textContent || '';
      const copy = document.getElementById('mobile-agent-summary-copy')?.textContent || '';
      return /task/i.test(`${title} ${copy}`) &&
        !/unavailable/i.test(title) &&
        (title.includes('1') || copy.includes(expectedTaskId) || title.length > 0);
    },
    30000,
    'mobile Agent TaskBoard summary after worker blocked',
    taskId,
  );
  await evalInPage(cdp, () => {
    document.getElementById('open-mobile-agent-sheet-button')?.click();
  });
  await waitForFunction(
    cdp,
    (expectedTaskId) => {
      const sheet = document.getElementById('mobile-agent-sheet');
      const rect = sheet?.getBoundingClientRect();
      const cards = Array.from(document.querySelectorAll('#mobile-agent-task-list .mobile-agent-card'));
      const hasExpectedCard = cards.some((card) => card.dataset.taskId === expectedTaskId && !!card.dataset.workerSessionId);
      return document.body.dataset.mobileAgentSheet === 'open' &&
        sheet?.getAttribute('aria-hidden') === 'false' &&
        !!rect &&
        rect.height >= 120 &&
        rect.top <= window.innerHeight - 80 &&
        rect.bottom >= window.innerHeight - 20 &&
        hasExpectedCard;
    },
    20000,
    'TaskBoard-projected Worker card for path task',
    taskId,
  );
  await delay(800);
  await screenshot(cdp, path.join(artifactDir, '04-mobile-agent-sheet.png'));

  const sheetState = await evalInPage(cdp, (expectedTaskId) => {
    const cards = Array.from(document.querySelectorAll('#mobile-agent-task-list .mobile-agent-card'));
    return {
      taskStatus: document.getElementById('mobile-agent-task-status')?.textContent || '',
      cards: cards.map((card) => ({
        title: card.querySelector('.mobile-agent-card-title')?.textContent || '',
        meta: card.querySelector('.mobile-agent-card-meta')?.textContent || '',
        copy: card.querySelector('.mobile-agent-card-copy')?.textContent || '',
        taskId: card.dataset.taskId || '',
        workerSessionId: card.dataset.workerSessionId || '',
        matchesExpected: card.dataset.taskId === expectedTaskId,
      })),
      globalSessionListHasWorker: (document.getElementById('session-list')?.innerText || '').includes('worker-task-'),
      sheetText: document.getElementById('mobile-agent-sheet')?.innerText || '',
    };
  }, taskId);
  await fs.writeFile(path.join(artifactDir, 'webui-sheet-state.json'), JSON.stringify(sheetState, null, 2));
  if (sheetState.globalSessionListHasWorker) {
    throw new Error('global WebUI session list exposed worker-task-* internal sessions');
  }
  const expectedCard = sheetState.cards.find((card) => card.taskId === taskId);
  if (!expectedCard || !expectedCard.workerSessionId) {
    throw new Error(`missing expected WebUI worker card identity for ${taskId}: ${JSON.stringify(sheetState.cards)}`);
  }

  await verifyHeaderSessionTree(expectedCard);
  await waitForFunction(
    cdp,
    (workerSessionId) => {
      const shell = document.querySelector('[data-webui-shell="true"]');
      const selected = shell?.dataset.selectedSession || '';
      const text = document.getElementById('message-list')?.innerText || '';
      const lowerText = text.toLowerCase();
      const loading = lowerText.includes('loading conversation') ||
        lowerText.includes('loading selected session transcript');
      return selected === workerSessionId &&
        document.getElementById('worker-session-nav')?.hidden === false &&
        !loading &&
        text.length > 0;
    },
    30000,
    'worker session selected and loaded',
    expectedCard.workerSessionId,
  );
  await screenshot(cdp, path.join(artifactDir, '05-worker-session-detail.png'));
  const workerDom = await evalInPage(cdp, () => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    const selectedTurn = shell?.dataset.selectedTurn || '';
    const selectedAssistant = Array.from(document.querySelectorAll('.chat-message-assistant'))
      .find((node) => node.dataset.turnId === selectedTurn) || null;
    const text = document.getElementById('message-list')?.innerText || '';
    return {
      selectedSession: shell?.dataset.selectedSession || '',
      selectedTurn,
      selectedTerminalStatus: shell?.dataset.selectedTerminalStatus || '',
      turnStatus: document.getElementById('turn-status')?.textContent?.trim() || '',
      assistantStatus: selectedAssistant?.querySelector('.chat-state-pill')?.textContent?.trim() || '',
      finalStatus: selectedAssistant?.querySelector('.chat-section-final .chat-row-status')?.textContent?.trim() || '',
      workerNavHidden: document.getElementById('worker-session-nav')?.hidden ?? true,
      userMessageCount: document.querySelectorAll('.message.user').length,
      messageText: text,
      hasRequestedPath: text.includes('/Users/fanzhang/github/codex'),
      hasCanonicalPath: text.includes('/Users/fanzhang/Documents/github'),
      hasMissingSuffix: text.includes('missing_suffix=codex') || text.includes('missing suffix codex') || text.includes('codex'),
      fakePromptVisible:
        text.includes('Execute the assigned Task Center task') ||
        text.includes('The tool result has been returned') ||
        text.includes('Task ID:'),
    };
  });
  webuiEvidence.workerDom = workerDom;
  await fs.writeFile(path.join(artifactDir, 'webui-worker-dom.json'), JSON.stringify(workerDom, null, 2));
  if (workerDom.selectedSession !== expectedCard.workerSessionId) {
    throw new Error(`worker DOM selected session mismatch: ${workerDom.selectedSession}`);
  }
  if (
    workerDom.selectedTerminalStatus !== 'blocked' ||
    workerDom.turnStatus.toLowerCase() !== 'blocked' ||
    workerDom.assistantStatus.toLowerCase() !== 'blocked' ||
    workerDom.finalStatus.toLowerCase() !== 'blocked'
  ) {
    throw new Error(`worker DOM misrepresented blocked lifecycle status: ${JSON.stringify(workerDom)}`);
  }
  if (workerDom.fakePromptVisible || workerDom.userMessageCount !== 0) {
    throw new Error(`worker DOM leaked internal prompt/user rows: ${JSON.stringify(workerDom)}`);
  }
  if (!workerDom.hasRequestedPath || !workerDom.hasCanonicalPath || !workerDom.hasMissingSuffix) {
    throw new Error(`worker DOM did not show path diagnostic result fields: ${JSON.stringify(workerDom)}`);
  }

  await evalInPage(cdp, () => {
    document.getElementById('worker-session-back-button')?.click();
  });
  await waitForFunction(
    cdp,
    (sessionId) => {
      const shell = document.querySelector('[data-webui-shell="true"]');
      const text = document.getElementById('message-list')?.innerText || '';
      const lowerText = text.toLowerCase();
      const loading = lowerText.includes('loading conversation') ||
        lowerText.includes('loading selected session transcript');
      return shell?.dataset.selectedSession === sessionId &&
        document.getElementById('worker-session-nav')?.hidden === true &&
        !loading &&
        text.includes('Waiting for lifecycle:');
    },
    20000,
    'returned to fixed parent session with loaded transcript',
    parentSessionId,
  );
  await screenshot(cdp, path.join(artifactDir, '06-returned-parent.png'));
  const returnedParentDom = await readLifecycleDom();
  webuiEvidence.returnedParentDom = returnedParentDom;
  await fs.writeFile(
    path.join(artifactDir, 'webui-returned-parent-dom.json'),
    JSON.stringify(returnedParentDom, null, 2),
  );
}

async function verifyHeaderSessionTree(expectedCard) {
  await evalInPage(cdp, () => {
    document.getElementById('close-mobile-agent-sheet-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => document.body.dataset.mobileAgentSheet !== 'open',
    10000,
    'mobile Agent sheet closed before Header tree inspection',
  );
  await evalInPage(cdp, () => {
    document.getElementById('session-relation-toggle-button')?.click();
  });
  const headerTree = await waitForFunction(
    cdp,
    (parentSessionId, workerSessionId, expectedTaskId) => {
      const button = document.getElementById('session-relation-toggle-button');
      const dropdown = document.getElementById('session-tree-dropdown');
      const rect = dropdown?.getBoundingClientRect();
      const nodes = Array.from(document.querySelectorAll('#session-tree .session-tree-node')).map((node) => ({
        kind: node.classList.contains('is-worker') ? 'worker' : 'master',
        relationSchema: node.dataset.relationSchema || '',
        relationSource: node.dataset.relationSource || '',
        sessionId: node.dataset.sessionId || '',
        taskId: node.dataset.taskId || '',
        selected: node.classList.contains('is-selected'),
        text: node.textContent || '',
      }));
      const masterNode = nodes.find((node) => node.kind === 'master' && node.sessionId === parentSessionId);
      const workerNode = nodes.find((node) =>
        node.kind === 'worker' &&
        node.sessionId === workerSessionId &&
        node.taskId === expectedTaskId &&
        node.relationSchema === 'UiTaskSnapshotProjection' &&
        node.relationSource === 'TaskBoard.worker_session_id'
      );
      if (!dropdown || dropdown.hidden || !rect || !masterNode || !workerNode) {
        return false;
      }
      return {
        ariaExpanded: button?.getAttribute('aria-expanded') || '',
        dropdownHidden: dropdown.hidden,
        dropdownHeight: rect.height,
        viewportHeight: window.innerHeight,
        halfScreenOk: rect.height <= (window.innerHeight / 2) + 1,
        masterNode,
        expectedWorkerNode: workerNode,
        nodes,
        text: document.getElementById('session-tree')?.innerText || '',
      };
    },
    20000,
    'Header session tree from protocol schema projection',
    parentSessionId,
    expectedCard.workerSessionId,
    expectedCard.taskId,
  );
  webuiEvidence.headerTree = headerTree;
  await fs.writeFile(path.join(artifactDir, 'webui-header-tree.json'), JSON.stringify(headerTree, null, 2));
  if (headerTree.ariaExpanded !== 'true' || headerTree.dropdownHidden || !headerTree.halfScreenOk) {
    throw new Error(`Header session tree is not an open half-screen dropdown: ${JSON.stringify(headerTree)}`);
  }
  await screenshot(cdp, path.join(artifactDir, '04b-header-session-tree.png'));
  await evalInPage(cdp, (workerSessionId, expectedTaskId) => {
    const nodes = Array.from(document.querySelectorAll('#session-tree .session-tree-node.is-worker'));
    const target = nodes.find((node) => node.dataset.sessionId === workerSessionId && node.dataset.taskId === expectedTaskId);
    target?.click();
  }, expectedCard.workerSessionId, expectedCard.taskId);
}

async function readLifecycleDom() {
  return evalInPage(cdp, () => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    const selectedTurn = shell?.dataset.selectedTurn || '';
    const selectedAssistant = Array.from(document.querySelectorAll('.chat-message-assistant'))
      .find((node) => node.dataset.turnId === selectedTurn) || null;
    return {
      selectedSession: shell?.dataset.selectedSession || '',
      selectedTurn,
      selectedTerminalStatus: shell?.dataset.selectedTerminalStatus || '',
      turnStatus: document.getElementById('turn-status')?.textContent?.trim() || '',
      assistantStatus: selectedAssistant?.querySelector('.chat-state-pill')?.textContent?.trim() || '',
      finalStatus: selectedAssistant?.querySelector('.chat-section-final .chat-row-status')?.textContent?.trim() || '',
      messageText: document.getElementById('message-list')?.innerText || '',
      workerNavHidden: document.getElementById('worker-session-nav')?.hidden ?? true,
    };
  });
}

async function refreshWebUiState() {
  await evalInPage(cdp, () => {
    document.getElementById('refresh-session-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const status = document.getElementById('command-status')?.textContent || '';
      return status.includes('selected session refreshed') || status.includes('task status refresh failed') || status.length > 0;
    },
    30000,
    'WebUI selected session refresh',
  );
  await delay(1000);
}

async function waitForFixtureState(predicate, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) {
      return;
    }
    await delay(250);
  }
  await snapshotWebUiDom(`timeout-${label.replace(/[^a-z0-9]+/gi, '-').toLowerCase()}`).catch(() => null);
  throw new Error(`timeout waiting for ${label}; fixtureState=${JSON.stringify(fixtureState)}`);
}

async function waitForParentCurrentTaskWaiting(timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    const turns = (await queryAdp(
      { QuerySessionTurns: { session_id: parentSessionId } },
      'wait-parent-current-task-waiting',
    )).SessionTurns;
    last = turns;
    if (findParentWaitingTurn(turns)) {
      return;
    }
    await delay(1000);
  }
  await fs.writeFile(
    path.join(artifactDir, 'parent-waiting-timeout.json'),
    JSON.stringify(last, null, 2),
  );
  throw new Error(`parent session never closed current ${taskId} dispatch as claim=waiting`);
}

async function waitForTaskStatus(expectedStatus, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    const board = await queryAdp({ QueryTaskBoard: { include_terminal: true } }, 'wait-task-board');
    const task = ((board.TaskBoard && board.TaskBoard.tasks) || []).find((candidate) => candidate.task_id === taskId);
    last = task || board;
    if (task && `${task.status || ''}`.toLowerCase() === expectedStatus) {
      return task;
    }
    await delay(1000);
  }
  throw new Error(`timeout waiting for ${taskId} status=${expectedStatus}; last=${JSON.stringify(last)}`);
}

async function waitForTaskHistoryEvent(expectedEventType, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    const history = (await queryAdp({ QueryTaskHistory: { task_id: taskId } }, 'wait-task-history')).TaskHistory;
    const events = (history.events || []).map((event) => event.event_type);
    last = events;
    if (events.includes(expectedEventType)) {
      return history;
    }
    await delay(1000);
  }
  throw new Error(`timeout waiting for ${taskId} history event ${expectedEventType}; last=${JSON.stringify(last)}`);
}

async function collectTruth() {
  const taskBoard = (await queryAdp({ QueryTaskBoard: { include_terminal: true } }, 'final-task-board')).TaskBoard;
  const task = (taskBoard.tasks || []).find((candidate) => candidate.task_id === taskId);
  if (!task) {
    throw new Error(`expected task missing from TaskBoard: ${taskId}`);
  }
  if (!task.worker_session_id) {
    throw new Error(
      `TaskBoard projection for ${taskId} is missing UiTaskSnapshotProjection.worker_session_id; verifier must not synthesize worker-task-* ids`,
    );
  }
  const workerSessionId = task.worker_session_id;
  const taskHistory = (await queryAdp({ QueryTaskHistory: { task_id: taskId } }, 'final-task-history')).TaskHistory;
  const parentTurns = (await queryAdp({ QuerySessionTurns: { session_id: parentSessionId } }, 'final-parent-turns')).SessionTurns;
  const workerTurns = (await queryAdp({ QuerySessionTurns: { session_id: workerSessionId } }, 'final-worker-turns')).SessionTurns;
  const events = (taskHistory.events || []).map((event) => event.event_type);
  if (!(parentTurns.turns || []).some((turn) => turn.user_text === prompt)) {
    throw new Error(`parent session does not contain the WebUI-submitted prompt: ${JSON.stringify(parentTurns)}`);
  }
  const parentWaitingTurn = findParentWaitingTurn(parentTurns);
  if (!parentWaitingTurn) {
    throw new Error(`parent session did not persist current-run waiting/lifecycle status after dispatch for ${taskId}: ${JSON.stringify(parentTurns)}`);
  }
  const workerTerminal = (workerTurns.turns || []).find((turn) => `${turn.terminal_status || ''}`.toLowerCase() === 'blocked');
  if (!workerTerminal) {
    throw new Error(`worker session did not persist blocked terminal turn: ${JSON.stringify(workerTurns)}`);
  }
  const terminalText = workerTerminal.terminal_text || '';
  for (const required of [requestedPath, canonicalTargetCwd, `missing_suffix=${missingSuffix}`]) {
    if (!terminalText.includes(required)) {
      throw new Error(`worker terminal text missing ${required}: ${terminalText}`);
    }
  }
  if (!events.includes('TaskCreated') || !events.includes('TaskAssigned') || !events.includes('TaskBlocked')) {
    throw new Error(`task history missing required lifecycle events: ${events.join(',')}`);
  }
  if (!events.includes('TaskProgressed') || !fixtureState.masterLifecycleAppendRequested) {
    throw new Error(`Master lifecycle did not persist a blocked decision through task append: events=${events.join(',')} fixture=${JSON.stringify(fixtureState)}`);
  }
  if (!fixtureState.secondHadToolResult || !fixtureState.secondHadDiagnostic) {
    throw new Error(`fixture did not observe diagnostic tool_result in second worker request: ${JSON.stringify(fixtureState)}`);
  }
  if (fixtureState.masterRequests.length !== 4) {
    throw new Error(`Master fixture must do exactly create, assign, waiting, and blocked-decision append: ${JSON.stringify(fixtureState)}`);
  }
  return { taskBoard, task, taskHistory, parentTurns, parentWaitingTurn, workerTurns, workerSessionId };
}

function buildSummary(truth) {
  const taskEvents = (truth.taskHistory.events || []).map((event) => event.event_type);
  const workerTurns = truth.workerTurns.turns || [];
  const parentTurns = truth.parentTurns.turns || [];
  const workerTerminal = workerTurns[workerTurns.length - 1] || {};
  const sameParentSchemaTasks = ((truth.taskBoard && truth.taskBoard.tasks) || [])
    .filter((candidate) =>
      candidate &&
      (
        candidate.parent_session_id === parentSessionId ||
        (candidate.attached_session_ids || []).includes(parentSessionId)
      )
    );
  const missingWorkerSessionIds = sameParentSchemaTasks
    .filter((candidate) => !candidate.worker_session_id)
    .map((candidate) => candidate.task_id || '<missing-task-id>');
  if (missingWorkerSessionIds.length > 0) {
    throw new Error(
      `TaskBoard same-parent child task(s) missing UiTaskSnapshotProjection.worker_session_id: ${missingWorkerSessionIds.join(', ')}`,
    );
  }
  const schemaChildTasks = sameParentSchemaTasks
    .map((candidate) => ({
      taskId: candidate.task_id,
      workerSessionId: candidate.worker_session_id,
      status: candidate.status,
    }));
  const headerWorkerNodes = (webuiEvidence.headerTree?.nodes || [])
    .filter((node) => node.kind === 'worker')
    .map((node) => ({
      taskId: node.taskId,
      workerSessionId: node.sessionId,
      relationSchema: node.relationSchema,
      relationSource: node.relationSource,
    }));
  const headerCoversEverySchemaChild = schemaChildTasks.every((task) =>
    headerWorkerNodes.some((node) =>
      node.taskId === task.taskId &&
      node.workerSessionId === task.workerSessionId &&
      node.relationSchema === 'UiTaskSnapshotProjection' &&
      node.relationSource === 'TaskBoard.worker_session_id'
    )
  );
  const headerHasNoExtraWorkerProjection = headerWorkerNodes.every((node) =>
    schemaChildTasks.some((task) =>
      task.taskId === node.taskId &&
      task.workerSessionId === node.workerSessionId
    )
  );
  return {
    ok: true,
    artifactDir,
    baseUrl,
    adpUrl,
    parentSessionId,
    taskId,
    workerSessionId: truth.workerSessionId,
    prompt,
    targetCwd,
    canonicalTargetCwd,
    requestedPath,
    fixture: {
      masterRequests: fixtureState.masterRequests.length,
      workerRequests: fixtureState.workerRequests.length,
      secondHadToolResult: fixtureState.secondHadToolResult,
      secondHadDiagnostic: fixtureState.secondHadDiagnostic,
      secondBodyLength: fixtureState.secondBodyLength,
      diagnosticChecks: fixtureState.diagnosticChecks,
      masterLifecycleAppendRequested: fixtureState.masterLifecycleAppendRequested,
      masterLifecycleAppendChecks: fixtureState.masterLifecycleAppendChecks,
    },
    taskStatus: truth.task.status,
    taskEvents,
    parentTurnStatuses: parentTurns.map((turn) => `${turn.turn_id}:${turn.terminal_status}`),
    parentWaitingTurn: {
      turnId: truth.parentWaitingTurn.turn_id,
      terminalStatus: truth.parentWaitingTurn.terminal_status,
      terminalText: truth.parentWaitingTurn.terminal_text || '',
    },
    workerTurnStatuses: workerTurns.map((turn) => `${turn.turn_id}:${turn.terminal_status}`),
    workerTerminalText: workerTerminal.terminal_text || '',
    webuiEvidence,
    schemaChildTasks,
    headerWorkerNodes,
    checks: {
      webuiSubmittedFixedParentSession: true,
      masterDispatchUsedWaitingClaim: !!truth.parentWaitingTurn,
      taskBlockedInOwnerTruth: truth.task.status === 'blocked' && taskEvents.includes('TaskBlocked'),
      masterLifecyclePersistedBlockedDecision: taskEvents.includes('TaskProgressed') &&
        fixtureState.masterLifecycleAppendRequested,
      workerSessionPersistedBlockedDiagnostic: `${workerTerminal.terminal_status || ''}` === 'Blocked' &&
        `${workerTerminal.terminal_text || ''}`.includes(requestedPath) &&
        `${workerTerminal.terminal_text || ''}`.includes(canonicalTargetCwd),
      fixtureSecondRequestHadPathDiagnostic: fixtureState.secondHadToolResult && fixtureState.secondHadDiagnostic,
      headerTreeIsHalfScreenDropdown: webuiEvidence.headerTree?.halfScreenOk === true,
      headerTreeUsesTaskBoardSchema: webuiEvidence.headerTree?.expectedWorkerNode?.relationSchema === 'UiTaskSnapshotProjection' &&
        webuiEvidence.headerTree?.expectedWorkerNode?.relationSource === 'TaskBoard.worker_session_id' &&
        webuiEvidence.headerTree?.expectedWorkerNode?.sessionId === truth.workerSessionId &&
        webuiEvidence.headerTree?.expectedWorkerNode?.taskId === taskId,
      headerWorkerClickSelectedProjectedSession: webuiEvidence.workerDom?.selectedSession === truth.workerSessionId,
      headerBackRestoredExactParent: webuiEvidence.returnedParentDom?.selectedSession === parentSessionId &&
        webuiEvidence.returnedParentDom?.workerNavHidden === true,
      headerTreeCoversEverySchemaChild: headerCoversEverySchemaChild,
      headerTreeHasNoExtraWorkerProjection: headerHasNoExtraWorkerProjection,
    },
    screenshots: [
      path.join(artifactDir, '01-parent-before-submit.png'),
      path.join(artifactDir, '02-parent-after-submit.png'),
      path.join(artifactDir, '03-parent-after-worker-blocked.png'),
      path.join(artifactDir, '04-mobile-agent-sheet.png'),
      path.join(artifactDir, '04b-header-session-tree.png'),
      path.join(artifactDir, '05-worker-session-detail.png'),
      path.join(artifactDir, '06-returned-parent.png'),
    ],
  };
}

function findParentWaitingTurn(parentTurns) {
  return (parentTurns.turns || []).find((turn) =>
    turn.user_text === prompt &&
    `${turn.terminal_status || ''}` === 'ToolPending' &&
    `${turn.terminal_text || ''}`.includes(taskId),
  );
}

async function startFixtureServer(port) {
  const server = http.createServer((req, res) => {
    let body = '';
    req.on('data', (chunk) => {
      body += chunk;
    });
    req.on('end', () => {
      try {
        const parsed = JSON.parse(body);
        const model = `${parsed.model || ''}`;
        const route = model === masterModel ? 'master' : model === workerModel ? 'worker' : 'unknown';
        if (route === 'unknown') {
          throw new Error(`unexpected fixture model ${model || '<missing>'}`);
        }
        const response = route === 'master' ? nextMasterResponse(body) : nextWorkerResponse(body);
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify(response));
      } catch (error) {
        res.writeHead(500, { 'content-type': 'application/json' });
        res.end(JSON.stringify({
          type: 'error',
          error: {
            type: 'fixture_error',
            message: error instanceof Error ? error.message : String(error),
          },
        }));
      }
    });
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, '127.0.0.1', () => {
      server.off('error', reject);
      resolve();
    });
  });
  return server;
}

function nextMasterResponse(body) {
  fixtureState.masterRequests.push({
    step: fixtureState.masterStep + 1,
    bodyLength: body.length,
    hasPrompt: body.includes(requestedPath),
    isLifecycleCoordinator: body.includes('production Master lifecycle coordinator'),
    hasTaskBlocked: body.includes('TaskBlocked') || body.includes('execution_blocked'),
    hasAttentionResolution: body.includes('freehand_attention_resolution'),
  });
  fixtureState.masterStep += 1;
  if (fixtureState.masterStep === 1) {
    return anthropicToolUse('toolu_webui_path_create', 'task', {
      op: 'create',
      task_id: taskId,
      title: 'WebUI path diagnostic worker',
      content: [
        `Inspect requested path ${requestedPath}.`,
        `Use target cwd ${targetCwd}.`,
        'Call ls on the requested path. If it fails, report the exact path_diagnostic fields returned by the tool.',
        'Do not guess that the symlink was not expanded.',
      ].join('\n'),
      goal: 'Verify path-tool diagnostics for a missing leaf under a symlink parent.',
      deliverables: ['Worker result with requested path, canonical nearest parent, missing suffix, and symlink ancestor evidence.'],
      acceptance: [
        `result mentions requested=${requestedPath}`,
        `result mentions nearest_existing_canonical=${canonicalTargetCwd}`,
        `result mentions missing_suffix=${missingSuffix}`,
      ],
      target_cwd: targetCwd,
      dispatch: { mode: 'none' },
      priority: 99,
    });
  }
  if (fixtureState.masterStep === 2) {
    return anthropicToolUse('toolu_webui_path_assign', 'task', {
      op: 'assign',
      task_id: taskId,
      agent_id: 'worker',
    });
  }
  if (fixtureState.masterStep === 3) {
    return anthropicText(
      [
        'Worker task dispatched. Waiting for Task Center truth before final user completion.',
        completionBlock({
          claim: 'waiting',
          next_step: `Inspect TaskBoard/TaskHistory for ${taskId}; when the Worker result is ready, review whether it proves requested=${requestedPath} and nearest_existing_canonical=${canonicalTargetCwd}.`,
        }),
      ].join('\n'),
    );
  }
  if (fixtureState.masterStep === 4) {
    const checks = {
      lifecycleCoordinator: body.includes('production Master lifecycle coordinator'),
      taskId: body.includes(taskId),
      taskBlocked: body.includes('TaskBlocked') || body.includes('execution_blocked'),
      pathDiagnostic: body.includes('path_diagnostic'),
      requested: body.includes(requestedPath),
      nearestExistingCanonical: body.includes(canonicalTargetCwd),
      missingSuffix: body.includes(missingSuffix),
    };
    fixtureState.masterLifecycleAppendChecks = checks;
    if (!Object.values(checks).every(Boolean)) {
      throw new Error(`master lifecycle request missing blocked decision context: ${JSON.stringify(checks)}`);
    }
    fixtureState.masterLifecycleAppendRequested = true;
    return anthropicToolUse('toolu_webui_path_blocked_decision_append', 'task', {
      op: 'append',
      task_id: taskId,
      note: [
        `blocked_decision: Worker path diagnostic proves requested=${requestedPath} is blocked because the leaf is missing.`,
        `nearest_existing=${targetCwd}; nearest_existing_canonical=${canonicalTargetCwd}; missing_suffix=${missingSuffix}.`,
        'Required external action: provide or create the requested repository path before retrying; do not broad-search or guess a replacement path.',
      ].join(' '),
    });
  }
  throw new Error(`master sequence exhausted at step ${fixtureState.masterStep}`);
}

function nextWorkerResponse(body) {
  fixtureState.workerRequests.push({
    step: fixtureState.workerStep + 1,
    bodyLength: body.length,
    hasDiagnostic: body.includes('path_diagnostic'),
    hasToolResult: body.includes('tool_result'),
  });
  fixtureState.workerStep += 1;
  if (fixtureState.workerStep === 1) {
    return anthropicToolUse('toolu_webui_path_ls', 'ls', { path: requestedPath });
  }
  if (fixtureState.workerStep === 2) {
    const checks = {
      pathDiagnostic: body.includes('path_diagnostic'),
      toolResult: body.includes('tool_result'),
      requested: body.includes(requestedPath),
      absolute: body.includes(`absolute=\`${requestedPath}\``) || body.includes(`absolute=${requestedPath}`) || body.includes(requestedPath),
      nearestExisting: body.includes('nearest_existing=') && body.includes(targetCwd),
      nearestExistingCanonical: body.includes(canonicalTargetCwd),
      missingSuffix: body.includes(`missing_suffix=\`${missingSuffix}\``) || body.includes(`missing_suffix=${missingSuffix}`),
      symlinkAncestor: body.includes(`${targetCwd}`) && body.includes(canonicalTargetCwd),
    };
    fixtureState.secondHadToolResult = checks.toolResult;
    fixtureState.secondHadDiagnostic = Object.values(checks).every(Boolean);
    fixtureState.secondBodyLength = body.length;
    fixtureState.diagnosticChecks = checks;
    if (!fixtureState.secondHadDiagnostic) {
      throw new Error(`worker second request missing path diagnostic fields: ${JSON.stringify(checks)}`);
    }
    return anthropicText(
      [
        'Path diagnostic observed from the built-in ls tool.',
        completionBlock({
          claim: 'blocked',
          blocked_reason: `path_diagnostic requested=${requestedPath}; nearest_existing=${targetCwd}; nearest_existing_canonical=${canonicalTargetCwd}; missing_suffix=${missingSuffix}; symlink_ancestor=${targetCwd}->${canonicalTargetCwd}. Requested leaf does not exist, so the Worker is blocked with tool-owned evidence instead of guessing.`,
        }),
      ].join('\n'),
    );
  }
  throw new Error(`worker sequence exhausted at step ${fixtureState.workerStep}`);
}

function anthropicToolUse(id, name, input) {
  return {
    content: [{ type: 'tool_use', id, name, input }],
    usage: { input_tokens: 100, output_tokens: 40 },
    stop_reason: 'tool_use',
  };
}

function anthropicText(text) {
  return {
    content: [{ type: 'text', text }],
    usage: { input_tokens: 120, output_tokens: 80 },
    stop_reason: 'end_turn',
  };
}

function completionBlock(value) {
  return `<freehand_completion>\n${JSON.stringify(value)}\n</freehand_completion>`;
}

async function queryAdp(query, label) {
  const requestId = `${label}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(adpUrl);
    const timeout = setTimeout(() => {
      socket.close();
      reject(new Error(`timeout waiting for ADP query ${label}`));
    }, 30000);
    socket.addEventListener('open', () => {
      socket.send(JSON.stringify({ kind: 'query', request_id: requestId, query }));
    });
    socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data);
      if (message.request_id !== requestId) {
        return;
      }
      clearTimeout(timeout);
      socket.close();
      if (message.error) {
        reject(new Error(`ADP query ${label} failed: ${JSON.stringify(message.error)}`));
      } else {
        resolve(message.result);
      }
    });
    socket.addEventListener('error', () => {
      clearTimeout(timeout);
      reject(new Error(`ADP socket error for ${label}`));
    });
  });
}

async function restoreRuntime() {
  if (originalConfig !== null) {
    await fs.writeFile(configPath, originalConfig);
  }
  if (originalDaemonEnv !== null) {
    await fs.writeFile(daemonEnvPath, originalDaemonEnv);
  }
  if (originalWorkerEnv !== null) {
    await fs.writeFile(workerEnvPath, originalWorkerEnv);
  }
  if (originalConfig !== null && originalDaemonEnv !== null) {
    await must(['scripts/install-launchd.sh', 'restartS']);
    await waitHealth('after restore');
    await must(['scripts/install-launchd.sh', 'restartWorkerS']);
  }
  const config = await must([cli, 'adp-config-query', '--url', adpUrl]);
  const envGrep = await run([
    'grep',
    '-n',
    'FREEHAND_WEBUI_PATH_DIAGNOSTIC_FIXTURE_KEY\\|FREEHAND_PROVIDER_RETRY_FIXTURE_KEY\\|FREEHAND_PROVIDER_RETRY_BACKOFF_MS',
    daemonEnvPath,
    workerEnvPath,
  ]);
  await fs.writeFile(path.join(artifactDir, 'config.after.txt'), config.stdout);
  await fs.writeFile(path.join(artifactDir, 'fixture-env-grep.after.txt'), envGrep.stdout);
}

function stripFixtureEnv(input) {
  return `${input || ''}`
    .split(/\n/)
    .filter((line) => !line.startsWith(`${fixtureKeyName}=`))
    .join('\n')
    .trimEnd();
}

function redactEnv(input) {
  return `${input || ''}`
    .split(/\n/)
    .map((line) => {
      if (/(_KEY|_SECRET|_CREDENTIAL)=/.test(line)) {
        const [key] = line.split('=');
        return `${key}="<redacted>"`;
      }
      return line;
    })
    .join('\n');
}

function redactConfig(input) {
  return `${input || ''}`.replace(/(api_key\s*=\s*)".*?"/g, '$1"<redacted>"');
}

async function waitHealth(label) {
  const deadline = Date.now() + 90000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(healthUrl);
      if (response.ok && (await response.text()).trim() === 'ok') {
        return;
      }
    } catch (_) {
      // wait for daemon health
    }
    await delay(1000);
  }
  throw new Error(`daemon did not become healthy ${label}: ${healthUrl}`);
}

async function must(argv) {
  const result = await run(argv);
  if (result.code !== 0) {
    throw new Error(`command failed (${result.code}): ${argv.join(' ')}\nstdout=${result.stdout}\nstderr=${result.stderr}`);
  }
  return result;
}

function run(argv) {
  return new Promise((resolve) => {
    const child = spawn(argv[0], argv.slice(1), { cwd: repo, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    child.on('close', (code) => {
      resolve({ code, stdout: stdout.trim(), stderr: stderr.trim() });
    });
    child.on('error', (error) => {
      resolve({ code: 127, stdout: stdout.trim(), stderr: `${stderr}\n${error.message}`.trim() });
    });
  });
}

async function assertFileExists(file, label) {
  const stat = await fs.stat(file).catch(() => null);
  if (!stat || !stat.isFile()) {
    throw new Error(`missing ${label}: ${file}`);
  }
}

async function getFreePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const address = server.address();
  const port = address && typeof address === 'object' ? address.port : null;
  await new Promise((resolve) => server.close(resolve));
  if (!port) {
    throw new Error('failed to allocate a local port');
  }
  return port;
}

async function waitForPageTarget(url, timeoutMs) {
  const expected = new URL(url);
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
      const targets = await response.json();
      const target = targets.find((item) => {
        if (item.type !== 'page' || !item.webSocketDebuggerUrl) {
          return false;
        }
        try {
          const itemUrl = new URL(item.url);
          return itemUrl.origin === expected.origin;
        } catch (_) {
          return false;
        }
      });
      if (target) {
        return target;
      }
    } catch (_) {
      // wait for Chrome DevTools
    }
    await delay(250);
  }
  throw new Error('timeout waiting for Chrome DevTools page target');
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
      const message = JSON.parse(event.data);
      if (message.id && pending.has(message.id)) {
        const { resolve: resolvePending, reject: rejectPending } = pending.get(message.id);
        pending.delete(message.id);
        if (message.error) {
          rejectPending(new Error(JSON.stringify(message.error)));
        } else {
          resolvePending(message.result || {});
        }
        return;
      }
      if (message.method) {
        listeners.forEach((listener) => listener(message.method, message.params || {}));
      }
    });
    socket.addEventListener('error', () => reject(new Error('CDP websocket error')));
  });
}

async function waitForLoad(client) {
  await new Promise((resolve) => {
    const timeout = setTimeout(resolve, 5000);
    const onEvent = (method) => {
      if (method === 'Page.loadEventFired') {
        clearTimeout(timeout);
        client.offEvent(onEvent);
        resolve();
      }
    };
    client.onEvent(onEvent);
  });
  await delay(500);
}

async function waitForFunction(client, fn, timeoutMs, label, ...args) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = await evalInPage(client, fn, ...args);
    if (result) {
      return result;
    }
    await delay(250);
  }
  throw new Error(`timeout waiting for ${label}`);
}

async function evalInPage(client, fn, ...args) {
  const expression = `(${fn.toString()})(...${JSON.stringify(args)})`;
  const response = await client.send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    throw new Error(response.exceptionDetails.text || 'Runtime.evaluate failed');
  }
  return response.result.value;
}

async function snapshotWebUiDom(label) {
  if (!cdp) {
    return;
  }
  const state = await evalInPage(cdp, () => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    return {
      selectedSession: shell?.dataset.selectedSession || '',
      selectedTurn: shell?.dataset.selectedTurn || '',
      selectedTerminalStatus: shell?.dataset.selectedTerminalStatus || '',
      commandStatus: document.getElementById('command-status')?.textContent || '',
      composerValue: document.getElementById('composer-input')?.value || '',
      sendDisabled: document.getElementById('send-button')?.disabled || false,
      pendingCards: document.querySelectorAll('[data-turn-id="pending-submit"]').length,
      messageText: document.getElementById('message-list')?.innerText || '',
      mobileAgentTitle: document.getElementById('mobile-agent-summary-title')?.textContent || '',
      mobileAgentCopy: document.getElementById('mobile-agent-summary-copy')?.textContent || '',
    };
  });
  await fs.writeFile(path.join(artifactDir, `${label}.dom.json`), JSON.stringify(state, null, 2));
}

async function screenshot(client, file) {
  const result = await client.send('Page.captureScreenshot', { format: 'png', fromSurface: true });
  await fs.writeFile(file, Buffer.from(result.data, 'base64'));
}

function waitForProcessExit(child, timeoutMs) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('timeout waiting for process exit')), timeoutMs);
    child.once('exit', () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
