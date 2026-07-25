import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const chromePath = process.env.FREEHAND_WEBUI_CHROME ||
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_DEBUG_PORT || '9247', 10);
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WEBUI_BASE_URL || 'http://127.0.0.1:4042/');
const runId = `mobile-ui-tree-phase1-${new Date().toISOString().replace(/[-:.]/g, '').slice(0, 15)}-${process.pid}`;
const artifactDir = path.join(process.cwd(), 'artifacts', 'webui-online', runId);
const assetVersion = '20260725-session-panel-ui';
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
  await assertProductionPageReachable();
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
  await cdp.send('Page.navigate', { url: baseUrl });
  await waitForLoad(cdp);
  await waitForFunction(cdp, () => {
    return document.body.dataset.webuiJsReady === 'true' &&
      !!document.querySelector('[data-webui-shell="true"]') &&
      !!document.getElementById('mobile-home-dashboard');
  }, 20_000, 'production WebUI shell 就绪');

  const snapshots = [];
  for (const viewport of viewports) {
    snapshots.push(await captureViewport(cdp, viewport));
  }
  const settings = await captureSettingsTree(cdp);
  const summary = buildSummary({ snapshots, settings });
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

async function assertProductionPageReachable() {
  const response = await fetch(baseUrl, { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`production WebUI not reachable: ${response.status} ${response.statusText}`);
  }
  const html = await response.text();
  if (!html.includes(assetVersion)) {
    throw new Error(`served WebUI asset version mismatch: expected ${assetVersion}`);
  }
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
  const state = await evalInPage(cdp, collectPhaseOneState);
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
  const state = await evalInPage(cdp, collectPhaseOneState);
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

function buildSummary({ snapshots, settings }) {
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
    settings,
    checks: {
      productionAssetVersion: snapshots.every((snapshot) => snapshot.state.assetVersionSeen),
      viewportMatrixCovered: snapshots.length === viewports.length,
      noHorizontalOverflow: snapshots.every((snapshot) => snapshot.state.noHorizontalOverflow),
      portraitQuickEntriesIconOnly: portraitEntriesVisibleAndSeparated,
      mobileHomeDashboardVisible: portraitSnapshots.every((snapshot) => snapshot.state.mobileHomeDashboardVisible),
      desktopDoesNotForceMobileHome: snapshots
        .filter((snapshot) => snapshot.viewport.width >= 1180)
        .every((snapshot) => !snapshot.state.mobileHomeDashboardVisible),
      globalSessionListExcludesInternalSessions: !internalSessionTerms.some((pattern) => pattern.test(globalSessionText)),
      homeShowsOnlyActivityAndHistory: portraitSnapshots.every((snapshot) =>
        snapshot.state.mobileHomeActiveVisible &&
        snapshot.state.mobileHomeHistoryVisible &&
        snapshot.state.mobileHomeRunningClass &&
        snapshot.state.mobileHomeStaticClass &&
        snapshot.state.mobileHomeCardCount === 2 &&
        snapshot.state.mobileHomeRunningHistoryOverlap.length === 0 &&
        !snapshot.state.mobileHomeFloatingTree &&
        snapshot.state.mobileHomeText.includes('正在运行') &&
        snapshot.state.mobileHomeText.includes('历史会话') &&
        !snapshot.state.homeHasTimerList &&
        !snapshot.state.homeHasTimerMarker &&
        !snapshot.state.homeHasCurrentCard &&
        !snapshot.state.homeHasNewEntryButtonInsideHome &&
        !snapshot.state.mobileHomeText.includes('timer dashboard') &&
        !snapshot.state.mobileHomeText.includes('Timer 权威真源')
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
      noForbiddenUiStorageTerms: !forbiddenUiTerms.some((pattern) => pattern.test(allTexts)),
      statusMarkersAreHollow: rootSettings.statusMarkerCount > 0 && rootSettings.statusMarkerAllHollow,
    },
  };
  summary.ok = Object.values(summary.checks).every(Boolean);
  return summary;
}

function collectPhaseOneState() {
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
    assetVersionSeen: html.includes('20260725-session-panel-ui'),
    bodyText,
    bodyWidth: document.body.scrollWidth,
    docWidth: document.documentElement.scrollWidth,
    viewportWidth: window.innerWidth,
    noHorizontalOverflow:
      Math.max(document.body.scrollWidth, document.documentElement.scrollWidth) <= window.innerWidth + 2,
    globalSessionText,
    mobileHomeDashboardVisible: localIsVisible(document.getElementById('mobile-home-dashboard')),
    mobileHomeText: document.getElementById('mobile-home-dashboard')?.innerText || '',
    mobileHomeActiveVisible: localIsVisible(document.getElementById('mobile-home-active-list')),
    mobileHomeHistoryVisible: localIsVisible(document.getElementById('mobile-home-session-list')),
    mobileHomeRunningClass: document.getElementById('mobile-home-active-list')?.classList.contains('mobile-running-session-list') || false,
    mobileHomeStaticClass: document.getElementById('mobile-home-session-list')?.classList.contains('mobile-static-session-list') || false,
    mobileHomeRunningIds: Array.from(document.querySelectorAll('#mobile-home-active-list [data-session-id]'))
      .map((node) => node.dataset.sessionId || '')
      .filter(Boolean),
    mobileHomeHistoryIds: Array.from(document.querySelectorAll('#mobile-home-session-list [data-session-id]'))
      .map((node) => node.dataset.sessionId || '')
      .filter(Boolean),
    mobileHomeRunningHistoryOverlap: (() => {
      const running = new Set(Array.from(document.querySelectorAll('#mobile-home-active-list [data-session-id]'))
        .map((node) => node.dataset.sessionId || '')
        .filter(Boolean));
      return Array.from(document.querySelectorAll('#mobile-home-session-list [data-session-id]'))
        .map((node) => node.dataset.sessionId || '')
        .filter((sessionId) => sessionId && running.has(sessionId));
    })(),
    mobileHomeCardCount: document.querySelectorAll('#mobile-home-dashboard .mobile-home-card').length,
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
