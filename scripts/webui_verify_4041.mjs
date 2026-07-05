import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const chromePath = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = 9223;
const baseUrl = 'http://127.0.0.1:4041/';
const adpUrl = 'ws://127.0.0.1:4041/adp';
const successPrompt =
  'ADP success sample: answer with one short sentence and a valid Freehand completion schema. Do not call tools.';
const failurePrompt =
  'ADP failure sample: call the read_file tool exactly once with path definitely-missing-freehand-file.txt, then use the failed tool result to continue and report success through the required Freehand completion schema.';

const runId = `${new Date().toISOString().slice(0, 10).replace(/-/g, '')}-verify-4041-${Date.now()}`;
const artifactDir = path.join(process.cwd(), 'artifacts', 'webui-online', runId);

await fs.mkdir(artifactDir, { recursive: true });

const chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-webui-verify-'));
let chrome;

try {
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
      '--window-size=1600,1200',
      baseUrl,
    ],
    {
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  );

  const chromeLog = [];
  chrome.stdout.on('data', (chunk) => chromeLog.push(`[stdout] ${chunk}`));
  chrome.stderr.on('data', (chunk) => chromeLog.push(`[stderr] ${chunk}`));

  const pageTarget = await waitForPageTarget(baseUrl, 15_000);
  const cdp = await createCdpClient(pageTarget.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Log.enable').catch(() => null);
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: `
      window.__freehandVerify = { pageErrors: [], consoleErrors: [] };
      window.addEventListener('error', (event) => {
        window.__freehandVerify.pageErrors.push(String(event.message || event.error || 'error'));
      });
      window.addEventListener('unhandledrejection', (event) => {
        const reason = event.reason && (event.reason.stack || event.reason.message || String(event.reason));
        window.__freehandVerify.pageErrors.push(String(reason || 'unhandledrejection'));
      });
      const originalError = console.error.bind(console);
      console.error = (...args) => {
        try {
          window.__freehandVerify.consoleErrors.push(args.map((value) => String(value)).join(' '));
        } catch (_) {}
        return originalError(...args);
      };
    `,
  });

  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(cdp, () => {
    return !!document.querySelector('[data-webui-shell="true"]') && !!document.getElementById('composer-input');
  }, 20_000, 'shell ready');

  await evalInPage(cdp, () => {
    document.getElementById('new-conversation-button')?.click();
  });
  await delay(250);
  await captureState(cdp, '01-after-new-conversation');

  const prompt1 = `verify pending input success ${Date.now()}`;
  await submitPrompt(cdp, `${successPrompt}\nMarker: ${prompt1}`);
  await delay(300);
  const postSubmit1 = await captureState(cdp, '02-after-first-submit');

  await waitForFunction(cdp, () => {
    const composer = document.getElementById('composer-input');
    const text = document.getElementById('message-list')?.innerText || '';
    return composer && composer.value === '' && text.includes('Marker:');
  }, 20_000, 'first prompt visible with cleared composer');
  const materialized1 = await captureState(cdp, '03-first-materialized');

  await waitForTerminal(cdp, 90_000, 'first terminal');
  const terminal1 = await captureState(cdp, '04-first-terminal');

  await submitPrompt(cdp, failurePrompt);
  await delay(300);
  const postSubmit2 = await captureState(cdp, '05-after-second-submit');

  await waitForFunction(cdp, () => {
    const live = Array.from(document.querySelectorAll('[data-live="true"]'));
    return live.length >= 1;
  }, 20_000, 'second live turn visible');
  const running2 = await captureState(cdp, '06-second-running');

  await waitForTerminal(cdp, 120_000, 'second terminal');
  const terminal2 = await captureState(cdp, '07-second-terminal');

  await cdp.send('Page.reload', { ignoreCache: true });
  await waitForLoad(cdp);
  await waitForFunction(cdp, () => {
    return !!document.querySelector('[data-webui-shell="true"]');
  }, 20_000, 'shell reloaded');
  const refreshed = await captureState(cdp, '08-after-refresh');

  const sessionId = refreshed.state.selectedSession || terminal2.state.selectedSession || terminal1.state.selectedSession;
  const adpQuery = sessionId
    ? await runCommand(['~/.local/bin/freehand-cli', 'adp-session-query', '--url', adpUrl, '--session', sessionId])
    : { code: 1, stdout: '', stderr: 'missing session id' };

  const summary = {
    runId,
    artifactDir,
    sessionId,
    checks: {
      firstSubmitComposerCleared: postSubmit1.state.composer === '',
      firstPromptVisibleAfterSubmit: materialized1.state.messageText.includes(prompt1),
      secondSubmitComposerCleared: postSubmit2.state.composer === '',
      secondRunningHasLiveCard: running2.state.liveCount >= 1,
      staleHistoricalLiveAfterSecondSubmit: running2.state.nonLastLiveCount,
      refreshPreservedFirstPrompt: refreshed.state.messageText.includes(prompt1),
      refreshPreservedFailurePrompt: refreshed.state.messageText.includes('definitely-missing-freehand-file.txt'),
      terminal2NoLive: terminal2.state.liveCount === 0,
    },
    snapshots: {
      postSubmit1,
      materialized1,
      terminal1,
      postSubmit2,
      running2,
      terminal2,
      refreshed,
    },
    adpQuery,
    chromeProfileDir,
    chromeLog,
  };

  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));

  await cdp.close();
} finally {
  if (chrome && !chrome.killed) {
    chrome.kill('SIGTERM');
    await onceExit(chrome, 5_000).catch(() => null);
  }
}

async function submitPrompt(cdp, text) {
  await evalInPage(
    cdp,
    (value) => {
      const input = document.getElementById('composer-input');
      input.focus();
      input.value = value;
      input.dispatchEvent(new Event('input', { bubbles: true }));
      document.getElementById('composer-form').requestSubmit();
    },
    text,
  );
}

async function captureState(cdp, label) {
  const screenshot = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  await fs.writeFile(path.join(artifactDir, `${label}.png`), Buffer.from(screenshot.data, 'base64'));
  const state = await evalInPage(cdp, () => {
    const messages = Array.from(document.querySelectorAll('#message-list .chat-message'));
    const live = messages.filter((node) => node.dataset.live === 'true');
    const lastMessage = messages[messages.length - 1] || null;
    return {
      selectedSession: document.getElementById('strip-session')?.textContent?.trim() || '',
      selectedTurn: document.getElementById('strip-turn')?.textContent?.trim() || '',
      composer: document.getElementById('composer-input')?.value || '',
      commandStatus: document.getElementById('command-status')?.textContent?.trim() || '',
      turnStatus: document.getElementById('turn-status')?.textContent?.trim() || '',
      workspaceStatus: document.getElementById('workspace-status')?.textContent?.trim() || '',
      liveCount: live.length,
      nonLastLiveCount: live.filter((node) => node !== lastMessage).length,
      messageCount: messages.length,
      messageText: document.getElementById('message-list')?.innerText || '',
      pageErrors: window.__freehandVerify?.pageErrors || [],
      consoleErrors: window.__freehandVerify?.consoleErrors || [],
    };
  });
  await fs.writeFile(path.join(artifactDir, `${label}.json`), JSON.stringify(state, null, 2));
  return { label, state };
}

async function waitForTerminal(cdp, timeoutMs, label) {
  await waitForFunction(cdp, () => {
    const live = document.querySelectorAll('[data-live="true"]').length;
    const turnStatus = document.getElementById('turn-status')?.textContent?.toLowerCase() || '';
    const commandStatus = document.getElementById('command-status')?.textContent?.toLowerCase() || '';
    return live === 0 && (turnStatus.includes('completed') || commandStatus.includes('turn completed'));
  }, timeoutMs, label);
}

async function waitForFunction(cdp, fn, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = await evalInPage(cdp, fn);
    if (result) {
      return;
    }
    await delay(250);
  }
  throw new Error(`timeout waiting for ${label}`);
}

async function evalInPage(cdp, fn, arg) {
  const expression = `(${fn.toString()})(${arg === undefined ? '' : JSON.stringify(arg)})`;
  const response = await cdp.send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    throw new Error(response.exceptionDetails.text || 'Runtime.evaluate failed');
  }
  return response.result.value;
}

async function waitForPageTarget(urlPrefix, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
      if (response.ok) {
        const targets = await response.json();
        const page = targets.find((target) => target.type === 'page' && `${target.url || ''}`.startsWith(urlPrefix));
        if (page && page.webSocketDebuggerUrl) {
          return page;
        }
      }
    } catch (_) {
      // wait for Chrome DevTools
    }
    await delay(250);
  }
  throw new Error('timeout waiting for Chrome DevTools page target');
}

async function waitForLoad(cdp) {
  await new Promise((resolve) => {
    const onEvent = (method) => {
      if (method === 'Page.loadEventFired') {
        cdp.offEvent(onEvent);
        resolve();
      }
    };
    cdp.onEvent(onEvent);
  });
}

async function runCommand(argv) {
  const shellCommand = argv.join(' ');
  return new Promise((resolve) => {
    const child = spawn('/bin/zsh', ['-lc', shellCommand], { stdio: ['ignore', 'pipe', 'pipe'] });
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
  });
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
        async close() {
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
          return;
        }
        entry.resolve(payload.result || {});
        return;
      }
      if (payload.method) {
        listeners.forEach((listener) => listener(payload.method, payload.params || {}));
      }
    });
    socket.addEventListener('error', (event) => {
      reject(new Error(`CDP socket error: ${event.message || 'unknown'}`));
    });
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function onceExit(child, timeoutMs) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('exit timeout')), timeoutMs);
    child.once('exit', () => {
      clearTimeout(timer);
      resolve();
    });
  });
}
