# OpenMinis Config UI Closeout Constraints

## Allowed In L1

- Read OpenMinis public website/repo metadata.
- Read Freehand source, docs, memory, function maps, and tests.
- Write loop documentation and a goal plan.
- Append report-only entries to `note.md` and this loop's run log.

## Denylist

- No product code edits in L1.
- No direct `~/.freehand/config.toml` mutation.
- No provider API key, auth, secret, or `.rcc` config edits.
- No launchd install/restart unless a later approved L2 verification explicitly requires it.
- No release/global install in L1.
- No Android install/device mutation in L1.
- No broad process kill.
- No fallback endpoint logic.
- No UI-only fake config state that does not come from an owner-backed projection.

## L2 Guard

Each L2 batch must:

- touch one primary owner path
- update the owner function map and test design before implementation
- include positive and negative tests for config/error paths
- prove WebUI online behavior with browser evidence before claiming success
- use release/Android true-device proof only when the user-visible phone surface is claimed updated

## Escalation Rules

Escalate instead of acting when:

- config write semantics require restart policy decisions not in docs
- API key storage/display requirements are ambiguous
- provider routing/model-group semantics require new runtime behavior
- implementation would cross more than one owner without a split plan
- checker cannot verify the result online
