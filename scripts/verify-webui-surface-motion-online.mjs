#!/usr/bin/env node
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const chromePath = process.env.FREEHAND_WEBUI_SURFACE_MOTION_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(
  process.env.FREEHAND_WEBUI_SURFACE_MOTION_DEBUG_PORT || '9279',
  10,
);
const baseUrl = normalizedBaseUrl(
  process.env.FREEHAND_WEBUI_SURFACE_MOTION_BASE_URL || 'http://127.0.0.1:4042/',
);
const runId = `webui-surface-motion-${Date.now()}-${process.pid}`;
const artifactDir = path.join(process.cwd(), 'artifacts', 'webui-online', runId);
await fs.mkdir(artifactDir, { recursive: true });

let chrome = null;
let cdp = null;
let summary = null;
let chromeExitResolve = null;
let chromeLog = [];
let chromeExitCode = null;
let chromeSignal = null;
const chromeExited = new Promise((resolve) => {
  chromeExitResolve = resolve;
});
const chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-surface-motion-'));

try {
  await waitHealth();
  chrome = spawn(chromePath, [
    '--headless=new',
    `--remote-debugging-port=${debugPort}`,
    '--remote-debugging-address=127.0.0.1',
    `--user-data-dir=${chromeProfileDir}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-networking',
    '--disable-extensions',
    '--disable-sync',
    '--disable-gpu',
    '--no-sandbox',
    baseUrl,
  ], { stdio: ['ignore', 'pipe', 'pipe'] });
  chrome.stdout.on('data', (chunk) => chromeLog.push(`[stdout] ${chunk}`));
  chrome.stderr.on('data', (chunk) => chromeLog.push(`[stderr] ${chunk}`));
  chrome.on('exit', (code, signal) => {
    chromeExitCode = code;
    chromeSignal = signal;
    chromeExitResolve({ code, signal });
  });

  const pageTarget = await waitForPageTarget(baseUrl);
  cdp = await createCdpClient(pageTarget.webSocketDebuggerUrl);
  cdp.addEventListener((method, params) => {
    if (method === 'Runtime.exceptionThrown') {
      summary.errors.push(String(params.exceptionDetails?.exception?.description || params.exceptionDetails?.text));
    }
  });
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: `
      window.__surfaceMotionErrors = [];
      window.addEventListener('error', event => window.__surfaceMotionErrors.push(String(event.message || event.error)));
      window.addEventListener('unhandledrejection', event => window.__surfaceMotionErrors.push(String(event.reason)));
    `,
  });

  summary = {
    asset_version: '20260824-webui-surface-motion',
    viewports: {},
    reduced_motion: null,
    errors: [],
  };

  for (const [viewport, width, height, newButton] of [
    ['desktop', 1440, 900, 'new-conversation-button'],
    ['mobile', 430, 932, 'mobile-new-entry-button'],
  ]) {
    summary.viewports[viewport] = await verifyViewport(width, height, newButton, viewport);
  }

  await setViewport(390, 832, true);
  await navigate();
  await waitForShell();
  await documentClick('mobile-new-entry-button');
  const reduced = await runtimeEvaluate(`(() => {
    const dialog = document.getElementById('new-session-dialog');
    const panel = dialog.querySelector('.new-session-panel');
    return {
      open: dialog.open,
      dialogTransition: getComputedStyle(dialog).transitionDuration,
      panelTransition: getComputedStyle(panel).transitionDuration,
    };
  })()`);
  assertEqual(reduced.open, true, 'reduced-motion New dialog is open');
  assertEqual(reduced.dialogTransition, '0s', 'reduced-motion dialog transition');
  assertEqual(reduced.panelTransition, '0s', 'reduced-motion panel transition');
  summary.reduced_motion = reduced;

  summary.errors.push(...await runtimeEvaluate('window.__surfaceMotionErrors'));
  if (summary.errors.length > 0) throw new Error(`WebUI surface motion errors: ${summary.errors.join('; ')}`);
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(`webui_surface_motion_ok url=${baseUrl} artifactDir=${artifactDir}`);
} catch (error) {
  if (summary) {
    await fs.writeFile(path.join(artifactDir, 'failure.json'), JSON.stringify({
      error: error.message,
      summary,
      chrome_log: chromeLog.slice(-120),
    }, null, 2)).catch(() => null);
  }
  throw error;
} finally {
  if (cdp) cdp.close();
  if (chrome) {
    chrome.kill('SIGTERM');
    await onceExit(chrome);
  }
  await fs.rm(chromeProfileDir, { recursive: true, force: true }).catch(() => null);
}

async function verifyViewport(width, height, newButtonId, viewport) {
  await setViewport(width, height, false);
  await navigate();
  await waitForShell();
  await screenshot(`${viewport}-home`);
  await documentClick(newButtonId);
  await waitForFunction(`document.getElementById('new-session-dialog')?.classList.contains('is-open') === true`, 'dialog opening transition');
  const opened = await dialogState(true);
  assertEqual(opened.open, true, `${viewport} New dialog remains open`);
  assertEqual(opened.className.includes('is-closing'), false, `${viewport} reopen clears closing state`);
  assertEqual(opened.panelTransition, '0.21s, 0.21s', `${viewport} open duration`);
  assertNotEqual(opened.focusOutlineWidth, '0px', `${viewport} focus ring`);
  await screenshot(`${viewport}-dialog-open`);

  await documentClick('new-session-cancel-button');
  const closing = await dialogState(false);
  assertEqual(closing.open, true, `${viewport} close waits for animation`);
  assertEqual(closing.className.includes('is-closing'), true, `${viewport} closing state`);
  await documentClick(newButtonId);
  await waitForFunction(`document.getElementById('new-session-dialog')?.classList.contains('is-open') === true`, 'dialog reopen race');
  const reopened = await dialogState(false);
  assertEqual(reopened.open, true, `${viewport} reopen wins pending close race`);
  await delay(250);
  const settledAfterReopen = await dialogState(false);
  assertEqual(settledAfterReopen.open, true, `${viewport} stale close does not close reopened dialog`);
  assertEqual(settledAfterReopen.className.includes('is-closing'), false, `${viewport} stale close does not leave closing state`);
  assertEqual(
    await runtimeEvaluate('document.body.dataset.webuiRoute'),
    'new_session',
    `${viewport} reopen preserves New session route truth`,
  );

  await documentClick('new-session-cancel-button');
  await waitForFunction(`document.getElementById('new-session-dialog')?.open === false`, 'animated dialog close');
  return { opened, closing, reopened };
}

async function dialogState(includeFocus) {
  return runtimeEvaluate(`(() => {
    const dialog = document.getElementById('new-session-dialog');
    const panel = dialog.querySelector('.new-session-panel');
    const focusButton = document.getElementById('new-session-cancel-button');
    const focusStyle = getComputedStyle(focusButton);
    return {
      open: dialog.open,
      className: dialog.className,
      panelTransition: getComputedStyle(panel).transitionDuration,
      focusOutlineWidth: ${includeFocus ? 'focusStyle.outlineWidth' : 'null'},
      focusOutlineColor: ${includeFocus ? 'focusStyle.outlineColor' : 'null'},
    };
  })()`);
}

async function setViewport(width, height, reducedMotion) {
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width,
    height,
    deviceScaleFactor: 2,
    mobile: width < 800,
  });
  await cdp.send('Emulation.setEmulatedMedia', {
    features: [{ name: 'prefers-reduced-motion', value: reducedMotion ? 'reduce' : 'no-preference' }],
  });
}

async function navigate() {
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad();
}

async function waitForShell() {
  await waitForFunction(
    `document.body.dataset.webuiJsReady === 'true' && !!document.querySelector('[data-webui-shell="true"]')`,
    'surface-motion WebUI shell',
  );
}

async function screenshot(label) {
  const { data } = await cdp.send('Page.captureScreenshot', { format: 'png' });
  await fs.writeFile(path.join(artifactDir, `${label}.png`), Buffer.from(data, 'base64'));
}

async function documentClick(id) {
  await runtimeEvaluate(`document.getElementById(${JSON.stringify(id)})?.click()`);
}

async function runtimeEvaluate(expression) {
  const result = await cdp.send('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text);
  }
  return result.result.value;
}

async function waitForFunction(expression, label, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  let lastValue = null;
  while (Date.now() < deadline) {
    lastValue = await runtimeEvaluate(`!!(${expression})`);
    if (lastValue === true) return;
    await delay(50);
  }
  throw new Error(`timeout waiting for ${label}`);
}

async function waitForLoad(timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      await runtimeEvaluate('document.readyState');
      return;
    } catch {
      await delay(100);
    }
  }
  throw new Error('page load timeout');
}

async function waitForPageTarget(urlPrefix) {
  const deadline = Date.now() + 20_000;
  let lastFetchError = null;
  while (Date.now() < deadline) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${debugPort}/json/list`)).json();
      const target = targets.find((entry) => entry.type === 'page' && `${entry.url || ''}`.startsWith(urlPrefix));
      if (target) return target;
    } catch (error) {
      lastFetchError = error;
      if (chromeExitCode !== null) {
        throw new Error(
          `Chrome exited before page target (code=${chromeExitCode}, signal=${chromeSignal}); ${chromeLog.join('')}`,
        );
      }
    }
    await delay(100);
  }
  const exit = await Promise.race([chromeExited, Promise.resolve(null)]);
  throw new Error(
    `Chrome page target timeout${exit ? `; Chrome exited code=${exit.code} signal=${exit.signal}` : ''}${lastFetchError ? `; last fetch: ${lastFetchError.message}` : ''}; ${chromeLog.join('')}`,
  );
}

function createCdpClient(webSocketUrl) {
  const socket = new WebSocket(webSocketUrl);
  let nextId = 1;
  const listeners = new Set();
  const pending = new Map();
  socket.addEventListener('message', (event) => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      const { resolve, reject } = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) reject(new Error(message.error.message || JSON.stringify(message.error)));
      else resolve(message.result || {});
      return;
    }
    if (message.method) listeners.forEach((listener) => listener(message.method, message.params || {}));
  });
  const ready = new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', () => reject(new Error('CDP socket error')), { once: true });
  });
  return {
    addEventListener(listener) { listeners.add(listener); },
    close() { socket.close(); },
    async send(method, params = {}) {
      await ready;
      const id = nextId++;
      socket.send(JSON.stringify({ id, method, params }));
      return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
    },
  };
}

async function waitHealth(timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(new URL('/health', baseUrl));
      if (response.ok && await response.text() === 'ok') return;
    } catch {
      // The service may still be binding when the verifier starts.
    }
    await delay(250);
  }
  throw new Error(`surface-motion service health timeout: ${baseUrl}`);
}

function onceExit(child) {
  return new Promise((resolve) => child.once('exit', resolve));
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) throw new Error(`${label}: expected ${expected}, got ${actual}`);
}

function assertNotEqual(actual, expected, label) {
  if (actual === expected) throw new Error(`${label}: unexpectedly ${actual}`);
}

function normalizedBaseUrl(value) {
  return value.endsWith('/') ? value : `${value}/`;
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
