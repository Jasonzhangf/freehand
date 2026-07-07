# OpenMinis Config UI Provider Edit Plan

## Objective

Complete OpenMinis config UI closeout L2 Batch 3 by adding an owner-backed provider/model edit flow.

The user-facing result is:

- WebUI Settings can edit provider endpoint and default model through protocol/runtime/config owners.
- Invalid config is rejected with visible validation errors.
- Valid saved config is persisted safely and projected as `restart required`.
- Runtime does not pretend hot reload happened before restart.
- No credential/API key value is ever returned in UI projection, logs, screenshots, or smoke summaries.

## Acceptance Criteria

1. Provider/model edit commands exist only through the owner chain:

   ```text
   app.webui-smoke
     -> ui.protocol
     -> runtime.ui-command-dispatch
     -> config.core
     -> canonical config persistence
     -> ui.protocol safe projection
     -> app.webui-smoke rendering
   ```

2. WebUI never parses or writes `~/.freehand/config.toml` directly.
3. Config writes are atomic or explicitly fail without corrupting the previous config.
4. Save success shows a restart-required state until the daemon is restarted.
5. Invalid provider protocol/base URL/model/auth source returns structured validation errors.
6. API key and credential text do not appear in protocol projections, logs, screenshots, evidence JSON, or DOM-visible settings text.
7. Online WebUI proof on S-profile fixed port `127.0.0.1:4042` demonstrates both invalid and valid edit paths.

## Scope

### In Scope

- Add provider/model edit intent to `ui.protocol`.
- Add config owner mutation/validation/persistence API in `config.core`.
- Add runtime dispatch routing for config edit/query results.
- Add WebUI Settings form for provider endpoint, default model, and env-var-first auth source.
- Add visible validation error and restart-required UI state.
- Add headless CLI/smoke path if needed to prove command/query behavior without DOM coupling.
- Update docs, function maps, test design, loop state, run log, memory, and gates as required.

### Out Of Scope

- No local secure secret store unless a separate secret owner design lands first.
- No model groups, fallback routing, load balancing, or provider health checks.
- No silent hot reload.
- No Android native settings redesign in this batch.
- No release-profile `4041` claim unless separately requested; normal online validation uses S-profile `4042`.

## Design Principles

1. `config.core` is the only owner of config schema, validation, safe projection, and persistence semantics.
2. `ui.protocol` owns only transport DTOs and structured result/error shapes.
3. `runtime.ui-command-dispatch` owns routing and must not duplicate config validation logic.
4. `app.webui-smoke` owns rendering and user interaction only; it must not infer config truth from TOML, localStorage, debug payloads, or DOM state.
5. User-facing labels must say service/connection/config/provider/model, not internal transport jargon.
6. Failure must be explicit. No fallback provider, fallback endpoint, silent partial save, or fake success projection.
7. Restart semantics are part of the visible product state, not an implementation detail.

## Technical Plan

### 1. Pre-Implementation Routing

- Read `CACHE.md`, `MEMORY.md`, `note.md`, `docs/architecture/feature-map.md`.
- Search MemoryPalace before code changes.
- Resolve owners and docs for:
  - `config.core`
  - `ui.protocol`
  - `runtime.ui-command-dispatch`
  - `app.webui-smoke`
- Read and update bound function maps and test designs before implementation.
- Use `scripts/source-search.sh` for source searches; do not search generated/runtime artifacts as source truth.

### 2. `config.core`

Add owner-backed config mutation primitives:

- Create/update provider entry with fields:
  - provider id/name
  - provider kind/protocol
  - base URL
  - default model
  - auth source kind
  - auth env var name when env-var auth is used
- Select provider for the active agent where applicable.
- Validate before persistence:
  - provider id is well-formed and not ambiguous
  - protocol is supported
  - base URL parses and has a supported scheme
  - default model is non-empty
  - auth source is allowed and never projects secret value
  - unknown fields are rejected
- Persist through the existing canonical config path only.
- Produce a safe post-save projection:
  - active selected provider
  - default model
  - auth source type/name only
  - restart-required flag
  - validation errors when save fails

### 3. `ui.protocol`

Add explicit protocol shapes:

- Query: config status remains UI-safe.
- Command: provider/model update intent.
- Result:
  - success with safe projection and restart-required state
  - validation failure with structured field errors
  - runtime/system failure with explicit error code/message

Rules:

- Command/query ingress must stay separate.
- Config mutation commands must use owner-routing envelope.
- No credential value field may exist in returned projection.
- Invalid command payloads must be rejected explicitly, not interpreted as query or fallback.

### 4. `runtime.ui-command-dispatch`

Route config update intents:

- Decode protocol command.
- Call `config.core` mutation/validation/persistence owner.
- Return `ui.protocol` result.
- Keep active runtime config unchanged until restart.
- Project restart-required truth after successful save.
- Do not duplicate config parsing, validation, or secret redaction logic.

### 5. WebUI Settings

Add provider/model edit UI:

- Compact OpenMinis-style settings section.
- Provider endpoint/default model fields.
- Env-var-first auth input: user provides env var name, not raw API key as projected status.
- Save button disabled while request is in flight.
- Validation errors shown near fields and in a compact summary.
- Save success shows restart-required badge/callout.
- Query refresh shows persisted projection, not locally invented state.
- No internal transport jargon in user-facing copy.

### 6. CLI/Headless Verification Surface

If existing CLI is insufficient, add a small headless command for:

- Query config status.
- Submit invalid provider/model update.
- Submit valid env-var-based update to an isolated test config path/profile.
- Assert safe projection and restart-required state.

This surface is for black-box verification; it must use the same protocol/runtime path as WebUI where possible.

## Files And Ownership

Expected touch areas:

- `crates/freehand-config/**`
- `crates/freehand-ui-protocol/**`
- `crates/freehand-runtime/**`
- `crates/freehand-cli/**` if headless coverage needs a command
- `apps/freehand-server/assets/**`
- `scripts/webui_verify_online.mjs` or related WebUI smoke harness
- `docs/function-maps/**`
- `docs/testing/**`
- `docs/mainline-calls/**` if migrated call maps are affected
- `docs/loops/openminis-config-ui-closeout/**`
- `MEMORY.md`
- `note.md`

Forbidden touch patterns:

- WebUI directly reading or writing `~/.freehand/config.toml`.
- Returning API key or secret text in protocol/UI projections.
- Adding fallback endpoint/provider behavior.
- Hiding config write failures behind success UI.
- Searching generated artifacts as source truth.

## Risk Matrix

| Risk | Required Control |
| --- | --- |
| Secret leakage | Redaction tests plus online DOM/evidence scan |
| Invalid TOML corruption | Validate-before-write and atomic write tests |
| Fake hot reload | Restart-required projection and active-runtime mismatch test |
| UI local truth drift | WebUI re-queries owner-backed projection after save |
| Protocol misuse | Negative tests for command/query split and unknown fields |
| Runtime duplicate logic | Function map/call map must show config validation owner only |
| Evidence false positive | Online browser proof plus headless protocol/CLI proof |

## Verification Matrix

### Unit / Module

- `cargo test -p freehand-config -- --nocapture`
  - positive provider update fixture
  - invalid protocol/base URL/model/auth source
  - unknown field rejection
  - no secret projection
  - restart-required projection
- `cargo test -p freehand-ui-protocol -- --nocapture`
  - command/result serialization
  - validation error DTOs
  - query/command split rejection
- `cargo test -p freehand-runtime -- --nocapture`
  - runtime dispatch routes to config owner
  - save success does not mutate active runtime before restart
  - validation failure returns structured error
- `cargo test -p freehand-cli -- --nocapture` if CLI surface changes.
- `cargo test -p freehand-server -- --nocapture` if WebUI/server smoke surface changes.

### Static / Gates

- `node --check apps/freehand-server/assets/webui.js`
- `node --check scripts/webui_verify_online.mjs` if changed
- `cargo fmt --check`
- `git diff --check`
- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`

### Online WebUI

Run S-profile fixed port validation:

- Start/restart only through service-scoped script for S-profile.
- Verify `http://127.0.0.1:4042/health`.
- Verify ADP/WebUI connection path.
- Open WebUI Settings.
- Submit invalid provider config:
  - visible field/schema error
  - no silent success
  - old config remains intact
- Submit valid env-var-based provider/model update in isolated test config/profile:
  - visible save success
  - restart-required badge shown
  - projection updates with provider/model/auth source kind
  - no secret text visible
- Capture evidence under `artifacts/webui-online/<timestamp>/summary.json` and screenshots if harness supports them.

## Implementation Steps

1. Update test design/function map for Batch 3 owner path.
2. Implement `config.core` mutation/validation/safe projection.
3. Implement `ui.protocol` command/result/error DTOs.
4. Implement runtime dispatch wiring.
5. Add CLI/headless smoke if needed.
6. Implement WebUI Settings edit rendering.
7. Add positive and negative tests.
8. Run static/gate validation.
9. Run online WebUI proof on `127.0.0.1:4042`.
10. Update loop state/run log, `note.md`, `MEMORY.md`, and re-mine/search memory.
11. Commit only the relevant files and leave worktree clean.

## Definition Of Done

- Provider/model config edit works through owner-backed command path.
- Invalid provider/model/auth input is visible and recoverable.
- Valid save persists config and shows restart-required state.
- Runtime does not fake hot reload.
- No secret appears in projection, DOM, logs, screenshots, or summary evidence.
- Function maps, test designs, loop docs, memory, and gates are synchronized.
- Online WebUI proof exists on fixed S-profile `4042`.
- Commit exists and `git status --short` is clean.
