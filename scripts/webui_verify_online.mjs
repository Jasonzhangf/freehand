import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const chromePath = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const debugPort = Number.parseInt(process.env.FREEHAND_WEBUI_DEBUG_PORT || '9223', 10);
const baseUrl = normalizedBaseUrl(process.env.FREEHAND_WEBUI_BASE_URL || 'http://127.0.0.1:4042/');
const adpUrl = process.env.FREEHAND_WEBUI_ADP_URL || adpUrlFromBaseUrl(baseUrl);
const cliPath = process.env.FREEHAND_WEBUI_CLI || `${process.env.HOME}/.local/bin/freehand-cliS`;
const profileName = process.env.FREEHAND_WEBUI_PROFILE || portLabelFromBaseUrl(baseUrl);
const successPrompt =
  'Online success sample: answer with one short sentence and a valid Freehand completion schema. Do not call tools.';
const failurePrompt =
  'Online failure sample: call the read_file tool exactly once with path definitely-missing-freehand-file.txt, then use the failed tool result to continue and report success through the required Freehand completion schema.';
const configUpdateEnvName = process.env.FREEHAND_WEBUI_CONFIG_UPDATE_ENV || 'FREEHAND_WEBUI_VERIFY_CREDENTIAL';
const configUpdateEnvValue = process.env.FREEHAND_WEBUI_CONFIG_UPDATE_VALUE || 'webui-verify-provider-key';
const configUpdateProviderId = process.env.FREEHAND_WEBUI_CONFIG_UPDATE_PROVIDER || 'minimax';
const configUpdateBaseUrl =
  process.env.FREEHAND_WEBUI_CONFIG_UPDATE_BASE_URL || 'https://api.minimaxi.com/anthropic';
const configUpdateModel = process.env.FREEHAND_WEBUI_CONFIG_UPDATE_MODEL || 'MiniMax-M3';
const configUpdateType = process.env.FREEHAND_WEBUI_CONFIG_UPDATE_TYPE || 'anthropic';
const configUpdateProtocol = process.env.FREEHAND_WEBUI_CONFIG_UPDATE_PROTOCOL || 'messages';
const runtimeHome = process.env.FREEHAND_RUNTIME_HOME || path.join(process.env.HOME, '.freehand');
const sEnvPath = process.env.FREEHAND_WEBUI_DAEMONS_ENV || path.join(runtimeHome, 'daemonS.env');
const configPath = process.env.FREEHAND_WEBUI_CONFIG_PATH || path.join(runtimeHome, 'config.toml');

const runId = `${new Date().toISOString().slice(0, 10).replace(/-/g, '')}-verify-${profileName}-${Date.now()}`;
const artifactDir = path.join(process.cwd(), 'artifacts', 'webui-online', runId);

await fs.mkdir(artifactDir, { recursive: true });

const chromeProfileDir = await fs.mkdtemp(path.join(os.tmpdir(), 'freehand-webui-verify-'));
let chrome;
let cleanupResult;
let runtimeFixture;

try {
  runtimeFixture = await prepareRuntimeFixture();

  cleanupResult = await removeExistingSessions();
  if (cleanupResult.failed.length > 0) {
    await fs.writeFile(path.join(artifactDir, 'session-cleanup-failed.json'), JSON.stringify(cleanupResult, null, 2));
    throw new Error(`failed to remove existing sessions before /new: ${cleanupResult.failed.map((entry) => entry.sessionId).join(', ')}`);
  }

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
  await waitForFunction(
    cdp,
    () => document.getElementById('new-session-dialog')?.open === true,
    10_000,
    'new conversation dialog open',
  );
  await evalInPage(cdp, () => {
    document.getElementById('new-session-confirm-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const shell = document.querySelector('[data-webui-shell="true"]');
      return !(document.getElementById('new-session-dialog')?.open) &&
        (shell?.dataset.selectedSession || '').includes('webui-session-');
    },
    10_000,
    'new conversation draft selected',
  );
  const cleanNewSession = await captureState(cdp, '01-after-new-conversation');

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
    const text = document.getElementById('message-list')?.innerText || '';
    const turnStatus = document.getElementById('turn-status')?.textContent?.toLowerCase() || '';
    const commandStatus = document.getElementById('command-status')?.textContent?.toLowerCase() || '';
    const completed = turnStatus.includes('completed') || commandStatus.includes('turn completed');
    return live.length >= 1 || (completed && text.includes('definitely-missing-freehand-file.txt'));
  }, 20_000, 'second turn progress or terminal visible');
  const running2 = await captureState(cdp, '06-second-running');

  await waitForTerminal(cdp, 120_000, 'second terminal');
  const terminal2 = await captureState(cdp, '07-second-terminal');

  await cdp.send('Page.reload', { ignoreCache: true });
  await waitForLoad(cdp);
  await waitForFunction(cdp, () => {
    return !!document.querySelector('[data-webui-shell="true"]');
  }, 20_000, 'shell reloaded');
  await waitForFunction(
    cdp,
    (firstMarker) => {
      const text = document.getElementById('message-list')?.innerText || '';
      return text.includes(firstMarker) && text.includes('definitely-missing-freehand-file.txt');
    },
    20_000,
    'refreshed transcript visible',
    prompt1,
  );
  const refreshed = await captureState(cdp, '08-after-refresh');
  const settingsProof = await captureSettingsProof(cdp);
  const viewportSnapshots = await captureViewportMatrix(cdp);
  const mobileDrawerProof = await captureMobileDrawerProof(cdp);
  const mobileComposerProof = await captureMobileComposerProof(cdp);

  const sessionId = refreshed.state.selectedSession || terminal2.state.selectedSession || terminal1.state.selectedSession;
  const adpQuery = sessionId
    ? await runCommand([cliPath, 'adp-session-query', '--url', adpUrl, '--session', sessionId])
    : { code: 1, stdout: '', stderr: 'missing session id' };

  const summary = {
    runId,
    artifactDir,
    baseUrl,
    adpUrl,
    cliPath,
    sessionId,
    cleanupResult,
    checks: {
      removedExistingSessions: cleanupResult.failed.length === 0,
      firstSubmitComposerCleared: postSubmit1.state.composer === '',
      firstPromptVisibleAfterSubmit: materialized1.state.messageText.includes(prompt1),
      secondSubmitComposerCleared: postSubmit2.state.composer === '',
      secondProgressObserved:
        running2.state.liveCount >= 1 || running2.state.turnStatus.toLowerCase().includes('completed') || running2.state.commandStatus.toLowerCase().includes('turn completed'),
      staleHistoricalLiveAfterSecondSubmit: running2.state.nonLastLiveCount,
      refreshPreservedFirstPrompt: refreshed.state.messageText.includes(prompt1),
      refreshPreservedFailurePrompt: refreshed.state.messageText.includes('definitely-missing-freehand-file.txt'),
      terminal2NoLive: terminal2.state.liveCount === 0,
      viewportShapesCovered: viewportSnapshots.every((entry) => entry.state.layoutShape === entry.expectedShape),
      desktopSettingsOpensProviderConfig:
        settingsProof.desktopOpen.state.settingsShellVisible &&
        settingsProof.desktopOpen.state.settingsText.includes('Provider and model') &&
        !settingsProof.desktopOpen.state.settingsText.includes('Active agent') &&
        !settingsProof.desktopOpen.state.settingsText.includes('Sessions and workspace') &&
        !settingsProof.desktopOpen.state.settingsText.includes('Task settings pending') &&
        settingsProof.desktopOpen.state.settingsProvider !== '' &&
        settingsProof.desktopOpen.state.settingsProvider !== 'loading' &&
        settingsProof.desktopOpen.state.settingsProviderHost !== '' &&
        settingsProof.desktopOpen.state.settingsProviderHost !== 'loading' &&
        settingsProof.desktopOpen.state.settingsProviderAuth !== '' &&
        settingsProof.desktopOpen.state.settingsProviderAuth !== 'loading' &&
        settingsProof.desktopOpen.state.settingsConfigError === 'none' &&
        !settingsProof.desktopOpen.state.settingsText.includes('Connection state') &&
        settingsProof.desktopOpen.state.passwordInputCount === 0 &&
        settingsProof.desktopOpen.state.apiKeyTextVisible === false &&
        settingsProof.desktopOpen.state.secretTextVisible === false,
      settingsInvalidUpdateVisible:
        settingsProof.invalidUpdate.state.settingsProviderSaveStatus.toLowerCase().includes('save failed') &&
        settingsProof.invalidUpdate.state.settingsProviderSaveStatus.toLowerCase().includes('base_url'),
      settingsValidUpdateRestartRequired:
        settingsProof.validUpdate.state.settingsProviderSaveStatus.toLowerCase().includes('restart required') &&
        settingsProof.validUpdate.state.settingsProvider === configUpdateProviderId &&
        settingsProof.validUpdate.state.settingsModel === configUpdateModel &&
        settingsProof.validUpdate.state.settingsProviderHost === 'api.minimaxi.com' &&
        !settingsProof.validUpdate.state.commandStatus.includes('provider_config_saved_restart_required') &&
        !settingsProof.validUpdate.state.commandStatus.includes('config.core'),
      settingsUpdateNoSecretLeak:
        settingsProof.validUpdate.state.passwordInputCount === 0 &&
        settingsProof.validUpdate.state.apiKeyTextVisible === false &&
        settingsProof.validUpdate.state.secretTextVisible === false,
      settingsCloseKeepsConversation:
        !settingsProof.afterClose.state.settingsShellVisible &&
        settingsProof.afterClose.state.messageText.includes(prompt1) &&
        settingsProof.afterClose.state.messageText.includes('definitely-missing-freehand-file.txt'),
      viewportComposerVisible: viewportSnapshots.every((entry) => entry.state.composerVisible),
      viewportMessageListVisible: viewportSnapshots.every((entry) => entry.state.messageListVisible),
      mobileNoLeftEdgeIndicators: viewportSnapshots
        .filter((entry) => isMobileDrawerShape(entry.expectedShape))
        .every((entry) => mobileChromeHasNoLeftEdge(entry.state)),
      mobileFocusedComposerCompact:
        mobileComposerProof.focused.state.layoutShape === 'tall_phone' &&
        mobileComposerProof.focused.state.composerFocused &&
        mobileComposerProof.focused.state.composerControlStripDisplay === 'none' &&
        mobileComposerProof.focused.state.attachmentTrayDisplay === 'none' &&
        mobileComposerProof.focused.state.commandStatusDisplay === 'none' &&
        pxNumber(mobileComposerProof.focused.state.conversationRegionPaddingBottom) <= 132 &&
        mobileComposerProof.focused.state.composerCardRect?.height <= 136 &&
        mobileComposerProof.focused.state.composerInputRect?.height <= 84,
      mobileFocusedNoLeftEdgeIndicators: mobileChromeHasNoLeftEdge(mobileComposerProof.focused.state),
      mobileConversationPrimary: viewportSnapshots
        .filter((entry) => isMobileDrawerShape(entry.expectedShape))
        .every((entry) =>
          entry.state.mobileSessionButtonVisible &&
          entry.state.mobileDetailButtonVisible &&
          entry.state.mobileSettingsButtonVisible &&
          entry.state.messageListVisible &&
          entry.state.composerVisible &&
          !entry.state.sessionDrawerVisible &&
          !entry.state.detailDrawerVisible &&
          !entry.state.mobileDrawer
        ),
      mobileSessionDrawerOpens:
        mobileDrawerProof.sessionOpen.state.mobileDrawer === 'sessions' &&
        mobileDrawerProof.sessionOpen.state.sessionDrawerVisible &&
        !mobileDrawerProof.sessionOpen.state.detailDrawerVisible,
      mobileSessionDrawerSwipeOpens:
        mobileDrawerProof.swipeSessionOpen.state.mobileDrawer === 'sessions' &&
        mobileDrawerProof.swipeSessionOpen.state.sessionDrawerVisible &&
        mobileDrawerProof.swipeSessionOpen.state.sessionAgentGroupCount >= 1 &&
        mobileDrawerProof.swipeSessionOpen.state.sessionAgentNestedCount >= 1,
      mobileSessionDrawerAgentHierarchy:
        mobileDrawerProof.sessionOpen.state.sessionAgentGroupCount >= 1 &&
        mobileDrawerProof.sessionOpen.state.sessionAgentNestedCount >= 1 &&
        mobileDrawerProof.sessionOpen.state.sessionAgentExpandedCount >= 1,
      mobileDetailDrawerOpens:
        mobileDrawerProof.detailOpen.state.mobileDrawer === 'details' &&
        mobileDrawerProof.detailOpen.state.detailDrawerVisible &&
        !mobileDrawerProof.detailOpen.state.sessionDrawerVisible,
      mobileSettingsDrawerOpens:
        mobileDrawerProof.settingsOpen.state.mobileDrawer === 'settings' &&
        mobileDrawerProof.settingsOpen.state.detailDrawerVisible &&
        mobileDrawerProof.settingsOpen.state.settingsShellVisible &&
        mobileDrawerProof.settingsOpen.state.settingsText.includes('Provider and model') &&
        !mobileDrawerProof.settingsOpen.state.settingsText.includes('Active agent') &&
        !mobileDrawerProof.settingsOpen.state.sessionDrawerVisible,
      mobileDrawerCloses:
        !mobileDrawerProof.afterSessionClose.state.mobileDrawer &&
        !mobileDrawerProof.afterSessionClose.state.sessionDrawerVisible &&
        !mobileDrawerProof.afterDetailClose.state.mobileDrawer &&
        !mobileDrawerProof.afterDetailClose.state.detailDrawerVisible &&
        !mobileDrawerProof.afterSettingsClose.state.mobileDrawer &&
        !mobileDrawerProof.afterSettingsClose.state.detailDrawerVisible,
      newSessionStartsClean:
        cleanNewSession.state.selectedTurn === '-' &&
        cleanNewSession.state.messageCount === 0 &&
        cleanNewSession.state.messageText.includes('New conversation') &&
        cleanNewSession.state.messageText.includes('Send a message to start this session.'),
      newSessionDoesNotLeakPreviousTurn:
        !cleanNewSession.state.messageText.includes('Online success sample') &&
        !cleanNewSession.state.messageText.includes('Online failure sample') &&
        !cleanNewSession.state.messageText.includes('Read file') &&
        !cleanNewSession.state.messageText.includes('WebUI 正在查询最新 turn。') &&
        !cleanNewSession.state.messageText.includes('等待数据'),
    },
    snapshots: {
      cleanNewSession,
      postSubmit1,
      materialized1,
      terminal1,
      postSubmit2,
      running2,
      terminal2,
      refreshed,
      settingsProof,
      viewportSnapshots,
      mobileDrawerProof,
      mobileComposerProof,
    },
    adpQuery,
    chromeProfileDir,
    chromeLog,
  };

  await fs.writeFile(path.join(artifactDir, 'summary.json'), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));

  const failedChecks = Object.entries(summary.checks).filter(([key, value]) => {
    if (key === 'staleHistoricalLiveAfterSecondSubmit') return value !== 0;
    return value !== true;
  });
  if (failedChecks.length > 0) {
    throw new Error(`WebUI online verification failed checks: ${failedChecks.map(([key, value]) => `${key}=${value}`).join(', ')}`);
  }

  await cdp.close();
} finally {
  if (chrome && !chrome.killed) {
    chrome.kill('SIGTERM');
    await onceExit(chrome, 5_000).catch(() => null);
  }
  if (runtimeFixture) {
    await restoreRuntimeFixture(runtimeFixture).catch(async (error) => {
      await fs.writeFile(
        path.join(artifactDir, 'runtime-fixture-restore-failed.json'),
        JSON.stringify({ message: error.message, stack: error.stack }, null, 2),
      );
      throw error;
    });
  }
}

async function prepareRuntimeFixture() {
  await fs.mkdir(path.join(runtimeHome, 'tmp'), { recursive: true });
  const backupDir = await fs.mkdtemp(path.join(runtimeHome, 'tmp', 'webui-online-config-'));
  const configBackup = path.join(backupDir, 'config.toml');
  const envBackup = path.join(backupDir, 'daemonS.env');
  await fs.copyFile(configPath, configBackup);
  await fs.copyFile(sEnvPath, envBackup);

  const envText = await fs.readFile(sEnvPath, 'utf8');
  const envPattern = new RegExp(`^${escapeRegExp(configUpdateEnvName)}=`, 'm');
  if (!envPattern.test(envText)) {
    const nextEnv = `${envText.replace(/\s*$/, '\n')}${configUpdateEnvName}="${configUpdateEnvValue}"\n`;
    await fs.writeFile(sEnvPath, nextEnv);
  }
  const restart = await runCommand(['scripts/install-launchd.sh', 'restartS']);
  if (restart.code !== 0) {
    throw new Error(`failed to restart S profile after WebUI verifier env injection: ${restart.stderr || restart.stdout}`);
  }
  return { backupDir, configBackup, envBackup };
}

async function restoreRuntimeFixture(fixture) {
  await fs.copyFile(fixture.configBackup, configPath);
  await fs.copyFile(fixture.envBackup, sEnvPath);
  const restart = await runCommand(['scripts/install-launchd.sh', 'restartS']);
  if (restart.code !== 0) {
    throw new Error(`failed to restart S profile after WebUI verifier restore: ${restart.stderr || restart.stdout}`);
  }
}

function escapeRegExp(value) {
  return String(value).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
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
    const shell = document.querySelector('[data-webui-shell="true"]');
    return {
      selectedSession: shell?.dataset.selectedSession || '',
      selectedTurn: shell?.dataset.selectedTurn || '-',
      composer: document.getElementById('composer-input')?.value || '',
      commandStatus: document.getElementById('command-status')?.textContent?.trim() || '',
      turnStatus: document.getElementById('turn-status')?.textContent?.trim() || '',
      workspaceStatus: document.getElementById('workspace-status')?.textContent?.trim() || '',
      layoutShape: document.body.dataset.layoutShape || '',
      shellLayoutShape: document.querySelector('[data-webui-shell="true"]')?.dataset.layoutShape || '',
      mobileDrawer: document.body.dataset.mobileDrawer || '',
      composerFocused: document.body.dataset.composerFocused === 'true',
      composerVisible: isVisible(document.getElementById('composer-form')),
      messageListVisible: isVisible(document.getElementById('message-list')),
      composerControlStripDisplay: displayOf(document.querySelector('.composer-control-strip')),
      attachmentTrayDisplay: displayOf(document.getElementById('attachment-tray')),
      commandStatusDisplay: displayOf(document.getElementById('command-status')),
      conversationRegionPaddingBottom: styleValue(document.querySelector('.conversation-region'), 'paddingBottom'),
      mobileSessionButtonVisible: isVisible(document.getElementById('open-session-drawer-button')),
      mobileDetailButtonVisible: isVisible(document.getElementById('open-detail-drawer-button')),
      mobileSettingsButtonVisible: isVisible(document.getElementById('open-settings-drawer-button')),
      sessionDrawerVisible: isVisible(document.querySelector('.sidebar')),
      detailDrawerVisible: isVisible(document.querySelector('.inspector')),
      settingsShellVisible: isVisible(document.getElementById('settings-shell')),
      settingsText: document.getElementById('settings-shell')?.innerText || '',
      settingsProvider: document.getElementById('settings-provider-id')?.textContent?.trim() || '',
      settingsProviderHost: document.getElementById('settings-provider-host')?.textContent?.trim() || '',
      settingsProviderAuth: document.getElementById('settings-provider-auth')?.textContent?.trim() || '',
      settingsModel: document.getElementById('settings-model-value')?.textContent?.trim() || '',
      settingsConfigError: document.getElementById('settings-config-error')?.textContent?.trim() || '',
      settingsProviderSaveStatus: document.getElementById('settings-provider-save-status')?.textContent?.trim() || '',
      passwordInputCount: document.querySelectorAll('input[type="password"]').length,
      apiKeyTextVisible: /api[-_ ]?key/i.test(document.getElementById('settings-shell')?.innerText || ''),
      pageApiKeyTextVisible: /api[-_ ]?key/i.test(document.body.innerText || ''),
      secretTextVisible: /api_key|pair_token|sk-|secret/i.test(document.getElementById('settings-shell')?.innerText || ''),
      sessionAgentGroupCount: document.querySelectorAll('.session-agent-group').length,
      sessionAgentNestedCount: document.querySelectorAll('.session-agent-sessions .session-item').length,
      sessionAgentExpandedCount: document.querySelectorAll('.session-agent-group[data-expanded="true"]').length,
      composerRect: rectOf(document.getElementById('composer-form')),
      composerCardRect: rectOf(document.querySelector('.composer-card')),
      composerInputRect: rectOf(document.getElementById('composer-input')),
      messageListRect: rectOf(document.getElementById('message-list')),
      sessionDrawerRect: rectOf(document.querySelector('.sidebar')),
      detailDrawerRect: rectOf(document.querySelector('.inspector')),
      viewport: { width: window.innerWidth, height: window.innerHeight },
      scrollY: window.scrollY,
      liveCount: live.length,
      nonLastLiveCount: live.filter((node) => node !== lastMessage).length,
      messageCount: messages.length,
      messageText: document.getElementById('message-list')?.innerText || '',
      mobileChromeProbe: {
        assistant: styleProbe('.chat-message-assistant'),
        assistantSuccess: styleProbe('.chat-message-assistant.success-state'),
        assistantFailed: styleProbe('.chat-message-assistant.failed-state'),
        tool: styleProbe('.chat-section-tool'),
        toolSuccess: styleProbe('.chat-section-tool.success'),
        toolFailed: styleProbe('.chat-section-tool.failed'),
        finalItem: styleProbe('.final-summary-item'),
      },
      pageErrors: window.__freehandVerify?.pageErrors || [],
      consoleErrors: window.__freehandVerify?.consoleErrors || [],
    };
    function isVisible(node) {
      if (!node) return false;
      const rect = node.getBoundingClientRect();
      return rect.width > 0 &&
        rect.height > 0 &&
        rect.bottom >= 0 &&
        rect.top <= window.innerHeight &&
        rect.right >= 0 &&
        rect.left <= window.innerWidth;
    }
    function rectOf(node) {
      if (!node) return null;
      const rect = node.getBoundingClientRect();
      return {
        top: rect.top,
        bottom: rect.bottom,
        left: rect.left,
        right: rect.right,
        width: rect.width,
        height: rect.height,
      };
    }
    function displayOf(node) {
      if (!node) return '';
      return window.getComputedStyle(node).display;
    }
    function styleValue(node, propertyName) {
      if (!node) return '';
      return window.getComputedStyle(node)[propertyName] || '';
    }
    function styleProbe(selector) {
      const node = document.querySelector(selector);
      if (!node) return null;
      const style = window.getComputedStyle(node);
      return {
        borderLeftWidth: style.borderLeftWidth,
        borderLeftStyle: style.borderLeftStyle,
        boxShadow: style.boxShadow,
        paddingLeft: style.paddingLeft,
        backgroundColor: style.backgroundColor,
      };
    }
  });
  await fs.writeFile(path.join(artifactDir, `${label}.json`), JSON.stringify(state, null, 2));
  return { label, state };
}

async function captureViewportMatrix(cdp) {
  const viewports = [
    { label: '11-viewport-phone-portrait-375x812', width: 375, height: 812, expectedShape: 'tall_phone' },
    { label: '12-viewport-tall-phone-430x932', width: 430, height: 932, expectedShape: 'tall_phone' },
    { label: '13-viewport-phone-landscape-844x390', width: 844, height: 390, expectedShape: 'phone_landscape' },
    { label: '14-viewport-tablet-portrait-768x1024', width: 768, height: 1024, expectedShape: 'tablet_portrait' },
    { label: '15-viewport-tablet-landscape-1024x768', width: 1024, height: 768, expectedShape: 'foldable_unfolded' },
    { label: '16-viewport-foldable-900x1000', width: 900, height: 1000, expectedShape: 'foldable_unfolded' },
    { label: '17-viewport-desktop-1280x900', width: 1280, height: 900, expectedShape: 'desktop_large' },
  ];
  const results = [];
  for (const viewport of viewports) {
    await cdp.send('Emulation.setDeviceMetricsOverride', {
      width: viewport.width,
      height: viewport.height,
      deviceScaleFactor: 1,
      mobile: viewport.width < 900,
    });
    await evalInPage(cdp, () => {
      window.dispatchEvent(new Event('resize'));
      return window.__freehandLayout?.applyLayoutShape?.();
    });
    await delay(260);
    await evalInPage(cdp, () => {
      window.scrollTo(0, 0);
      const streamStage = document.querySelector('.stream-stage');
      if (streamStage) {
        streamStage.scrollTop = streamStage.scrollHeight;
      }
      const messageList = document.getElementById('message-list');
      if (messageList) {
        messageList.scrollTop = messageList.scrollHeight;
      }
    });
    try {
      await waitForFunction(
        cdp,
        (expected) => document.body.dataset.layoutShape === expected,
        10_000,
        `${viewport.label} layout shape`,
        viewport.expectedShape,
      );
    } catch (error) {
      const snapshot = await captureState(cdp, `${viewport.label}-failure`);
      await fs.writeFile(
        path.join(artifactDir, `${viewport.label}-failure-context.json`),
        JSON.stringify({ ...viewport, error: error.message, snapshot }, null, 2),
      );
      throw error;
    }
    const snapshot = await captureState(cdp, viewport.label);
    results.push({ ...viewport, ...snapshot });
  }
  await cdp.send('Emulation.clearDeviceMetricsOverride');
  return results;
}

async function captureMobileComposerProof(cdp) {
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 1,
    mobile: true,
  });
  await evalInPage(cdp, () => {
    document.getElementById('close-session-drawer-button')?.click();
    document.getElementById('close-detail-drawer-button')?.click();
    window.dispatchEvent(new Event('resize'));
    return window.__freehandLayout?.applyLayoutShape?.();
  });
  await waitForFunction(
    cdp,
    () => document.body.dataset.layoutShape === 'tall_phone' && !document.body.dataset.mobileDrawer,
    10_000,
    'mobile composer proof layout',
  );
  await evalInPage(cdp, () => {
    const input = document.getElementById('composer-input');
    input?.focus();
    input?.dispatchEvent(new Event('focus', { bubbles: true }));
  });
  await waitForFunction(
    cdp,
    () => document.body.dataset.composerFocused === 'true',
    5_000,
    'mobile composer focused',
  );
  await delay(260);
  const focused = await captureState(cdp, '26-mobile-focused-composer');
  await cdp.send('Emulation.clearDeviceMetricsOverride');
  return { focused };
}

async function captureSettingsProof(cdp) {
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 1280,
    height: 900,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    return window.__freehandLayout?.applyLayoutShape?.();
  });
  await waitForFunction(
    cdp,
    () => document.body.dataset.layoutShape === 'desktop_large',
    10_000,
    'desktop settings layout',
  );
  await evalInPage(cdp, () => {
    document.getElementById('settings-shell-toggle')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const settings = document.getElementById('settings-shell');
      return !settings?.hidden &&
        settings.innerText.includes('Provider and model') &&
        !settings.innerText.includes('Active agent') &&
        !settings.innerText.includes('Sessions and workspace') &&
        !settings.innerText.includes('Task settings pending') &&
        !settings.innerText.includes('loading') &&
        document.querySelectorAll('input[type="password"]').length === 0 &&
        !/api_key|pair_token|sk-|secret/i.test(settings.innerText);
    },
    5_000,
    'desktop settings shell open',
  );
  const desktopOpen = await captureState(cdp, '09-settings-desktop-open');
  const invalidUpdate = await submitSettingsConfigUpdate(cdp, {
    providerId: configUpdateProviderId,
    providerType: configUpdateType,
    providerProtocol: configUpdateProtocol,
    baseUrl: 'not-a-url',
    model: configUpdateModel,
    envName: configUpdateEnvName,
  }, '09a-settings-invalid-update');
  const validUpdate = await submitSettingsConfigUpdate(cdp, {
    providerId: configUpdateProviderId,
    providerType: configUpdateType,
    providerProtocol: configUpdateProtocol,
    baseUrl: configUpdateBaseUrl,
    model: configUpdateModel,
    envName: configUpdateEnvName,
  }, '09b-settings-valid-update');
  await evalInPage(cdp, () => {
    document.getElementById('settings-shell-toggle')?.click();
  });
  await waitForFunction(
    cdp,
    () => document.getElementById('settings-shell')?.hidden === true,
    5_000,
    'desktop settings shell closed',
  );
  const afterClose = await captureState(cdp, '10-settings-desktop-closed');
  await cdp.send('Emulation.clearDeviceMetricsOverride');
  return { desktopOpen, invalidUpdate, validUpdate, afterClose };
}

async function submitSettingsConfigUpdate(cdp, values, label) {
  await evalInPage(
    cdp,
    (payload) => {
      const setValue = (id, value) => {
        const node = document.getElementById(id);
        if (!node) return;
        node.value = value;
        node.dispatchEvent(new Event('input', { bubbles: true }));
        node.dispatchEvent(new Event('change', { bubbles: true }));
      };
      setValue('settings-provider-id-input', payload.providerId);
      setValue('settings-provider-type-input', payload.providerType);
      setValue('settings-provider-protocol-input', payload.providerProtocol);
      setValue('settings-provider-url-input', payload.baseUrl);
      setValue('settings-provider-model-input', payload.model);
      setValue('settings-provider-env-input', payload.envName);
      document.getElementById('settings-provider-form')?.requestSubmit();
    },
    values,
  );
  await waitForFunction(
    cdp,
    () => {
      const status = document.getElementById('settings-provider-save-status')?.textContent || '';
      return /Save failed|Saved\. Restart required/i.test(status);
    },
    20_000,
    `${label} status`,
  );
  return captureState(cdp, label);
}

async function captureMobileDrawerProof(cdp) {
  await cdp.send('Emulation.setDeviceMetricsOverride', {
    width: 390,
    height: 844,
    deviceScaleFactor: 1,
    mobile: true,
  });
  await evalInPage(cdp, () => {
    window.dispatchEvent(new Event('resize'));
    return window.__freehandLayout?.applyLayoutShape?.();
  });
  await delay(260);
  await waitForFunction(
    cdp,
    () => document.body.dataset.layoutShape === 'tall_phone',
    10_000,
    'mobile drawer proof layout',
  );
  const closed = await captureState(cdp, '18-mobile-drawer-closed-default');
  await dispatchRightSwipe(cdp, 120, 320, 260, 326);
  await waitForFunction(
    cdp,
    () => {
      const node = document.querySelector('.sidebar');
      const rect = node?.getBoundingClientRect();
      return document.body.dataset.mobileDrawer === 'sessions' &&
        document.querySelectorAll('.session-agent-group').length >= 1 &&
        document.querySelectorAll('.session-agent-sessions .session-item').length >= 1 &&
        !!rect &&
        rect.width > 0 &&
        rect.height > 0 &&
        rect.bottom >= 0 &&
        rect.top <= window.innerHeight &&
        rect.right >= 0 &&
        rect.left <= 1;
    },
    5_000,
    'session drawer opened by right swipe',
  );
  await delay(260);
  const swipeSessionOpen = await captureState(cdp, '19-mobile-session-drawer-open-swipe');
  await evalInPage(cdp, () => {
    document.getElementById('close-session-drawer-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => !document.body.dataset.mobileDrawer,
    5_000,
    'swipe-opened session drawer closed',
  );
  await evalInPage(cdp, () => {
    document.getElementById('open-session-drawer-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const node = document.querySelector('.sidebar');
      const rect = node?.getBoundingClientRect();
      return document.body.dataset.mobileDrawer === 'sessions' &&
        !!rect &&
        rect.width > 0 &&
        rect.height > 0 &&
        rect.bottom >= 0 &&
        rect.top <= window.innerHeight &&
        rect.right >= 0 &&
        rect.left <= 1 &&
        !visibleWithinViewport(document.querySelector('.inspector'));
      function visibleWithinViewport(candidate) {
        const candidateRect = candidate?.getBoundingClientRect();
        return !!candidateRect &&
          candidateRect.width > 0 &&
          candidateRect.height > 0 &&
          candidateRect.bottom >= 0 &&
          candidateRect.top <= window.innerHeight &&
          candidateRect.right >= 0 &&
          candidateRect.left <= window.innerWidth;
      }
    },
    5_000,
    'session drawer open',
  );
  await delay(260);
  const sessionOpen = await captureState(cdp, '20-mobile-session-drawer-open');
  await evalInPage(cdp, () => {
    document.getElementById('close-session-drawer-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const node = document.querySelector('.sidebar');
      const rect = node?.getBoundingClientRect();
      const visible = !!rect &&
        rect.width > 0 &&
        rect.height > 0 &&
        rect.bottom >= 0 &&
        rect.top <= window.innerHeight &&
        rect.right >= 0 &&
        rect.left <= window.innerWidth;
      return !document.body.dataset.mobileDrawer && !visible;
    },
    5_000,
    'session drawer closed',
  );
  const afterSessionClose = await captureState(cdp, '21-mobile-session-drawer-closed');
  await evalInPage(cdp, () => {
    document.getElementById('open-detail-drawer-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const node = document.querySelector('.inspector');
      const rect = node?.getBoundingClientRect();
      return document.body.dataset.mobileDrawer === 'details' &&
        !!rect &&
        rect.width > 0 &&
        rect.height > 0 &&
        rect.bottom >= 0 &&
        rect.top <= window.innerHeight &&
        rect.right >= 0 &&
        rect.left <= window.innerWidth &&
        rect.right >= window.innerWidth - 1 &&
        !visibleWithinViewport(document.querySelector('.sidebar'));
      function visibleWithinViewport(candidate) {
        const candidateRect = candidate?.getBoundingClientRect();
        return !!candidateRect &&
          candidateRect.width > 0 &&
          candidateRect.height > 0 &&
          candidateRect.bottom >= 0 &&
          candidateRect.top <= window.innerHeight &&
          candidateRect.right >= 0 &&
          candidateRect.left <= window.innerWidth;
      }
    },
    5_000,
    'detail drawer open',
  );
  await delay(260);
  const detailOpen = await captureState(cdp, '22-mobile-detail-drawer-open');
  await evalInPage(cdp, () => {
    document.getElementById('close-detail-drawer-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const node = document.querySelector('.inspector');
      const rect = node?.getBoundingClientRect();
      const visible = !!rect &&
        rect.width > 0 &&
        rect.height > 0 &&
        rect.bottom >= 0 &&
        rect.top <= window.innerHeight &&
        rect.right >= 0 &&
        rect.left <= window.innerWidth;
      return !document.body.dataset.mobileDrawer && !visible;
    },
    5_000,
    'detail drawer closed',
  );
  const afterDetailClose = await captureState(cdp, '23-mobile-detail-drawer-closed');
  await evalInPage(cdp, () => {
    document.getElementById('open-settings-drawer-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const node = document.querySelector('.inspector');
      const rect = node?.getBoundingClientRect();
      return document.body.dataset.mobileDrawer === 'settings' &&
        !document.getElementById('settings-shell')?.hidden &&
        document.getElementById('settings-shell')?.innerText.includes('Provider and model') &&
        !!rect &&
        rect.width > 0 &&
        rect.height > 0 &&
        rect.bottom >= 0 &&
        rect.top <= window.innerHeight &&
        rect.right >= 0 &&
        rect.left <= window.innerWidth &&
        rect.right >= window.innerWidth - 1 &&
        !visibleWithinViewport(document.querySelector('.sidebar'));
      function visibleWithinViewport(candidate) {
        const candidateRect = candidate?.getBoundingClientRect();
        return !!candidateRect &&
          candidateRect.width > 0 &&
          candidateRect.height > 0 &&
          candidateRect.bottom >= 0 &&
          candidateRect.top <= window.innerHeight &&
          candidateRect.right >= 0 &&
          candidateRect.left <= window.innerWidth;
      }
    },
    5_000,
    'settings drawer open',
  );
  await delay(260);
  const settingsOpen = await captureState(cdp, '24-mobile-settings-drawer-open');
  await evalInPage(cdp, () => {
    document.getElementById('close-detail-drawer-button')?.click();
  });
  await waitForFunction(
    cdp,
    () => {
      const node = document.querySelector('.inspector');
      const rect = node?.getBoundingClientRect();
      const visible = !!rect &&
        rect.width > 0 &&
        rect.height > 0 &&
        rect.bottom >= 0 &&
        rect.top <= window.innerHeight &&
        rect.right >= 0 &&
        rect.left <= window.innerWidth;
      return !document.body.dataset.mobileDrawer && !visible;
    },
    5_000,
    'settings drawer closed',
  );
  const afterSettingsClose = await captureState(cdp, '25-mobile-settings-drawer-closed');
  await cdp.send('Emulation.clearDeviceMetricsOverride');
  return {
    closed,
    swipeSessionOpen,
    sessionOpen,
    afterSessionClose,
    detailOpen,
    afterDetailClose,
    settingsOpen,
    afterSettingsClose,
  };
}

async function dispatchRightSwipe(cdp, startX, startY, endX, endY) {
  await cdp.send('Input.dispatchTouchEvent', {
    type: 'touchStart',
    touchPoints: [{ id: 1, x: startX, y: startY, radiusX: 2, radiusY: 2, force: 1 }],
  });
  await delay(80);
  await cdp.send('Input.dispatchTouchEvent', {
    type: 'touchMove',
    touchPoints: [{ id: 1, x: Math.round((startX + endX) / 2), y: Math.round((startY + endY) / 2), radiusX: 2, radiusY: 2, force: 1 }],
  });
  await delay(80);
  await cdp.send('Input.dispatchTouchEvent', {
    type: 'touchMove',
    touchPoints: [{ id: 1, x: endX, y: endY, radiusX: 2, radiusY: 2, force: 1 }],
  });
  await delay(40);
  await cdp.send('Input.dispatchTouchEvent', {
    type: 'touchEnd',
    touchPoints: [],
  });
}

function isMobileDrawerShape(shape) {
  return ['phone_portrait', 'tall_phone', 'tablet_portrait'].includes(shape);
}

function mobileChromeHasNoLeftEdge(state) {
  const probe = state.mobileChromeProbe || {};
  const required = [probe.assistant, probe.tool, probe.finalItem];
  const optional = [probe.assistantSuccess, probe.assistantFailed, probe.toolSuccess, probe.toolFailed].filter(Boolean);
  return required.every(Boolean) && [...required, ...optional].every((entry) => {
    if (!entry) return false;
    const hasLeftBorder = entry.borderLeftStyle !== 'none' && pxNumber(entry.borderLeftWidth) > 0;
    const hasInsetLeftShadow = /\binset\b/.test(entry.boxShadow || '') && /\b2px\b/.test(entry.boxShadow || '');
    const hasFinalIndent = entry === probe.finalItem && pxNumber(entry.paddingLeft) > 0;
    return !hasLeftBorder && !hasInsetLeftShadow && !hasFinalIndent;
  });
}

function pxNumber(value) {
  const parsed = Number.parseFloat(String(value || '').replace('px', ''));
  return Number.isFinite(parsed) ? parsed : Number.POSITIVE_INFINITY;
}

async function waitForTerminal(cdp, timeoutMs, label) {
  await waitForFunction(cdp, () => {
    const live = document.querySelectorAll('[data-live="true"]').length;
    const turnStatus = document.getElementById('turn-status')?.textContent?.toLowerCase() || '';
    const commandStatus = document.getElementById('command-status')?.textContent?.toLowerCase() || '';
    return live === 0 && (turnStatus.includes('completed') || commandStatus.includes('turn completed'));
  }, timeoutMs, label);
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

async function removeExistingSessions() {
  const query = await runCommand([cliPath, 'adp-session-query', '--url', adpUrl]);
  const sessionIds = parseSessionIds(query.stdout);
  const removed = [];
  const failed = [];
  for (const sessionId of sessionIds) {
    const result = await runCommand([
      cliPath,
      'adp-session-manage',
      '--url',
      adpUrl,
      '--action',
      'delete',
      '--session',
      sessionId,
    ]);
    const entry = { sessionId, code: result.code, stdout: result.stdout, stderr: result.stderr };
    if (result.code === 0) {
      removed.push(entry);
    } else {
      failed.push(entry);
    }
  }
  const after = await runCommand([cliPath, 'adp-session-query', '--url', adpUrl]);
  return {
    before: query,
    after,
    requested: sessionIds,
    removed,
    failed,
  };
}

function parseSessionIds(stdout) {
  const match = `${stdout || ''}`.match(/\bids=([^\s]+)/);
  if (!match || !match[1] || match[1] === '-') {
    return [];
  }
  return match[1]
    .split(',')
    .map((entry) => entry.split(':')[0].trim())
    .filter(Boolean);
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

function adpUrlFromBaseUrl(value) {
  const url = new URL(value);
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  url.pathname = '/adp';
  url.search = '';
  url.hash = '';
  return url.toString();
}

function portLabelFromBaseUrl(value) {
  const url = new URL(value);
  return url.port ? url.port : url.protocol === 'https:' ? '443' : '80';
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
