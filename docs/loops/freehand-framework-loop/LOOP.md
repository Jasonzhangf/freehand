# Freehand Framework Loop

- loop_id: `freehand-framework-loop`
- owner feature: `foundation.workspace`
- initial mode: `L1 report-only`
- cadence: manual trigger only
- implementer: current agent run
- checker: a separate review pass or Jason before any future L2 action
- kill switch: `docs/loops/freehand-framework-loop/KILL_SWITCH`

## Purpose

Keep Freehand framework governance inspectable across long work sessions.

The loop checks whether framework work remains bound to:

- feature map ownership
- function maps
- mainline call maps
- generated wiki artifacts
- test design docs
- required gates
- local memory and run evidence

## Modes

### L1 Report-Only

Allowed:

- read repo state
- read loop files
- read `CACHE.md`, `MEMORY.md`, and `note.md`
- inspect `git status --short`
- run mapped read/check commands
- append one run-log entry
- update `STATE.md` with report-only findings

Forbidden:

- product code edits
- config/auth/secret/infra edits
- runtime truth edits
- service restarts except explicit validation requested by Jason
- release/global install
- auto-merge

### L2 Assisted

Not enabled.

Future enablement requires:

- at least three useful L1 runs
- stable signal quality
- exact `feature_id`
- exact owner paths
- exact required tests
- checker separate from implementer
- Jason approval

### L3 Unattended

Not enabled.

## Start-Of-Run Checklist

1. Confirm kill switch is inactive.
2. Read `LOOP.md`, `STATE.md`, `loop-constraints.md`, `loop-budget.md`, and recent `loop-run-log.md`.
3. Read project entry truth: `AGENTS.md`, `CACHE.md`, `MEMORY.md`, `note.md`.
4. Confirm current run is within budget.
5. Inspect `git status --short`.
6. Run report-only checks listed in `STATE.md` when within budget.
7. If no actionable item exists, log `no-op`.
8. If a finding exists, map it to one `feature_id`, owner, function map, mainline call map, and test design.
9. Do not edit code in L1. Escalate instead.

## Required Report Fields

Each run report must include:

- run_id
- mode
- budget used
- checked sources
- items found
- owner mapping
- tests/checks run
- evidence summary
- outcome
- next allowed step

