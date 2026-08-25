import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const chromePath =
  process.env.FREEHAND_WEBUI_CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_IMAGE_DEBUG_PORT || '9245', 10);
const baseUrl = androidBaseUrl(
  process.env.FREEHAND_WEBUI_BASE_URL || 'http://127.0.0.1:4042/',
);
const fixedSessionId =
  process.env.FREEHAND_WEBUI_IMAGE_SESSION || 'webui-image-attachment-proof-fixed';
const timestamp = new Date().toISOString().replaceAll(':', '').replaceAll('.', '');
const artifactDir =
  process.env.FREEHAND_WEBUI_IMAGE_ARTIFACT_DIR ||
  path.join(process.cwd(), 'artifacts', 'webui-online', `image-attachment-notification-${timestamp}`);

await fs.mkdir(artifactDir, { recursive: true });
const profileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-webui-image-'));
let chrome;

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
      '--window-size=1200,900',
      baseUrl,
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );
  chrome.stdout.on('data', () => {});
  chrome.stderr.on('data', () => {});

  const target = await waitPageTarget();
  const cdp = await createCdpClient(target.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: 'window.__freehandEnableTestHooks = true;',
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitFor(
    () =>
      evalPage(
        cdp,
        () =>
          !!document.querySelector('[data-webui-shell="true"]') &&
          !!window.__freehandWebUiTest &&
          document.body.dataset.layoutClient === 'android-webview',
      ),
    20_000,
    'Android WebUI shell and test hooks',
  );

  const result = await evalPage(cdp, runProof, fixedSessionId);
  const screenshot = await cdp.send('Page.captureScreenshot', { format: 'png' });
  await fs.writeFile(path.join(artifactDir, 'webui.png'), Buffer.from(screenshot.data, 'base64'));
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(result, null, 2));

  const failed = Object.entries(result.checks).filter(([, passed]) => !passed);
  if (failed.length > 0) {
    throw new Error(`image attachment/notification proof failed: ${JSON.stringify(result.checks)}`);
  }
  console.log(
    `webui_image_attachment_online_ok session=${fixedSessionId} artifact=${path.join(artifactDir, 'summary.json')}`,
  );
  await cdp.close();
} finally {
  if (chrome?.pid && chrome.exitCode === null) {
    chrome.kill('SIGTERM');
    await new Promise((resolve) => chrome.once('exit', resolve));
  }
  await fs.rm(profileDir, { recursive: true, force: true }).catch(() => {});
}

async function runProof(sessionId) {
  const hook = window.__freehandWebUiTest;
  if (!hook) throw new Error('WebUI test hook is unavailable');

  const imageBase64 =
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl2n1cAAAAASUVORK5CYII=';
  const image = {
    name: 'proof.png',
    type: 'image/png',
    size: Math.ceil((imageBase64.length * 3) / 4),
    dataBase64: imageBase64,
  };
  const capturedCommands = [];
  const notificationCalls = [];

  hook.prepareAttachmentProofSession(sessionId);
  hook.setAdpQueryForTest(async (query) => {
    const name = typeof query === 'string' ? query : Object.keys(query || {})[0] || '';
    if (name === 'QuerySessionListPage') {
      return {
        SessionListPage: {
          sessions: [{ session_id: sessionId, title: 'Image attachment proof', archived: false }],
          page: { has_older: false, next_cursor: null, unavailable_sessions: [] },
        },
      };
    }
    if (name === 'QuerySessionTurns') {
      return { SessionTurns: { session_id: sessionId, turns: [] } };
    }
    if (name === 'QueryLatestActiveTurn') return { Turn: null };
    if (name === 'QueryTaskBoard') return { TaskBoard: { tasks: [] } };
    if (name === 'QueryAgentBoard') return { AgentBoard: { agents: [] } };
    if (name === 'QueryEventInbox') return { EventInbox: { events: [], cursor: null } };
    return {};
  });
  hook.setAdpCommandForTest(async (command) => {
    capturedCommands.push(command);
    return {
      dispatch_status: 'reason_live_turn_completed rounds=1 schema_rejections=0 tool_executions=0',
      ingress: { submit_id: 'submit-image-proof-fixed' },
    };
  });
  window.FreehandAndroidNotifications = {
    turnFinished(payload) {
      notificationCalls.push(JSON.parse(payload));
    },
  };

  const selected = hook.addImageAttachmentForTest(image);
  const previewOpen = hook.clickFirstAttachmentPreviewForTest();
  const previewClosed = hook.closeAttachmentPreviewForTest();
  const removed = hook.removeFirstAttachmentForTest();
  hook.addImageAttachmentForTest(image);
  await hook.submitComposerForTest('image attachment proof prompt');
  await new Promise((resolve) => setTimeout(resolve, 50));
  const afterSubmit = hook.captureAttachmentState();

  hook.applyTurnProjectionForTest({
    session_id: sessionId,
    turn_id: 'runtime-turn-image-proof-history',
    user_text: 'image attachment proof prompt',
    attachments: [{
      attachment_id: 'attachment-proof-fixed',
      kind: 'image',
      name: image.name,
      media_type: image.type,
      size_bytes: image.size,
    }],
    text: ['image metadata was accepted'],
    terminal_status: 'Success',
    terminal_text: 'done',
  });
  const projected = hook.captureAttachmentState();

  hook.clearAndroidNotificationMemoryForTest();
  const notificationTurnId = 'runtime-turn-image-proof-notification';
  hook.applyTurnProjectionForTest({
    session_id: sessionId,
    turn_id: notificationTurnId,
    user_text: 'notification lifecycle proof',
    text: [],
    tool_activities: [],
    terminal_status: null,
    terminal_text: null,
  });
  hook.applyTurnProjectionForTest({
    session_id: sessionId,
    turn_id: notificationTurnId,
    user_text: 'notification lifecycle proof',
    text: ['notification lifecycle finished'],
    tool_activities: [],
    terminal_status: 'Success',
    terminal_text: 'notification lifecycle finished',
  });
  hook.applyTurnProjectionForTest({
    session_id: sessionId,
    turn_id: notificationTurnId,
    user_text: 'notification lifecycle proof',
    text: ['notification lifecycle finished'],
    tool_activities: [],
    terminal_status: 'Success',
    terminal_text: 'notification lifecycle finished',
  });
  hook.applyTurnProjectionForTest({
    session_id: sessionId,
    turn_id: 'runtime-turn-restored-terminal',
    user_text: 'restored history',
    text: ['historical result'],
    terminal_status: 'Success',
    terminal_text: 'historical result',
  });

  const command = capturedCommands.find((value) => value?.SubmitUserInput)?.SubmitUserInput || null;
  const attachment = command?.metadata?.attachments?.[0] || null;
  return {
    sessionId,
    selected,
    previewOpen,
    previewClosed,
    removed,
    afterSubmit,
    projected,
    command,
    notificationCalls,
    checks: {
      selectedPool:
        selected.attachmentCount === 1 && selected.thumbCount === 1 && selected.removeCount === 1,
      previewLifecycle: previewOpen.overlayCount === 1 && previewClosed.overlayCount === 0,
      removeLifecycle: removed.attachmentCount === 0,
      submitMetadata:
        !!attachment &&
        attachment.kind === 'image' &&
        attachment.media_type === image.type &&
        attachment.name === image.name &&
        attachment.data_base64 === imageBase64 &&
        command.text === 'image attachment proof prompt' &&
        !command.text.includes(imageBase64) &&
        !command.text.includes('Attachments'),
      submitClearsPool: afterSubmit.attachmentCount === 0,
      metadataOnlyHistory:
        projected.messageText.includes('Submitted attachments') &&
        projected.messageText.includes(image.name) &&
        projected.messageText.includes(image.type) &&
        projected.messageText.includes('metadata-only') &&
        !projected.messageText.includes(imageBase64),
      oneLiveTerminalNotification:
        notificationCalls.length === 1 &&
        notificationCalls[0].sessionId === sessionId &&
        notificationCalls[0].turnId === notificationTurnId &&
        notificationCalls[0].status === 'Success' &&
        notificationCalls[0].title === '任务已经完成',
      restoredTerminalDoesNotNotify: notificationCalls.length === 1,
    },
  };
}

async function waitPageTarget() {
  return await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
    if (!response.ok) return null;
    const targets = await response.json();
    return targets.find((target) => target.type === 'page' && String(target.url || '').startsWith(baseUrl));
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
        if (payload.error) entry.reject(new Error(payload.error.message || 'CDP error'));
        else entry.resolve(payload.result || {});
        return;
      }
      if (payload.method) listeners.forEach((listener) => listener(payload.method, payload.params || {}));
    });
    socket.addEventListener('error', () => reject(new Error('CDP socket error')));
  });
}

async function waitForLoad(cdp) {
  await new Promise((resolve) => {
    const listener = (method) => {
      if (method === 'Page.loadEventFired') {
        cdp.offEvent(listener);
        resolve();
      }
    };
    cdp.onEvent(listener);
  });
}

async function waitFor(fn, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let lastError = null;
  while (Date.now() < deadline) {
    try {
      const value = await fn();
      if (value) return value;
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`timeout waiting for ${label}${lastError ? `: ${lastError.message}` : ''}`);
}

async function evalPage(cdp, fn, ...args) {
  const response = await cdp.send('Runtime.evaluate', {
    expression: `(${fn})(...${JSON.stringify(args)})`,
    awaitPromise: true,
    returnByValue: true,
  });
  if (response.exceptionDetails) {
    throw new Error(response.exceptionDetails.exception?.description || response.exceptionDetails.text || 'Runtime.evaluate failed');
  }
  return response.result.value;
}

function androidBaseUrl(value) {
  const parsed = new URL(value);
  if (!parsed.pathname.endsWith('/')) parsed.pathname = `${parsed.pathname}/`;
  parsed.searchParams.set('client', 'android-webview');
  return parsed.toString();
}
