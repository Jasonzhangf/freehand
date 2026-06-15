# Reason Turn Design

## Status

Confirmed discussion only. Unconfirmed details remain `TBD`.

## Confirmed

### Turn truth granularity

- turn truth is stored per turn
- conversation view is projected from per-turn records

### Single writer rule

- only `freehand-reason` may write session truth

### Event broadcast granularity

First version semantic broadcast includes:

- reasoning
- text
- tool
- usage
- terminal
- error

### Tool result re-entry

- `freehand-reason` writes tool result re-entry back into turn truth

### Multi-subscriber policy

- slow subscribers may drop frames
- slow subscribers must not back-pressure the main reasoning path
- debug and replay truth use ledger/replay paths instead of subscriber delivery guarantees

### Provider raw event placement

- provider raw events do not enter session truth
- provider raw events go to debug ledger paths

### Terminal classes

First version terminal classes are explicitly distinct:

- success
- tool_pending / needs_tool_result
- blocked
- interrupted
- failed
- cancelled

### Stop decision rule

- provider `finish_reason=stop` or `finish_reason=end_turn` does not by itself stop Freehand turn execution
- stop is controlled by Freehand completion schema validation

### Completion schema rule

The system prompt requires a completion schema.

Confirmed terminal evaluation rules:

1. completed
   - if the model claims task completion, it must provide:
     - completion reason
     - evidence
     - summary
     - learned
   - if completion is claimed and evidence field has content, Freehand treats it as completed
   - Freehand extracts summary and evidence and composes terminal text output
   - if completion is claimed but required completion schema is missing or invalid, Freehand rejects it and asks again

2. not completed with next step
   - if not completed and `next_step` is present, Freehand requires execution of the next step instead of stopping

3. blocked
   - if not completed and blocked, `blocked_reason` must be present
   - if blocked data is valid, Freehand may stop in blocked state

4. invalid or missing schema
   - if schema is missing, invalid, or conditions are not satisfied, Freehand rejects it
   - Freehand must explain what is wrong and how the schema should be provided

## Open Questions / TBD

- exact per-turn storage schema
- exact format for terminal text composition from summary and evidence
- exact retry/reprompt limit for invalid completion schema
- exact mapping between terminal classes and persisted turn outcome records

## Update trigger

Update this doc when:

- turn truth ownership changes
- event broadcast granularity changes
- subscriber policy changes
- stop/completion schema rules change
- terminal class policy changes
