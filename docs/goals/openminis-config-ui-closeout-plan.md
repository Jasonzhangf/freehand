# OpenMinis-Inspired Config UI Closeout Plan

## Objective

Close Freehand's missing user-facing configuration surfaces by using OpenMinis as an information-architecture and visual-density reference while preserving Freehand's protocol/owner boundaries.

This plan is not a request to copy OpenMinis implementation. OpenMinis public source is not currently available; the reference is the public website and README.

## Reference Summary

OpenMinis presents configuration as a clear product surface:

- first-run setup guides provider addition, model selection, and conversation start
- provider table covers Anthropic, OpenAI, Google Gemini, OpenRouter, and custom OpenAI-compatible endpoints
- advanced settings include model groups, agent loop model pools, skills, and session filesystem namespaces
- FAQ/settings explain API/network, model choice, skills, files/storage, terminal, automation/background behavior
- style is low-noise: compact cards, disclosure sections, status labels, tables, and restrained typography

## Freehand Current Baseline

- `config.core` owns `~/.freehand/config.toml`, provider registry, selected agent, selected provider, and restart-only activation.
- `app.webui-smoke` owns WebUI shell/rendering and must remain protocol-only.
- `app.android-client` owns Android app-owned daemon connection config, but connected Android now loads daemon-hosted WebUI as the main UI.
- WebUI already has session CRUD basics, task/session creation, cwd selection, attachment draft UI, mobile drawers, lifecycle rendering, and ADP status transparency.
- WebUI does not yet have a full settings/config surface for provider/agent/daemon/skills/filesystem configuration.

## Architecture Rule

UI may render config projections and submit config intents, but it must not parse/write config semantics directly.

Required owner chain for editable config:

```text
WebUI / Android UI
  -> ui.protocol command/query shape
  -> runtime.ui-command-dispatch routing
  -> config.core or specific owner
  -> canonical persistence / restart-required projection
  -> ui.protocol projection
  -> WebUI / Android render
```

## L2 Batch Plan

### Batch 1: Read-Only Settings Shell

Goal: add a settings entry and responsive settings surface to WebUI, styled with OpenMinis-like compact cards/disclosures.

Scope:

- add Settings button/sheet/drawer in desktop and mobile layouts
- sections: Connection, Active Agent, Provider, Model, Sessions/Workspace, Skills, Files, About/Diagnostics
- read-only placeholders must clearly say "not configured" or "not implemented" and must not pretend mutation works
- Android remote WebUI automatically gets the same settings entry

Owners:

- `app.webui-smoke`

Validation:

- WebUI asset smoke for settings entry and mobile drawer/sheet
- S-profile browser screenshot showing conversation unaffected and settings visible
- no config writes

### Batch 2: Owner-Backed Config Projection

Goal: expose active config status safely.

Scope:

- `config.core` defines UI-safe projection: active agent, mode, node id, selected provider id/type/protocol/base URL host, default model, auth source type without secret
- `ui.protocol` defines query result for config status if needed
- `runtime.ui-command-dispatch` routes read-only query
- WebUI renders real values instead of placeholders

Owners:

- `config.core`
- `ui.protocol`
- `runtime.ui-command-dispatch`
- `app.webui-smoke`

Validation:

- config tests prove no API key leakage
- ADP/headless query test
- WebUI online proof

### Batch 3: Provider/Model Edit Flow

Goal: let users add/edit provider endpoint and default model through owner-backed commands.

Precondition:

- restart semantics are explicit: save config -> show "restart required"; no silent hot reload.

Scope:

- add `CreateProvider`, `UpdateProvider`, `SelectAgentProvider` or equivalent protocol command only after config owner design is updated
- validate provider type/protocol/base_url/default_model/auth source
- API key input must never be returned in projections; prefer env-var-first flow unless a secure local secret owner is designed
- UI shows validation errors and restart-required state

Owners:

- `config.core`
- `ui.protocol`
- `runtime.ui-command-dispatch`
- `app.webui-smoke`

Validation:

- positive add/update provider fixture
- negative unknown field/invalid protocol/auth leakage tests
- WebUI online add invalid provider -> visible error
- WebUI online valid env-based provider config -> projection updates with restart-required badge

### Batch 4: Android Connection Settings Convergence

Goal: Android keeps pre-connection repair native, but normal connected mode uses the same WebUI settings surface.

Scope:

- native config-error/connection drawer remains for daemon endpoint repair
- connected WebView route includes settings entry
- release asset hash check before phone proof

Owners:

- `app.android-client`
- `app.webui-smoke`

Validation:

- Android JVM tests for config edit remain passing
- release 4041 served assets match workspace
- true-device screenshot shows settings entry/layout in remote WebUI

### Batch 5: Advanced Surfaces

Goal: add non-fake advanced sections only after owners exist.

Candidates:

- skills registry view: `instruction.capability-loader`
- session filesystem namespace view: future file/session artifact owner
- task/background automation view: `task.orchestration`
- model groups/fallback/load-balance: future provider routing owner

Rule:

- do not render editable controls before owner-backed truth exists
- read-only "planned" documentation panels are acceptable only if clearly labeled

## UI Direction

Adopt the OpenMinis pattern without copying its generic landing page:

- settings is a practical app surface, not a marketing page
- compact grouped cards with clear labels, status pills, and disclosure rows
- providers/models shown as tables/cards with "active", "needs restart", "error" states
- no protocol jargon in user-facing text
- mobile uses full-screen sheet/stack page; desktop uses right drawer or inspector tab
- avoid large static headers that waste vertical space on phone

## Loop Governance

The active loop is `docs/loops/openminis-config-ui-closeout/`.

Start at L1 report-only. Move to L2 only when Jason approves a batch.

L2 implementation rules:

- one batch per loop action
- one primary owner per first diff where possible
- positive and negative tests
- online WebUI evidence before success claim
- Android true-device evidence only for phone-facing claims

## Completion Criteria

- settings entry exists and is usable on desktop WebUI and mobile/Android WebView
- config values shown to users come from owner-backed projections
- invalid/missing config is visible and actionable
- provider/model/agent/daemon basics are configurable or clearly read-only with no fake controls
- docs/maps/tests/mainline/wiki/memory are synchronized
- final proof includes browser screenshots and, for mobile release claims, Android true-device screenshot
