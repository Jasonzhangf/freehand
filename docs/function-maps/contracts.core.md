# Function Map: `contracts.core`

- feature_id: `contracts.core`
- owner crate: `crates/freehand-contracts`
- owner module: `crates/freehand-contracts/src/lib.rs`
- owner entry symbols:
  - `AgentId`
  - `SessionId`
  - `TurnId`
  - `TraceId`
  - `FeatureId`
  - `ReasonReq01UserRawInput`
  - `ReasonReq02ContextComposedInput`
  - `ReasonReq03ProviderPayload`
  - `ReasonResp01SemanticEvent`
  - `ErrorErr01RuntimeClassified`
  - `validate_reason_req01`

## Request Mainline

- request-chain semantic nodes are defined and exported as cross-module contracts

## Response Mainline

- response-chain semantic nodes are defined and exported as cross-module contracts

## Error Mainline

- error-chain semantic nodes and base error contracts are defined and exported as cross-module contracts

## Shared Multi-Reference Functions

- `validate_reason_req01`
  - owner: `crates/freehand-contracts/src/lib.rs`
  - purpose: shared request contract guard for non-empty user input
  - allowed callers: request builders, orchestrators, tests
  - related tests: shared contract serialization tests, request validation tests
  - why shared: avoids duplicate non-empty request guards in multiple crates

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ReasonReq01UserRawInput` | `crates/freehand-contracts/src/lib.rs` | define raw request node | request node spec | serializable request contract | all owner crates | contract module | bound |
| 02 | `ReasonReq02ContextComposedInput` | `crates/freehand-contracts/src/lib.rs` | define composed request node | context-composed request spec | serializable request contract | reason/provider crates | contract module | bound |
| 03 | `ReasonReq03ProviderPayload` | `crates/freehand-contracts/src/lib.rs` | define provider payload semantic node | provider payload spec | serializable request contract | reason/provider crates | contract module | bound |
| 04 | `ReasonResp01SemanticEvent` | `crates/freehand-contracts/src/lib.rs` | define semantic response node | semantic event spec | serializable response contract | reason/ui/node crates | contract module | bound |
| 05 | `ErrorErr01RuntimeClassified` | `crates/freehand-contracts/src/lib.rs` | define classified error node | error policy spec | serializable error contract | all owner crates | contract module | bound |
| 06 | `validate_reason_req01` | `crates/freehand-contracts/src/lib.rs` | validate non-empty user input | raw request contract | validated request contract | request builders | shared validator | bound |

## Sync Status Against Code

- shared IDs, request nodes, semantic response nodes, tool contracts, and error contracts are bound in code
