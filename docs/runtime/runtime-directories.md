# Runtime Directories

## Standard Paths

- `~/.freehand/state`
- `~/.freehand/state/checkpoints`
- `~/.freehand/state/config`
- `~/.freehand/state/turns`
- `~/.freehand/state/ui`
- `~/.freehand/logs`
- `~/.freehand/ledgers`
- `~/.freehand/ledgers/checkpoints`
- `~/.freehand/ledgers/metadata`
- `~/.freehand/ledgers/reason`
- `~/.freehand/ledgers/providers`
- `~/.freehand/replays`
- `~/.freehand/replays/metadata`
- `~/.freehand/cache`
- `~/.freehand/cache/session-index`
- `~/.freehand/tmp`

## Meaning

- `state`: local durable runtime state
- `state/checkpoints`: runtime-owned writable-tool checkpoint snapshots and restore manifests
- `state/config`: per-agent startup configs and resolved runtime config state
- `state/turns`: authoritative session-history snapshots, active-turn snapshots, and closed-turn truth
- `state/ui`: rebuildable UI/session sidecars
- `logs`: subsystem logs
- `ledgers`: append-only event, debug, and audit records
- `ledgers/checkpoints`: append-only checkpoint create/restore/discard audit records
- `ledgers/metadata`: future append-only internal metadata provenance records
- `ledgers/reason`: append-only semantic turn and rewrite records used for replay/recovery
- `ledgers/providers`: provider raw/debug ledgers, retained only when that debug evidence is enabled
- `replays`: captured runtime exchanges for replay/debug
- `replays/metadata`: future replay fixtures for metadata admission and provenance checks
- `cache`: runtime caches
- `cache/session-index`: rebuildable session lists and lookup caches
- `tmp`: bounded temporary work area
