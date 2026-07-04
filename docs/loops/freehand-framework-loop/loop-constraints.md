# Freehand Framework Loop Constraints

## Allowed In L1

- Read repo files.
- Read runtime logs and evidence paths when needed.
- Run mapped check commands.
- Append `loop-run-log.md`.
- Update `STATE.md` with report-only status.
- Report findings with owner mapping.

## Denylist

- No product code edits.
- No provider/config/auth/secret edits.
- No release/global install.
- No production service restart.
- No broad process kill.
- No destructive git operations.
- No cleanup of unrelated untracked artifacts.
- No fallback, disabled tests, weakened assertions, or silent success.
- No auto-merge.

## Escalation Rules

Escalate instead of acting when:

- owner cannot be mapped uniquely
- required tests are unknown
- fix would cross more than one owner
- change touches denied paths
- same item reaches three attempts
- checker cannot independently verify
- kill switch is active

## L2 Guard

L2 is disabled until Jason explicitly approves it after stable L1 history.

