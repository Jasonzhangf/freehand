# Reason Persistence Design

## Status

Locked design truth for first baseline.

This document defines:

- authoritative persistence ownership
- snapshot, ledger, and sidecar boundaries
- runtime directory layout for reason persistence
- restart and crash recovery rules

## Owner

- authoritative persistence owner crate: `crates/freehand-reason`
- project black-box harness owner crates may include `crates/freehand-testkit` and `apps/freehand-cli`, but they do not own persistence truth
- provider adapters may write provider-family raw debug ledgers only; they do not own session or turn persistence

## Reference Evidence

Codex evidence:

- `~/code/codex/codex-rs/core/src/context_manager/history.rs`
  - normalized model-visible history is separated from broader mutable session state
  - rewrite-sensitive history carries an explicit version counter
- `~/code/codex/codex-rs/core/src/state/session.rs`
  - session-scoped runtime state sits beside history, not inside provider-visible prompt truth
- `~/code/codex/codex-rs/core/src/session/rollout_reconstruction.rs`
  - effective history and resume metadata are rebuilt from replay/rollout evidence

Reasonix evidence:

- `../Deepseek-reasonix/internal/agent/session.go`
  - persisted session core stays intentionally small and single-writer
- `../Deepseek-reasonix/internal/agent/cache_shape.go`
  - cache diagnostics explicitly record system hash, tool hash, prefix hash, and rewrite version
- `../Deepseek-reasonix/desktop/sessions.go`
  - transcript durability is separate from titles, display metadata, and trash/restore sidecars

## Design Goals

- keep `freehand-reason` as the only writer of authoritative session and turn truth
- recover after restart or crash without trusting UI projections
- preserve replay-first debugging and rewrite auditability
- keep provider raw payloads and metadata outside session truth
- support multi-UI access through derived projections instead of duplicated persistence owners

## Persistence Split

Freehand reason persistence is fixed to three layers.

- authoritative snapshots
- append-only ledgers
- derived UI and index sidecars

### 1. Authoritative snapshots

Location:

- `~/.freehand/state/turns/<agent_id>/<session_id>/`

Purpose:

- durable session and turn truth
- authoritative restore source before ledger replay

Planned v1 files:

- `session-history.json`
  - session-owned rewrite truth
  - `session_id`
  - `rewrite_version`
  - `current_rewrite_mode`
  - `base_context_segments`
  - `rewrite_ledger`
- `session-cursor.json`
  - snapshot schema version
  - `last_applied_reason_seq`
  - latest known turn id
  - active turn id if any
- `active-turn.json`
  - current mutable turn truth while a turn is still running
  - includes completion-schema rejection counters for that turn
- `turns/<turn_id>.json`
  - immutable closed-turn truth after terminalization

Rules:

- only `freehand-reason` may write these files
- writes must be atomic replace operations, not in-place partial mutation
- provider raw payloads do not belong here
- UI projection state does not belong here

### 2. Append-only ledgers

Location:

- `~/.freehand/ledgers/reason/<agent_id>/<session_id>.jsonl`
- `~/.freehand/ledgers/providers/<provider_family>/<agent_id>/<session_id>/<turn_id>.jsonl`

Purpose:

- replay
- recovery tail application
- audit
- debug scene evidence

Reason ledger contains:

- monotonic `seq`
- `session_id`
- `turn_id`
- semantic event kind
- turn lifecycle events
- rewrite staged and rewrite consumed evidence
- completion-schema rejection evidence
- terminal evidence
- metadata-side cache diagnostics

Provider raw ledger contains:

- raw wire payload or SSE chunk evidence
- adapter-side parsing scene position
- optional request/response headers needed for debug

Rules:

- reason ledger is append-only and recovery-relevant
- provider raw payloads are debug-only artifacts
- provider raw ledger is debug-only and must not be promoted into session truth
- normal runtime behavior must not depend on provider raw ledger presence

### 3. Derived sidecars and indexes

Location:

- `~/.freehand/state/ui/`
- `~/.freehand/cache/session-index/`

Purpose:

- session list ordering
- WebUI card summaries
- CLI or WebUI display metadata
- rebuildable query acceleration

Rules:

- these files are derived from authoritative snapshots plus ledgers
- they may be deleted and rebuilt
- recovery must not depend on them

## Write Lifecycle

### Ordinary turn start

1. restore current snapshot state for the session
2. allocate the new turn truth in memory
3. append a reason-ledger row for turn start
4. atomically refresh `active-turn.json`
5. update `session-cursor.json` with the durable reason-ledger sequence

### Provider semantic event apply

1. `freehand-reason` applies semantic output into in-memory turn truth
2. append corresponding reason-ledger rows
3. atomically refresh `active-turn.json`
4. if debug raw retention is enabled, append provider raw ledger rows independently

### Rewrite gate stage

1. `reason.rewrite-policy` decides the action
2. `freehand-reason` mutates `SessionHistory` through explicit gate methods only
3. append rewrite evidence to the reason ledger
4. atomically refresh `session-history.json`
5. update `session-cursor.json`

### Terminal turn close

1. append terminal evidence to the reason ledger
2. atomically write `turns/<turn_id>.json`
3. atomically remove or clear `active-turn.json`
4. atomically refresh `session-cursor.json`
5. rebuild or update UI/index sidecars from authoritative truth

## Recovery Lifecycle

### Preferred path

1. load `session-history.json`
2. load `session-cursor.json`
3. load `active-turn.json` if present
4. replay reason-ledger rows with `seq > last_applied_reason_seq`
5. continue from the rebuilt in-memory truth

### Snapshot-missing or snapshot-invalid path

1. if authoritative snapshots are missing or invalid, try ledger-only rebuild from reason ledger
2. rebuild session rewrite truth and turn truth from ordered reason-ledger evidence
3. if ledger-only rebuild succeeds, write fresh authoritative snapshots before continuing

### Explicit block path

Block instead of guessing when:

- snapshot coherence is invalid and no complete reason ledger exists
- reason-ledger sequence has a gap or duplicate that prevents deterministic replay
- active turn truth and latest terminal turn truth conflict after replay

Rules:

- UI sidecars are never recovery truth
- provider raw ledgers are never recovery truth
- recovery must fail explicitly when authoritative truth cannot be reconstructed

## Crash Window Rules

- reason-ledger append happens before snapshot cursor advancement
- snapshot files are written by temp file plus rename
- if a crash happens after ledger append but before snapshot refresh, recovery uses ledger-tail replay
- if a crash happens before ledger append, previous authoritative snapshot remains the last committed truth
- no fallback path may invent missing turn truth from UI projection or provider raw payloads

## Metadata And Request Isolation

- persisted request-chain content remains separate from persisted metadata and diagnostics
- cache diagnostics, scene position, provider family, and raw transport evidence remain metadata-side fields
- request content must not be reconstructed from provider raw ledgers
- metadata-side fields must not be re-injected into request text without an explicit request builder

## Non-Goals For V1

- database storage
- distributed cross-host persistence consensus
- deduplicated provider raw blob storage
- user-edited title or display truth inside `freehand-reason`
- replay recovery from UI sidecars alone

## Implementation Binding Status

- current code baseline already has session-history JSON/file round-trip helpers in `crates/freehand-reason/src/session_history.rs`
- current code baseline does not yet have:
  - runtime snapshot coordinator
  - reason-ledger append writer
  - terminal turn file materializer
  - ledger-only rebuild path
  - CLI restart/resume recovery smoke

## Update Trigger

Update this doc when:

- snapshot file shapes change
- reason-ledger schema changes
- recovery ordering changes
- provider raw retention policy changes
- derived sidecar boundaries change
- runtime directory paths change
