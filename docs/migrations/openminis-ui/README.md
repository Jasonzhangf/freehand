# OpenMinis UI Foundation Migration

## Status

- status: `design_baseline`
- scope: OpenMinis-inspired UI foundation and tool/configuration interaction migration
- browser scope: excluded
- Freehand UI owner: `app.webui-smoke`
- protocol owner: `ui.protocol`
- command/query bridge: `runtime.ui-command-dispatch`
- canonical migration UI tree: `docs/migrations/openminis-ui/ui-tree.md`
- machine-readable tree: `docs/migrations/openminis-ui/ui-tree.manifest.json`
- migration function-map SOP: `docs/migrations/openminis-ui/function-map-sop.md`
- execution plan: `docs/goals/openminis-ui-foundation-migration-plan.md`

This directory defines what “migrate the OpenMinis UI foundation to Freehand”
means before implementation starts. It does not make OpenMinis local stores,
view models, provider factories, or agent loops valid Freehand owners.

The existing product tree remains:

- `docs/design/mobile-webui-ui-tree.md`
- `docs/design/mobile-webui-ui-tree.manifest.json`

The migration tree is a source-to-target review and execution registry. A
migration task may update the product tree only after its source semantics,
Freehand owner path, protocol path, and verification path are accepted through
the SOP.

## Non-Negotiable Boundary

```text
OpenMinis source UI behavior
  -> semantic inventory only
  -> Freehand resource/feature owner
  -> ui.protocol projection or generated command
  -> app.webui-smoke surface model/view/controls
  -> daemon owner re-query
```

Never migrate this path:

```text
OpenMinis local ViewModel/store/runtime
  -> browser-local or Android-local Freehand truth
```

## Scope Summary

Included:

- app shell and route hierarchy
- Home and SessionDetail
- transcript/turn blocks
- composer, attachments, queue, stop/continue
- tool activity and read-only tool registry
- Settings hierarchy and owner-backed basic configuration
- session search and new-session flow
- Timer, diagnostics, files/artifacts, Skills, Memory, and integrations as
  explicit migration nodes
- shared surface/model/view/control layering
- Android thin-shell platform bridges

Excluded:

- browser, cookies, account profiles, browser takeover
- copying OpenMinis layout, colors, typography, or SwiftUI code
- moving provider/runtime/session/config truth into WebUI or Android
- local Agent fallback
- anti-detect browser behavior

Nodes whose Freehand resource owner or protocol path does not exist remain
`blocked_owner_missing`; their UI controls must not be implemented as fake
local state.
