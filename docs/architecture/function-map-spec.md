# Function Map Spec

Each feature owner crate should eventually add its own machine-readable function map file.

For features that have migrated mainline documentation, the machine-readable mainline call source is a separate truth from the human-readable function map and generated wiki.

Suggested filename:

- `function-map.toml`
- durable human-readable companion doc under `docs/function-maps/<feature-id>.md`
- migrated machine-readable mainline call source under `docs/mainline-calls/<feature-id>.json`
- generated wiki under `docs/wiki/<feature-id>.md`

## Required Keys

```toml
feature_id = "provider.semantic"
owner = "crates/freehand-provider-core"
owner_module = "crate root or module path"
owner_entry_symbols = ["TBD until implementation lands"]
allowed_paths = ["crates/freehand-provider-core/**"]
forbidden_paths = ["apps/**", "crates/freehand-ui-protocol/**"]
required_checks = ["cargo test -p freehand-provider-core"]
required_white_box_tests = ["semantic_event_maps_provider_fixture"]
required_module_black_box_tests = ["provider_stream_contract_smoke"]
required_project_black_box_tests = ["reason_provider_end_to_end_smoke"]
test_design_doc = "docs/testing/provider.semantic.md"
function_map_doc = "docs/function-maps/provider.semantic.md"
mainline_call_doc = "docs/mainline-calls/provider.semantic.json"
generated_wiki_doc = "docs/wiki/provider.semantic.md"
debug_artifacts = ["fixtures/providers/openai/*.json"]
lifecycle_checks = ["information sufficient", "logic closed-loop", "lifecycle management complete"]
```

## Required Human-Readable Sections

Each feature must also keep a durable function-map doc with at least:

- owner crate
- owner module
- owner entry symbols
- request mainline description
- response mainline description
- error mainline description
- shared multi-reference function registry
- function call table
- sync status against code
- when migrated, one `mainline call source` reference
- when migrated, one `generated wiki` reference

Tool-owning features must also document:

- tool spec owner
- spec-to-runtime exposure rule
- explicit implemented vs unimplemented state
- execution entry symbol
- permission or side-effect notes when applicable

## Code Binding Rule

The function map is not a loose prose note. It must bind to code.

Minimum binding fields for each function-call-table row:

- step id
- symbol path
- file path
- responsibility
- input semantic
- output semantic
- caller / callee relationship
- notes on adjacent-node boundary
- data/control isolation notes for cross-module calls
- metadata owner/write-node notes when the feature writes internal control metadata

If implementation is not landed yet, the row must say binding is pending. Do not pretend symbols exist.

## Shared Multi-Reference Function Rule

Any function called from multiple sites must have one shared semantic description in the function map:

- canonical symbol path
- owner
- purpose
- allowed callers
- why it is shared instead of duplicated
- related tests
- related pipeline or lifecycle position

Possible additional shared-contract fields when needed:

```toml
shared_ids = ["agent_id", "session_id", "turn_id", "trace_id", "feature_id"]
serialization = ["serializable", "replayable", "persistable"]
```

## Rules

- one feature id, one owner
- one function semantic, one owning crate or module
- one request/response/error mainline, one explicit logic description
- shared logic must move into `freehand-blocks`
- new functions require a prior library search inside existing owner crates and blocks
- Temporary helper functions are forbidden in orchestrator crates.
- helper functions are forbidden in orchestrator crates unless they are entrypoint glue with no reusable semantic logic
- provider wire formats must stay in provider adapter crates
- each feature map entry should state lifecycle closure checks, not only tests
- each feature map entry should classify tests into white-box, module black-box, and project black-box
- each feature map entry should point to one durable `test_design_doc`
- each feature map entry should point to one durable `function_map_doc`
- migrated features should point to one `mainline_call_doc`
- migrated features should point to one `generated_wiki_doc`
- test design must describe the logic path and lifecycle path being covered, not only test names
- test design and test implementation must be updated together when feature truth changes
- function-map logic descriptions and function-call tables must be updated together with code changes
- generated wiki must be produced from the machine-readable mainline call source, not manually edited
- tool-facing features must not expose a new tool before the function map binds that tool surface and its execution path
- tool-facing features must describe which symbols own spec declaration, implemented-state gating, runtime exposure, and execution
- request/response/error mainlines must be described even when they cross multiple crates
- multi-reference functions must be documented once and reused by reference, not redescribed ad hoc in each caller
- shared contract features should state ID and serialization guarantees explicitly
- metadata/debug/provider/cache/control fields must not be mixed into request-chain data fields
- cancellation, retry, routing, checkpoint, gate, and debug control state must not be represented by ad hoc request payload, provider payload, prompt, or context rewrites
- if a feature crosses module boundaries, its function map must identify the allowed envelope builder that combines metadata and request data for transport, ledger, or debug output
