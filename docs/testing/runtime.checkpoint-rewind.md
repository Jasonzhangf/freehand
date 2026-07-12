# Test Design: `runtime.checkpoint-rewind`

- feature_id: `runtime.checkpoint-rewind`
- owner: `crates/freehand-runtime`
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `workspace_path.checkpoint_before_write`
  - `runtime_command.rewind_checkpoint`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `workspace_path.checkpoint_before_write` | bound | `cargo test -p freehand-runtime checkpoint -- --nocapture` covers checkpoint manifest, preview path-set snapshot, runtime-home root, no-preview rejection, and corrupt ledger tests | `cargo test -p freehand-runtime checkpoint -- --nocapture` covers writable tool loop creates checkpoint before execute and can list checkpoint summaries from owner truth | `cargo test -p freehand-daemon checkpoint -- --nocapture` covers daemon/CLI writable-tool path mutating files only after checkpoint owner admission |
| `runtime_command.rewind_checkpoint` | bound | `cargo test -p freehand-runtime checkpoint -- --nocapture` covers rewind restore create/modify/delete, missing manifest/blob, corrupt ledger, and bootstrap failure tests | `cargo test -p freehand-runtime checkpoint -- --nocapture` covers runtime dispatcher routes explicit rewind to checkpoint owner and refreshes protocol state | `cargo test -p freehand-daemon checkpoint -- --nocapture` covers daemon HTTP/ADP command ingress returning owner-backed rewind receipt or explicit target-not-found failure |

- lifecycle path under test:
  - runtime receives writable tool execution intent
  - runtime requests preview truth from `tool.preview`
  - master runtime binds checkpoint workspace truth to the canonical runtime home,
    independent of process cwd or inherited workspace environment
  - runtime snapshots previewed pre-image paths before tool execute
  - runtime records checkpoint lifecycle rows
  - runtime exposes read-only checkpoint summaries from runtime-owned manifest and ledger truth
  - explicit rewind restores workspace state through runtime owner path
  - runtime refreshes UI checkpoint projections after create / rewind without making UI a recovery truth source
  - checkpoint truth remains separate from reason persistence truth
- white-box plan:
  - checkpoint manifest round-trip tests
  - checkpoint create / apply / restore ledger tests
  - preview-derived path-set snapshot tests
  - runtime-home workspace root canonicalization tests
  - process cwd / workspace environment cannot redirect master checkpoint rewind
  - restore create / modify / delete state tests
  - no-preview writable-tool rejection tests
  - missing manifest / blob / ledger corruption rejection tests
  - runtime bootstrap failure when checkpoint projection reads corrupt ledger truth
  - checkpoint list/query summary tests from manifest plus ledger truth
- module black-box plan:
  - runtime writable tool loop creates checkpoint before execute
  - runtime explicit rewind restores prior workspace state
  - daemon live submit defaults the master session cwd to runtime home, so
    checkpointed writes and rewind use one root even when the daemon process was
    launched from another directory
  - runtime restart can inspect checkpoint ledger and manifests without treating them as reason truth
  - runtime dispatcher materializes checkpoint summaries into protocol state for query consumers
- project black-box impact:
  - CLI or daemon live writable-tool path can mutate files and later rewind through runtime owner path
  - daemon HTTP query can show checkpoint summaries without app-owned filesystem parsing
- fixtures / replay inputs / runtime evidence paths:
  - checkpoint manifest fixture path
  - checkpoint ledger fixture path
  - `~/.freehand/state/checkpoints`
  - `~/.freehand/ledgers/checkpoints`
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/reason`
- known gaps:
  - checkpoint subscribe/SSE is intentionally out of scope for this slice; WebUI uses query refresh after command receipt
- sync status between design and implementation:
  - design is locked
  - runtime checkpoint store, live writable pre-execute checkpointing, and explicit rewind owner API are now code-bound
  - runtime checkpoint workspace root is the canonical runtime home and is
    regression-locked against process-cwd/environment drift
  - runtime tests now cover create-file rewind, modify-file rewind, and previewless writable-tool rejection
  - runtime tests now also cover missing manifest rewind, missing blob rewind, corrupt checkpoint-ledger query failure, and corrupt checkpoint-ledger bootstrap failure
  - checkpoint summary query/projection is runtime-owned and code-bound
  - migrated mainline-call source and generated wiki must stay in sync with this test design
