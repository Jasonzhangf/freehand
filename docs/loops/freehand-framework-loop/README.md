# Freehand Framework Loop

This directory owns the initial Freehand loop-governance surface.

The loop starts in `L1 report-only` mode. It may inspect repo state, maps, gates,
and logs, then write report/state updates. It must not edit product code, config,
runtime truth, secrets, infrastructure, release artifacts, or production services.

Canonical files:

- `LOOP.md`: purpose, cadence, owner, gates, and kill switch.
- `STATE.md`: current baseline, watchlist, and last run summary.
- `loop-constraints.md`: allowed actions, denylist, and escalation rules.
- `loop-budget.md`: per-run token, time, command, and action caps.
- `loop-run-log.md`: append-only JSONL run history.

The loop is bound to `foundation.workspace` until a narrower loop owner is added.

