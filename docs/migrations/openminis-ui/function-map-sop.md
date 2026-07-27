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
8. real OpenMinis source files listed by the migration node
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
| `surface_path` | Target WebUI Surface directory |
| `model_responsibility` | Projection-to-render mapping only |
| `view_responsibility` | Rendering only |
| `control_responsibility` | User event to generated command/route edge |
| `route_edge_ids` | Registered adjacent UI edges |
| `function_map_docs` | Every affected feature function map |
| `mainline_call_docs` | Every affected machine call map |
| `test_design_docs` | Every affected test design |
| `verification_gates` | Must include `openminis_ui_migration_manifest` and the mapped static/module/project/online/Android gates |
| `status` | One state from the lifecycle below |
| `evidence` | Paths to tests/artifacts; empty until verified |

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

- every included manifest node is `online_verified` or explicitly removed from
  scope by an accepted design update
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
