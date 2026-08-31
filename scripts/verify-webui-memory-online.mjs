import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { adpVerifierRequest, requireQueryVariant } from './lib/adp-verifier-client.mjs';

const repo = process.cwd();
const home = process.env.HOME;
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(home, '.freehand');
const baseUrl = new URL(process.env.FREEHAND_WEBUI_MEMORY_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_WEBUI_MEMORY_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const authToken = process.env.FREEHAND_ADP_AUTH_TOKEN || '';
const cli = process.env.FREEHAND_WEBUI_MEMORY_CLI || path.join(home, '.local/bin/freehand-cliS');
const chromePath =
  process.env.FREEHAND_WEBUI_MEMORY_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_MEMORY_DEBUG_PORT || '9243', 10);
const runId = `webui-memory-${Date.now()}`;
const sessionId = `webui-memory-${runId}`;
const artifactDir = path.join(repo, 'artifacts', 'webui-online', runId);
const memoryPath = process.env.FREEHAND_WEBUI_MEMORY_PATH ||
  path.join(runtimeHome, 'memory', 'tool-results.jsonl');
const searchTerm = `complete-${runId}`;
const entries = [
  `# Memory SQLite Alpha\n\n\`\`\`markdown\nalpha ${searchTerm} tool output\n\`\`\``,
  `# Memory SQLite Beta\n\n\`\`\`markdown\nbeta ${searchTerm} tool output\n\`\`\``,
  `# Memory SQLite Gamma\n\n\`\`\`markdown\ngamma ${searchTerm} tool output\n\`\`\``,
];

let chrome;
let cdp;
let sessionCreated = false;
const browserErrors = [];

await fs.mkdir(artifactDir, { recursive: true });

try {
  await waitHealth();
  await adpCommand({
    CreateSession: {
      session_id: sessionId,
      title: `SQLite memory online ${runId}`,
    },
  });
  sessionCreated = true;
  for (let index = 0; index < entries.length; index += 1) {
    await adpCommand({
      AddToMemory: {
        session_id: sessionId,
        turn_id: `runtime-turn-memory-${index + 1}`,
        tool_call_id: `memory-tool-${index + 1}`,
        content: entries[index],
      },
    });
  }

  const recent = await queryMemory({ query: searchTerm, sort: 'recent', limit: 2 });
  const oldest = await queryMemory({ query: searchTerm, sort: 'oldest', limit: 2 });
  const firstPage = await queryMemory({ query: searchTerm, sort: 'relevance', limit: 2 });
  const secondPage = await queryMemory({
    query: searchTerm,
    sort: 'relevance',
    limit: 2,
    offset: 2,
  });
  const databaseStat = await fs.stat(`${memoryPath}.sqlite3`);

  await openBrowser();
  const browserBeforeDelete = await inspectMemorySurface(searchTerm);

  await adpCommand({ DeleteSession: { session_id: sessionId } });
  const afterDelete = await queryMemory({ query: searchTerm, sort: 'oldest', limit: 100 });
  await restartService();
  const afterRestart = await queryMemory({ query: searchTerm, sort: 'oldest', limit: 100 });

  const summary = {
    ok: true,
    runId,
    sessionId,
    baseUrl: baseUrl.toString(),
    adpUrl,
    memoryPath,
    sqlitePath: `${memoryPath}.sqlite3`,
    databaseBytes: databaseStat.size,
    recent: projectPage(recent),
    oldest: projectPage(oldest),
    firstPage: projectPage(firstPage),
    secondPage: projectPage(secondPage),
    browserBeforeDelete,
    afterDelete: projectPage(afterDelete),
    afterRestart: projectPage(afterRestart),
    checks: {
      sqliteFileCreated: databaseStat.isFile() && databaseStat.size > 0,
      keywordSearch: recent.entries.length === 2 && recent.total_matching === 3,
      recentSort: recent.entries[0]?.tool_call_id === 'memory-tool-3',
      oldestSort: oldest.entries[0]?.tool_call_id === 'memory-tool-1',
      relevancePageOne: firstPage.entries.length === 2 && firstPage.has_older,
      relevancePageTwo: secondPage.entries.length === 1 &&
        (secondPage.next_offset === null || secondPage.next_offset === undefined),
      browserMemorySurface: browserBeforeDelete.cardCount === 3 &&
        browserBeforeDelete.hasAlphaMarkdown &&
        browserBeforeDelete.hasCopyButtons,
      survivesSessionDelete: afterDelete.total_matching === 3,
      survivesDaemonRestart: afterRestart.total_matching === 3 &&
        afterRestart.entries.every((entry) => entries.includes(entry.content)),
    },
  };
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  const failed = Object.entries(summary.checks).filter(([, value]) => !value);
  if (failed.length > 0) {
    throw new Error(`webui memory checks failed: ${failed.map(([key]) => key).join(', ')}`);
  }
} finally {
  if (sessionCreated) {
    await adpCommand({ DeleteSession: { session_id: sessionId } }).catch(() => null);
  }
  if (cdp) {
    try {
      cdp.close();
    } catch {
      // Closing an already-failed CDP socket must not mask the verifier error.
    }
  }
  if (chrome?.pid && chrome.exitCode === null) {
    chrome.kill('SIGTERM');
    await new Promise((resolve) => chrome.once('exit', resolve));
  }
}

function adpUrlFromBaseUrl(url) {
  const result = new URL('/adp', url);
  result.protocol = result.protocol === 'https:' ? 'wss:' : 'ws:';
  return result.toString();
}

async function adpCommand(command) {
  return adpVerifierRequest({
    url: adpUrl,
    authToken,
    kind: 'command',
    payloadKey: 'command',
    payload: command,
    clientName: 'freehand-webui-memory-verifier',
  });
}

async function adpQuery(query) {
  return adpVerifierRequest({
    url: adpUrl,
    authToken,
    kind: 'query',
    payloadKey: 'query',
    payload: query,
    clientName: 'freehand-webui-memory-verifier',
  });
}

async function queryMemory({ query, sort, limit, offset }) {
  const result = await adpQuery({
    QueryMemory: {
      query,
      sort,
      limit,
      ...(offset === undefined ? {} : { offset }),
    },
  });
  return requireQueryVariant(result, 'Memory', 'memory query');
}

function projectPage(page) {
  return {
    total_matching: page.total_matching,
    has_older: page.has_older,
    next_offset: page.next_offset ?? null,
    entries: (page.entries || []).map((entry) => ({
      id: entry.id,
      session_id: entry.session_id,
      tool_call_id: entry.tool_call_id,
      content: entry.content,
    })),
  };
}

async function inspectMemorySurface(query) {
  await clickElement('open-session-drawer-button');
  await waitForFunction(
    () => document.getElementById('open-memory-button'),
    10_000,
    'memory entry',
  );
  await clickElement('open-memory-button');
  try {
    await waitForFunction(
      () => document.getElementById('memory-dialog')?.open === true,
      10_000,
      'memory dialog',
    );
  } catch (error) {
    const state = await evalInPage(() => ({
      readyState: document.readyState,
      route: document.body.dataset.webuiRoute || null,
      button: document.getElementById('open-memory-button')?.outerHTML || null,
      dialog: document.getElementById('memory-dialog')?.outerHTML.slice(0, 500) || null,
      dialogOpen: document.getElementById('memory-dialog')?.open || false,
      commandStatus: document.getElementById('command-status')?.textContent || null,
      buttonRect: (() => {
        const rect = document.getElementById('open-memory-button')?.getBoundingClientRect();
        return rect ? { left: rect.left, top: rect.top, width: rect.width, height: rect.height } : null;
      })(),
    }));
    await fs.writeFile(
      path.join(artifactDir, 'memory-dialog-failure.json'),
      JSON.stringify({ ...state, browserErrors }, null, 2),
    );
    throw error;
  }
  await cdp.send('Runtime.evaluate', {
    expression: `(() => {
      const input = document.getElementById('memory-query-input');
      input.value = ${JSON.stringify(query)};
      input.dispatchEvent(new Event('input', { bubbles: true }));
    })()`,
  });
  await clickElement('memory-submit-button');
  await waitForFunction(
    () => document.querySelectorAll('.memory-card').length === 3,
    20_000,
    'memory cards',
  );
  return await evalInPage((term) => ({
    cardCount: document.querySelectorAll('.memory-card').length,
    hasAlphaMarkdown: Array.from(document.querySelectorAll('.memory-card-content'))
      .some((node) => node.textContent.includes('```markdown') && node.textContent.includes(term)),
    hasCopyButtons: Array.from(document.querySelectorAll('.memory-card'))
      .every((card) => card.querySelectorAll('.memory-card-copy').length === 1),
  }), searchTerm);
}

async function clickElement(id) {
  const bounds = await evalInPage((elementId) => {
    const element = document.getElementById(elementId);
    if (!element) return null;
    const rect = element.getBoundingClientRect();
    return {
      x: rect.left + rect.width / 2,
      y: rect.top + rect.height / 2,
    };
  }, id);
  if (!bounds) throw new Error(`missing clickable element: ${id}`);
  await cdp.send('Input.dispatchMouseEvent', {
    type: 'mousePressed',
    x: bounds.x,
    y: bounds.y,
    button: 'left',
    clickCount: 1,
  });
  await cdp.send('Input.dispatchMouseEvent', {
    type: 'mouseReleased',
    x: bounds.x,
    y: bounds.y,
    button: 'left',
    clickCount: 1,
  });
}

async function openBrowser() {
  const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-webui-memory-chrome-'));
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
    { stdio: ['ignore', 'ignore', 'ignore'] },
  );
  const target = await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
    if (!response.ok) return null;
    return (await response.json()).find((item) => item.type === 'page') || null;
  }, 20_000, 'Chrome DevTools page');
  cdp = await createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.navigate', { url: baseUrl.toString() });
  await waitForFunction(
    () => document.querySelector('[data-webui-shell="true"]') && document.getElementById('message-list'),
    30_000,
    'WebUI shell',
  );
}

async function createCdpClient(webSocketUrl) {
  const socket = new WebSocket(webSocketUrl);
  const pending = new Map();
  let nextId = 0;
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve);
    socket.addEventListener('error', () => reject(new Error('CDP socket error')));
  });
  socket.addEventListener('message', (event) => {
    const payload = JSON.parse(event.data);
    if (payload.method === 'Runtime.exceptionThrown') {
      browserErrors.push(payload.params.exceptionDetails?.text || 'Runtime exception');
      return;
    }
    if (payload.method === 'Runtime.consoleAPICalled') {
      const values = payload.params.args || [];
      browserErrors.push(`console.${payload.params.type}: ${values.map((value) => value.value ?? value.description ?? '').join(' ')}`);
      return;
    }
    if (!payload.id) return;
    const request = pending.get(payload.id);
    if (!request) return;
    pending.delete(payload.id);
    if (payload.error) request.reject(new Error(payload.error.message));
    else request.resolve(payload.result || {});
  });
  return {
    send(method, params = {}) {
      const id = ++nextId;
      socket.send(JSON.stringify({ id, method, params }));
      return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
    },
    close() {
      socket.close();
    },
  };
}

async function evalInPage(fn, ...args) {
  const response = await cdp.send('Runtime.evaluate', {
    expression: `(${fn})(...${JSON.stringify(args)})`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    throw new Error(response.exceptionDetails.text || 'page evaluation failed');
  }
  return response.result.value;
}

async function waitForFunction(fn, timeoutMs, label, ...args) {
  return waitFor(() => evalInPage(fn, ...args), timeoutMs, label);
}

async function waitFor(fn, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await Promise.resolve(fn()).catch(() => null);
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`timeout waiting for ${label}`);
}

async function waitHealth() {
  await waitFor(async () => {
    const response = await fetch(new URL('/health', baseUrl));
    return response.ok;
  }, 60_000, 'daemon health');
}

async function restartService() {
  const result = await run(['scripts/install-launchd.sh', 'restartS']);
  if (result.code !== 0) {
    throw new Error(`restartS failed: ${result.stderr || result.stdout}`);
  }
  await waitHealth();
}

function run(argv) {
  return new Promise((resolve) => {
    const child = spawn(argv[0], argv.slice(1), { cwd: repo, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('close', (code) => resolve({ code, stdout, stderr }));
  });
}
