import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const chromePath =
  process.env.FREEHAND_WEBUI_CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_DEBUG_PORT || '9257', 10);
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WEBUI_BASE_URL || 'http://100.66.1.82:4042/');
const runId = `mobile-command-tool-${Date.now()}`;
const artifactDir = path.join(process.cwd(), 'artifacts', 'webui-online', runId);

await fs.mkdir(artifactDir, { recursive: true });
const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-mobile-command-tool-'));
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
      '--disable-gpu',
      '--no-sandbox',
      '--window-size=1280,900',
      baseUrl,
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );
  chrome.stdout.on('data', () => {});
  chrome.stderr.on('data', () => {});

  const pageTarget = await waitForPageTarget(baseUrl, 15_000);
  cdp = await createCdpClient(pageTarget.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: 'window.__freehandEnableTestHooks = true;',
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(cdp, () =>
    document.body.dataset.webuiJsReady === 'true' && !!window.__freehandWebUiTest && !!window.__freehandState,
    20_000,
    'WebUI ready and test hooks',
  );

  await evalInPage(cdp, () => {
    window.__freehandWebUiTest.prepareAttachmentProofSession('webui-tool-expand-fixed');
    const st = window.__freehandState;
    if (!st.sessions.some((s) => s.session_id === 'webui-tool-expand-fixed')) {
      st.sessions.push({ session_id: 'webui-tool-expand-fixed', title: 'tool expand fixture', archived: false });
    }
    st.sessionListLoaded = true;
    st.selectedSessionId = 'webui-tool-expand-fixed';
    st.route = 'session_detail';
    document.body.dataset.webuiRoute = 'session_detail';
    window.__freehandWebUiTest.renderAll();
  });
  await delay(500);

  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 2,
    mobile: true,
  });
  await delay(500);

  const initial = await evalInPage(cdp, () => {
    const imageButton = document.getElementById('attach-image-button');
    const composerShell = document.querySelector('.composer-input-shell');
    const commandMenu = document.getElementById('composer-command-menu');
    const commandButton = document.getElementById('composer-command-menu-button');
    const imageRect = imageButton?.getBoundingClientRect();
    const shellRect = composerShell?.getBoundingClientRect();
    return {
      imageButtonVisible: !!(imageButton && imageRect && imageRect.width > 0 && imageRect.height > 0),
      imageInsideComposer: !!(shellRect && imageRect &&
        imageRect.left >= shellRect.left - 2 && imageRect.left < shellRect.right &&
        imageRect.bottom <= shellRect.bottom + 2 && imageRect.top >= shellRect.top - 2),
      commandMenuHidden: commandMenu?.hidden === true,
      commandButtonVisible: !!(commandButton && commandButton.getBoundingClientRect().width > 0),
    };
  });

  await evalInPage(cdp, () => {
    document.getElementById('composer-command-menu-button')?.click();
  });
  await delay(300);

  const menuOpen = await evalInPage(cdp, () => {
    const menu = document.getElementById('composer-command-menu');
    const rect = menu?.getBoundingClientRect();
    return {
      hidden: menu?.hidden === true,
      fixed: menu ? getComputedStyle(menu).position : '',
      bottom: rect ? Math.round(rect.bottom) : null,
      viewportHeight: window.innerHeight,
      left: rect ? Math.round(rect.left) : null,
      right: rect ? Math.round(rect.right) : null,
      scrim: document.getElementById('mobile-drawer-scrim')?.getAttribute('aria-hidden') || '',
      bodyAttr: document.body.dataset.mobileCommandMenu || '',
    };
  });

  await cdp.send('Page.captureScreenshot', { format: 'png' });
  const screenshotResult = await cdp.send('Page.captureScreenshot', { format: 'png' });
  await fs.writeFile(path.join(artifactDir, 'mobile-command-menu.png'), Buffer.from(screenshotResult.data, 'base64'));

  await evalInPage(cdp, () => {
    document.querySelector('[data-composer-action="compact"]')?.click();
  });
  await delay(200);
  const menuClosed = await evalInPage(cdp, () => ({
    hidden: document.getElementById('composer-command-menu')?.hidden === true,
    bodyAttr: document.body.dataset.mobileCommandMenu || '',
  }));

  await evalInPage(cdp, () => {
    const st = window.__freehandState;
    st.selectedSessionId = 'webui-tool-expand-fixed';
    st.draftSessionId = 'webui-tool-expand-fixed';
    st.sessionListLoaded = false;
    window.__freehandWebUiTest.applyTurnProjectionForTest({
      session_id: 'webui-tool-expand-fixed',
      turn_id: 'runtime-turn-tool-expand-fixed',
      user_text: 'tool expand',
      tool_activities: [{
        tool_call_id: 'tool-expand-fixed-a',
        tool_name: 'bash',
        status: 'completed',
        title: 'bash',
        detail: 'result line one\nresult line two',
        display: {
          kind: 'shell',
          outcome: 'success',
          action: 'bash',
          target: '/tmp/example.txt',
          parameter_summary: 'echo hello',
          summary: 'ran shell command',
          result_summary: 'hello',
          fields: [{ label: 'command', value: 'echo hello' }],
          diff: { target: '/tmp/example.txt', before: 'old', after: 'new' },
        },
      }],
      terminal_text: 'done',
      terminal_status: 'Success',
    });
  });
  await delay(300);

  const toolInitial = await evalInPage(cdp, () => {
    const section = document.querySelector('.chat-section-tool');
    const detail = section?.querySelector('.tool-chat-detail');
    const head = section?.querySelector('.tool-chat-head');
    return {
      toolCount: document.querySelectorAll('.chat-section-tool').length,
      detailExists: !!detail,
      detailHidden: detail?.hidden === true,
      expanded: section?.dataset.toolExpanded || '',
      headType: head?.tagName || '',
    };
  });

  await evalInPage(cdp, () => {
    document.querySelector('.chat-section-tool .tool-chat-head')?.click();
  });
  await delay(200);

  const toolExpanded = await evalInPage(cdp, () => {
    const section = document.querySelector('.chat-section-tool');
    const detail = section?.querySelector('.tool-chat-detail');
    return {
      expanded: section?.dataset.toolExpanded || '',
      detailHidden: detail?.hidden === true,
      detailText: detail?.innerText || '',
      headAria: section?.querySelector('.tool-chat-head')?.getAttribute('aria-expanded') || '',
    };
  });

  await cdp.send('Page.captureScreenshot', { format: 'png' });
  const toolShot = await cdp.send('Page.captureScreenshot', { format: 'png' });
  await fs.writeFile(path.join(artifactDir, 'mobile-tool-expanded.png'), Buffer.from(toolShot.data, 'base64'));

  const summary = {
    assetVersion: await productionAssetVersion(),
    initial,
    menuOpen,
    menuClosed,
    toolInitial,
    toolExpanded,
    passed:
      initial.imageButtonVisible &&
      initial.imageInsideComposer &&
      initial.commandMenuHidden &&
      menuOpen.hidden === false &&
      menuOpen.fixed === 'fixed' &&
      menuOpen.bottom <= menuOpen.viewportHeight &&
      menuOpen.bodyAttr === 'open' &&
      menuOpen.scrim === 'false' &&
      menuClosed.hidden === true &&
      menuClosed.bodyAttr === '' &&
      toolInitial.toolCount === 1 &&
      toolInitial.detailExists &&
      toolInitial.detailHidden &&
      toolExpanded.expanded === 'true' &&
      toolExpanded.detailHidden === false &&
      toolExpanded.detailText.includes('diff') &&
      toolExpanded.headAria === 'true',
  };

  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(`webui_mobile_command_tool_online ${summary.passed ? 'PASS' : 'FAIL'} ${JSON.stringify(summary)}`);
  if (!summary.passed) {
    process.exitCode = 1;
  }
} finally {
  if (cdp) {
    await cdp.close();
  }
  if (chrome?.pid && chrome.exitCode === null) {
    chrome.kill('SIGTERM');
    await new Promise((resolve) => chrome.once('exit', resolve));
  }
}

async function productionAssetVersion() {
  const response = await fetch(baseUrl, { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`production WebUI not reachable: ${response.status}`);
  }
  const html = await response.text();
  const match = html.match(/(?:^|["'(\/])assets\/webui\.js\?v=([^"'&<>\s]+)/);
  if (!match || !match[1]) {
    throw new Error('served WebUI does not expose asset version');
  }
  return decodeURIComponent(match[1]);
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
    } catch (_) {}
    await delay(250);
  }
  throw new Error('timeout waiting for Chrome DevTools page target');
}

async function createCdpClient(webSocketUrl) {
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
        if (!entry) return;
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

async function waitForFunction(cdp, fn, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = await evalInPage(cdp, fn);
    if (result) return;
    await delay(250);
  }
  throw new Error(`timeout waiting for ${label}`);
}

async function evalInPage(cdp, fn, arg) {
  const args = arg === undefined ? '' : JSON.stringify(arg);
  const response = await cdp.send('Runtime.evaluate', {
    expression: `(${fn.toString()})(${args})`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    const desc = response.exceptionDetails.exception?.description || '';
    const text = response.exceptionDetails.text || 'Runtime.evaluate failed';
    throw new Error(desc ? `${text}: ${desc}` : text);
  }
  return response.result.value;
}

async function waitForLoad(cdp, timeoutMs = 15_000) {
  await new Promise((resolve) => {
    const timer = setTimeout(() => {
      cdp.offEvent(onEvent);
      resolve();
    }, timeoutMs);
    const onEvent = (method) => {
      if (method === 'Page.loadEventFired') {
        clearTimeout(timer);
        cdp.offEvent(onEvent);
        resolve();
      }
    };
    cdp.onEvent(onEvent);
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function normalizedBaseUrl(value) {
  const url = new URL(value);
  if (!url.pathname.endsWith('/')) {
    url.pathname = `${url.pathname}/`;
  }
  return url.toString();
}
