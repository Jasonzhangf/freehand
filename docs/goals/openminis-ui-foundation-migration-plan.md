# OpenMinis UI Foundation Migration Plan

## Objective

Complete the non-browser OpenMinis-to-Freehand UI foundation migration using
the migration SOP and UI tree as the execution registry. Migrate semantics,
surface layering, configuration calls, transcript blocks, composer behavior,
and tool interaction while preserving Freehand daemon ownership.

## Acceptance Criteria

1. Every included node in
   `docs/migrations/openminis-ui/ui-tree.manifest.json` reaches
   `online_verified`.
2. Missing resources/owners/protocols are established resource-first before
   their UI controls are implemented.
3. Every read and mutation follows:

   ```text
   owner truth
     <-> ui.protocol
     <-> runtime.ui-command-dispatch
     <-> app.webui-smoke Surface
   ```

4. Major surfaces use explicit `index/model/view/controls` responsibilities.
5. All controls use registered route edges or generated ADP commands.
6. Browser, Cookie, browser profile, and takeover work remain absent.
7. Android remains daemon-hosted WebUI plus platform bridges.
8. Migrated duplicate code is removed after dependency and online proof.
9. Headless WebUI runner verification passes; Android true-device verification
   passes for Android-facing changes.

## Scope

### In scope

- app shell, routes, edge registry, Surface contract
- Home, SessionDetail, search, new session
- transcript/turn blocks
- composer, attachments, queue, stop/continue where an owner exists or is
  created
- tool activity, registry, detail, permissions owner path
- Settings and basic configuration
- Timer
- files/artifacts, Skills, Memory, integrations after resource owners and
  protocol contracts are established
- Android platform bridges
- legacy WebUI retirement
- maps, tests, gates, wiki, memory, and real online evidence

### Out of scope

- all browser/Cookie/profile/takeover work
- visual copying of OpenMinis layout, colors, typography, or SwiftUI code
- local Agent runtime, provider client, conversation store, or config truth in
  Android/WebUI
- fallback UI or fallback execution paths

## Canonical Documents

1. `docs/migrations/openminis-ui/README.md`
2. `docs/migrations/openminis-ui/function-map-sop.md`
3. `docs/migrations/openminis-ui/ui-tree.md`
4. `docs/migrations/openminis-ui/ui-tree.manifest.json`
5. `docs/design/mobile-webui-ui-tree.md`
6. `docs/design/mobile-webui-ui-tree.manifest.json`
7. `docs/resource-maps/core.json`
8. `docs/architecture/feature-map.md`

## Design Rules

1. Work by migration node, not by screenshot or source file.
2. Resource map precedes function map; function map precedes implementation.
3. OpenMinis is source behavior evidence, never target owner truth.
4. One semantic owner and one generated protocol path.
5. Surface models derive render state; views render; controls dispatch.
6. Missing capability fails explicitly and blocks the node.
7. No new semantics enter `legacy-monolith.js`.
8. No dead or duplicate implementation remains after migration proof.

## Implementation Phases

### Phase 0 — Migration governance

- keep the existing `xtask gates check` migration-manifest gate green
- advance node status only after the gate-required operation, target symbol,
  map, test, and evidence fields exist
- link the migration registry from design/function-map indexes
- reconcile current product UI tree with migration target nodes

### Phase 1 — Foundation

- finish app shell, route controller, edge registry, generated protocol call
  boundary, and Surface module contract
- make `legacy-monolith.js` compatibility-only
- add static import/edge/growth gates

### Phase 2 — Core conversation

- close Home and SessionDetail
- close chronological transcript and all non-browser turn blocks
- close composer text, attachment, queue, cancel/continue lifecycle
- prove consecutive submits, live/nonterminal state, terminal freeze, restart,
  stale response isolation, and selected-session continuity

### Phase 3 — Tools

- align tool registry, tool detail, session tool activity, and permission/
  confirmation behavior
- preserve tool owner projection and structured terminal state
- prove no provider-hosted search appears as a local function tool

### Phase 4 — Settings and configuration

- align Settings stack and basic provider/model/runtime/connection/
  observability/about surfaces
- establish an owner for UI appearance preferences before persistence
- prove safe config projection, write-only secrets, validation errors, and
  owner re-query

### Phase 5 — Secondary resources

- close Timer
- establish resources and owners for files/artifacts, Memory, and integrations
- expose Skills through instruction capability owner plus protocol
- implement each Surface only after its owner path and tests exist

### Phase 6 — Platform and legacy closeout

- verify Android platform bridges and daemon-hosted WebUI behavior
- prove no Android business truth
- remove migrated legacy functions/styles/selectors
- regenerate wiki and close every migration node with evidence

## Files

Expected target areas:

- `docs/resource-maps/core.json`
- `docs/architecture/feature-map.md`
- `docs/design/mobile-webui-ui-tree*`
- `docs/migrations/openminis-ui/**`
- `docs/function-maps/**`
- `docs/mainline-calls/**`
- `docs/testing/**`
- `docs/wiki/**` generated only
- `apps/freehand-server/assets/webui/**`
- `apps/freehand-server/assets/webui.css`
- `apps/freehand-server/src/{page,assets,lib}.rs`
- `crates/freehand-ui-protocol/**`
- `crates/freehand-runtime/**` for thin routing only
- resource owner crates identified per node
- `apps/freehand-android/**` only for platform bridge work
- focused online verifiers under `scripts/`

Do not broadly rewrite unrelated runtime/provider/task code.

## Risk Controls

| Risk | Control |
| --- | --- |
| visual copy without owner semantics | migration-unit SOP and owner gate |
| WebUI-local state becomes truth | manifest exclusion plus static/online checks |
| giant migration hides regressions | one vertical node slice at a time |
| fake module split | import/edge checks and live browser bootstrap proof |
| blocked nodes get placeholder controls | blocked states forbid implementation |
| secret/debug leakage | protocol, DOM, log, screenshot, artifact scans |
| stale legacy remains active | import/call proof plus online path evidence before deletion |
| dirty multi-worker repository | semantic claims and targeted commits only |

## Verification Matrix

### Per node

- owner white-box tests
- protocol/runtime module black-box tests
- WebUI project black-box tests
- positive and negative lifecycle tests
- syntax/schema checks
- mainline generation/check
- architecture gates

### Per phase

- S-profile real WebUI interaction through the headless runner
- DOM compared with owner query truth
- console/exception capture
- viewport matrix and no horizontal overflow
- reload/reconnect/restart continuity
- secret/internal metadata leakage scan

### Final

- complete migration-manifest audit
- `cargo build --workspace`
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- real S-profile end-to-end WebUI proof
- Android true-device proof for packaged/bridge changes
- codex review PASS

## Execution Order

1. Reconcile and gate migration docs.
2. Close foundation nodes.
3. Close core conversation nodes.
4. Close tools nodes.
5. Close Settings/config nodes.
6. Create missing resource owners/contracts and close secondary nodes.
7. Close Android platform nodes.
8. Retire legacy code.
9. Run full verification and migration audit.
10. Update memory and commit only owned changes.

## Definition of Done

- all included manifest nodes are `online_verified`
- blocked nodes are resolved rather than hidden or skipped
- product UI tree and migration tree agree
- all code-bound maps resolve real symbols
- all user actions use the owner protocol path
- browser scope is untouched
- legacy duplicate UI implementation is removed
- required local, online, and Android evidence exists
- final review returns `VERDICT: PASS`

Do not claim completion merely because the UI renders or local tests pass.
