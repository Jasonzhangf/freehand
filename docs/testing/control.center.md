# Test Design: `control.center`

- feature_id: `control.center`
- owner: `crates/freehand-control`
- lifecycle path under test:
  - model emits hidden status schema block
  - control parser validates shape and state-specific required fields
  - runtime records accepted/rejected hook metadata with watermark provenance
  - rhythm decision may allow simple stop or task-complete stop
  - UI/public projection strips hidden control blocks

## White-Box Coverage

- `freehand-control` parses valid `simple_request=true` status and returns `AllowNaturalStop`
- `freehand-control` rejects `task_complete=true` without `evidence`
- `freehand-control` returns `ContinueWithNextStep` for non-terminal next-step status
- parser accepts the documented closing tag and the symmetric closing tag for format compatibility, without inventing missing semantics

## Module Black-Box Coverage

- `freehand-runtime` mock Anthropic turn accepts `simple_request=true` status stop without legacy `<freehand_completion>`
- runtime metadata ledger contains `control.center` records for `ControlHook03AfterModelResponse` and `ControlHook04BeforeClientReturn`
- runtime metadata ledger does not contain raw status block text or assistant content
- `freehand-ui-protocol` public conversation projection strips hidden status blocks from assistant and terminal text

## Project Black-Box Impact

- current slice has no new online UI behavior claim beyond projection stripping
- WebUI online verification is required before claiming task-management UI lifecycle behavior

## Required Checks

```bash
cargo test -p freehand-control
cargo test -p freehand-runtime live_bridge_accepts_simple_status_stop_hook_without_completion_schema -- --nocapture
cargo test -p freehand-ui-protocol public_conversation_strips_hidden_control_status_blocks -- --nocapture
cargo run -p xtask -- gates check
```

## Known Gaps

- built-in task action tool is not implemented
- task lifecycle persistence and subagent dispatch are still design work
- error.center is not implemented
- selectable options projection is not implemented
