import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const chromePath = process.env.FREEHAND_WEBUI_CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WORKER_SUBTASKS_WEBUI_DEBUG_PORT || '9237', 10);
const baseUrl = process.env.FREEHAND_WORKER_SUBTASKS_WEBUI_BASE_URL || 'http://127.0.0.1:4042/?verify=worker-subtasks';
const artifactRoot = process.env.FREEHAND_WORKER_SUBTASKS_WEBUI_ARTIFACT_DIR ||
  path.join(process.cwd(), 'artifacts', 'webui-online', `worker-subtasks-${Date.now()}`);

await fs.mkdir(artifactRoot, { recursive: true });
const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-worker-subtasks-webui-'));

let chrome;
let cdp;
try {
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

  const target = await waitForPageTarget(baseUrl, 15_000);
  cdp = await createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
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
      return !!shell && !!document.getElementById('open-mobile-agent-sheet-button');
    },
    20_000,
    'WebUI shell ready',
  );

  await waitForFunction(
    cdp,
    () => {
      const title = document.getElementById('mobile-agent-summary-title')?.textContent || '';
      return /delegated task/.test(title) && !/unavailable/i.test(title);
    },
    20_000,
    'mobile agent summary loaded',
  );

  const parentState = await evalInPage(cdp, () => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    return {
      selectedSession: shell?.dataset.selectedSession || '',
      selectedTurn: shell?.dataset.selectedTurn || '',
      summaryTitle: document.getElementById('mobile-agent-summary-title')?.textContent || '',
      summaryCopy: document.getElementById('mobile-agent-summary-copy')?.textContent || '',
    };
  });

  await evalInPage(cdp, () => {
    document.getElementById('open-mobile-agent-sheet-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const sheet = document.getElementById('mobile-agent-sheet');
      const rect = sheet?.getBoundingClientRect();
      return document.body.dataset.mobileAgentSheet === 'open' &&
        sheet?.getAttribute('aria-hidden') === 'false' &&
        document.querySelectorAll('#mobile-agent-task-list .mobile-agent-card').length > 0 &&
        !!rect &&
        rect.height >= 120 &&
        rect.top <= window.innerHeight - 80 &&
        rect.bottom >= window.innerHeight - 20;
    },
    10_000,
    'mobile agent sheet with task cards and settled geometry',
  );
  await delay(800);
  await screenshot(cdp, path.join(artifactRoot, '01-mobile-agent-sheet.png'));

  const sheetState = await evalInPage(cdp, () => {
    const cards = Array.from(document.querySelectorAll('#mobile-agent-task-list .mobile-agent-card'));
    return {
      taskStatus: document.getElementById('mobile-agent-task-status')?.textContent || '',
      cardCount: cards.length,
      cards: cards.map((card) => ({
        title: card.querySelector('.mobile-agent-card-title')?.textContent || '',
        meta: card.querySelector('.mobile-agent-card-meta')?.textContent || '',
        copy: card.querySelector('.mobile-agent-card-copy')?.textContent || '',
        taskId: card.dataset.taskId || '',
        workerSessionId: card.dataset.workerSessionId || '',
      })),
      sheetText: document.getElementById('mobile-agent-sheet')?.innerText || '',
    };
  });

  if (sheetState.cardCount < 1) {
    throw new Error('expected at least one delegated task card in current session');
  }
  const missingIdentity = sheetState.cards.filter((card) => !card.taskId || !card.workerSessionId);
  if (missingIdentity.length > 0) {
    throw new Error(`delegated task cards missing identity fields: ${JSON.stringify(missingIdentity)}`);
  }

  await evalInPage(cdp, () => {
    document.querySelector('#mobile-agent-task-list .mobile-agent-card')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const shell = document.querySelector('[data-webui-shell="true"]');
      const selected = shell?.dataset.selectedSession || '';
      const workerNavHidden = document.getElementById('worker-session-nav')?.hidden;
      const text = document.getElementById('message-list')?.innerText || '';
      const loading = text.includes('Loading conversation');
      const sheet = document.getElementById('mobile-agent-sheet');
      const sheetRect = sheet?.getBoundingClientRect();
      const sheetClosed = document.body.dataset.mobileAgentSheet !== 'open' &&
        sheet?.getAttribute('aria-hidden') === 'true' &&
        !!sheetRect &&
        sheetRect.top >= window.innerHeight - 10;
      return selected.startsWith('worker-task-') &&
        workerNavHidden === false &&
        sheetClosed &&
        (loading || text.length > 0);
    },
    20_000,
    'worker transcript selected and Agent sheet closed',
  );
  await waitForFunction(
    cdp,
    () => {
      const text = document.getElementById('message-list')?.innerText || '';
      return !text.includes('Loading conversation');
    },
    30_000,
    'worker transcript loaded',
  );
  await screenshot(cdp, path.join(artifactRoot, '02-worker-transcript.png'));

  const workerState = await evalInPage(cdp, () => {
    const shell = document.querySelector('[data-webui-shell="true"]');
    const text = document.getElementById('message-list')?.innerText || '';
    return {
      selectedSession: shell?.dataset.selectedSession || '',
      workerNavHidden: document.getElementById('worker-session-nav')?.hidden ?? true,
      userMessageCount: document.querySelectorAll('.message.user').length,
      messageText: text,
      fakePromptVisible:
        text.includes('Execute the assigned Task Center task') ||
        text.includes('The tool result has been returned') ||
        text.includes('Task ID:'),
    };
  });
  if (workerState.fakePromptVisible) {
    throw new Error('worker transcript exposes internal task/continuation prompt text');
  }

  await evalInPage(cdp, () => {
    document.getElementById('worker-session-back-button')?.click();
  });
  await waitForFunction(
    cdp,
    (parentSessionId) => {
      const shell = document.querySelector('[data-webui-shell="true"]');
      return (shell?.dataset.selectedSession || '') === parentSessionId &&
        document.getElementById('worker-session-nav')?.hidden === true;
    },
    20_000,
    'returned to parent session',
    parentState.selectedSession,
  );
  await screenshot(cdp, path.join(artifactRoot, '03-returned-parent.png'));

  const result = {
    ok: true,
    baseUrl,
    artifactRoot,
    parentState,
    sheetState,
    workerState: {
      selectedSession: workerState.selectedSession,
      workerNavHidden: workerState.workerNavHidden,
      userMessageCount: workerState.userMessageCount,
      fakePromptVisible: workerState.fakePromptVisible,
    },
    screenshots: [
      path.join(artifactRoot, '01-mobile-agent-sheet.png'),
      path.join(artifactRoot, '02-worker-transcript.png'),
      path.join(artifactRoot, '03-returned-parent.png'),
    ],
  };
  await fs.writeFile(path.join(artifactRoot, 'summary.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} catch (error) {
  const failure = {
    ok: false,
    error: error instanceof Error ? error.message : String(error),
    artifactRoot,
  };
  await fs.writeFile(path.join(artifactRoot, 'failure.json'), JSON.stringify(failure, null, 2));
  console.error(JSON.stringify(failure, null, 2));
  process.exitCode = 1;
} finally {
  if (cdp) {
    await cdp.close().catch(() => null);
  }
  if (chrome && !chrome.killed) {
    chrome.kill('SIGTERM');
    await waitForProcessExit(chrome, 5_000).catch(() => null);
  }
}

async function waitForPageTarget(url, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  const expected = new URL(url);
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
      const targets = await response.json();
      const target = targets.find((item) => {
        if (item.type !== 'page' || !item.webSocketDebuggerUrl) {
          return false;
        }
        try {
          const targetUrl = new URL(item.url);
          return targetUrl.origin === expected.origin;
        } catch (_) {
          return false;
        }
      });
      if (target) {
        return target;
      }
    } catch (error) {
      lastError = error;
    }
    await delay(250);
  }
  throw new Error(`Chrome page target not available: ${lastError?.message || 'timeout'}`);
}

function createCdpClient(webSocketUrl) {
  const socket = new WebSocket(webSocketUrl);
  const pending = new Map();
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
        async close() {
          socket.close();
        },
      });
    });
    socket.addEventListener('message', (event) => {
      const payload = JSON.parse(event.data);
      if (!payload.id) {
        return;
      }
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
    });
    socket.addEventListener('error', () => reject(new Error('CDP socket error')));
  });
}

async function waitForLoad(client) {
  await client.send('Page.getFrameTree').catch(() => null);
  await delay(500);
}

async function waitForFunction(client, fn, timeoutMs, label, ...args) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() < deadline) {
    try {
      const result = await evalInPage(client, fn, ...args);
      if (result) {
        return result;
      }
      last = result;
    } catch (error) {
      last = error.message;
    }
    await delay(250);
  }
  throw new Error(`Timed out waiting for ${label}; last=${JSON.stringify(last)}`);
}

async function evalInPage(client, fn, ...args) {
  const expression = `(${fn.toString()})(...${JSON.stringify(args)})`;
  const result = await client.send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.text || 'page evaluation failed');
  }
  return result.result?.value;
}

async function screenshot(client, filePath) {
  const result = await client.send('Page.captureScreenshot', {
    format: 'png',
    fromSurface: true,
  });
  await fs.writeFile(filePath, Buffer.from(result.data, 'base64'));
}

function waitForProcessExit(child, timeoutMs) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('process exit timeout')), timeoutMs);
    child.once('exit', (code, signal) => {
      clearTimeout(timer);
      resolve({ code, signal });
    });
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
