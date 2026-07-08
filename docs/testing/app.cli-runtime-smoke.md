# Test Design: `app.cli-runtime-smoke`

- feature_id: `app.cli-runtime-smoke`
- owner: `apps/freehand-cli`
- lifecycle path under test:
  - CLI loads default config
  - CLI selects one named agent
  - CLI resolves selected provider truth and prints safe provider metadata
  - CLI runs scripted reason E2E smoke from app boundary
  - provider semantic outputs reach reason turn truth and rewrite runtime through the shared harness
  - CLI prints explicit rewrite outcome or explicit blocked recovery outcome
  - CLI runs no-UI ADP smoke against a daemon `/adp` WebSocket URL
  - CLI ADP smoke verifies subscribe accepted, subscription event, query result, and query-as-command explicit failure
  - CLI runs no-UI ADP success/failure/schema-mismatch/provider-retry turn samples against a daemon `/adp` WebSocket URL
  - CLI ADP turn samples use an isolated sample session, verify command outcome plus matching terminal projection, then query the sample session transcript; the failure sample must show `Success` terminal status plus transcript evidence for at least two rounds and at least one unique failed tool activity; the schema-mismatch sample must show at least one schema-polishing retry; the provider-retry sample must show provider retry/error evidence instead of being treated as schema or tool failure
  - provider retry online verifier temporarily redirects S-profile provider config to a local 500 fixture, requires five real upstream attempts, queries provider-domain error-center rows, and restores config/env before exit
  - CLI runs no-UI same-session continuation sample by submitting two turns into one isolated session and verifying the second terminal answer uses a unique token from the first turn's effective history
  - CLI runs no-UI task lifecycle sample by sending protocol-owned task create/review/approve/close commands and verifying task list/history truth reaches `Closed`
  - CLI runs no-UI Phase 1 foundation sample by driving TaskBoard, AgentBoard, ExecutionFact, SchedulerTick, and restart same-id verification through ADP
  - CLI runs no-UI Phase 2A master-worker foundation sample by driving worker creation, task assignment, claim-next with execution id, progress/blocked/recovering/review facts, reject, retry, approve, close, and restart same-id verification through ADP
  - CLI runs no-UI Phase 2B master poll foundation sample by driving EventInbox
    query and MasterPoll command through ADP, verifying classifications,
    cursor persistence, no task status mutation, and restart same-cursor
    verification
  - CLI Phase 2B create path uses `replay_from_start=true` plus omitted
    EventInbox/MasterPoll limit to ignore stale persisted cursors and drain all
    pending rows before persisting the cursor; verify mode then proves no new
    events after that cursor
  - CLI Phase 2B create path rereads the final owner-backed persisted cursor
    after the command poll and uses that cursor for same-cursor verification
  - CLI ADP session manage sends create/rename/archive/restore/delete-as-archive/rollback command frames and reports command receipts without owning session truth
  - CLI ADP task query sends task list/history query frames and reports task projection summaries without WebUI
  - CLI ADP task subscribe sends task list subscribe frames and reports the first task projection event without WebUI
  - CLI ADP error query sends error-center query frames and reports metadata projection summaries without WebUI
- white-box plan:
  - none in app crate beyond argument dispatch helpers
- module black-box plan:
  - CLI startup config smoke
  - CLI reason compaction smoke
  - CLI recovery block smoke
  - CLI ADP mock WebSocket smoke
  - CLI ADP success turn sample mock WebSocket smoke
  - CLI ADP failure turn sample mock WebSocket smoke with isolated-session transcript evidence and unique tool-call counting
  - CLI ADP schema-mismatch turn sample mock WebSocket smoke with schema retry evidence
  - CLI ADP provider-retry turn sample mock WebSocket smoke with provider retry evidence and explicit terminal provider failure
  - provider retry online script smoke with real S-profile daemon, local fixture provider, ADP sample output, ADP error-center query, session truth check, and config restoration
  - CLI ADP same-session continuation sample mock WebSocket smoke with two turns in one session and second-turn token evidence
  - CLI ADP task lifecycle sample mock WebSocket smoke with closed task list projection and create/review/approve/close history evidence, and with no `SubmitUserInput` prompt dependency
  - CLI ADP Phase 1 foundation sample mock WebSocket smoke with blocked/review/stale board evidence, agent lifecycle query evidence, recovering history event evidence, and explicit verify mode
  - CLI ADP Phase 2A master-worker foundation sample mock WebSocket smoke with worker claim, execution id, ordered history, lifecycle evidence, and explicit verify mode
  - CLI ADP Phase 2B master poll foundation sample mock WebSocket smoke with
    EventInbox rows, classification kinds, persisted cursor evidence, no task
    status mutation, explicit verify mode, replay-from-start create mode, and
    no use of finite page limits as same-cursor closeout evidence
  - CLI ADP session manage argument/result summary smoke
  - CLI ADP task query argument/result summary smoke
  - CLI ADP task subscribe argument/result summary smoke
  - CLI ADP error query argument/result summary smoke
  - CLI ADP local `freehand-server webui-serve-smoke` smoke
- project black-box impact:
  - one app entrypoint can now drive config + provider selection plus reason runtime E2E smoke
  - provider usage and recovery policy remain wired through the shared harness path
  - no-UI ADP smoke can diagnose status/control failures without WebUI or Android
  - no-UI ADP turn samples can populate and verify WebUI-visible success, failed-tool-result recovery, schema-polishing, and provider-retry projections without relying on manual DOM inspection; the failure sample rejects one-round or system-failure outcomes, the schema sample rejects no-retry success, and the provider sample rejects schema/tool failures mislabeled as provider retry
  - no-UI provider retry fixture proof validates provider retry/backoff truth without accepting model-generated retry prose
  - no-UI same-session continuation sample verifies session history inclusion without WebUI DOM inspection
  - no-UI task lifecycle sample verifies task owner truth through ADP task list/history without WebUI DOM inspection; CLI only sends protocol-owned task mutation commands and does not write task storage directly
  - no-UI Phase 1 foundation sample verifies TaskBoard, AgentBoard, ExecutionFact, SchedulerTick, and restart same-id proof without model prose or UI DOM inspection
  - no-UI Phase 2A master-worker foundation sample verifies assign/claim/progress/blocked/recovering/review/reject/retry/approve/close and restart same-id proof without model prose or UI DOM inspection
  - no-UI Phase 2B master poll foundation sample verifies EventInbox, MasterPoll
    classifications, persisted cursor, and restart same-cursor proof without
    model prose or UI DOM inspection; create mode must use replay-from-start,
    and finite EventInbox/MasterPoll limits are only pagination and cannot prove
    cursor closeout
  - no-UI ADP session manage can verify daemon session CRUD and rollback receipt paths without WebUI DOM inspection
  - no-UI ADP task query can verify daemon task list/history visibility without WebUI DOM inspection
  - no-UI ADP task subscribe can verify daemon task list subscription visibility without WebUI DOM inspection
  - no-UI ADP error query can verify daemon error-center metadata visibility without WebUI DOM inspection
  - machine-readable mainline truth remains the only source for generated wiki artifacts
- fixtures / replay inputs / runtime evidence paths:
  - temp `HOME` with `~/.freehand/config.toml`
  - scripted provider semantic outputs in CLI tests
  - local ADP WebSocket mock in CLI tests
  - local ADP success/failure/schema-mismatch/provider-retry sample WebSocket mock in CLI tests
  - S-profile provider retry fixture verifier `scripts/verify-provider-retry-online.sh`
  - local same-session continuation WebSocket mock in CLI tests
  - local task lifecycle WebSocket mock in CLI tests
  - local Phase 1 foundation WebSocket mock in CLI tests
  - local Phase 2B master poll WebSocket mock in CLI tests
  - local `freehand-server webui-serve-smoke` for manual/agent positive ADP smoke
  - `~/.freehand/state/config`
  - `~/.freehand/state/turns`
- known gaps:
  - production non-smoke CLI/server runtime loop is still pending
- sync status between design and implementation:
  - CLI smoke baseline is implemented in integration tests
  - CLI ADP smoke baseline is implemented in integration tests and verified against a real local `/adp` server
  - CLI ADP success/failure/schema-mismatch/provider-retry sample baseline is implemented in integration tests; failure means recovered failed tool result with `rounds>=2` transcript evidence, schema-mismatch means visible schema-polishing retry evidence, and provider-retry means provider-domain retry/error evidence instead of schema/tool failure
  - provider retry online verifier is implemented and requires five local fixture provider attempts plus provider-domain error-center rows
  - CLI ADP same-session continuation sample is implemented in integration tests and verifies token recovery from prior effective history
  - CLI ADP task lifecycle sample is implemented in integration tests and verifies closed task/history evidence after protocol-owned task mutation commands
  - CLI ADP Phase 1 foundation sample is implemented in integration tests and verifies TaskBoard/AgentBoard/ExecutionFact/SchedulerTick evidence through protocol-owned ADP frames
  - CLI ADP Phase 2A master-worker foundation sample is implemented in integration tests and live-validated on S-profile `127.0.0.1:4042` with restart same-id verification
  - CLI ADP Phase 2B master poll foundation sample is the active next sample
    target and must verify EventInbox, classifications, persisted cursor, and
    restart same-cursor evidence
  - CLI ADP session manage command is implemented for live daemon session CRUD and rollback checks
  - CLI ADP task query command is implemented for live daemon task list/history checks
  - CLI ADP task subscribe command is implemented for live daemon task list subscription checks
  - CLI ADP error query command is implemented for live daemon error-center metadata checks
  - migrated mainline-call source and generated wiki are kept in sync with this test design
