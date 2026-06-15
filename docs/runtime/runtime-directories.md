# Runtime Directories

## Standard Paths

- `~/.freehand/state`
- `~/.freehand/state/config`
- `~/.freehand/logs`
- `~/.freehand/ledgers`
- `~/.freehand/replays`
- `~/.freehand/cache`
- `~/.freehand/tmp`

## Meaning

- `state`: local durable runtime state
- `state/config`: per-agent startup configs and resolved runtime config state
- `logs`: subsystem logs
- `ledgers`: append-only event, debug, and audit records
- `replays`: captured runtime exchanges for replay/debug
- `cache`: runtime caches
- `tmp`: bounded temporary work area
