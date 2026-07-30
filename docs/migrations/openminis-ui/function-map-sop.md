# OpenMinis UI Migration Function Map SOP

## 1. Purpose

This SOP is the required path for migrating one OpenMinis UI capability into
Freehand. It prevents visual copying from bypassing Freehand resource,
function-map, protocol, and owner boundaries.

The unit of migration is a `migration_unit_id`, not a Swift file, JS file, or
screen screenshot.

Example:

```text
ui_migration.session_detail.tool_activity
```

One migration unit represents one user-visible semantic path:

```text
source behavior
  -> target owner truth
  -> projection/query/command
  -> surface model
  -> view
  -> control
  -> owner receipt/re-query
```

## 2. Canonical Inputs

Read in this order before changing a migration unit:

1. `AGENTS.md`, `CACHE.md`, `MEMORY.md`, `note.md`
2. `docs/resource-maps/core.json`
3. `docs/migrations/openminis-ui/ui-tree.manifest.json`
4. `docs/architecture/feature-map.md`
5. affected `docs/function-maps/<feature-id>.md`
6. affected `docs/mainline-calls/<feature-id>.json`
7. affected `docs/testing/<feature-id>.md`
8. pinned `external/OpenMinis` source files listed by the migration node
9. real Freehand source files and symbols

Search results are only locators. Open both source and target files before
binding a symbol.

## 3. Migration Unit Record

Every unit must record these fields before implementation:

| Field | Requirement |
| --- | --- |
| `migration_unit_id` | Stable `ui_migration.<surface>.<semantic>` id |
| `target_node_id` | Existing node from the migration UI tree |
| `source_paths` | Real OpenMinis source paths |
| `source_semantic` | Behavior to preserve, independent of layout/color |
| `non_migrated_source_semantics` | Local owners/runtime behavior that must not cross |
| `source_resources` | Resources entering the Freehand path |
| `target_resource` | Resource or projection produced by the path |
| `owner_feature_id` | One Freehand migration-semantic owner |
| `touched_feature_ids` | Adjacent resource/protocol/dispatch participants; not co-owners |
| `operation_id` | Existing resource operation, or `pending` |
| `projection_or_query` | Owner-backed UI input, or `pending` |
| `generated_command` | Owner-backed UI action, or `none`/`pending` |
| `surface_path` | Normalized repository-relative target WebUI Surface file |
| `model_responsibility` | Projection-to-render mapping only |
| `view_responsibility` | Rendering only |
| `control_responsibility` | User event to generated command/route edge |
| `route_edge_ids` | Registered adjacent UI edges |
| `function_map_docs` | Every affected feature function map |
| `mainline_call_docs` | Every affected machine call map |
| `test_design_docs` | Every affected test design |
| `verification_gates` | Must include `openminis_ui_migration_manifest` and the mapped static/module/project/online/Android gates |
| `status` | One state from the lifecycle below |
| `evidence` | Structured gate proof records with node/gate/command/result/run/artifact identity; empty until verified |
| `legacy_retirement` | Required only for `legacy_retired`; removed identities, exact owner-mainline `legacy_scan_roots`, and dedicated no-touch gate |

Do not invent operation ids, protocol commands, symbols, or paths. Missing
bindings remain explicit `pending`.

## 4. Lifecycle

Allowed states:

```text
inventoried
  -> owner_mapped
  -> contract_ready
  -> implementation_in_progress
  -> source_bound
  -> online_verified
  -> legacy_retired
```

Blocking states:

```text
blocked_resource_missing
blocked_owner_missing
blocked_protocol_missing
blocked_verification_missing
```

Meaning:

- `inventoried`: OpenMinis behavior and source evidence are known.
- `owner_mapped`: Freehand resources, owner, operation, and forbidden
  shortcuts are registered.
- `contract_ready`: product UI tree, protocol shape, route edges, function
  maps, call maps, and test design agree.
- `implementation_in_progress`: claim acquired; code work started.
- `source_bound`: all call-map rows bind to real target symbols.
- `online_verified`: real daemon-hosted WebUI path has passed the mapped
  black-box proof.
- `legacy_retired`: duplicate legacy implementation is physically removed
  after dependency proof.

No state may be skipped by prose. `online_verified` requires evidence.
Unknown states are rejected rather than treated as blocking aliases.

Lifecycle field contracts are independent:

- `inventoried` requires the pinned source inventory; protocol and target
  binding fields may remain `pending`/empty.
- `owner_mapped` requires real source/target resources and a registered
  operation; it does not require `target_symbols`.
- `contract_ready` and `implementation_in_progress` additionally require
  non-pending projection/query, generated command or explicit `none`, surface,
  and target paths; those three UI contract fields must exactly repeat the
  canonical resource operation binding's `ui_contract`, and target symbols may
  still be empty.
- `source_bound`, `online_verified`, and `legacy_retired` require target symbols
  plus the complete contract.
- `blocked_resource_missing` and `blocked_owner_missing` must retain a pending
  resource/owner boundary; `blocked_protocol_missing` retains pending operation
  and projection truth; `blocked_verification_missing` is source-bound but lacks
  verification only.

`source_bound` and later states require target and mainline bindings to:

1. use canonical repository-relative `target_paths` and `mainline_call_docs`;
   absolute paths, dot segments, canonical escapes, and symlink traversal fail;
2. resolve every `target_symbol` exactly once in `target_paths` and retain that
   declaration's canonical repository-relative file;
3. equal an exact `symbol_path` segment on a real row in one of
   `mainline_call_docs`;
4. require that row's exact `file_path` to equal the retained declaration file
   and share that row with the node's `operation_id`;
5. use `binding_status: bound` on that row;
6. reference a resource-map operation binding whose own status is `bound`.

The foundation gate parses Rust syntax, modules, module/callable/block-local imports,
free functions, module and lexical impl/trait methods, associated calls,
imported receiver types, supported built-in container item types, and
statically resolvable method calls with `syn`. Trait-impl dispatch is indexed by receiver, including
inherited default methods when an implementation does not override them,
instead of silently dropping a missing inherent-method candidate. Closure,
loop, match-arm, direct/chained if-let, while-let, and destructured `let`
patterns replace outer receiver and iterable-item bindings; initializers and
conditional scrutinees are visited before the new binding enters scope, loop
binders clear/restore iterable metadata, and typed closure containers reuse the
single iterable-item inference owner. Callable parameters and local values
plus block-local `const`/`static` callable items shadow same-named module
functions. Active module, impl, and trait const/static initializers retain
deterministic owner-qualified call-site identities; inactive cfg initializers
fail closed. Imported aliases resolve before the bare-name fast path, nested
imports are restored with their block, and named struct patterns derive each
binder from the canonical field-type index rather than the whole item type. Method identities
include receiver and trait scope; test ownership comes from Rust test/cfg
attributes whose parsed predicate requires `test`, rather than symbol names or
any predicate that merely mentions `test`; parsed external module ancestry
propagates `#[cfg(test)]` identity before callable indexing. Unresolved
local-method collisions fail closed in test and production callables;
lexical method bodies are never attributed to the enclosing function. The gate
automatically derives the complete set of production migration callables with
multiple direct references and requires the machine `shared_functions` set plus
each module-qualified `allowed_callers` and direct `related_tests` set to match
exactly. Same-named functions in different modules remain distinct; no
hand-maintained tracked-symbol whitelist exists. Every
`graph_id=openminis_ui_migration` mainline row represents one real
module-qualified direct caller-to-callee edge, including recursion; bare
identities, prose aliases, grouped callers, missing edges, and extra edges fail.
For migration-owned callees, every legitimate non-test inbound caller remains
in graph truth. For callees outside the migration module, only direct
migration-owned callers remain; outside-to-outside edges are excluded. Inline
`Result<Local, E>` and `Option<Local>` `unwrap`/`expect` chains preserve the
local receiver identity for the following method edge. The current exact
registry has 69 total shared functions, 68 migration-owned shared functions,
and 300 production direct-call edges. Callable-local associated paths resolve
against callable scope; block-local const/static initializers own independent
callers; unsupported active nested impl/trait declarations fail closed.

Declaration admission is parser-owned per language: Swift compiler parse AST,
Rust `syn`, and JS/TS/Kotlin tree-sitter ASTs. Syntax-error trees fail before declaration traversal. CI and release workflows install the Swift toolchain before `make ci`. Comments, strings, regex literals, and token adjacency cannot create declaration truth.

Every lifecycle state routes `function_map_docs`, `mainline_call_docs`, and `test_design_docs` through the canonical repository path validator. Paths must live directly under their unique map directories, must not escape through absolute/dot/backslash/symlink forms, must carry the same feature-id set, must include `owner_feature_id`, and each Markdown/JSON document must identify its own feature and canonical path.

The pinned source gate requires `external/OpenMinis` to exist at exactly the
manifest SHA. It reads each node with `git ls-tree -r` and `git show`; missing
paths/symbols or BrowserUse/Cookie/Profile/Takeover paths/symbols fail. CI is
parsed only through `jobs.*.steps[]`: exactly one job must execute exactly one
`make ci`, and that same job must contain exactly one checkout step whose
`uses`, `with.repository`, `with.ref`, and `with.path` jointly select
`OpenMinis/OpenMinis`, the manifest SHA, and `external/OpenMinis`. Top-level
metadata, another job, or values scattered across unrelated steps do not count.
Local `make test`, `make gates`, `make ci`, and pre-commit first run
`scripts/provision-openminis-source.sh`. It reads the same manifest SHA,
creates only a missing sparse checkout from the canonical repository, and
validates the exact checkout root through Git so `.git`-file worktree/submodule
layouts remain valid. Concurrent first-run installation uses one atomic
owner-bearing lock; same-host stale owners are reclaimed only after PID
liveness fails. Waiters verify the winner's exact checkout, and every process
cleans only its own staging/lock artifacts. It rejects existing origin, HEAD, or dirty-state drift. The Rust gate remains
read-only; bypassing the wired entrypoint with a missing checkout is an
explicit gate failure.

`online_verified` and `legacy_retired` evidence is an array of:

```json
{
  "node_id": "session_detail.root",
  "gate_id": "webui_online_e2e",
  "command": "make verify-webui-online",
  "artifact_path": "docs/migrations/openminis-ui/evidence/session_detail.root/webui_online_e2e.json",
  "result": "passed",
  "online_run_id": "stable-online-run-id"
}
```

The artifact and report must both live under the dedicated
`docs/migrations/openminis-ui/evidence/` root. The artifact must be valid JSON
and repeat the same `node_id`, `gate_id`,
code-locked canonical `command`, `result`, and `online_run_id`. It must also
carry the gate's exact `proof_kind`, a distinct canonical
`verifier_report_path`, and that report's exact `verifier_report_sha256`. The
bound report must use schema `freehand.verifier-report.v1` and match the
code-owned verifier id, exact node `node_id` and `migration_unit_id`, command,
online run id, `result=passed`, `exit_code=0`,
valid start/finish timestamps, one attested `repository_commit` that resolves
specifically to a Git commit object, one `repository_tree` shared by all node
reports, and every required assertion as `true` (for WebUI:
`daemon_hosted`, `owner_truth_verified`, and `dom_assertions_passed`).
Assertions in the attestation itself do not count. A copied or self-selected
command, proof kind, generic `passed` field, report digest drift, or incomplete
report assertion set cannot promote a node. WebUI, Android-device, and legacy
no-touch reports enter the same verifier-report admission path as repository
gates and must carry a valid Ed25519 signature over the complete report payload
from the externally held runner key pinned in the verifier. Categorical
rejection by gate id makes terminal lifecycle states unrepresentable, while
unsigned locally authored JSON is not provenance. The admission gate reads Git diff/untracked
status and permits changes after that
source revision only for the exact attestation and verifier-report paths named
by the node plus the canonical
`docs/migrations/openminis-ui/ui-tree.manifest.json` lifecycle transition. It
loads the manifest from the attested commit and the current worktree, removes
only top-level `status` plus per-node `status`, `evidence`, and
`legacy_retirement`, and requires the remaining contract to be identical. The
current manifest is still independently validated by the complete
lifecycle/binding/evidence/topology gate. Any committed, staged, unstaged, or
untracked source, call registry, verifier, manifest-contract, or other repository path drift
against the report's commit/tree fails. Evidence
must cover every declared `verification_gates` entry exactly; prose and
non-JSON files are rejected.

Repository-gate evidence is generated by:

```bash
cargo run -p xtask -- openminis-ui verify-node <node_id>
```

This verifier projects promoted nodes back to `source_bound`, removes evidence
and retirement records from that verification projection, and executes the
source/map/topology/binding/call-graph gates. It does not consume the report it
is producing and therefore does not let `xtask gates check` certify itself.
The report remains valid for only its exact `node_id` and
`migration_unit_id`; generic or cross-node report reuse fails.

The Rust call-map gate derives every direct production caller/callee edge under
`xtask/src/openminis_ui_migration/` without a function-name whitelist. Nested
function items fail explicitly because they do not yet have an independent
module-qualified registry identity; cfg-disabled statements, nested
expressions, and match arms cannot be credited to the outer caller. Module,
callable-local, and nested-block glob imports resolve qualified paths against
real definitions; callable-local declarations shadow same-named module
declarations, associated initializers retain their `Self` owner, and block-local
initializers retain callable scope. Multiple unshadowed matching candidates fail as ambiguous. Production
and test cfg projections are discovered independently, each processes only file
modules reachable through declarations active in that projection, then they are
merged by retaining all production truth plus test-only definitions and
callers. Mutually exclusive external modules, imports, or same-name definitions
therefore cannot create false ambiguity, duplicate identity, or production
leakage.

Every manifest node must be forward-reachable from `foundation.root`. A node at
`contract_ready` or later, including `blocked_verification_missing`, must also
repeat the exact non-empty set of incident manifest edge ids in
`route_edge_ids`; missing, extra, unrelated, or unknown route ownership fails
before lifecycle promotion.

`legacy_retired` additionally requires exactly one row in the owning feature's
machine mainline map before the manifest may reference the roots:

```json
{
  "feature_id": "app.webui-smoke",
  "legacy_scan_roots": [
    {
      "node_id": "session_detail.root",
      "owner_feature_id": "app.webui-smoke",
      "scan_paths": ["apps/freehand-server/assets/webui/surfaces/session-detail"],
      "removed_paths": [],
      "removed_symbols": ["legacyOwnedSymbol"],
      "removed_import_tokens": [],
      "removed_callers": []
    }
  ]
}
```

The row must live in a canonical `docs/mainline-calls/*.json` document whose
`mainline_call_doc` self-identity matches that path and whose `feature_id` equals
the node `owner_feature_id`. Exactly one row may match the node. The manifest
then repeats the exact registered roots and removed identity sets:

```json
{
  "legacy_retirement": {
    "required": true,
    "scan_paths": ["apps/freehand-server/assets/webui"],
    "removed_paths": [],
    "removed_symbols": ["legacyOwnedSymbol"],
    "removed_import_tokens": [],
    "removed_callers": [],
    "online_no_touch_gate_id": "openminis_ui_legacy_online_no_touch"
  }
}
```

At least one owner-registered removed identity is mandatory. `scan_paths`, evidence paths, and
removed paths must be canonical repository-relative paths: absolute paths,
backslashes, `.`/`..`, non-canonical spellings, and canonicalized escapes outside
the repository fail. Every registered scan root must be an existing non-symlink directory and recursive scans reject nested symbolic links;
the roots must cover every bound `target_path` and every `removed_path`. Removed
paths must be absent even as broken symlinks; removed symbols/import tokens/callers must not occur under
the owner-bound roots; the dedicated evidence artifact must also contain
`legacy_touched: false`. An arbitrary in-repository empty directory, a row in a
touched feature's mainline, wrong/duplicate owner rows, registry/manifest drift,
an unregistered/fabricated removed identity, or an uncovered target/removed path is a hard failure.

The human UI tree and machine manifest both own the exact entrypoint, forward
edge ids/from/to/semantics, and return from/to/semantics. Mermaid node/forward
pairs and explicit human registry tables must match the machine sets
bidirectionally.

## 5. Required Function-Map Chain

### 5.1 Read path

```text
resource owner truth
  -> owner safe projection
  -> runtime query port
  -> ui.protocol query/projection
  -> app-shell ADP client
  -> surface model
  -> surface view
```

### 5.2 Mutation path

```text
surface control
  -> registered route/control edge
  -> generated ADP command
  -> ui.protocol ingress validation
  -> runtime.ui-command-dispatch
  -> resource owner operation
  -> structured receipt/error
  -> owner re-query
  -> new projection
  -> surface model/view
```

### 5.3 Error path

```text
owner validation/runtime error
  -> typed protocol failure
  -> surface error render model
  -> visible recoverable or terminal state
```

Forbidden:

- view directly opening WebSocket or constructing ADP payloads
- controls mutating projected owner truth
- WebUI reading config/session/task files
- Android holding conversation/config/tool truth
- localStorage as owner state
- parsing model prose to infer tool/session/config terminal state
- command failure rendered as success
- fallback to another owner or transport

## 6. Surface Contract

Target Surface modules use this responsibility split:

```text
surfaces/<surface>/
├── index.js       # assembly and lifecycle
├── model.js       # owner projection -> render model
├── view.js        # render only
├── controls.js    # event -> route edge/generated command
└── optional focused modules
```

Rules:

1. `index.js` may wire dependencies but does not own reusable business logic.
2. `model.js` is deterministic and never writes owner truth.
3. `view.js` receives a render model and emits no protocol frames.
4. `controls.js` uses registered edges and generated protocol constructors.
5. Cross-surface navigation goes through the route controller.
6. Shared code moves to one shared owner only after reuse is proven.
7. `legacy-monolith.js` is a temporary compatibility boundary, not a valid
   destination for new migrated semantics.

## 7. Source-to-Target Decision Rules

Classify every OpenMinis behavior:

| Classification | Action |
| --- | --- |
| presentation semantic | migrate into Surface model/view |
| interaction semantic | map to route edge or generated command |
| resource management semantic | map to existing Freehand owner |
| missing Freehand resource | block; add resource/owner/maps/gates first |
| OpenMinis local runtime semantic | do not migrate into WebUI/Android |
| layout/color/platform styling | reference only; do not copy |
| browser semantic | exclude from this migration goal |

OpenMinis types and functions are evidence of source behavior. They never
become Freehand owner names automatically.

## 8. Per-Unit Execution SOP

### Step 1 — Inventory

- open every listed OpenMinis source file
- identify visible states, controls, errors, empty states, and lifecycle
- separate presentation semantics from local runtime/store behavior
- update the migration node if source evidence differs

### Step 2 — Resource and owner gate

- identify source and target resources in `core.json`
- confirm direct/indirect relation
- confirm operation id and source-edge binding
- confirm one owner feature
- if missing, stop implementation and mark the exact blocking state

### Step 3 — Contract design

- update the product UI tree and machine manifest
- bind query/projection, command, receipt, and error shapes
- register adjacent route/control edges
- update function maps, mainline call maps, and test designs before code

### Step 4 — Implementation

- acquire one semantic claim
- migrate one vertical slice end-to-end
- preserve real payload semantics
- do not add local truth or fallback
- do not grow `legacy-monolith.js` with new owner semantics

### Step 5 — Verification

Run the unit's mapped stack:

- syntax/schema checks
- owner white-box tests
- protocol/runtime module black-box tests
- server/WebUI project black-box tests
- mainline generation/check
- architecture gates
- real S-profile WebUI interaction through the headless runner
- Android true-device proof when Android-facing behavior or packaged assets
  change

Positive and negative proof must cover:

- accepted owner truth renders correctly
- rejected command remains rejected and visible
- nonterminal state does not become terminal early
- terminal state does not continue animating
- stale response does not overwrite the current route/resource
- secret/debug/metadata does not enter public UI

### Step 6 — Legacy retirement

- prove all imports/callers moved
- prove current online path no longer reaches the legacy implementation
- obtain authorization if deletion scope is destructive
- physically remove the duplicate implementation
- rerun mapped gates and online proof

## 9. Batch Ordering

Migration order is dependency-locked:

1. `foundation.*`: protocol, shell, route, Surface contract
2. `home.*` and `session_detail.*`
3. `turn_blocks.*` and `composer.*`
4. `tools.*`
5. `settings.*`
6. `search.*`, `new_session.*`, `timer.*`
7. `files_artifacts.*`
8. `skills.*`, `memory.*`, `integrations.*` after owners exist
9. legacy retirement

Do not start a later unit when its required owner/protocol foundation remains
pending.

## 10. Completion Rules

The foundation migration is complete only when:

- every included manifest node is `legacy_retired`; `online_verified` proves
  runtime behavior but is not aggregate completion
- no included node remains `inventoried`, `pending`, or blocked
- all implemented controls use generated commands or registered route edges
- all visible business state comes from owner projections
- browser scope remains absent
- duplicate legacy implementations are retired
- current product UI tree, resource map, feature/function maps, mainline call
  maps, tests, wiki, and migration manifest agree
- WebUI online proof and required Android proof exist

Until then, report exact completed and blocked nodes; never report “all UI
migrated.”
