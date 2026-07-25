#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const repoRoot = process.cwd();
const runId =
  process.env.FREEHAND_MOBILE_UI_TREE_AUDIT_RUN_ID ||
  `mobile-ui-tree-goal-audit-${Date.now()}`;
const artifactDir =
  process.env.FREEHAND_MOBILE_UI_TREE_AUDIT_ARTIFACT_DIR ||
  path.join(repoRoot, "artifacts", "webui-online", runId);

const adpUrl =
  process.env.FREEHAND_ADP_URL || "ws://127.0.0.1:4042/adp";
const androidSerial =
  process.env.FREEHAND_ANDROID_SERIAL || "100.104.163.65:5555";

const statusRank = {
  passed: 0,
  weak: 1,
  blocked: 2,
  missing: 3,
  failed: 4,
};

function readJson(relativePath) {
  if (!relativePath) {
    return { ok: false, error: "missing_file", relativePath, absolutePath: null };
  }
  const absolutePath = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolutePath)) {
    return { ok: false, error: "missing_file", relativePath, absolutePath };
  }
  try {
    return {
      ok: true,
      value: JSON.parse(fs.readFileSync(absolutePath, "utf8")),
      relativePath,
      absolutePath,
    };
  } catch (error) {
    return {
      ok: false,
      error: `invalid_json:${error.message}`,
      relativePath,
      absolutePath,
    };
  }
}

function allTruthy(object, keys) {
  return keys.every((key) => object?.[key] === true);
}

function item(status, id, title, evidence, details = {}) {
  return { status, id, title, evidence, ...details };
}

function artifactStatus(relativePath, requiredChecks, title, options = {}) {
  const json = readJson(relativePath);
  if (!json.ok) {
    return item(
      "missing",
      options.id,
      title,
      relativePath,
      { reason: json.error },
    );
  }
  const checks = json.value.checks || {};
  const okFlag = options.okPath
    ? options.okPath.split(".").reduce((acc, key) => acc?.[key], json.value)
    : json.value.ok;
  const requiredOk = allTruthy(checks, requiredChecks);
  const status = okFlag === false || !requiredOk ? "failed" : "passed";
  return item(status, options.id, title, relativePath, {
    checks: Object.fromEntries(
      requiredChecks.map((key) => [key, checks[key] === true]),
    ),
    summary: options.summary?.(json.value),
  });
}

function runCommand(command, args, options = {}) {
  try {
    return {
      ok: true,
      stdout: execFileSync(command, args, {
        cwd: repoRoot,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
        timeout: options.timeoutMs || 30_000,
      }).trim(),
    };
  } catch (error) {
    return {
      ok: false,
      stdout: error.stdout?.toString?.().trim() || "",
      stderr: error.stderr?.toString?.().trim() || error.message,
      status: error.status,
      signal: error.signal,
    };
  }
}

function latestJsonSummaryRelative(rootRelativePath) {
  const root = path.join(repoRoot, rootRelativePath);
  if (!fs.existsSync(root)) {
    return null;
  }
  const pending = [root];
  let latest = null;
  while (pending.length > 0) {
    const current = pending.pop();
    const stat = fs.statSync(current);
    if (stat.isDirectory()) {
      for (const entry of fs.readdirSync(current)) {
        pending.push(path.join(current, entry));
      }
      continue;
    }
    if (path.basename(current) !== "summary.json") {
      continue;
    }
    if (!latest || stat.mtimeMs > latest.mtimeMs) {
      latest = {
        relativePath: path.relative(repoRoot, current),
        mtimeMs: stat.mtimeMs,
      };
    }
  }
  return latest?.relativePath || null;
}

function summarizeConfig() {
  const config = runCommand("freehand-cliS", [
    "adp-config-query",
    "--url",
    adpUrl,
  ]);
  const envPath = path.join(os.homedir(), ".freehand", "daemonS.env");
  const envText = fs.existsSync(envPath)
    ? fs.readFileSync(envPath, "utf8")
    : "";
  const fixturePattern =
    /FREEHAND_PROVIDER_RETRY_FIXTURE_KEY|FREEHAND_PROVIDER_RETRY_BACKOFF_MS|FREEHAND_MASTER_AUTONOMY_FIXTURE_KEY|FREEHAND_MASTER_AUTONOMY_TARGET_CWD|FREEHAND_PROVIDER_WEB_SEARCH_UI_FIXTURE_KEY|FREEHAND_PROVIDER_REGISTRY_UI_FIXTURE_KEY|FREEHAND_WEBUI_DIAGNOSTICS_FIXTURE_KEY|FREEHAND_PATH_DIAGNOSTIC_FIXTURE_KEY/g;
  const fixtureMatches = [...envText.matchAll(fixturePattern)].map(
    (match) => match[0],
  );
  const stdout = config.stdout || "";
  const restored =
    config.ok &&
    stdout.includes("provider=minimax") &&
    stdout.includes("provider_protocol=messages") &&
    stdout.includes("base_url_host=api.minimaxi.com") &&
    stdout.includes("default_model=MiniMax-M3") &&
    stdout.includes("auth_source=inline") &&
    stdout.includes("web_search_effective=hosted_declared") &&
    fixtureMatches.length === 0;
  return item(
    restored ? "passed" : "failed",
    "s_profile_restore",
    "S-profile config and fixture env restored",
    "freehand-cliS adp-config-query + ~/.freehand/daemonS.env",
    {
      cliOk: config.ok,
      cli: stdout,
      fixtureMatches,
      envPath,
    },
  );
}

function summarizeAndroid() {
  const latestSummaryPath =
    process.env.FREEHAND_ANDROID_SUMMARY ||
    latestJsonSummaryRelative("artifacts/android-device");
  const latest = readJson(latestSummaryPath);
  const adbDevices = runCommand("adb", ["devices", "-l"], { timeoutMs: 10_000 });
  const windowDump = runCommand(
    "adb",
    ["-s", androidSerial, "shell", "dumpsys", "window"],
    { timeoutMs: 10_000 },
  );
  const windowText = windowDump.stdout || "";
  const locked =
    windowText.includes("mDreamingLockscreen=true") ||
    windowText.includes("mShowingLockscreen=true") ||
    latest.value?.reason === "device_locked_or_dreaming";
  const connected =
    adbDevices.ok && adbDevices.stdout.includes(`${androidSerial}`);
  if (latest.ok && latest.value?.status === "passed" && !locked) {
    return item(
      "passed",
      "android_true_device",
      "Android true-device WebView/update/permission/notification proof",
      latestSummaryPath || "artifacts/android-device/<missing>/summary.json",
      { serial: androidSerial, adbDevices: adbDevices.stdout },
    );
  }
  return item(
    connected ? "blocked" : "missing",
    "android_true_device",
    "Android true-device WebView/update/permission/notification proof",
    latestSummaryPath || "artifacts/android-device/<missing>/summary.json",
    {
      reason: locked ? "device_locked_or_dreaming" : "device_not_available",
      serial: androidSerial,
      latestSummary: latest.ok ? latest.value : null,
      adbDevices: adbDevices.stdout,
      windowSignals: windowText
        .split("\n")
        .filter((line) =>
          /mDreamingLockscreen|mShowingLockscreen|mCurrentFocus|mFocusedApp/.test(
            line,
          ),
        ),
    },
  );
}

function summarizeTimerDue() {
  const evidencePath =
    "artifacts/webui-online/webui-timer-dashboard-20260724T165550-6177/timer-list-after-cancel.json";
  const json = readJson(evidencePath);
  if (!json.ok) {
    return item(
      "missing",
      "timer_due_restart",
      "Timer due wakeup and restart-overdue recovery",
      evidencePath,
      { reason: json.error },
    );
  }
  const timers = json.value.timers || [];
  const events = json.value.events || [];
  const expected = [
    "timer-online-proof-1784901474-25671",
    "timer-online-proof-1784901568-34363",
  ];
  const proofs = expected.map((timerId) => {
    const timer = timers.find((row) => row.timer_id === timerId);
    const eventTypes = events
      .filter((row) => row.timer_id === timerId)
      .map((row) => row.event_type);
    return {
      timerId,
      status: timer?.status,
      firedCount: timer?.fired_count,
      sourceSessionId: timer?.source_session_id,
      sourceTurnId: timer?.source_turn_id,
      eventTypes,
      ok:
        timer?.status === "completed" &&
        timer?.fired_count === 1 &&
        ["TimerScheduled", "TimerFired", "TimerCompleted"].every((event) =>
          eventTypes.includes(event),
        ),
    };
  });
  const ok = proofs.every((proof) => proof.ok);
  return item(
    ok ? "passed" : "failed",
    "timer_due_restart",
    "Timer due wakeup and restart-overdue recovery",
    evidencePath,
    { proofs },
  );
}

const results = [
  artifactStatus(
    "artifacts/webui-online/mobile-ui-tree-phase1-20260724T165539-5547/summary.json",
    [
      "productionAssetVersion",
      "viewportMatrixCovered",
      "noHorizontalOverflow",
      "portraitQuickEntriesIconOnly",
      "mobileHomeDashboardVisible",
      "globalSessionListExcludesInternalSessions",
      "settingsReviewTreeVisible",
      "noForbiddenUiStorageTerms",
      "statusMarkersAreHollow",
    ],
    "Phase 1 production mobile UI shell",
    {
      id: "phase1_shell",
      summary: (summary) => ({
        artifactDir: summary.artifactDir,
        assetVersion: summary.assetVersion,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/webui-timer-dashboard-20260724T165550-6177/summary.json",
    [
      "dialogOpened",
      "domShowsScheduledTimer",
      "adpHasScheduledTimer",
      "scheduleLedgerVisible",
      "cancelUpdatedAdpTruth",
      "cancelLedgerVisible",
      "domShowsCancelHistory",
      "noTopLevelSessionCreated",
    ],
    "Timer dashboard list/schedule/cancel owner wiring",
    {
      id: "timer_dashboard",
      summary: (summary) => ({
        timerId: summary.scheduledTimerId,
        artifactDir: summary.artifactDir,
      }),
    },
  ),
  summarizeTimerDue(),
  artifactStatus(
    "artifacts/webui-online/provider-registry-ui-1784913165666/summary.before-restore.json",
    [],
    "Provider registry add/switch UI owner wiring",
    {
      id: "provider_registry",
      summary: (summary) => ({
        addedProvider: summary.testProviderId,
        switchedProvider: summary.switchTarget,
        providersAfterUpsert: summary.afterUpsertProviderIds,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/model-group-ui-1784912168782/summary.json",
    [],
    "Model group primary/sub/search/title/fallback UI owner wiring",
    {
      id: "model_groups",
      okPath: "restored",
      summary: (summary) => ({
        testGroupId: summary.testGroupId,
        finalProvider: summary.final?.provider_id,
        finalModel: summary.final?.default_model,
        finalGroups: summary.final?.groups,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/provider-web-search-settings-ui-1784945356860/summary.before-restore.json",
    [
      "minimaxVisibleHosted",
      "minimaxUiTestPassed",
      "openaiResponsesVisibleHosted",
      "openaiResponsesUiTestPassed",
      "fixtureDeclaredHostedWebSearch",
      "fixtureDidNotDeclareFunctionWebSearch",
    ],
    "Provider web_search Settings live-test UI",
    {
      id: "provider_web_search_settings",
      summary: (summary) => ({
        minimaxStatus: summary.minimaxStatus,
        fixtureStatus: summary.fixtureStatus,
        fixtureRequestCount: summary.fixtureRequestCount,
        fixtureHostedTools: summary.fixtureHostedTools,
        fixtureFunctionTools: summary.fixtureFunctionTools,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/webui-tools-registry-20260724T165955-27717/summary.json",
    [
      "domMatchesAdpToolNames",
      "coreToolsVisible",
      "noLocalWebSearchTool",
      "taskMasterOnly",
      "timerMasterOnly",
      "webFetchMasterWorker",
      "bashHiddenFromMasterWorker",
      "workerToolsVisible",
      "workerOnlyToolsHiddenFromMaster",
      "pathGuidanceVisible",
      "noTopLevelSessionCreated",
      "noHorizontalOverflow",
    ],
    "Tools registry and guidance owner projection",
    {
      id: "tools_registry",
      summary: (summary) => ({
        registryVersion: summary.registryVersion,
        toolCount: summary.toolCount,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/webui-session-search-1784932659523/summary.json",
    [
      "ownerProjectionContainsFixedSession",
      "browserDialogOpened",
      "browserRowsMatchOwnerProjection",
      "noTopLevelWorkerResultCards",
      "selectedSessionOpened",
      "dialogClosedAfterOpen",
      "noUnexpectedTopLevelSessionCreated",
      "noTopLevelWorkerSessionsAfter",
      "noHorizontalOverflow",
    ],
    "Persisted session Search owner projection",
    {
      id: "session_search",
      summary: (summary) => ({
        fixedSessionId: summary.fixedSessionId,
        workerTopLevelCards: summary.workerTopLevelCards,
        workerTopLevelSessions: summary.workerTopLevelSessions,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/webui-new-session-1784934872925/summary.json",
    [
      "conversationDialogOpenedFromMobileEntry",
      "conversationCreatedThroughOwnerTruth",
      "conversationSelectedInUi",
      "conversationEmptyStateClean",
      "taskDialogOpenedAndCwdEntered",
      "taskCreatedThroughOwnerTruth",
      "taskSelectedInUi",
      "taskCwdProjectedInUi",
      "noTopLevelWorkerSessions",
      "noHorizontalOverflow",
    ],
    "New conversation and New task owner-backed creation",
    {
      id: "new_conversation_task",
      summary: (summary) => ({
        conversationSessionId: summary.conversationSessionId,
        taskSessionId: summary.taskSessionId,
        taskCwd: summary.taskCwd,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/ambiguous-submit-recovery-fixed/summary.json",
    [
      "attachmentSessionCreatedThroughOwnerTruth",
      "attachmentTaskSelectedWithCwd",
      "imageSelectedThroughInput",
      "failureKeepsSessionCwdAndPendingCard",
      "failureKeepsAttachmentDraft",
      "ownerSessionStillCwdBoundAfterFailure",
      "materializedClearsPending",
      "taskTruthClearsPending",
      "unverifiedKeepsPendingSession",
    ],
    "Attachment pool and ambiguous submit failure retention",
    {
      id: "attachment_failure_retention",
      summary: (summary) => ({
        fixedAttachmentSessionId: summary.fixedAttachmentSessionId,
        taskCwd: summary.taskCwd,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/image-attachment-notification-2026-07-24T053116863Z/summary.json",
    [
      "selectedPool",
      "previewLifecycle",
      "removeLifecycle",
      "submitMetadata",
      "submitClearsPool",
      "metadataOnlyHistory",
      "oneLiveTerminalNotification",
      "restoredTerminalDoesNotNotify",
    ],
    "Image attachment metadata contract and WebUI notification bridge",
    {
      id: "image_attachment_notification_bridge",
      summary: (summary) => ({
        sessionId: summary.sessionId,
        notificationCalls: summary.notificationCalls,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/worker-subtasks-current-dashboard-fixed/summary.json",
    [],
    "Current-session Worker dashboard and child navigation",
    {
      id: "current_session_worker_dashboard",
      summary: (summary) => ({
        parentSession: summary.parentState?.selectedSession,
        parentSummary: summary.parentState?.summaryTitle,
        childTasks: summary.sheetState?.cards?.map((card) => ({
          taskId: card.taskId,
          workerSessionId: card.workerSessionId,
          meta: card.meta,
        })),
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/provider-recovery-20260721T120214-73395/summary.json",
    [
      "fixtureRetriedThenRecovered",
      "providerRetryVisible",
      "providerRetryDetailVisible",
      "retryStayedOnSingleTurnCard",
      "noDuplicateCycleKeys",
      "fixedSessionReused",
      "domTerminalSuccess",
      "domNoLiveRows",
    ],
    "Provider retry observability without duplicate cards",
    {
      id: "provider_retry_observability",
      summary: (summary) => ({
        fixedSessionId: summary.fixedSessionId,
        requestCount: summary.requestCount,
        retryObserved: summary.retryObserved,
        selectedTurn: summary.selectedTurn,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/stop-continue-1784555593364/summary.json",
    [
      "providerActivityVisibleBeforeCancel",
      "cancelMaterializedInAdp",
      "cancelVisibleInDom",
      "cancelClearedLiveState",
      "cancelPreventedProviderRetry",
      "continuedTurnAppendedAfterCancelledTurn",
      "cancelledCardPreservedAfterContinue",
      "continuedTurnSucceeded",
      "noRandomSessionCreated",
      "noBrowserErrors",
    ],
    "Session stop and continue lifecycle",
    {
      id: "stop_continue",
      summary: (summary) => ({
        fixedSessionId: summary.fixedSessionId,
        cancelledTurnId: summary.cancelledTurnId,
        successTurnId: summary.successTurnId,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/tool-turn-render-1784351742-dom/summary.json",
    [],
    "Worker tool-turn semantic card rendering",
    {
      id: "tool_turn_render",
      summary: (summary) => ({
        selectedSession: summary.selectedSession,
        selectedTurn: summary.selectedTurn,
        toolCardCount: summary.toolCardCount,
        successToolCardCount: summary.successToolCardCount,
        failedToolCardCount: summary.failedToolCardCount,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/tool-failure-detail-visible-1784700372618/summary.json",
    [],
    "Tool failure detail remains visible in session",
    {
      id: "tool_failure_detail_visible",
      summary: (summary) => ({
        selectedSession: summary.selectedSession,
        selectedTurn: summary.selectedTurn,
        failureLines: summary.failureLines?.length,
        hiddenFailureLineCount: summary.hiddenFailureLineCount,
      }),
    },
  ),
  artifactStatus(
    "artifacts/webui-online/webui-diagnostics-1784942454320/summary.json",
    [
      "adpProjectionSafe",
      "runtimeHomeRedacted",
      "logsDirRelative",
      "domRowsMatchAdp",
      "domNoSecretsOrAbsolutePaths",
      "noTopLevelSessionCreated",
      "noHorizontalOverflow",
    ],
    "Diagnostics logs owner-safe projection",
    {
      id: "diagnostics",
      summary: (summary) => ({
        files: summary.files,
        artifactDir: summary.artifactDir,
      }),
    },
  ),
  summarizeAndroid(),
  summarizeConfig(),
];

const counts = results.reduce((acc, result) => {
  acc[result.status] = (acc[result.status] || 0) + 1;
  return acc;
}, {});
const overallStatus = results.reduce((status, result) => {
  return statusRank[result.status] > statusRank[status] ? result.status : status;
}, "passed");

const summary = {
  ok: overallStatus === "passed",
  status: overallStatus,
  runId,
  generatedAt: new Date().toISOString(),
  repoRoot,
  adpUrl,
  artifactDir,
  counts,
  results,
};

function markdownReport(audit) {
  const lines = [];
  lines.push(`# Mobile UI Tree Goal Audit`);
  lines.push("");
  lines.push(`- status: ${audit.status}`);
  lines.push(`- generated_at: ${audit.generatedAt}`);
  lines.push(`- adp_url: ${audit.adpUrl}`);
  lines.push(`- artifact_dir: ${audit.artifactDir}`);
  lines.push("");
  lines.push(`## Result Matrix`);
  lines.push("");
  lines.push(`| status | id | evidence |`);
  lines.push(`| --- | --- | --- |`);
  for (const result of audit.results) {
    lines.push(`| ${result.status} | ${result.id} | ${result.evidence} |`);
  }
  lines.push("");
  lines.push(`## Blockers`);
  lines.push("");
  const blockers = audit.results.filter((result) =>
    ["blocked", "missing", "failed", "weak"].includes(result.status),
  );
  if (blockers.length === 0) {
    lines.push("- none");
  } else {
    for (const blocker of blockers) {
      const reason = blocker.reason || blocker.details?.reason || "";
      lines.push(`- ${blocker.id}: ${blocker.status}${reason ? ` (${reason})` : ""}`);
    }
  }
  lines.push("");
  lines.push(`## Key Evidence`);
  lines.push("");
  for (const result of audit.results) {
    lines.push(`- ${result.id}: ${result.title}; evidence=${result.evidence}`);
  }
  lines.push("");
  lines.push(`## Android Current State`);
  lines.push("");
  const android = audit.results.find((result) => result.id === "android_true_device");
  lines.push("```json");
  lines.push(JSON.stringify(android, null, 2));
  lines.push("```");
  lines.push("");
  lines.push(`## S-Profile Restore`);
  lines.push("");
  const config = audit.results.find((result) => result.id === "s_profile_restore");
  lines.push("```json");
  lines.push(JSON.stringify(config, null, 2));
  lines.push("```");
  lines.push("");
  return `${lines.join("\n")}\n`;
}

fs.mkdirSync(artifactDir, { recursive: true });
fs.writeFileSync(
  path.join(artifactDir, "summary.json"),
  `${JSON.stringify(summary, null, 2)}\n`,
);
fs.writeFileSync(path.join(artifactDir, "report.md"), markdownReport(summary));

const label =
  overallStatus === "passed"
    ? "mobile_ui_tree_goal_audit_ok"
    : `mobile_ui_tree_goal_audit_${overallStatus}`;
console.log(
  `${label} artifactDir=${artifactDir} passed=${counts.passed || 0} blocked=${
    counts.blocked || 0
  } missing=${counts.missing || 0} failed=${counts.failed || 0} weak=${
    counts.weak || 0
  }`,
);

if (counts.failed || counts.missing) {
  process.exit(1);
}
