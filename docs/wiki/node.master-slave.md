# Wiki: `node.master-slave`

Generated from `docs/mainline-calls/node.master-slave.json`. Do not edit by hand.

- owner crate: `crates/freehand-node`
- owner module: `crates/freehand-node/src/lib.rs`
- function map: `docs/function-maps/node.master-slave.md`
- generated wiki: `docs/wiki/node.master-slave.md`
- test design: `docs/testing/node.master-slave.md`

## Request Mainline

- local master accepts user input or task delegation intent
- master may dispatch to the paired slave only after `LocalNodeRuntime::pair_slave`
- slave accepts task, projection, or direct-message input only from the active paired source node
- pairing loss reverts slave runtime back to listening state for later re-pairing

## Response Mainline

- slave returns progress, status, direct conversation, or turn stream updates
- `UiProtocolState` stores node status, progress, and latest slave turn
- master may subscribe to slave output while preserving node and source identity through protocol projections

## Error Mainline

- pairing failure, health failure, or unauthorized input to slave return explicit node errors
- pairing rejection materializes node status as `rejected`
- pairing loss materializes node status as `listening`
- slave continues listening after pairing loss

## Shared Multi-Reference Functions

- `UiProtocolState::set_node_status`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: store node status through the protocol owner instead of a duplicate node-local cache
  - allowed callers: freehand-node, tests
  - related tests: node status snapshot smoke
  - why shared: keeps node-status query truth aligned with the UI protocol owner
- `UiProtocolState::set_progress`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: store delegated task progress through the protocol owner
  - allowed callers: freehand-node, tests
  - related tests: slave progress query smoke
  - why shared: keeps progress snapshots aligned with the shared query surface
- `UiProtocolState::apply_turn_projection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: publish slave turn projections through the shared UI truth
  - allowed callers: freehand-node, tests
  - related tests: slave turn publication smoke
  - why shared: keeps slave turn subscription and latest-turn query on one stored projection truth

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `LocalNodeRuntime::new` | `crates/freehand-node/src/lib.rs` | validate local one-master/one-slave bootstrap and seed listening state | master/slave runtime config | node runtime with listening status | CLI/server wiring | node runtime bootstrap | bound |
| 02 | `LocalNodeRuntime::pair_slave` | `crates/freehand-node/src/lib.rs` | validate websocket pairing source, ip, and token and materialize paired or rejected status | pairing request | paired or rejected node status | master runtime | slave runtime state | bound |
| 03 | `LocalNodeRuntime::lose_slave_pairing` | `crates/freehand-node/src/lib.rs` | materialize pairing loss and return slave to listening state | paired slave runtime | listening node status | health/runtime wiring | slave runtime state | bound |
| 04 | `LocalNodeRuntime::delegate_task` | `crates/freehand-node/src/lib.rs` | accept master delegated task and materialize progress snapshot | delegated task intent | progress projection | master runtime | slave progress truth | bound |
| 05 | `LocalNodeRuntime::send_direct_message` | `crates/freehand-node/src/lib.rs` | accept authorized direct message from paired source and materialize paired conversation event | direct message intent | slave direct-message projection | master runtime | paired slave runtime | bound |
| 06 | `LocalNodeRuntime::publish_slave_turn` | `crates/freehand-node/src/lib.rs` | accept authorized slave turn projection and publish to subscribers | slave turn projection | UI turn projection stream | slave runtime | subscribed master or UI surfaces | bound |
| 07 | `LocalNodeRuntime::query_node_status` | `crates/freehand-node/src/lib.rs` | expose latest slave node status snapshot | node id | node status snapshot | query surface | UiProtocolState | bound |
| 08 | `LocalNodeRuntime::query_task_progress` | `crates/freehand-node/src/lib.rs` | expose latest delegated task progress snapshot | turn id | progress snapshot | query surface | UiProtocolState | bound |

## Sync Status Against Mainline Call

- function-map bindings now cover pairing, pairing loss, direct message, progress query, and slave turn publication on `LocalNodeRuntime`
- direct white-box locks now cover unauthorized pair source node, unauthorized pair source ip, empty delegated task status, pre-pair or intruder slave-turn publication, and existing pair-token/direct-message guardrails
- node runtime still writes status, progress, and slave turn through `freehand-ui-protocol` instead of duplicate storage
- real websocket IO adapter remains intentionally out of scope for this first runtime semantic layer
- generated wiki must be regenerated from `docs/mainline-calls/node.master-slave.json` when this function-map truth changes
