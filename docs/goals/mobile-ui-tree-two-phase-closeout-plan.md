# Mobile UI Tree Two-Phase Closeout Plan

## Objective

Deliver the reviewed Freehand mobile UI tree in two strictly separated phases: first complete the production UI shell and visual information architecture, then wire owner-backed functionality without inventing parallel truth.

## Acceptance Standards

Phase 1 is accepted when the production WebUI and Android WebView surface match the reviewed UI tree and can be inspected live without adding new runtime semantics.

Phase 2 is accepted only after Phase 1 is committed, and all visible entries are backed by their canonical owners, commands, projections, and verification gates.

## Scope And Boundary

### In Scope

- Production WebUI mobile home tree.
- Four corner icon-only quick entries.
- Persisted-session-only top-level home/session list.
- Master session header/session-tree relationship display for worker sessions.
- Settings/config UI tree matching Freehand ownership categories.
- Timer, tools, search, provider/model group, Android update/permissions, memory, skills, MCP, daemon, logs, and about entry surfaces.
- Phase 2 owner-backed command/query wiring for entries that have canonical owners.

### Out Of Scope For Phase 1

- New timer create/list execution semantics.
- New search index semantics.
- New provider write/test semantics.
- New tool execution semantics.
- New runtime command semantics.
- Config persistence changes.
- Worker/task lifecycle semantics changes.

## Locked UI Direction

- Home is the main page.
- Home contains persisted session history, timer dashboard, and current session dashboard.
- Top-level session list never shows worker/subagent temporary sessions.
- Worker sessions appear only inside their owning Master session, through the header/session tree with a clear return path.
- Four corner quick entries use icons, not text labels.
- Visual language is black, white, light gray, and logo green `#75daa7`.
- Avoid large dark blocks and saturated category colors.
- Status uses small hollow squares: green for normal/configured, red for attention/error.
- Android app is a daemon-hosted WebUI shell and must not expose phone-local filesystem management concepts.

## Resource And Owner Rules

- `app.webui-smoke` owns WebUI shell, layout, rendering, and UI smoke tests.
- `ui.protocol` owns UI projection and command/query contracts.
- `reason.persistence` owns persisted session truth.
- `runtime.master-worker-loop` owns timer semantics.
- `runtime.ui-command-dispatch` owns UI command routing.
- `config.core` owns provider/model/config truth.
- `task.orchestration` and `agent.lifecycle` own task and worker lifecycle truth.
- `app.android-client` owns Android install, permission, update, and WebView shell concerns.
- UI can render owner projections and submit existing owner commands, but must not parse, persist, or infer owner semantics directly.

## Phase 1: UI Completion

### Goal

Finish the reviewed static UI tree in production WebUI/Android WebView using existing projections only.

### Required Work

1. Replace mobile navigation with icon-only quick entries in fixed corner positions.
2. Render home as persisted session history plus timer dashboard plus current session dashboard.
3. Filter top-level session lists to persisted user sessions only.
4. Keep worker/subagent sessions out of global history and show them only under the owning Master session header/session tree.
5. Add the reviewed settings/config UI tree as a readable shell.
6. Mark unconnected entries as Phase 1 UI-only instead of pretending the function works.
7. Remove phone-local storage-management concepts from Android-facing settings.
8. Keep desktop and mobile responsive without horizontal overflow.
9. Update function map, mainline call map, test design, and prototype docs to match the locked UI tree.

### Phase 1 Verification

- `node --check apps/freehand-server/assets/webui.js`.
- Focused WebUI smoke tests for the shell and asset routes.
- `cargo fmt --check`.
- `cargo run -p xtask -- mainlines check`.
- `cargo run -p xtask -- gates check`.
- `git diff --check`.
- Production browser proof on S-profile WebUI at mobile and desktop widths.
- Browser proof must assert icon-only quick entries, mobile dashboard, settings tree, no forbidden phone filesystem words, no top-level worker temporary sessions, and no horizontal overflow.
- Android true-device WebView proof if device access is available; otherwise report this as an explicit verification gap.

### Phase 1 Completion

Commit Phase 1 separately after verification. The commit must not contain Phase 2 runtime/function wiring.

## Phase 2: Function Completion

### Goal

Wire the Phase 1 UI entries to canonical owner-backed functions and make every visible function observable, testable, and non-fake.

### Required Work

1. For each UI entry, confirm the resource owner, function map owner, command/query contract, projection shape, and required tests before coding.
2. Timer entry must use `runtime.master-worker-loop` or the canonical timer owner for relative, absolute, cron, recurrence, persistence, overdue-after-restart handling, and activation evidence.
3. Search entry must use an owner-backed session search/index/query contract and must not scan ad hoc UI state.
4. Tools entry must render canonical internal tools, examples, and execution/result states from owner truth.
5. Provider/model group settings must use `config.core` projections and commands, never UI-side config writes.
6. Worker capability and lifecycle surfaces must use `task.orchestration` and `agent.lifecycle` state truth.
7. Android update/permission entries must use `app.android-client` owner flows and device proof.
8. Every action must display success, failure, retry, blocked, and in-progress states where applicable.
9. Update resource map, function maps, mainline call maps, test designs, and docs with any changed truth.

### Phase 2 Verification

- Owner-specific positive and negative tests for each newly wired function.
- UI protocol command/query tests proving contracts reject invalid shapes and do not leak secrets or debug-only metadata.
- Focused WebUI tests proving UI consumes projections and renders all action states.
- Online production WebUI proof for each wired entry.
- Android true-device proof for Android-facing claims.
- Workspace gates required by the touched owners.

### Phase 2 Completion

Commit Phase 2 separately only after each visible function is backed by owner truth and live evidence. Do not mark Phase 2 complete if any visible entry is still fake, silent, or only UI-local.

## Risk Controls

- No fallback or silent degradation.
- No broad kill commands.
- Do not touch release `4041` unless explicitly required.
- Do not modify unrelated dirty files.
- Do not claim completion from static screenshots alone.
- If an owner contract is missing, stop that function at documented gap or create the owner-backed contract with tests; do not wire around it in UI.

## Definition Of Done

- Phase 1 has a separate commit with production UI, docs, tests, and browser evidence.
- Phase 2 has a separate commit with owner-backed functions, docs, tests, and live evidence.
- The final report includes changed files, verification commands, browser/Android evidence paths, remaining risks, and explicit confirmation that top-level session history excludes worker temporary sessions.

## Progress Evidence

### Phase 2 Provider Registry UI

- Status: closed for the Provider registry UI entry only; full Phase 2 remains open.
- Online proof: `FREEHAND_PROVIDER_REGISTRY_UI_CHROME=$HOME/Library/Caches/ms-playwright/chromium_headless_shell-1194/chrome-mac/headless_shell FREEHAND_PROVIDER_REGISTRY_UI_DEBUG_PORT=9273 node scripts/verify-provider-registry-ui-online.mjs`.
- Result: `provider_registry_ui_online_ok url=ws://127.0.0.1:4042/adp run_id=provider-registry-ui-1784913165666 added_provider=ui-verify-provider-registry switched_provider=cc final_provider=minimax final_fallback=cc final_registry=cc,minimax`.
- Artifact: `artifacts/webui-online/provider-registry-ui-1784913165666`.
- Coverage: WebUI loaded owner-backed provider registry projection, added a provider through `UpsertProviderConfig`, proved upsert did not change current primary/fallback, switched active provider through `UpdateAgentProviderSelection`, and restored S-profile config/env.
- Restore: final config returned `minimax/MiniMax-M3` with fallback `cc`, `web_search=auto`, `web_search_effective=hosted_declared`, inline auth, and no fixture env matches.
