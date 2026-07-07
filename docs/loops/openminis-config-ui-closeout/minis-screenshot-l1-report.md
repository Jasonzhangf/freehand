# Minis Screenshot Functional Fit L1 Report

- run_id: `2026-07-07-openminis-screenshot-functional-fit-l1`
- mode: `L1 report-only`
- source: Jason-provided Minis screenshots plus existing Freehand function maps
- scope: screen what should be built for Freehand, what should be deferred, and what should be rejected
- non-scope: no product code, runtime config, provider secret, launchd, release, or Android device mutation

## Decision Rules

A Minis feature is eligible for Freehand only if it passes all three checks:

1. It fits Freehand's architecture: WebUI/Android/CLI consume one owner-backed truth and do not create UI-local product truth.
2. It has a current owner in `docs/architecture/feature-map.md`, or the next action is an owner-design task rather than implementation.
3. It can be verified through protocol/runtime query or real online UI evidence, not static UI alone.

If a feature looks useful but lacks owner truth, it is a design candidate, not an implementation candidate.

## Current Real Baseline

These are real Freehand capabilities today:

| Area | Current truth | Owner |
| --- | --- | --- |
| Active agent/provider/model/auth source projection | Real runtime-backed Settings projection | `config.core`, `ui.protocol`, `runtime.ui-command-dispatch`, `app.webui-smoke` |
| Provider endpoint/default model/env-var credential save | Real owner-backed config write, restart-only | `config.core`, `ui.protocol`, `runtime.ui-command-dispatch` |
| WebUI Settings shell and mobile Settings drawer | Real UI shell consuming protocol/runtime truth | `app.webui-smoke` |
| Android connected mode | Loads daemon-hosted WebUI; native config shell is pre-connection repair only | `app.android-client`, `app.webui-smoke` |
| Android daemon endpoint config | Real app-owned JSON config before connection | `app.android-client` |
| Sessions/workspace/cwd basics | Real WebUI/session protocol behavior exists | `ui.protocol`, `reason.persistence`, `app.webui-smoke` |
| Tool registry basics | Real built-in tool registry exists | `tool.registry` |
| Task runtime/query skeleton | Real task owner exists; UI task panel is not productized | `task.orchestration`, `runtime.ui-command-dispatch` |
| Debug/error metadata basics | Real debug/error owners exist; product log UI is not productized | `debug.core`, `error.center` |
| Instruction/skill manifest | Real capability manifest index exists; runtime consumption/settings UI are pending | `instruction.capability-loader` |

## Accepted Near-Term Product Surfaces

These fit Freehand and can be implemented incrementally because an owner exists.

| Minis-inspired surface | Freehand version | Required next step |
| --- | --- | --- |
| Provider settings | Provider list/status for configured providers, edit active provider endpoint/model/env credential, restart-required state | Extend `config.core` projection from selected provider to safe provider registry projection |
| Model selector | Active/default model display and owner-backed update; no model groups yet | Keep current restart-only default-model semantics unless per-session model owner is designed |
| Connection settings | WebUI service status plus Android pre-connection endpoint editor | Keep Android endpoint config native only before remote WebUI loads; connected mode uses WebUI Settings |
| Sessions/workspace | Session CRUD/cwd/task/global-session info with clean mobile drawer | Improve product presentation from existing `ui.protocol`/session truth |
| Skills read-only registry | Show indexed AGENTS/skills manifest entries and compile errors | Add `instruction.capability-loader` runtime query/projection before UI |
| Task/background read-only status | Show task list/history/agent status; do not add background scheduling settings yet | Use existing task query/subscription truth |
| Diagnostics/logs read-only | Show health, config status, error-center/debug summary, copied diagnostic bundle path | Add UI-safe projections from `debug.core`/`error.center`; no raw provider/request payload |

## Design-First Candidates

These look valuable but are not implementation-ready. The next action is an owner/design doc, not UI work.

| Minis-inspired surface | Why not ready | Needed owner/design |
| --- | --- | --- |
| Token usage / billing | Usage exists for rewrite pressure, not a user-facing accounting product | Decide usage aggregation owner across provider/reason/session |
| Appearance/theme/density | WebUI has responsive layout/theme assets, but no persisted user preference owner | Decide UI preference persistence and cross-device sync boundary |
| Persona / SOUL.md | No persona instruction owner or prompt-admission policy exists | New instruction/persona owner or extension of capability-loader plus context-planner |
| Memory management | Session history/rewrite exists, but no user-facing memory CRUD semantics | `reason.session-history` design for safe projection/mutation |
| MCP integrations | No MCP/integration runtime owner in feature map | New tool/integration owner before UI |
| Environment variable manager | Config can reference env vars, but UI must not manage shell secrets blindly | Config/secret owner decision; likely diagnostics-only first |
| Storage/shared folders/mounts | Session cwd and attachments exist; shared-folder/mount permissions do not | File/storage owner plus permission model |
| Permissions preflight | Android/runtime permissions are platform-specific and not centralized | Permission/status owner and platform adapters |
| Background/notifications | Task runtime exists, but scheduling/notification lifecycle does not | Task scheduling owner plus Android notification policy |
| Model groups/fallback/load balance | Provider routing semantics do not exist | Provider routing owner and failure/retry policy before UI |

## Rejected For Current Freehand

These should not be copied from Minis into Freehand as-is.

| Item | Reason |
| --- | --- |
| Copying Minis visual style | User explicitly said function can be referenced, style should not be copied; Freehand already has its own WebUI/mobile layout direction |
| UI-local fake controls | Violates one-truth architecture and previous failures; every editable setting needs owner-backed command/query |
| Raw API-key editor in WebUI | Current accepted path is env-var credential reference; raw secret storage/display needs a dedicated secret owner first |
| Per-platform duplicate settings | Android connected mode must use daemon-hosted WebUI Settings; native Android settings stay pre-connection repair only |
| Marketplace-like skills/MCP toggles without runtime owner | Would imply runtime behavior that does not exist and cannot be verified |
| `Archive` session product surface revival | Current WebUI intentionally removed archive/restore affordance; session deletion/CRUD needs clearer product semantics before exposing archive again |

## Recommended L2 Order

1. Provider registry projection: show all configured providers safely, not only selected provider.
2. Connection/diagnostics panel: service health, config status, release asset/version status, error-center summary.
3. Skills read-only registry: show compiled skills/AGENTS manifest entries and manifest errors.
4. Task read-only panel: show task list/history/agent state from existing task query truth.
5. Design doc only for usage, appearance persistence, persona/memory, MCP, storage/permissions, notifications, and model groups.

## Acceptance For Moving A Surface To L2

- Has one owner feature and function map entry.
- Has protocol/runtime query or command contract.
- Has positive and negative tests.
- Has WebUI online proof when visible in UI.
- Has Android true-device proof only when claiming phone surface update.
- Has no user-visible protocol jargon and no secret leakage.
