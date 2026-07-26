# Test Design: `reason.persistence`

- feature_id: `reason.persistence`
- owner: `crates/freehand-reason`
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `session.restore`
  - `session.append_turn_to_turn`
  - `session.list_persisted`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `session.restore` | bound | `cargo test -p freehand-reason ui_restore_ -- --nocapture` covers inactive partial restore-with-warning and active incomplete hard-error behavior | `cargo test -p freehand-reason -- --nocapture` covers snapshot-plus-tail, ledger-only rebuild, exact-round selected UI restore, partial warning, and active incomplete rejection in one owner package | `node scripts/verify-webui-session-restore-error-exit-online.mjs` proves production ADP `QuerySessionTurns` returns the legacy partial transcript with a visible integrity warning instead of a dispatch-port failure |
| `session.append_turn_to_turn` | bound | `cargo test -p freehand-reason -- --nocapture` covers session snapshot, reason-ledger append, sequence, rollback, and recovery tests | `cargo test -p freehand-reason -- --nocapture` covers persistence save/reload, active-turn update, terminal materialization, sidecar rebuild, and metadata reload smokes | `cargo test -p freehand-runtime session_continue -- --nocapture` covers CLI/shared runtime persistence restore smokes and replay/debug inspection without provider raw truth |
| `session.list_persisted` | bound | `cargo test -p freehand-runtime runtime_query_session_search_returns_worker_hits_under_parent_session -- --nocapture --test-threads=1` covers persisted index/metadata rows consumed through runtime search projection | `cargo test -p freehand-ui-protocol session_search -- --nocapture --test-threads=1` covers route separation and DTO validation while persistence remains owner truth | `node scripts/verify-webui-session-search-online.mjs` covers WebUI Search querying persisted session index truth and keeping worker matches out of the top-level result list |

- lifecycle path under test:
  - authoritative session rewrite truth is snapshotted under `~/.freehand/state/turns`
  - active turn truth is refreshed after durable reason-ledger append
  - terminal turn truth is materialized as immutable per-turn files
  - turn-created time is persisted in `TurnRecord.created_at` and projected
    from authoritative active/closed turn truth, not synthesized by UI clients
  - restart recovery restores from snapshot plus reason-ledger tail, or from reason-ledger-only rebuild when snapshots are missing or invalid
  - selected-session UI restore reads complete authoritative closed/active snapshots plus rollback-marker sidecar truth when available, preserves exact `runtime-turn-N` / `runtime-turn-N-rM` rounds as separate UI snapshots, backfills from the reason ledger when authoritative snapshot truth is missing earlier observed rounds, displays inactive surviving authoritative snapshots only with an explicit partial-transcript integrity warning when the reason ledger is empty or retained at an unusable sequence offset, and keeps active incomplete snapshots as hard errors
  - daemon bootstrap and Master parent-workset reconciliation use the authoritative-only UI restore path and must not replay every historical reason ledger; incomplete old snapshots are expanded only when that session is explicitly queried
  - authoritative closed-turn restore reads only `*.json` turn snapshots; leftover `*.tmp-*` atomic write artifacts in the turns directory must not be parsed as turn truth or prevent daemon bootstrap
  - non-UI turn-start restore reads authoritative reason-ledger
    `TurnStarted` rows, honors rollback markers, and preserves first-round user
    intent even when effective UI snapshots only retain a repaired round
  - derived UI and index sidecars rebuild from authoritative truth and are never recovery truth
  - persisted session list/search reads consume derived index and metadata sidecars only; worker/subagent transcript sessions remain child/debug truth unless task parent truth attaches them to a persisted Master session
  - reason-owned session display metadata stores `title` and `archived` state for shared UI CRUD without entering provider-visible session history
  - append-only rollback markers filter effective transcript restore while preserving raw closed-turn files for audit and reserved turn-id allocation
  - authoritative restore and rollback persistence filter model-visible
    `historical_turn:*` session-memory segments against the same effective
    active/closed turn set used by transcript truth; stable memory without a
    historical-turn reference remains intact
  - provider raw ledgers remain debug-only and never become session truth
- white-box plan:
  - session snapshot render/load tests
  - invalid persisted snapshot JSON rejection tests
  - leftover atomic temp file rejection-from-truth test proving authoritative restore ignores non-`.json` turn-directory files
  - invalid snapshot coherence rejection tests
  - persistence cursor serialization tests
  - reason-ledger sequence monotonicity tests
  - same-session concurrent writer monotonic sequence allocation test
  - reason-ledger sequence gap rejection tests
  - duplicate reason-ledger sequence rejection tests
  - stale duplicate reason-ledger row skip test gated by presence of a later authoritative expected-sequence row
  - snapshot-plus-tail replay tests
  - ledger-only rebuild tests
  - authoritative-snapshot-backed exact-round UI restore tests through runtime bootstrap
  - exact round preservation tests for `runtime-turn-N` / `runtime-turn-N-rM` style turn ids
  - UI restore test that poisons the reason ledger after complete authoritative snapshots exist, proving transcript query does not depend on replaying the ledger
  - UI restore test that leaves authoritative closed-turn truth with only the terminal continuation round, proving earlier provider/tool rounds are backfilled from reason-ledger snapshots
  - UI restore test that removes the reason ledger for an inactive legacy partial transcript, proving surviving authoritative snapshots are returned with `reason_persistence_partial_ui_restore`
  - UI restore negative test that removes the reason ledger for an active incomplete transcript, proving active work remains a hard cursor-coherence error
  - runtime bootstrap test that removes the first authoritative round and poisons the historical reason ledger, proving global startup consumes the remaining authoritative snapshot without parsing the ledger
  - runtime/bootstrap-adjacent restore test that writes a zero-byte atomic temp file next to a closed turn and proves both `restore` and `restore_authoritative_turn_snapshots_for_ui` return only the closed `*.json` turn
  - turn-start restore test for an original `runtime-turn-1` start plus a closed
    repaired `runtime-turn-1-r2` snapshot, proving first-round request truth
    remains available and rollback does not resurrect it
  - atomic snapshot replace tests
  - created-time preservation assertions in active-turn and terminal-turn
    materialization smokes
  - provider-raw debug-ledger write tests
  - provider-raw-ledger exclusion tests
  - provider-raw-only recovery rejection tests
  - UI-sidecar-only recovery rejection tests
  - session metadata create/rename/archive/restore smoke tests
  - unknown-session metadata mutation rejection tests
  - non-destructive delete-as-archive session metadata test
  - append-only latest-turn rollback marker test
  - rollback marker later-write sidecar non-resurrection test
  - effective transcript filtering test after rollback
  - effective session-history filtering test proving valid historical-turn
    memory and ordinary stable memory remain while rolled-back and orphan
    historical-turn memory is removed
  - raw closed-turn file retention test after rollback
  - multi-round logical turn rollback test
- module black-box plan:
  - persistence save/reload smoke at the `freehand-reason` boundary
  - active-turn update then terminal materialization smoke
  - focused `persistence_save_reload_smoke` and
    `terminal_turn_materialization_smoke` prove restored turns carry
    non-zero created-time truth
  - snapshot-missing recovery smoke
  - inactive partial authoritative UI restore with warning smoke
  - active incomplete authoritative UI restore hard-error smoke
  - derived-sidecar rebuild smoke
  - turn-start snapshot restore smoke through parent-goal evaluation
  - session metadata sidecar reload smoke
  - rollback marker sidecar reload smoke through `restore` and `restore_turn_snapshots_for_ui`
  - provider-raw debug-ledger append smoke
- project black-box impact:
  - CLI persistence restore smoke
  - shared runtime harness persistence smoke
  - replay/debug consumer can inspect persisted reason history without using provider raw payloads as truth
- fixtures / replay inputs / runtime evidence paths:
  - persisted session snapshot fixture path
  - reason-ledger fixture path
  - corrupted persistence fixture path
  - `~/.freehand/state/turns`
  - `~/.freehand/state/ui`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/ledgers/providers`
  - `~/.freehand/cache/session-index`
- sync status between design and implementation:
  - design is locked
- session snapshot, active-turn snapshot, reason-ledger append, provider-raw debug-ledger append, terminal turn materialization, created-time preservation, sidecar rebuild, snapshot-plus-tail recovery, and ledger-only rebuild are implemented in `freehand-reason`
  - exact-round selected-session UI restore with incomplete-authoritative ledger backfill, inactive partial warning projection for empty or retained-offset ledgers, and active incomplete hard-error behavior is implemented through `ReasonPersistence::restore_turn_snapshots_for_ui`
  - global daemon bootstrap and Master parent-workset reconciliation use authoritative snapshot restore plus reason-owned raw/reserved turn-id helpers so historical retained-offset ledgers do not determine startup or lifecycle-poll latency and post-rollback follow-up ids do not collide; `restore_uses_authoritative_state_when_retained_ledger_starts_after_one` and `turn_start_restore_uses_authoritative_snapshots_when_retained_ledger_starts_after_one` lock the retained-offset recovery path
  - turn-start snapshot restore is implemented through
    `ReasonPersistence::restore_turn_start_snapshots`
  - shared harness and CLI smoke are implemented
  - live Anthropic `reason-live` path now persists start/output/rejection/terminal events plus provider raw debug bodies/events through `ReasonPersistence`
  - runtime white-box coverage now explicitly locks ledger sequence-gap rejection plus provider-raw-only and UI-sidecar-only missing-recovery rejection
  - runtime white-box coverage now explicitly locks invalid persisted snapshot JSON, invalid snapshot coherence, and duplicate-sequence recovery rejection
  - runtime white-box coverage now explicitly locks leftover atomic temp files out of authoritative closed-turn restore
  - session metadata CRUD is implemented with positive create/rename/archive/restore/delete-as-archive coverage and negative unknown-session rejection coverage
  - append-only latest-session-turn rollback is implemented with positive marker/effective-filter/raw-file-retention coverage
  - effective session-history rollback filtering is covered positively for
    retained effective/stable memory and negatively for rolled-back/orphan
    historical-turn memory
  - runtime parent evaluation has a regression proving original user-objective
    recovery when the authoritative closed snapshot contains only a repaired
    round
  - runtime parent reconciliation has a regression proving authoritative
    parent/evaluation snapshots are enough for background idempotency replay
    even when the session reason ledger is poisoned
  - migrated mainline-call source and generated wiki are kept in sync with this test design
