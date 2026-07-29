# OpenMinis UI Foundation Migration

## Status

- status: `design_baseline` (advances to `migration_in_progress` and `migration_complete` only through the machine lifecycle gate)
- scope: OpenMinis-inspired UI foundation and tool/configuration interaction migration
- browser scope: excluded
- Freehand UI owner: `app.webui-smoke`
- protocol owner: `ui.protocol`
- command/query bridge: `runtime.ui-command-dispatch`
- canonical migration UI tree: `docs/migrations/openminis-ui/ui-tree.md`
- machine-readable tree: `docs/migrations/openminis-ui/ui-tree.manifest.json`
- migration function-map SOP: `docs/migrations/openminis-ui/function-map-sop.md`
- execution plan: `docs/goals/openminis-ui-foundation-migration-plan.md`
- pinned source checkout: `external/OpenMinis` at `9cf3a855fecd27bb5735b84cacbd56852a3ab8dd`
- architecture gate: `cargo run -p xtask -- gates check`

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

`xtask gates check` opens the deterministic `external/OpenMinis` checkout,
requires its HEAD to equal the manifest commit and requires one GitHub Actions
checkout step to bind `repository`, `ref`, and `path` together. It recursively
resolves every declared source path/symbol at that commit, rechecks every
resolved blob rather than trusting an allowed ancestor path, and rejects
BrowserUse/Cookie/Profile/Takeover scope. It also enforces the human/machine
entrypoint, forward-edge, and return-path registries plus forward reachability
of every node from `foundation.root`; lifecycle-specific fields; exact
promoted-node operation owner/source/target/direct-relation bindings and exact
non-empty incident `route_edge_ids` plus normalized repository-relative
in-repository UI surface files; canonical repository-relative, symlink-free
source-bound target/mainline/resource-operation bindings whose accepting row
names the target symbol's exact declaration file; canonical
function/mainline/test map directories, aligned feature sets, owner inclusion,
and document self identity in every lifecycle state; the automatically derived
`syn`-parsed Rust module/import/function/impl/trait graph with independently
discovered production and test cfg projections, production-plus-test-only
merge semantics, no callee-name whitelist, explicit nested-function rejection,
receiver-indexed trait dispatch, receiver/trait-qualified method identities,
module/impl/trait initializer identities, and module-qualified
shared-callable/caller/test/edge identities, including associated-initializer
`Self` ownership, callable-local shadow precedence, callable-scoped block-local
initializer callers, and fail-closed unsupported active nested impl/trait
declarations; code-locked evidence
command/proof-kind/assertion identity whose `repository_commit` names a real
commit object;
and owner-feature mainline-bound, canonical repository-relative legacy-retirement/no-touch proof. A `legacy_retired` node cannot choose its own scan directory or removed identity: exactly one `legacy_scan_roots` row in the owning feature mainline must match the node/owner/paths/removed identity sets and cover every bound target and removed path. `inventoried` does
not mean implemented, blocked states cannot masquerade as implementation, and aggregate `migration_complete` requires every included node to be `legacy_retired`.

On a fresh local clone, `make gates`, `make test`, and `make ci` first execute
`scripts/provision-openminis-source.sh`. The script reads the manifest commit
and creates a sparse exact-SHA checkout. Checkout identity comes from Git, so
normal clones, worktrees, and submodules use the same exact-root validator.
Concurrent first-run calls serialize through an atomic owner-bearing lock;
same-host stale owners are reclaimed only after PID liveness fails, waiters
verify the winner, and each process cleans only its own staging/lock artifacts.
An existing checkout with origin, HEAD, or worktree drift fails explicitly instead of being rewritten. CI and
release keep their explicit pinned `actions/checkout` step.

The map feature set must equal `touched_feature_ids`. Pinned Swift and promoted
target symbols must resolve as one real parser AST declaration using the Swift
compiler, `syn`, or the matching JS/TS/Kotlin grammar, never as a comment,
string/regex literal, call site, longer identifier, or duplicate. CI installs
the Swift parser before running the full gate. Online attestations bind a
distinct `freehand.verifier-report.v1` report by canonical path and SHA-256.
Repository-gate reports come from the non-recursive
`cargo run -p xtask -- openminis-ui verify-node <node_id>` command rather than
from `xtask gates check` certifying itself. Verifier identity, exact
`node_id`/`migration_unit_id`, command, run, process result, timestamps, assertions, and
one resolvable attested source commit/tree are validated from repository reports.
Committed or uncommitted source/manifest/verifier drift outside the exact
attestation/report paths invalidates the proof.

WebUI, Android-device, and legacy no-touch reports use the same source-attested
verifier-report contract as repository gates and additionally require an
Ed25519 signature from the external runner key pinned in the verifier. Required
online gate ids are not rejected categorically, but locally authored JSON cannot
promote lifecycle truth without that signature.
