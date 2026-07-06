# OpenMinis Config UI Closeout Loop

- loop_id: `openminis-config-ui-closeout`
- owner feature: `foundation.workspace` for loop governance; implementation work routes by item owner
- initial mode: `L1 report-only`
- cadence: manual trigger only
- implementer: current agent run
- checker: Jason or a separate review pass before any L2 implementation
- kill switch: `docs/loops/openminis-config-ui-closeout/KILL_SWITCH`

## Purpose

Use OpenMinis as an information-architecture and UI-style reference to close Freehand's missing configuration surfaces without breaking Freehand's owner boundaries.

This loop starts in report-only mode. It may inspect OpenMinis public material, inspect Freehand owner maps, and produce a scoped implementation plan. It must not edit product code, runtime config, provider secrets, or launch services in L1.

## Reference Baseline

OpenMinis public source is not available as implementation code. The public repository `OpenMinis/OpenMinis` says source is still being organized for release. The usable reference is the public website `https://openminis.app/`:

- first-run setup: add AI provider, select model, start conversation
- supported providers: Anthropic, OpenAI, Google Gemini, OpenRouter, custom OpenAI-compatible
- advanced configuration: model groups with fallback/load-balance, agent loop models, skills, session filesystem namespaces
- mobile/native capability IA: settings pages, API/network FAQ, model selection FAQ, skills, files, terminal, automation/background tasks
- visual direction: clean low-noise Apple-like settings pages, compact cards, tables for provider/model info, FAQ-style disclosure groups, simple badges and status labels

## Modes

### L1 Report-Only

Allowed:

- inspect public OpenMinis website/repo metadata
- inspect Freehand docs, function maps, test designs, source, and current UI surfaces
- write this loop report, state, constraints, budget, run log, and a goal plan
- update `note.md` with report-only findings

Forbidden:

- product code edits
- `~/.freehand/config.toml` edits
- API key, auth, secret, provider config mutation
- launchd install/restart or release promotion
- Android install/device mutation
- auto-implementation

### L2 Assisted

Not enabled until Jason approves a specific batch.

L2 must implement exactly one batch from `docs/goals/openminis-config-ui-closeout-plan.md`, with owner maps and tests updated in the same change set.

### L3 Unattended

Not enabled.

## Owner Routing

Implementation items are split by owner:

- `config.core`: persisted daemon/runtime provider and agent config schema, validation, selected-provider projection.
- `ui.protocol`: protocol-owned config query/update commands and UI-safe projections, if config becomes remotely editable.
- `runtime.ui-command-dispatch`: runtime command/query routing into config owner or config manager.
- `app.webui-smoke`: WebUI settings/config presentation, drawers/pages, online browser verification.
- `app.android-client`: Android app-owned daemon connection profile UI and file-backed client config editing.
- `foundation.workspace`: loop docs, release gate or verification-script wiring only.

No single implementation batch may bypass those owners by putting config semantics directly into WebUI JavaScript or Android local presentation code.

## Done Signal

The closeout is complete only after:

- Freehand has a visible settings/config surface comparable to the OpenMinis IA baseline for provider/model/agent/daemon connection basics.
- WebUI and Android share the same visible style language where Android loads daemon-hosted WebUI.
- Config mutations, if supported, route through an owner-backed protocol/runtime path and persist to the canonical truth.
- Online WebUI evidence proves settings UI renders and does not break conversation/session behavior.
- Android true-device evidence proves the phone WebView shows the updated settings entry/layout when release assets are updated.
- Docs, function maps, test designs, mainline JSON/wiki, memory, and loop run log are synchronized.
