# Function Map: `node.master-slave`

- feature_id: `node.master-slave`
- owner crate: `crates/freehand-node`
- owner module: `crates/freehand-node/src/lib.rs`
- owner entry symbols:
  - `LocalNodeRuntime::new`
  - `LocalNodeRuntime::pair_slave`
  - `LocalNodeRuntime::lose_slave_pairing`
  - `LocalNodeRuntime::delegate_task`
  - `LocalNodeRuntime::send_direct_message`
  - `LocalNodeRuntime::publish_slave_turn`
  - `LocalNodeRuntime::query_node_status`
  - `LocalNodeRuntime::query_task_progress`

## Request Mainline

- local master accepts user input or task delegation intent
- master may dispatch to the paired slave only after `LocalNodeRuntime::pair_slave`
- slave accepts task/projection/message input only from the active paired source node
- pairing loss reverts slave runtime back to listening state for later re-pairing

## Response Mainline

- slave returns progress, status, direct conversation, or turn stream updates
- `UiProtocolState` stores node status, progress, and latest slave turn
- master may subscribe to slave output while preserving node/source identity through `UiProjection`

## Error Mainline

- pairing failure, health failure, or unauthorized input to slave return explicit node errors
- pairing rejection materializes node status as `rejected`
- pairing loss materializes node status as `listening`
- slave continues listening after pairing loss

## Shared Multi-Reference Functions

- `UiProtocolState::set_node_status`
  - reused so node runtime writes status through UI protocol truth instead of duplicate caches
- `UiProtocolState::set_progress`
  - reused so progress query stays aligned with UI protocol query surface
- `UiProtocolState::apply_turn_projection`
  - reused so slave turn subscription and latest-turn query share one stored projection truth

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `LocalNodeRuntime::new` | `crates/freehand-node/src/lib.rs` | validate local one-master/one-slave bootstrap and seed listening state | master/slave runtime config | node runtime with listening status | CLI/server wiring | node runtime bootstrap | bound |
| 02 | `LocalNodeRuntime::pair_slave` | `crates/freehand-node/src/lib.rs` | validate websocket pairing source/ip/token and materialize paired or rejected status | pairing request | paired or rejected node status | master runtime | slave runtime state | bound |
| 03 | `LocalNodeRuntime::lose_slave_pairing` | `crates/freehand-node/src/lib.rs` | materialize pairing loss and return slave to listening state | paired slave runtime | listening node status | health/runtime wiring | slave runtime state | bound |
| 04 | `LocalNodeRuntime::delegate_task` | `crates/freehand-node/src/lib.rs` | accept master delegated task and materialize progress snapshot | delegated task intent | progress projection | master runtime | slave progress truth | bound |
| 05 | `LocalNodeRuntime::send_direct_message` | `crates/freehand-node/src/lib.rs` | accept authorized direct message from paired source and materialize paired conversation event | direct message intent | slave direct-message projection | master runtime | paired slave runtime | bound |
| 06 | `LocalNodeRuntime::publish_slave_turn` | `crates/freehand-node/src/lib.rs` | accept authorized slave turn projection and publish to subscribers | slave turn projection | UI turn projection stream | slave runtime | subscribed master/UI surfaces | bound |
| 07 | `LocalNodeRuntime::query_node_status` | `crates/freehand-node/src/lib.rs` | expose latest slave node status snapshot | node id | node status snapshot | query surface | `UiProtocolState` | bound |
| 08 | `LocalNodeRuntime::query_task_progress` | `crates/freehand-node/src/lib.rs` | expose latest delegated task progress snapshot | turn id | progress snapshot | query surface | `UiProtocolState` | bound |

## Sync Status Against Code

- function-map bindings now cover pairing, pairing loss, direct message, progress query, and slave turn publication on `LocalNodeRuntime`
- direct white-box locks now cover unauthorized pair source node, unauthorized pair source ip, empty delegated task status, pre-pair or intruder slave-turn publication, and existing pair-token/direct-message guardrails
- real websocket IO adapter remains intentionally out of scope for this first runtime semantic layer
- generated wiki must be regenerated from `docs/mainline-calls/node.master-slave.json` when this function-map truth changes
