# OpenMinis Config UI Closeout State

- loop_id: `openminis-config-ui-closeout`
- current_mode: `L2`
- status: `L2 Batch 4 release propagation verified; Android true-device proof blocked by no ADB device`
- kill_switch_state: `inactive`
- owner_feature: `config.core`, `ui.protocol`, `runtime.ui-command-dispatch`, `app.webui-smoke`
- last_baseline: `2026-07-07 L2 Batch 4 release 4041 propagation verified; served assets match workspace, config restored to auth_source=inline, Android true-device proof blocked because adb has no connected device`

## Current Findings

1. OpenMinis has no usable public implementation source yet; the website and README are the reference evidence.
2. Freehand already has strong conversation/session/mobile rendering, but lacks a full user-facing settings/config surface.
3. Android has file-backed daemon connection config and an edit path, but the primary conversation UI now loads daemon-hosted WebUI, so settings entry and visual language should converge with WebUI.
4. Freehand WebUI currently exposes model as a read-only selector and daemon connection implicitly via the serving origin; it does not expose provider CRUD, provider health, active agent/provider projection, model groups, skill management, session filesystem namespace view, or daemon connection profile management.
5. `config.core` owns `~/.freehand/config.toml`; `app.webui-smoke` must not import config semantics or write config directly.
6. L2 Batch 1 landed a read-only WebUI Settings shell.
7. L2 Batch 2 replaced placeholder provider/model/agent values with `config.core` -> `ui.protocol` -> `runtime.ui-command-dispatch` projections.
8. L2 Batch 3 landed owner-backed provider/model save through `app.webui-smoke` -> `ui.protocol` -> `runtime.ui-command-dispatch` -> `config.core`; invalid saves fail visibly without overwrite, valid env-var saves project restart-required status, active runtime config does not hot reload before restart, and WebUI evidence shows no credential leakage in Settings.
9. L2 Batch 4 fixed release launchd restart propagation: `scripts/install-launchd.sh restart` now rewrites env and plist before service-scoped restart, matching the S-profile restart semantics. Release 4041 currently serves workspace-matching WebUI JS/CSS and config query is restored to `auth_source=inline`.
10. Latest Minis screenshots are a functional IA reference only, not a visual style source. They add future surface gaps: token usage, appearance, persona/SOUL.md, memory, MCP integrations, environment variables, storage/shared folders/mounts, permissions, background/notifications, logs, about/privacy/feedback.

## Gap Matrix

| OpenMinis reference area | Freehand current state | Gap | Owner |
| --- | --- | --- | --- |
| Add provider on first launch | Config file exists; no WebUI setup wizard | Missing visible first-run/config-needed flow | `config.core`, `ui.protocol`, `app.webui-smoke` |
| Supported provider list and custom endpoint | Provider registry exists in TOML | Missing WebUI provider list/editor/status | `config.core`, `ui.protocol`, `runtime.ui-command-dispatch`, `app.webui-smoke` |
| Select model per conversation | WebUI has read-only runtime model display | Missing editable model/session selection contract | `config.core` first; later `ui.protocol` if per-session |
| Model groups fallback/load balance | Not implemented in config truth | Product gap; do not fake in UI | future `config.core` / provider routing design |
| Agent loop model pool | Agents select provider in config | Missing visible agent/provider topology view | `config.core`, `app.webui-smoke` |
| Skills settings | Instruction capability loader indexes skills | Missing visible skill registry/settings surface | `instruction.capability-loader`, `ui.protocol`, `app.webui-smoke` |
| Token usage | Provider usage exists in turn/rewrite paths | Missing user-facing usage/billing/token diagnostics | `provider.semantic`, `reason.rewrite-policy`, future usage UI owner |
| Appearance | Responsive WebUI/mobile layout exists | Missing user-configurable theme/density/text-size surface | `app.webui-smoke`, `app.android-client` |
| Persona / SOUL.md | No owner-backed persona settings surface | Missing persona/profile instruction management | future instruction/persona owner |
| Memory | Session history/rewrite exists | Missing memory visibility and management UI | `reason.session-history`, future memory UI owner |
| MCP integrations | Not implemented as a user-facing integration surface | Missing integration registry/config UI | future tool/integration owner |
| Environment variables | Config supports env-var credential references | Missing safe env-var diagnostics/editor; must not expose secret values | `config.core`, `runtime.ui-command-dispatch` |
| Storage/shared folders/mounts | Session cwd and attachments exist | Missing storage/shared-folder/mount management | future file/storage owner, `reason.persistence` |
| Permissions | Runtime/Android permissions are scattered by platform | Missing preflight permission status/remediation surface | `app.runtime-daemon`, `app.android-client` |
| Background/notifications | Task orchestration exists; Android shell exists | Missing background execution and notification settings | `task.orchestration`, `app.android-client` |
| Logs/about/privacy/feedback | Debug artifacts exist but no product settings page | Missing user-facing logs/about/privacy/feedback panels | `debug.core`, `app.webui-smoke`, docs owner |
| Session filesystem namespaces | Attachments/session cwd exist; no namespace UI | Missing file/workspace/attachment/browser/offload overview | likely `reason.persistence` / future file owner / `app.webui-smoke` |
| Daemon/network config | Android has file-backed daemon config; WebUI served same-origin | Missing WebUI/admin daemon status page and Android-visible connection profile entry after remote WebUI load | `app.runtime-daemon`, `app.android-client`, `app.webui-smoke` |
| Native/background automation settings | Task orchestration exists; no mobile settings | Missing scheduled/background task UI | `task.orchestration`, later `app.webui-smoke` |
| FAQ-style diagnostics | Some status banners exist | Missing compact troubleshooting/settings help panels | `app.webui-smoke` |

## L2 Batch Order

1. Settings shell and style alignment: completed in commit `292eab8`.
2. Config projection API: completed for active agent, selected provider id/type/protocol/base URL host, default model, auth source type, and restart-required status.
3. Provider/model config edit path: completed for provider endpoint/default model/env-var auth updates with restart-required semantics and no direct UI writes.
4. Android settings entry convergence: release propagation is verified on 4041; true-device Android screenshot proof remains blocked until an ADB device is connected/unlocked.
5. Advanced surfaces: skills registry, session filesystem namespaces, task/background settings, and Minis-derived usage/appearance/persona/memory/MCP/storage/permissions/logs surfaces as separate owner-backed slices.

## Required Evidence For Any L2 Batch

- mapped function map/test design updates before implementation
- `node --check apps/freehand-server/assets/webui.js`
- relevant Rust/Kotlin unit tests for owner paths
- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- S-profile online WebUI proof on `127.0.0.1:4042`
- release 4041 + Android true-device proof only when claiming phone UI updated

## Current Non-Actions

- Do not implement model groups until provider routing owner design exists.
- Do not make WebUI write `~/.freehand/config.toml` directly.
- Do not expose or store raw API keys in UI projections.
- Do not introduce fallback provider/daemon endpoints.
