import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { adpVerifierRequest } from './lib/adp-verifier-client.mjs';

const chromePath = process.env.FREEHAND_WEBUI_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_DEBUG_PORT || '9247', 10);
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WEBUI_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_WEBUI_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const adpAuthToken = process.env.FREEHAND_ADP_AUTH_TOKEN || '';
const adpProtocolVersion = 4;
const runId = `mobile-ui-tree-phase1-${new Date().toISOString().replace(/[-:.]/g, '').slice(0, 15)}-${process.pid}`;
const artifactDir = path.join(process.cwd(), 'artifacts', 'webui-online', runId);
let assetVersion = '';
const multiSelectSessionIds = [
  'webui-home-multiselect-fixed-a',
  'webui-home-multiselect-fixed-b',
];
const workerRailSessionId = 'webui-header-worker-rail-fixed';
const workerRailTaskIds = [
  'task-webui-header-worker-rail-a',
  'task-webui-header-worker-rail-b',
];
const agentDirectoryIds = ['master', 'worker', 'worker-2', 'worker-3'];
const localAgentTargets = [
  { agentId: 'worker', origin: 'http://127.0.0.1:4043', label: 'Worker 1', markerSessionId: 'webui-local-agent-namespace-worker' },
  { agentId: 'worker-2', origin: 'http://127.0.0.1:4044', label: 'Worker 2', markerSessionId: 'webui-local-agent-namespace-worker-2' },
  { agentId: 'worker-3', origin: 'http://127.0.0.1:4046', label: 'Worker 3', markerSessionId: 'webui-local-agent-namespace-worker-3' },
  { agentId: 'master', origin: 'http://127.0.0.1:4042', label: 'Master', markerSessionId: 'webui-local-agent-namespace-master' },
];
const forbiddenUiTerms = [
  /rootfs/i,
  /shared-folder/i,
  /mount-directory/i,
  /共享文件夹/,
  /挂载目录/,
];
const internalSessionTerms = [
  /worker-task-/,
  /master-lifecycle-/,
  /master-timer-/,
];
const quickEntryIds = [
  'open-settings-drawer-button',
  'open-timer-dashboard-button',
  'open-tools-dashboard-button',
  'mobile-new-entry-button',
  'open-session-drawer-button',
];
const viewports = [
  { label: '390-phone', width: 390, height: 844, mobile: true },
  { label: '430-phone', width: 430, height: 932, mobile: true },
  { label: '844-landscape', width: 844, height: 390, mobile: true },
  { label: '1280-desktop', width: 1280, height: 900, mobile: false },
];

await fs.mkdir(artifactDir, { recursive: true });

const chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-mobile-ui-tree-'));
let chrome = null;
let cdp = null;

try {
  assetVersion = await productionAssetVersion();
  await ensureMultiSelectSessions();
  await ensureHeaderWorkerRailTruth();
  await ensureWorkerOneNamespaceSessions();
  chrome = spawn(
    chromePath,
    [
      '--headless=new',
      `--remote-debugging-port=${debugPort}`,
      '--remote-debugging-address=0.0.0.0',
      `--user-data-dir=${chromeProfileDir}`,
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

  const chromeLog = [];
  chrome.stdout.on('data', (chunk) => chromeLog.push(`[stdout] ${chunk}`));
  chrome.stderr.on('data', (chunk) => chromeLog.push(`[stderr] ${chunk}`));

  const pageTarget = await waitForPageTarget(baseUrl, 15_000);
  cdp = await createCdpClient(pageTarget.webSocketDebuggerUrl);
  await cdp.send('Page.enable');
  await cdp.send('Runtime.enable');
  await cdp.send('Page.addScriptToEvaluateOnNewDocument', {
    source: 'window.__freehandEnableTestHooks = true;',
  });
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(cdp, () => {
    return document.body.dataset.webuiJsReady === 'true' &&
      !!document.querySelector('[data-webui-shell="true"]') &&
      !!document.getElementById('mobile-home-dashboard') &&
      window.__freehandSharedStateContract?.contractId === 'foundation.shared_states';
  }, 20_000, 'production WebUI shell 就绪');

  const moduleAssets = await captureModuleAssets();
  const sharedStateProjection = await captureSharedStateProjection(cdp);
  const snapshots = [];
  for (const viewport of viewports) {
    snapshots.push(await captureViewport(cdp, viewport));
  }
  const homeAgentDirectory = await captureHomeAgentDirectory(cdp);
  const localAgentClickChain = await captureLocalAgentClickChain(cdp);
  const homeMultiSelect = await captureHomeMultiSelect(cdp);
  const sessionDetail = await captureSessionDetailRoute(cdp);
  const settings = await captureSettingsTree(cdp);
  const homeSharedStateIntegration = await captureHomeSharedStateIntegration(cdp);
  const summary = buildSummary({
    snapshots,
    homeAgentDirectory,
    homeMultiSelect,
    sessionDetail,
    settings,
    moduleAssets,
    sharedStateProjection,
    homeSharedStateIntegration,
    localAgentClickChain,
  });
  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));

  const failed = Object.entries(summary.checks)
    .filter(([, value]) => value !== true)
    .map(([key]) => key);
  if (failed.length > 0) {
    throw new Error(`mobile_ui_tree_phase1_failed checks=${failed.join(',')} artifactDir=${artifactDir}`);
  }
  console.log(`mobile_ui_tree_phase1_ok url=${baseUrl} artifactDir=${artifactDir}`);
} finally {
  if (cdp) {
    await cdp.close().catch(() => null);
  }
  if (chrome && chrome.pid) {
    chrome.kill('SIGTERM');
  }
  await fs.rm(chromeProfileDir, { recursive: true, force: true }).catch(() => null);
}

async function productionAssetVersion() {
  const response = await fetch(baseUrl, { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`production WebUI not reachable: ${response.status} ${response.statusText}`);
  }
  const html = await response.text();
  const match = html.match(/(?:^|["'(\/])assets\/webui\.js\?v=([^"'&<>\s]+)/);
  if (!match || !match[1]) {
    throw new Error('served WebUI does not expose the owner-stamped asset version');
  }
  return decodeURIComponent(match[1]);
}

async function captureViewport(cdp, viewport) {
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: viewport.width,
    height: viewport.height,
    deviceScaleFactor: viewport.mobile ? 2 : 1,
    mobile: viewport.mobile,
  });
  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
  });
  await delay(350);
  const state = await evalInPage(cdp, collectPhaseOneState, assetVersion);
  const screenshot = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  const fileName = `${viewport.label}.png`;
  await fs.writeFile(path.join(artifactDir, fileName), Buffer.from(screenshot.data, 'base64'));
  const result = { viewport, screenshot: fileName, state };
  await fs.writeFile(path.join(artifactDir, `${viewport.label}.json`), JSON.stringify(result, null, 2));
  return result;
}

async function captureHomeAgentDirectory(cdp) {
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 2,
    mobile: true,
  });
  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
  });
  await waitForFunction(cdp, (agentIds) => {
    const summary = document.getElementById('mobile-agent-summary-strip');
    return document.body.dataset.webuiRoute === 'home_dashboard' &&
      summary && getComputedStyle(summary).display !== 'none' &&
      agentIds.every((agentId) => !!document.querySelector(`#mobile-agent-task-list [data-agent-id="${agentId}"]`));
  }, 20_000, 'Home Agent directory entry', agentDirectoryIds);
  await evalInPage(cdp, () => document.getElementById('open-mobile-agent-sheet-button')?.click());
  await waitForFunction(cdp, () => {
    const sheet = document.getElementById('mobile-agent-sheet');
    const rect = sheet?.getBoundingClientRect();
    return document.body.dataset.mobileAgentSheet === 'open' &&
      sheet && getComputedStyle(sheet).visibility === 'visible' &&
      Number.parseFloat(getComputedStyle(sheet).opacity || '0') >= 0.99 &&
      rect && rect.top < window.innerHeight && rect.bottom <= window.innerHeight + 1;
  }, 5_000, 'Home Agent directory sheet');
  const state = await evalInPage(cdp, (agentIds) => ({
    route: document.body.dataset.webuiRoute || '',
    selectedSession: document.querySelector('[data-webui-shell="true"]')?.dataset.selectedSession || '',
    agentIds: Array.from(document.querySelectorAll('#mobile-agent-task-list [data-agent-id]'))
      .map((node) => node.dataset.agentId || ''),
    rows: agentIds.map((agentId) => {
      const row = document.querySelector(`#mobile-agent-task-list [data-agent-id="${agentId}"]`);
      return {
        agentId,
        exists: !!row,
        text: row?.innerText || '',
      };
    }),
    noHorizontalOverflow:
      Math.max(document.body.scrollWidth, document.documentElement.scrollWidth) <= window.innerWidth + 2,
  }), agentDirectoryIds);
  const screenshot = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  const screenshotName = 'home-agent-directory.png';
  await fs.writeFile(path.join(artifactDir, screenshotName), Buffer.from(screenshot.data, 'base64'));
  await evalInPage(cdp, () => document.getElementById('close-mobile-agent-sheet-button')?.click());
  await waitForFunction(cdp, () => !document.body.dataset.mobileAgentSheet, 5_000, 'Home Agent directory closed');
  const result = { screenshot: screenshotName, state };
  await fs.writeFile(path.join(artifactDir, 'home-agent-directory.json'), JSON.stringify(result, null, 2));
  return result;
}

async function captureHomeMultiSelect(cdp) {
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 2,
    mobile: true,
  });
  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
  });
  await waitForFunction(cdp, (sessionIds) => {
    return document.body.dataset.webuiRoute === 'home_dashboard' &&
      sessionIds.every((sessionId) => !!document.querySelector(`#mobile-home-session-list [data-session-id="${sessionId}"] .mobile-home-session-checkbox`));
  }, 20_000, 'Home multi-select rows', multiSelectSessionIds);
  const stateBefore = await evalInPage(cdp, collectPhaseOneState, assetVersion);
  const selection = await evalInPage(cdp, (sessionIds) => {
    sessionIds.forEach((sessionId) => {
      const checkbox = document.querySelector(`#mobile-home-session-list [data-session-id="${sessionId}"] .mobile-home-session-checkbox`);
      if (checkbox && !checkbox.checked) {
        checkbox.click();
      }
    });
    return {
      selectedIds: Array.from(document.querySelectorAll('#mobile-home-session-list .mobile-home-session-checkbox:checked'))
        .map((node) => node.closest('[data-session-id]')?.dataset.sessionId || '')
        .filter(Boolean),
      bulkText: document.querySelector('#mobile-home-session-list .mobile-home-bulk-actions')?.innerText || '',
      bulkSelectedCount: document.querySelector('#mobile-home-session-list .mobile-home-bulk-actions')?.dataset.selectedCount || '',
      rows: sessionIds.map((sessionId) => {
        const row = document.querySelector(`#mobile-home-session-list [data-session-id="${sessionId}"]`);
        return {
          sessionId,
          exists: !!row,
          checked: !!row?.querySelector('.mobile-home-session-checkbox')?.checked,
          selectedClass: !!row?.classList.contains('is-selected'),
        };
      }),
    };
  }, multiSelectSessionIds);
  await delay(350);
  const stateAfter = await evalInPage(cdp, collectPhaseOneState, assetVersion);
  const screenshot = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  const screenshotName = 'home-multiselect.png';
  await fs.writeFile(path.join(artifactDir, screenshotName), Buffer.from(screenshot.data, 'base64'));
  const result = { screenshot: screenshotName, sessionIds: multiSelectSessionIds, stateBefore, selection, stateAfter };
  await fs.writeFile(path.join(artifactDir, 'home-multiselect.json'), JSON.stringify(result, null, 2));
  return result;
}

async function captureSessionDetailRoute(cdp) {
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 2,
    mobile: true,
  });
  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
  });
  await delay(350);
  const clickResult = await evalInPage(cdp, () => {
    const targetSessionId = 'webui-header-worker-rail-fixed';
    const row = document.querySelector(`#mobile-home-dashboard [data-session-id="${targetSessionId}"] .mobile-home-session-open`) ||
      document.querySelector('#mobile-home-dashboard [data-session-id] .mobile-home-session-open');
    if (!row) {
      return { clicked: false, reason: 'no_session_row' };
    }
    const host = row.closest('[data-session-id]');
    row.click();
    return { clicked: true, sessionId: host?.dataset.sessionId || '' };
  });
  if (!clickResult.clicked) {
    const state = await evalInPage(cdp, collectPhaseOneState, assetVersion);
    const result = { skipped: true, reason: clickResult.reason, state };
    await fs.writeFile(path.join(artifactDir, 'session-detail-route.json'), JSON.stringify(result, null, 2));
    return result;
  }
  await waitForFunction(cdp, (sessionId) => {
    return document.body.dataset.webuiRoute === 'session_detail' &&
      document.querySelector('[data-webui-shell="true"]')?.dataset.routeSession === sessionId &&
      !isVisibleForVerifier(document.getElementById('mobile-home-dashboard')) &&
      isVisibleForVerifier(document.querySelector('.conversation-region')) &&
      isVisibleForVerifier(document.getElementById('session-worker-rail')) &&
      document.querySelectorAll('#session-worker-rail .session-worker-row').length >= 2;

    function isVisibleForVerifier(node) {
      if (!node || node.hidden) return false;
      const style = getComputedStyle(node);
      const rect = node.getBoundingClientRect();
      return style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        Number.parseFloat(style.opacity || '1') > 0 &&
        rect.width > 0 &&
        rect.height > 0;
    }
  }, 12_000, 'SessionDetail route mutual exclusion', clickResult.sessionId);
  const state = await evalInPage(cdp, collectPhaseOneState, assetVersion);
  const expand = await evalInPage(cdp, () => {
    const pill = document.querySelector('#session-worker-rail .session-worker-pill');
    if (!pill) {
      return { clicked: false, reason: 'missing_worker_pill' };
    }
    const taskId = pill.dataset.taskId || '';
    pill.click();
    return { clicked: true, taskId };
  });
  if (expand.clicked) {
    await waitForFunction(cdp, (taskId) => {
      const rail = document.getElementById('session-worker-rail');
      const row = taskId ? document.querySelector(`#session-worker-rail .session-worker-row[data-task-id="${taskId}"]`) : null;
      return rail?.dataset.expandedTaskId === taskId &&
        !!row?.querySelector('.session-worker-detail') &&
        !!row?.querySelector('.session-worker-open-button');
    }, 5_000, 'Header worker detail expansion', expand.taskId);
  }
  const expandedState = await evalInPage(cdp, collectPhaseOneState, assetVersion);
  const screenshot = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  const screenshotName = 'session-detail-route.png';
  await fs.writeFile(path.join(artifactDir, screenshotName), Buffer.from(screenshot.data, 'base64'));
  const result = { skipped: false, clicked: clickResult, expand, screenshot: screenshotName, state, expandedState };
  await fs.writeFile(path.join(artifactDir, 'session-detail-route.json'), JSON.stringify(result, null, 2));
  return result;
}

async function captureSettingsTree(cdp) {
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 2,
    mobile: true,
  });
  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
    document.getElementById('open-settings-drawer-button')?.click();
  });
  await waitForFunction(cdp, () => {
    const shell = document.getElementById('settings-shell');
    return shell?.dataset.settingsCurrentPage === 'root' &&
      document.querySelector('[data-settings-page="root"]')?.dataset.settingsActive === 'true';
  }, 10_000, '设置 root visible');
  const root = await captureSettingsSnapshot(cdp, 'settings-01-root');

  await navigateSettingsPage(cdp, 'root', 'models');
  const models = await captureSettingsSnapshot(cdp, 'settings-02-models');

  await navigateSettingsPage(cdp, 'models', 'models.provider-config');
  await waitForFunction(cdp, () => {
    const providerId = document.getElementById('settings-provider-id')?.textContent?.trim() || '';
    return providerId && providerId !== '加载中';
  }, 10_000, '模型服务配置 权威真源');
  const providerConfiguration = await captureSettingsSnapshot(cdp, 'settings-03-provider-configuration');

  await navigateSettingsPage(cdp, 'models.provider-config', 'models');
  await navigateSettingsPage(cdp, 'models', 'models.strategy');
  const providerStrategy = await captureSettingsSnapshot(cdp, 'settings-04-provider-strategy');

  await navigateSettingsPage(cdp, 'models.strategy', 'models');
  await navigateSettingsPage(cdp, 'models', 'models.model-groups');
  const modelGroups = await captureSettingsSnapshot(cdp, 'settings-05-model-groups');

  await navigateSettingsPage(cdp, 'models.model-groups', 'models');
  await navigateSettingsPage(cdp, 'models', 'root');
  const returnedRoot = await captureSettingsSnapshot(cdp, 'settings-06-returned-root');

  const result = {
    root,
    models,
    providerConfiguration,
    providerStrategy,
    modelGroups,
    returnedRoot,
  };
  await fs.writeFile(path.join(artifactDir, 'settings-tree.json'), JSON.stringify(result, null, 2));
  return result;
}

async function captureModuleAssets() {
  const modules = [
    'assets/webui/app-shell/adp-client.js',
    'assets/webui/app-shell/route-controller.js',
    'assets/webui/app-shell/edge-registry.js',
    'assets/webui/app-shell/layout-shape.js',
    'assets/webui/app-shell/surface-registry.js',
    'assets/webui/app-shell/shared-states/index.js',
    'assets/webui/app-shell/shared-states/model.js',
    'assets/webui/app-shell/shared-states/view.js',
    'assets/webui/surfaces/home-dashboard/index.js',
    'assets/webui/surfaces/home-dashboard/view.js',
    'assets/webui/surfaces/home-dashboard/model.js',
    'assets/webui/surfaces/home-dashboard/controls.js',
    'assets/webui/surfaces/session-detail/index.js',
    'assets/webui/surfaces/session-detail/controls.js',
    'assets/webui/surfaces/tools-registry/index.js',
    'assets/webui/surfaces/tools-registry/view.js',
    'assets/webui/surfaces/tools-registry/controls.js',
    'assets/webui/surfaces/timer-dashboard/index.js',
    'assets/webui/surfaces/timer-dashboard/view.js',
    'assets/webui/surfaces/timer-dashboard/controls.js',
    'assets/webui/surfaces/settings/index.js',
    'assets/webui/surfaces/settings/view.js',
    'assets/webui/surfaces/settings/diagnostics.js',
    'assets/webui/surfaces/session-search/index.js',
    'assets/webui/surfaces/session-search/view.js',
    'assets/webui/surfaces/new-session/index.js',
    'assets/webui/surfaces/new-session/controls.js',
  ];
  const results = [];
  for (const assetPath of modules) {
    const response = await fetch(new URL(assetPath, baseUrl), { cache: 'no-store' });
    const text = await response.text();
    results.push({
      assetPath,
      ok: response.ok,
      status: response.status,
      containsModuleSyntax: /export\s+/.test(text),
      textContainsSurface:
        /home-dashboard|session-detail|tools-registry|timer-dashboard|settings|session-search|new-session|createAdpClient|createRouteController|edge-registry|classifyLayoutShape|renderHomeDashboard|createHomeDashboardModel|setSelectedSessionId|openToolsRegistrySurface|openTimerDashboardSurface/.test(text) ||
        /assets\/webui\/(app-shell|surfaces)\//.test(assetPath),
    });
  }
  return results;
}

async function captureSharedStateProjection(cdp) {
  const result = await evalInPage(cdp, async (expectedAssetVersion) => {
    const suffix = `?v=${encodeURIComponent(expectedAssetVersion)}`;
    const modelModule = await import(`/assets/webui/app-shell/shared-states/model.js${suffix}`);
    const viewModule = await import(`/assets/webui/app-shell/shared-states/view.js${suffix}`);
    const container = document.createElement('div');
    container.dataset.verifierOwned = 'shared-state-projection';
    document.body.append(container);
    try {
      const states = [];
      for (const [kind, title] of [
        [modelModule.SharedUiStateKind.Loading, '在线加载态验证'],
        [modelModule.SharedUiStateKind.Empty, '在线空态验证'],
      ]) {
        const model = modelModule.createSharedStateModel(kind, { title });
        viewModule.renderSharedState(container, model);
        states.push({
          kind,
          projectedKind: container.dataset.sharedState || '',
          role: container.querySelector('.shared-ui-state')?.getAttribute('role') || '',
          title: container.querySelector('.shared-ui-state-title')?.textContent || '',
          childCount: container.children.length,
        });
      }
      return { states };
    } finally {
      container.remove();
    }
  }, assetVersion);
  await fs.writeFile(
    path.join(artifactDir, 'shared-state-projection.json'),
    JSON.stringify(result, null, 2),
  );
  return result;
}

async function captureHomeSharedStateIntegration(cdp) {
  const result = await evalInPage(cdp, () => {
    const hooks = window.__freehandWebUiTest;
    if (!hooks || typeof hooks.projectHomeSharedStateForTest !== 'function') {
      throw new Error('Home shared-state test hook unavailable');
    }
    const loading = hooks.projectHomeSharedStateForTest({ loaded: false, sessions: [] });
    const empty = hooks.projectHomeSharedStateForTest({ loaded: true, sessions: [] });
    const populated = hooks.projectHomeSharedStateForTest({
      loaded: true,
      sessions: [{
        session_id: 'webui-home-shared-state-negative',
        title: '非空历史会话',
        active_turn_id: null,
        archived: false,
        updated_at: '2026-07-30T00:00:00Z',
      }],
    });
    const running = hooks.projectHomeSharedStateForTest({
      loaded: true,
      sessions: [{
        session_id: 'webui-home-shared-state-running-negative',
        title: '运行中会话',
        active_turn_id: 'turn-running',
        latest_turn_id: 'turn-running',
        latest_status: 'running',
        archived: false,
      }],
    });
    return { loading, empty, populated, running };
  });
  await fs.writeFile(
    path.join(artifactDir, 'home-shared-state-integration.json'),
    JSON.stringify(result, null, 2),
  );
  return result;
}

async function navigateSettingsPage(cdp, currentPage, targetPage) {
  const clickResult = await evalInPage(cdp, ({ currentPage, targetPage }) => {
    try {
      const current = Array.from(document.querySelectorAll('[data-settings-page]'))
        .find((panel) => panel.dataset.settingsPage === currentPage);
      const control = Array.from(current?.querySelectorAll('[data-settings-target]') || [])
        .find((candidate) => candidate.dataset.settingsTarget === targetPage);
      if (!current) {
        throw new Error(`missing current settings page ${currentPage}`);
      }
      if (!control) {
        throw new Error(`missing settings route ${currentPage} -> ${targetPage}`);
      }
      control.click();
    } catch (error) {
      return {
        ok: false,
        message: error?.message || String(error),
        currentPage,
        targetPage,
        settingsPage: document.getElementById('settings-shell')?.dataset.settingsCurrentPage || '',
        routes: Array.from(document.querySelectorAll('[data-settings-page]')).map((panel) => ({
          page: panel.dataset.settingsPage || '',
          hidden: panel.hidden,
          targets: Array.from(panel.querySelectorAll('[data-settings-target]')).map((control) => control.dataset.settingsTarget || ''),
        })),
      };
    }
    return { ok: true };
  }, { currentPage, targetPage });
  if (!clickResult?.ok) {
    throw new Error(`settings_navigation_failed ${JSON.stringify(clickResult)}`);
  }
  await waitForFunction(cdp, (targetPage) => {
    const shell = document.getElementById('settings-shell');
    const activePages = Array.from(document.querySelectorAll('[data-settings-page]'))
      .filter((panel) => !panel.hidden)
      .map((panel) => panel.dataset.settingsPage);
    return shell?.dataset.settingsCurrentPage === targetPage &&
      activePages.length === 1 &&
      activePages[0] === targetPage;
  }, 10_000, `设置 page ${targetPage}`, targetPage);
}

async function captureSettingsSnapshot(cdp, fileBase) {
  const state = await evalInPage(cdp, collectPhaseOneState, assetVersion);
  const screenshot = await cdp.send('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: true,
  });
  const screenshotName = `${fileBase}.png`;
  await fs.writeFile(path.join(artifactDir, screenshotName), Buffer.from(screenshot.data, 'base64'));
  const result = { screenshot: screenshotName, state };
  await fs.writeFile(path.join(artifactDir, `${fileBase}.json`), JSON.stringify(result, null, 2));
  return result;
}

async function captureLocalAgentClickChain(cdp) {
  const markerSessionIds = localAgentTargets.map((target) => target.markerSessionId);
  const captures = [];
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 2,
    mobile: true,
  });
  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    window.__freehandLayout?.applyLayoutShape?.();
  });
  for (const target of localAgentTargets) {
    await waitForFunction(cdp, (agentId) => {
      return document.body.dataset.webuiRoute === 'home_dashboard' &&
        document.body.dataset.webuiJsReady === 'true' &&
        !!document.querySelector(`#mobile-agent-task-list [data-agent-id="${agentId}"]`);
    }, 45_000, `Agent ${target.agentId} Home ready`, target.agentId);
    await evalInPage(cdp, () => document.getElementById('open-mobile-agent-sheet-button')?.click());
    await waitForFunction(cdp, () => {
      return document.body.dataset.mobileAgentSheet === 'open';
    }, 5_000, `Agent ${target.agentId} directory open`);
    const click = await evalInPage(cdp, (agentId) => {
      const row = document.querySelector(`#mobile-agent-task-list [data-agent-id="${agentId}"]`);
      if (!row) return false;
      row.click();
      return true;
    }, target.agentId);
    if (!click) {
      throw new Error(`missing Agent directory row ${target.agentId}`);
    }
    await waitForFunction(cdp, ({ origin, markerSessionId }) => {
      return window.location.origin === origin &&
        document.body.dataset.webuiRoute === 'home_dashboard' &&
        document.body.dataset.webuiJsReady === 'true' &&
        !!document.querySelector(`#mobile-home-session-list [data-session-id="${markerSessionId}"]`);
    }, 45_000, `Agent ${target.agentId} namespace`, {
      origin: target.origin,
      markerSessionId: target.markerSessionId,
    });
    await delay(500);
    const state = await evalInPage(cdp, ({ markerSessionIds, target }) => {
      const sessionIds = Array.from(document.querySelectorAll('#mobile-home-session-list [data-session-id]'))
        .map((row) => row.dataset.sessionId || '');
      return {
        agentId: target.agentId,
        expectedOrigin: target.origin,
        actualOrigin: window.location.origin,
        route: document.body.dataset.webuiRoute || '',
        selectedSession: document.querySelector('[data-webui-shell="true"]')?.dataset.selectedSession || '',
        sessionGroupLabel: document.querySelector('.session-agent-name')?.textContent?.trim() || '',
        markerSessionId: target.markerSessionId,
        sessionIds,
        foreignMarkerSessionIds: markerSessionIds.filter((sessionId) =>
          sessionId !== target.markerSessionId && sessionIds.includes(sessionId)
        ),
        noHorizontalOverflow:
          Math.max(document.body.scrollWidth, document.documentElement.scrollWidth) <= window.innerWidth + 2,
      };
    }, { markerSessionIds, target });
    const screenshot = await cdp.send('Page.captureScreenshot', {
      format: 'png',
      captureBeyondViewport: true,
    });
    const screenshotName = `local-agent-${target.agentId}.png`;
    await fs.writeFile(path.join(artifactDir, screenshotName), Buffer.from(screenshot.data, 'base64'));
    captures.push({ ...state, screenshot: screenshotName });
  }
  await fs.writeFile(
    path.join(artifactDir, 'local-agent-click-chain.json'),
    JSON.stringify(captures, null, 2),
  );
  return captures;
}

function buildSummary({
  snapshots,
  homeAgentDirectory,
  homeMultiSelect,
  sessionDetail,
  settings,
  moduleAssets,
  sharedStateProjection,
  homeSharedStateIntegration,
  localAgentClickChain,
}) {
  const portraitSnapshots = snapshots.filter((snapshot) =>
    ['phone_portrait', 'tall_phone', 'tablet_portrait'].includes(snapshot.state.layoutShape)
  );
  const rootSettings = settings.root.state;
  const modelsSettings = settings.models.state;
  const providerConfiguration = settings.providerConfiguration.state;
  const providerStrategy = settings.providerStrategy.state;
  const modelGroups = settings.modelGroups.state;
  const returnedRoot = settings.returnedRoot.state;
  const allTexts = [rootSettings.bodyText, ...snapshots.map((snapshot) => snapshot.state.bodyText)].join('\n');
  const globalSessionText = snapshots.map((snapshot) => snapshot.state.globalSessionText).join('\n');
  const portraitEntriesVisibleAndSeparated = portraitSnapshots.every((snapshot) =>
    snapshot.state.quickEntries.iconOnly &&
    snapshot.state.quickEntries.visibleCount === quickEntryIds.length &&
    snapshot.state.quickEntries.positionsSeparated
  );
  const summary = {
    ok: true,
    baseUrl,
    artifactDir,
    assetVersion,
    snapshots,
    homeAgentDirectory,
    homeMultiSelect,
    sessionDetail,
    settings,
    moduleAssets,
    sharedStateProjection,
    localAgentClickChain,
    checks: {
      productionAssetVersion: snapshots.every((snapshot) => snapshot.state.assetVersionSeen),
      viewportMatrixCovered: snapshots.length === viewports.length,
      noHorizontalOverflow: snapshots.every((snapshot) => snapshot.state.noHorizontalOverflow),
      portraitQuickEntriesIconOnly: portraitEntriesVisibleAndSeparated,
      mobileHomeDashboardVisible: portraitSnapshots.every((snapshot) => snapshot.state.mobileHomeDashboardVisible),
      mobileHomeAgentEntryVisible: portraitSnapshots.every((snapshot) => snapshot.state.mobileAgentSummaryVisible),
      mobileHomeAgentDirectoryOpens:
        homeAgentDirectory.state.route === 'home_dashboard' &&
        homeAgentDirectory.state.selectedSession === '' &&
        homeAgentDirectory.state.noHorizontalOverflow &&
        homeAgentDirectory.state.agentIds.join(',') === agentDirectoryIds.join(',') &&
        homeAgentDirectory.state.rows.every((row) => row.exists && row.text.includes('进入独立会话')),
      localAgentClickChainIsolated: localAgentClickChain.every((capture) =>
        capture.actualOrigin === capture.expectedOrigin &&
        capture.route === 'home_dashboard' &&
        capture.selectedSession === '' &&
        capture.sessionIds.includes(capture.markerSessionId) &&
        capture.foreignMarkerSessionIds.length === 0 &&
        capture.noHorizontalOverflow
      ),
      localAgentSessionGroupIdentity: localAgentClickChain.every((capture) =>
        capture.sessionGroupLabel === localAgentTargets.find((target) => target.agentId === capture.agentId)?.label
      ),
      sharedStateContractLoaded: snapshots.every((snapshot) => snapshot.state.sharedStateContractLoaded),
      sharedEmptyAndLoadingRendered:
        sharedStateProjection.states.length === 2 &&
        sharedStateProjection.states.every((state) =>
          state.projectedKind === state.kind &&
          state.role === 'status' &&
          state.title.length > 0 &&
          state.childCount === 1
        ),
      homeSharedStateIntegration:
        homeSharedStateIntegration.loading.activeState === 'loading' &&
        homeSharedStateIntegration.loading.historyStates.includes('loading') &&
        homeSharedStateIntegration.empty.activeState === 'empty' &&
        homeSharedStateIntegration.empty.historyStates.includes('empty'),
      populatedHomeDoesNotRenderSharedHistoryState:
        homeSharedStateIntegration.populated.historyStates.length === 0 &&
        homeSharedStateIntegration.populated.historySessionIds.includes('webui-home-shared-state-negative'),
      runningHomeClearsSharedActiveState:
        homeSharedStateIntegration.running.activeState === '' &&
        homeSharedStateIntegration.running.historySessionIds.length === 0,
      desktopDoesNotForceMobileHome: snapshots
        .filter((snapshot) => snapshot.viewport.width >= 1180)
        .every((snapshot) => !snapshot.state.mobileHomeDashboardVisible),
      sessionDetailMutualExclusion:
        sessionDetail &&
        !sessionDetail.skipped &&
        sessionDetail.state.webuiRoute === 'session_detail' &&
        sessionDetail.state.shellRoute === 'session_detail' &&
        !sessionDetail.state.mobileHomeDashboardVisible &&
        sessionDetail.state.conversationRegionVisible &&
        sessionDetail.state.sessionRelationHeaderVisible &&
        !sessionDetail.state.mobileAgentSummaryVisible,
      headerWorkerRailShowsDurationStatus:
        sessionDetail &&
        !sessionDetail.skipped &&
        sessionDetail.state.sessionWorkerRailVisible &&
        sessionDetail.state.sessionWorkerRailCount >= 2 &&
        workerRailTaskIds.every((taskId) => sessionDetail.state.sessionWorkerRailRows.some((row) =>
          row.taskId === taskId &&
          row.relationSchema === 'UiTaskSnapshotProjection' &&
          row.relationSource === 'TaskBoard.worker_session_id' &&
          row.workerSessionId &&
          row.workerLabel &&
          row.statusText &&
          row.durationText &&
          row.durationText !== '时间不可用' &&
          ['live', 'frozen'].includes(row.durationState) &&
          row.height <= 56
        )),
      headerWorkerRailClickExpandsDetails:
        sessionDetail &&
        !sessionDetail.skipped &&
        sessionDetail.expand?.clicked === true &&
        sessionDetail.expandedState?.sessionWorkerRailExpandedTaskId === sessionDetail.expand.taskId &&
        sessionDetail.expandedState?.sessionWorkerRailRows.some((row) =>
          row.taskId === sessionDetail.expand.taskId &&
          row.expanded &&
          row.detailVisible &&
          row.openButtonExists &&
          row.detailText.includes('持续')
        ),
      masterWaitComposerUsable:
        sessionDetail &&
        !sessionDetail.skipped &&
        sessionDetail.state.composerFormVisible &&
        sessionDetail.state.composerInputVisible &&
        !sessionDetail.state.composerInputDisabled,
      mobileHistoryBucketsFixed: portraitSnapshots.every((snapshot) =>
        snapshot.state.mobileHomeBucketLabels.join(',') === '今天,过去一周,所有更早的'
      ),
      mobileRowsSingleLine: portraitSnapshots.every((snapshot) =>
        snapshot.state.mobileHomeHistoryRows.every((row) =>
          row.height <= 42 &&
          row.hasCheckbox &&
          !row.hasActions &&
          !row.hasRenameAction &&
          !row.hasRemoveAction &&
          row.sessionKind !== 'worker' &&
          row.openButtonHeight <= 32
        )
      ),
      homeMultiSelectWorks:
        homeMultiSelect &&
        homeMultiSelect.selection.rows.every((row) => row.exists && row.checked && row.selectedClass) &&
        homeMultiSelect.selection.bulkSelectedCount === `${multiSelectSessionIds.length}` &&
        homeMultiSelect.stateAfter.mobileHomeBulkActionsText.includes(`已选 ${multiSelectSessionIds.length} 个会话`),
      homeRenameOnlyInSessionDetail:
        portraitSnapshots.every((snapshot) =>
          !snapshot.state.mobileHomeHistoryRows.some((row) => row.hasRenameAction) &&
          snapshot.state.selectedSessionRenameVisible === false
        ) &&
        sessionDetail &&
        !sessionDetail.skipped &&
        sessionDetail.state.selectedSessionRenameVisible === true,
      globalSessionListExcludesInternalSessions: !internalSessionTerms.some((pattern) => pattern.test(globalSessionText)),
      homeShowsOnlyActivityAndHistory: portraitSnapshots.every((snapshot) =>
        snapshot.state.mobileHomeActiveVisible &&
        snapshot.state.mobileHomeHistoryVisible &&
        snapshot.state.mobileHomeRunningClass &&
        snapshot.state.mobileHomeStaticClass &&
        snapshot.state.mobileHomeRunningHistoryOverlap.length === 0 &&
        !snapshot.state.mobileHomeFloatingTree &&
        snapshot.state.mobileHomeText.includes('正在运行') &&
        snapshot.state.mobileHomeText.includes('历史会话') &&
        !snapshot.state.mobileHomeText.includes('waitingUser') &&
        !snapshot.state.homeHasTimerList &&
        !snapshot.state.homeHasTimerMarker &&
        !snapshot.state.homeHasCurrentCard &&
        !snapshot.state.homeHasNewEntryButtonInsideHome &&
        !snapshot.state.mobileHomeText.includes('timer dashboard') &&
        !snapshot.state.mobileHomeText.includes('Timer 权威真源') &&
        snapshot.state.mobileHomeCardCount >= 2
      ),
      settingsRootOnlyTopLevel:
        rootSettings.settingsPage === 'root' &&
        rootSettings.visibleSettingsPages.join(',') === 'root' &&
        rootSettings.settingsNavTopTitles.join(',') === '模型,智能体运行时,连接,可观测性,外观,关于',
      settingsRootHidesAllDetailControls: rootSettings.visibleSettingsDetailControlIds.length === 0,
      settingsRootHasNoImplementationMapNode:
        !rootSettings.settingsReviewTreeExists &&
        !rootSettings.settingsReviewTreeVisible &&
        !rootSettings.settingsReviewTreeText,
      settingsRootHasNoStatusHeroCard: !rootSettings.settingsHeroExists,
      settingsRootHasNoDuplicateTopLevelLabels:
        ['模型', '智能体运行时', '连接', '可观测性', '外观', '关于']
          .every((label) => rootSettings.visibleSettingsTitleCounts[label] === 1),
      settingsModelSecondLevelOnly:
        modelsSettings.settingsPage === 'models' &&
        modelsSettings.visibleSettingsPages.join(',') === 'models' &&
        ['模型服务配置', '模型服务切换与策略', '模型组'].every(
          (label) => modelsSettings.visibleSettingsNavTitles.includes(label),
        ) &&
        modelsSettings.visibleSettingsDetailControlIds.length === 0,
      settingsProviderConfigurationDrilldown:
        providerConfiguration.settingsPage === 'models.provider-config' &&
        providerConfiguration.providerConfigPageVisible &&
        providerConfiguration.visibleSettingsDetailControlIds.includes('settings-provider-form') &&
        !providerConfiguration.providerStrategyPageVisible &&
        !providerConfiguration.modelGroupsPageVisible,
      settingsProviderStrategyDrilldown:
        providerStrategy.settingsPage === 'models.strategy' &&
        providerStrategy.providerStrategyPageVisible &&
        providerStrategy.visibleSettingsDetailControlIds.includes('settings-provider-current-select') &&
        providerStrategy.visibleSettingsDetailControlIds.includes('settings-provider-fallback-select') &&
        !providerStrategy.providerConfigPageVisible &&
        !providerStrategy.modelGroupsPageVisible,
      settingsModelGroupsDrilldown:
        modelGroups.settingsPage === 'models.model-groups' &&
        modelGroups.modelGroupsPageVisible &&
        modelGroups.visibleSettingsDetailControlIds.includes('settings-model-group-form') &&
        !modelGroups.providerConfigPageVisible &&
        !modelGroups.providerStrategyPageVisible,
      settingsBackReturnsCleanRoot:
        returnedRoot.settingsPage === 'root' &&
        returnedRoot.visibleSettingsPages.join(',') === 'root' &&
        returnedRoot.visibleSettingsDetailControlIds.length === 0,
      settingsProviderPagesAreSplit:
        rootSettings.providerConfigPageExists &&
        rootSettings.providerStrategyPageExists &&
        rootSettings.modelGroupsPageExists,
      settingsTopLevelGrouped: ['模型', '智能体运行时', '连接', '可观测性', '外观', '关于'].every((label) => rootSettings.settingsNavText.includes(label)),
      settingsNoFlatLlmProviderEntry: !rootSettings.settingsNavTopTitles.includes('LLM Provider'),
      settingsPartialMarkersPresent: rootSettings.statusMarkerToneCounts.partial > 0,
      settingsAttentionMarkersPresent: rootSettings.statusMarkerToneCounts.attention > 0,
      diagnosticsIsObservabilityDetail: rootSettings.diagnosticsPageExists && rootSettings.diagnosticsGroup === 'observability',
      modularWebuiAssets: Array.isArray(moduleAssets) && moduleAssets.length > 0 && moduleAssets.every((asset) => asset.ok && asset.containsModuleSyntax),
      modularSurfaceAssets: Array.isArray(moduleAssets) && moduleAssets.every((asset) => asset.textContainsSurface),
      noForbiddenUiStorageTerms: !forbiddenUiTerms.some((pattern) => pattern.test(allTexts)),
      statusMarkersAreHollow: rootSettings.statusMarkerCount > 0 && rootSettings.statusMarkerAllHollow,
    },
  };
  summary.ok = Object.values(summary.checks).every(Boolean);
  return summary;
}

function collectPhaseOneState(expectedAssetVersion) {
  function localRectOf(node) {
    if (!node) {
      return { left: 0, top: 0, width: 0, height: 0, right: 0, bottom: 0 };
    }
    const rect = node.getBoundingClientRect();
    return {
      left: Math.round(rect.left),
      top: Math.round(rect.top),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
      right: Math.round(rect.right),
      bottom: Math.round(rect.bottom),
    };
  }

  function localIsVisible(node) {
    if (!node || node.hidden) {
      return false;
    }
    const style = getComputedStyle(node);
    const rect = node.getBoundingClientRect();
    return style.display !== 'none' &&
      style.visibility !== 'hidden' &&
      Number.parseFloat(style.opacity || '1') > 0 &&
      rect.width > 0 &&
      rect.height > 0;
  }

  function localQuickEntryPositionsSeparated(entries) {
    const width = window.innerWidth;
    const height = window.innerHeight;
    const settings = entries['open-settings-drawer-button'];
    const timer = entries['open-timer-dashboard-button'];
    const tools = entries['open-tools-dashboard-button'];
    const create = entries['mobile-new-entry-button'];
    const search = entries['open-session-drawer-button'];
    if (![settings, timer, tools, create, search].every((entry) => entry && entry.visible)) {
      return false;
    }
    return (
      settings.rect.left < width * 0.35 &&
      settings.rect.top < 110 &&
      timer.rect.left > width * 0.50 &&
      tools.rect.left > timer.rect.left &&
      timer.rect.top < 110 &&
      tools.rect.top < 110 &&
      create.rect.left < width * 0.35 &&
      search.rect.left > width * 0.55 &&
      create.rect.top > height * 0.45 &&
      search.rect.top > height * 0.45
    );
  }

  const shell = document.querySelector('[data-webui-shell="true"]');
  const html = document.documentElement.outerHTML;
  const bodyText = document.body.innerText || '';
  const globalSessionText = document.getElementById('session-list')?.innerText || '';
  const quickEntries = {};
  for (const id of [
    'open-settings-drawer-button',
    'open-timer-dashboard-button',
    'open-tools-dashboard-button',
    'mobile-new-entry-button',
    'open-session-drawer-button',
  ]) {
    const node = document.getElementById(id);
    quickEntries[id] = {
      exists: !!node,
      visible: localIsVisible(node),
      text: node?.textContent?.trim() || '',
      hasSvg: !!node?.querySelector('svg'),
      rect: localRectOf(node),
    };
  }
  const visibleEntries = Object.values(quickEntries).filter((entry) => entry.visible);
  const markerNodes = Array.from(document.querySelectorAll('.settings-status-marker'));
  const settingsShell = document.getElementById('settings-shell');
  const settingsPages = Array.from(document.querySelectorAll('[data-settings-page]'));
  const sessionWorkerRail = document.getElementById('session-worker-rail');
  const detailControlIds = [
    'settings-provider-id',
    'settings-provider-form',
    'settings-provider-current-select',
    'settings-provider-fallback-select',
    'settings-model-group-current-select',
    'settings-model-group-form',
    'settings-agent-resource-count',
    'settings-apk-update-check-button',
    'settings-diagnostics-list',
  ];
  return {
    layoutShape: document.body.dataset.layoutShape || '',
    shellLayoutShape: shell?.dataset.layoutShape || '',
    webuiRoute: document.body.dataset.webuiRoute || '',
    shellRoute: shell?.dataset.webuiRoute || '',
    routeSession: shell?.dataset.routeSession || '',
    selectedSession: shell?.dataset.selectedSession || '',
    assetVersionSeen: html.includes(expectedAssetVersion),
    bodyText,
    bodyWidth: document.body.scrollWidth,
    docWidth: document.documentElement.scrollWidth,
    viewportWidth: window.innerWidth,
    noHorizontalOverflow:
      Math.max(document.body.scrollWidth, document.documentElement.scrollWidth) <= window.innerWidth + 2,
    globalSessionText,
    mobileHomeDashboardVisible: localIsVisible(document.getElementById('mobile-home-dashboard')),
    sharedStateContractLoaded: window.__freehandSharedStateContract?.contractId === 'foundation.shared_states',
    mobileHomeText: document.getElementById('mobile-home-dashboard')?.innerText || '',
    mobileHomeActiveVisible: localIsVisible(document.getElementById('mobile-home-active-list')),
    mobileHomeHistoryVisible: localIsVisible(document.getElementById('mobile-home-session-list')),
    mobileHomeSessionCountVisible: localIsVisible(document.getElementById('mobile-home-session-count')),
    conversationRegionVisible: localIsVisible(document.querySelector('.conversation-region')),
    sessionRelationHeaderVisible: localIsVisible(document.getElementById('session-relation-header')),
    sessionWorkerRailVisible: localIsVisible(sessionWorkerRail),
    sessionWorkerRailCount: Number(sessionWorkerRail?.dataset.workerCount || 0),
    sessionWorkerRailExpandedTaskId: sessionWorkerRail?.dataset.expandedTaskId || '',
    sessionWorkerRailRows: Array.from(document.querySelectorAll('#session-worker-rail .session-worker-row')).map((node) => {
      const detail = node.querySelector('.session-worker-detail');
      const openButton = node.querySelector('.session-worker-open-button');
      return {
        taskId: node.dataset.taskId || '',
        workerSessionId: node.dataset.workerSessionId || '',
        workerLabel: node.dataset.workerLabel || '',
        relationSchema: node.dataset.relationSchema || '',
        relationSource: node.dataset.relationSource || '',
        durationState: node.dataset.durationState || '',
        status: node.dataset.status || '',
        statusText: node.querySelector('.session-worker-meta')?.textContent?.trim() || '',
        durationText: node.querySelector('.session-worker-duration')?.textContent?.trim() || '',
        expanded: node.classList.contains('is-expanded'),
        selected: node.classList.contains('is-selected'),
        height: localRectOf(node.querySelector('.session-worker-pill')).height,
        detailVisible: localIsVisible(detail),
        openButtonExists: !!openButton,
        openButtonDisabled: !!openButton?.disabled,
        detailText: detail?.innerText || '',
        text: node.innerText || '',
      };
    }),
    mobileAgentSummaryVisible: localIsVisible(document.getElementById('mobile-agent-summary-strip')),
    mobileHomeRunningClass: document.getElementById('mobile-home-active-list')?.classList.contains('mobile-running-session-list') || false,
    mobileHomeStaticClass: document.getElementById('mobile-home-session-list')?.classList.contains('mobile-static-session-list') || false,
    mobileHomeBucketLabels: Array.from(document.querySelectorAll('#mobile-home-session-list .mobile-home-history-bucket span')).map((node) => node.textContent?.trim() || ''),
    mobileHomeRunningIds: Array.from(document.querySelectorAll('#mobile-home-active-list [data-session-id]'))
      .map((node) => node.dataset.sessionId || '')
      .filter(Boolean),
    mobileHomeHistoryIds: Array.from(document.querySelectorAll('#mobile-home-session-list [data-session-id]'))
      .map((node) => node.dataset.sessionId || '')
      .filter(Boolean),
      mobileHomeHistoryRows: Array.from(document.querySelectorAll('#mobile-home-session-list .mobile-home-session-item')).map((node) => ({
      sessionId: node.dataset.sessionId || '',
      sessionKind: node.dataset.sessionKind || '',
      height: localRectOf(node).height,
      width: localRectOf(node).width,
      openButtonHeight: localRectOf(node.querySelector('.mobile-home-session-open')).height,
      lineCount: (node.innerText || '').split('\n').length,
      hasCheckbox: !!node.querySelector('.mobile-home-session-checkbox'),
      checkboxChecked: !!node.querySelector('.mobile-home-session-checkbox')?.checked,
      selectedClass: node.classList.contains('is-selected'),
      hasActions: !!node.querySelector('.mobile-home-session-actions'),
      hasRenameAction: !!node.querySelector('[data-session-action="rename"]'),
      hasRemoveAction: !!node.querySelector('[data-session-action="remove"]'),
      text: node.innerText || '',
    })),
    mobileHomeBulkActionsText: document.querySelector('#mobile-home-session-list .mobile-home-bulk-actions')?.innerText || '',
    mobileHomeBulkSelectedCount: document.querySelector('#mobile-home-session-list .mobile-home-bulk-actions')?.dataset.selectedCount || '',
    mobileHomeBulkSelectableCount: document.querySelector('#mobile-home-session-list .mobile-home-bulk-actions')?.dataset.selectableCount || '',
    selectedSessionRenameVisible: localIsVisible(document.getElementById('selected-session-rename-button')),
    selectedSessionRenameDisabled: !!document.getElementById('selected-session-rename-button')?.disabled,
    composerFormVisible: localIsVisible(document.getElementById('composer-form')),
    composerInputVisible: localIsVisible(document.getElementById('composer-input')),
    composerInputDisabled: !!document.getElementById('composer-input')?.disabled,
    mobileHomeRunningHistoryOverlap: (() => {
      const running = new Set(Array.from(document.querySelectorAll('#mobile-home-active-list [data-session-id]'))
        .map((node) => node.dataset.sessionId || '')
        .filter(Boolean));
      return Array.from(document.querySelectorAll('#mobile-home-session-list [data-session-id]'))
        .map((node) => node.dataset.sessionId || '')
        .filter((sessionId) => sessionId && running.has(sessionId));
    })(),
    mobileHomeCardCount: document.querySelectorAll('#mobile-home-dashboard .mobile-home-card').length,
    mobileHomeHistoryGroupCount: document.querySelectorAll('#mobile-home-session-list .mobile-home-history-bucket').length,
    mobileHomeFloatingTree: (() => {
      const dropdown = document.getElementById('session-tree-dropdown');
      if (!dropdown) return false;
      const style = getComputedStyle(dropdown);
      return style.position === 'absolute' || style.position === 'fixed';
    })(),
    homeHasTimerList: !!document.getElementById('mobile-home-timer-list'),
    homeHasTimerMarker: !!document.getElementById('mobile-home-timer-marker'),
    homeHasCurrentCard: !!document.querySelector('#mobile-home-dashboard .mobile-current-card'),
    homeHasNewEntryButtonInsideHome: !!document.querySelector('#mobile-home-dashboard #mobile-new-entry-button'),
    settingsReviewTreeExists: !!document.getElementById('settings-review-tree'),
    settingsReviewTreeVisible: localIsVisible(document.getElementById('settings-review-tree')),
    settingsReviewTreeText: document.getElementById('settings-review-tree')?.textContent || '',
    settingsHeroExists: !!document.querySelector('.settings-hero, .settings-card'),
    settingsPage: settingsShell?.dataset.settingsCurrentPage || '',
    visibleSettingsPages: settingsPages.filter((panel) => !panel.hidden).map((panel) => panel.dataset.settingsPage || ''),
    visibleSettingsText: settingsPages.find((panel) => !panel.hidden)?.innerText || '',
    settingsNavText: document.querySelector('[data-settings-page="root"] .settings-nav-grid')?.innerText || '',
    settingsNavTopTitles: Array.from(document.querySelectorAll('[data-settings-page="root"] .settings-nav-card strong')).map((node) => node.textContent?.trim() || ''),
    visibleSettingsNavTitles: Array.from(document.querySelectorAll('[data-settings-page]:not([hidden]) .settings-nav-card strong')).map((node) => node.textContent?.trim() || ''),
    visibleSettingsTitleCounts: ['模型', '智能体运行时', '连接', '可观测性', '外观', '关于'].reduce((counts, label) => {
      const text = settingsPages.find((panel) => !panel.hidden)?.innerText || '';
      counts[label] = (text.match(new RegExp(`(^|\\n)${label}(\\n|$)`, 'g')) || []).length;
      return counts;
    }, {}),
    visibleSettingsDetailControlIds: detailControlIds.filter((id) => localIsVisible(document.getElementById(id))),
    providerConfigPageExists: !!document.getElementById('settings-provider-config-page'),
    providerStrategyPageExists: !!document.getElementById('settings-provider-strategy-page'),
    modelGroupsPageExists: !!document.getElementById('settings-model-groups-page'),
    providerConfigPageVisible: localIsVisible(document.getElementById('settings-provider-config-page')),
    providerStrategyPageVisible: localIsVisible(document.getElementById('settings-provider-strategy-page')),
    modelGroupsPageVisible: localIsVisible(document.getElementById('settings-model-groups-page')),
    diagnosticsPageExists: !!document.querySelector('.settings-diagnostics-page'),
    diagnosticsGroup: document.querySelector('.settings-diagnostics-page')?.dataset.settingsGroup || '',
    quickEntries: {
      items: quickEntries,
      visibleCount: visibleEntries.length,
      iconOnly: Object.values(quickEntries).every((entry) => entry.exists && entry.hasSvg && entry.text === ''),
      positionsSeparated: localQuickEntryPositionsSeparated(quickEntries),
    },
    statusMarkerCount: markerNodes.length,
    statusMarkerToneCounts: markerNodes.reduce((counts, node) => {
      ['ok', 'partial', 'attention'].forEach((tone) => {
        if (node.classList.contains(tone)) {
          counts[tone] = (counts[tone] || 0) + 1;
        }
      });
      return counts;
    }, {}),
    statusMarkerAllHollow: markerNodes.every((node) => {
      const style = getComputedStyle(node);
      return style.backgroundColor === 'rgba(0, 0, 0, 0)' || style.backgroundColor === 'transparent';
    }),
  };
}

function quickEntryPositionsSeparated(entries) {
  const width = window.innerWidth;
  const height = window.innerHeight;
  const settings = entries['open-settings-drawer-button'];
  const timer = entries['open-timer-dashboard-button'];
  const tools = entries['open-tools-dashboard-button'];
  const create = entries['mobile-new-entry-button'];
  const search = entries['open-session-drawer-button'];
  if (![settings, timer, tools, create, search].every((entry) => entry && entry.visible)) {
    return false;
  }
  return (
    settings.rect.left < width * 0.35 &&
    settings.rect.top < 110 &&
    timer.rect.left > width * 0.50 &&
    tools.rect.left > timer.rect.left &&
    timer.rect.top < 110 &&
    tools.rect.top < 110 &&
    create.rect.left < width * 0.35 &&
    search.rect.left > width * 0.55 &&
    create.rect.top > height * 0.45 &&
    search.rect.top > height * 0.45
  );
}

function rectOf(node) {
  if (!node) {
    return { left: 0, top: 0, width: 0, height: 0, right: 0, bottom: 0 };
  }
  const rect = node.getBoundingClientRect();
  return {
    left: Math.round(rect.left),
    top: Math.round(rect.top),
    width: Math.round(rect.width),
    height: Math.round(rect.height),
    right: Math.round(rect.right),
    bottom: Math.round(rect.bottom),
  };
}

function isVisible(node) {
  if (!node || node.hidden) {
    return false;
  }
  const style = getComputedStyle(node);
  const rect = node.getBoundingClientRect();
  return style.display !== 'none' &&
    style.visibility !== 'hidden' &&
    Number.parseFloat(style.opacity || '1') > 0 &&
    rect.width > 0 &&
    rect.height > 0;
}

async function waitForFunction(cdp, fn, timeoutMs, label, arg) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const result = await evalInPage(cdp, fn, arg);
    if (result) {
      return;
    }
    await delay(250);
  }
  throw new Error(`timeout waiting for ${label}`);
}

async function evalInPage(cdp, fn, arg) {
  const args = arg === undefined ? "" : JSON.stringify(arg);
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
      // Wait for Chrome DevTools.
    }
    await delay(250);
  }
  throw new Error('timeout waiting for Chrome DevTools page target');
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

function normalizedBaseUrl(value) {
  const url = new URL(value);
  if (!url.pathname.endsWith('/')) {
    url.pathname = `${url.pathname}/`;
  }
  return url.toString();
}

function adpUrlFromBaseUrl(url) {
  const parsed = new URL(url);
  parsed.protocol = parsed.protocol === 'https:' ? 'wss:' : 'ws:';
  parsed.pathname = '/adp';
  parsed.search = '';
  parsed.hash = '';
  return parsed.toString();
}

async function ensureMultiSelectSessions() {
  for (const sessionId of multiSelectSessionIds) {
    const title = `Home 多选验证 ${sessionId}`;
    try {
      await adpCommand({ CreateSession: { session_id: sessionId, title } }, 20_000);
    } catch (error) {
      await adpCommand({ RenameSession: { session_id: sessionId, title } }, 20_000);
    }
    await adpCommand({ RestoreSession: { session_id: sessionId } }, 20_000);
  }
}

async function ensureHeaderWorkerRailTruth() {
  await fs.mkdir('/tmp/freehand-header-worker-rail', { recursive: true });
  await ensureSession(workerRailSessionId, 'Header Worker 状态验证');
  for (const agentId of ['worker', 'worker-2']) {
    try {
      await adpCommand({ CreateTaskAgent: { agent: { agent_id: agentId, capabilities: ['repository'] } } }, 20_000);
    } catch (error) {
      if (!/already exists|already_exists|AgentAlreadyExists|exists/i.test(error.message || '')) {
        throw error;
      }
    }
  }
  for (const [index, taskId] of workerRailTaskIds.entries()) {
    const agentId = index === 0 ? 'worker' : 'worker-2';
    try {
      await adpCommand({
        CreateTask: {
          task: {
            task_id: taskId,
            title: `Header Worker ${index + 1}`,
            content: 'Verifier-owned task for Header worker rail rendering.',
            goal: 'Prove Header shows worker duration, realtime status, and expandable detail.',
            deliverables: ['Header Worker rail row with status, duration, and expandable details.'],
            acceptance: ['DOM row uses TaskBoard worker_session_id and never synthesizes a Worker session id.'],
            priority: 20 - index,
            target_cwd: '/tmp/freehand-header-worker-rail',
            execution_profile: 'workspace',
            session_id: workerRailSessionId,
            dispatch: { mode: 'agent', agent_id: agentId },
          },
        },
      }, 20_000);
    } catch (error) {
      if (!/already exists|already_exists|TaskAlreadyExists|exists/i.test(error.message || '')) {
        throw error;
      }
    }
  }
  const board = await adpQueryVariant({ QueryTaskBoard: { include_terminal: true } }, 'TaskBoard', 20_000);
  const tasksById = new Map((board.tasks || []).map((task) => [task.task_id, task]));
  const invalid = workerRailTaskIds
    .map((taskId) => tasksById.get(taskId))
    .filter((task) => !task || task.parent_session_id !== workerRailSessionId || !task.worker_session_id || ['approved', 'closed', 'cancelled', 'failed'].includes(`${task.status || ''}`.toLowerCase()));
  if (invalid.length > 0) {
    throw new Error(`Header worker rail fixture is not live owner truth: ${JSON.stringify(invalid)}`);
  }
}

async function ensureWorkerOneNamespaceSessions() {
  for (const target of localAgentTargets) {
    const targetAdpUrl = adpUrlFromBaseUrl(`${target.origin}/`);
    const title = `${target.label} namespace proof`;
    try {
      await adpCommandAt(targetAdpUrl, {
        CreateSession: { session_id: target.markerSessionId, title },
      }, 20_000);
    } catch (error) {
      await adpCommandAt(targetAdpUrl, {
        RenameSession: { session_id: target.markerSessionId, title },
      }, 20_000);
    }
    await adpCommandAt(targetAdpUrl, {
      RestoreSession: { session_id: target.markerSessionId },
    }, 20_000);
  }
}

async function ensureSession(sessionId, title) {
  try {
    await adpCommand({ CreateSession: { session_id: sessionId, title } }, 20_000);
  } catch (error) {
    await adpCommand({ RenameSession: { session_id: sessionId, title } }, 20_000);
  }
  await adpCommand({ RestoreSession: { session_id: sessionId } }, 20_000);
}

async function adpCommand(command, timeoutMs = 30_000) {
  return await adpRequest('command', 'command', command, timeoutMs);
}

async function adpCommandAt(targetAdpUrl, command, timeoutMs = 30_000) {
  return await adpRequest('command', 'command', command, timeoutMs, targetAdpUrl);
}

async function adpQueryVariant(query, variant, timeoutMs = 30_000) {
  const result = await adpRequest('query', 'query', query, timeoutMs);
  if (!result || typeof result !== 'object' || !Object.prototype.hasOwnProperty.call(result, variant)) {
    throw new Error(`ADP query expected ${variant}, got ${JSON.stringify(result)}`);
  }
  return result[variant];
}

function adpRequest(kind, payloadKey, payload, timeoutMs, targetAdpUrl = adpUrl) {
  return adpVerifierRequest({
    url: targetAdpUrl,
    authToken: adpAuthToken,
    kind,
    payloadKey,
    payload,
    timeoutMs,
    clientName: 'freehand-mobile-verifier',
    capabilities: ['query', 'command', 'subscribe'],
  });
}
