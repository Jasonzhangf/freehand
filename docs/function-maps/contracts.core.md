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
  - `FREEHAND_REMOTE_ACCESS_SCOPE_HEADER`
  - `FREEHAND_REMOTE_ACCESS_SCOPE_VALUE`
  - `ContextSegmentKind`
  - `ContextSegment`
  - `ReasonReq01UserRawInput`
  - `ReasonReq02ContextComposedInput`
  - `ReasonReq03ProviderPayload`
  - `ToolArgument`
  - `ToolPreviewChangeKind`
  - `ToolPreviewFileChange`
  - `ToolPreviewContract`
  - `ReasonResp01SemanticEvent`
  - `ErrorErr01RuntimeClassified`
  - `SearchEvidenceDelivery`
  - `SearchEvidenceTurnDelivery`
  - `validate_reason_req01`
  - `validate_reason_req02`
  - `validate_reason_req03`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources: shared cross-module contract types and transport-control constants
- touched resources: relay_data_tunnel, ui_projection
- forbidden shortcuts:
  - remote access-scope constants must not be serialized into business request/response payload nodes
  - Relay must not import UI protocol or runtime semantics

## Request Mainline

- request-chain semantic nodes are defined and exported as cross-module contracts
- typed context segments now replace ad hoc context item pairs, including
  first-class task contract, task-space snapshot, and attention-resolution kinds
- provider payload semantic contract now carries ordered `input_segments` rather than one rendered prompt string
- writable-tool preview contracts remain separate from provider request content while staying replay-safe across runtime/tool boundaries
- shared Relay-to-WebUI access-scope header names remain transport control constants and never enter request/response business payload structs

## Response Mainline

- response-chain semantic nodes are defined and exported as cross-module contracts
- search-evidence stage and turn deliveries are strict, versioned business contracts; retry and routing control stay outside them

## Error Mainline

- error-chain semantic nodes and base error contracts are defined and exported as cross-module contracts

## Shared Multi-Reference Functions

- `validate_reason_req01`
  - owner: `crates/freehand-contracts/src/lib.rs`
  - purpose: shared request contract guard for non-empty user input
  - allowed callers: request builders, orchestrators, tests
  - related tests: shared contract serialization tests, request validation tests
  - why shared: avoids duplicate non-empty request guards in multiple crates
- `validate_reason_req02`
  - owner: `crates/freehand-contracts/src/lib.rs`
  - purpose: validate typed context-composed requests, including user-turn segment admission
  - allowed callers: reason orchestrator, planner builders, tests
  - related tests: context segment validation tests
  - why shared: keeps request-side boundary checks centralized instead of revalidated differently in each crate
- `validate_reason_req03`
  - owner: `crates/freehand-contracts/src/lib.rs`
  - purpose: validate provider payload semantic contract before adapter rendering
  - allowed callers: provider semantic boundary, tests
  - related tests: provider semantic request validation tests
  - why shared: keeps provider-boundary request checks centralized

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ReasonReq01UserRawInput` | `crates/freehand-contracts/src/lib.rs` | define raw request node | request node spec | serializable request contract | all owner crates | contract module | bound |
| 02 | `ContextSegment` | `crates/freehand-contracts/src/lib.rs` | define typed model-visible context node | context segment spec | serializable context contract | planner/reason/provider crates | contract module | bound |
| 03 | `ReasonReq02ContextComposedInput` | `crates/freehand-contracts/src/lib.rs` | define composed request node | typed context-composed request spec | serializable request contract | reason/provider crates | contract module | bound |
| 04 | `ReasonReq03ProviderPayload` | `crates/freehand-contracts/src/lib.rs` | define provider payload semantic node | typed provider input segment spec | serializable request contract | reason/provider crates | contract module | bound |
| 05 | `ToolArgument` | `crates/freehand-contracts/src/lib.rs` | define shared structured tool-argument node | tool argument spec | serializable JSON-capable argument contract | provider/reason/ui crates | contract module | bound |
| 06 | `ToolPreviewChangeKind` | `crates/freehand-contracts/src/lib.rs` | define shared writable-preview change-kind node | preview change-kind spec | serializable preview enum | tool/runtime/debug crates | contract module | bound |
| 07 | `ToolPreviewFileChange` | `crates/freehand-contracts/src/lib.rs` | define shared writable-preview file-change node | preview file-change spec | serializable preview contract | tool/runtime/debug crates | contract module | bound |
| 08 | `ToolPreviewContract` | `crates/freehand-contracts/src/lib.rs` | define shared writable-preview envelope | preview contract spec | serializable preview contract | tool/runtime/debug crates | contract module | bound |
| 09 | `ReasonResp01SemanticEvent` | `crates/freehand-contracts/src/lib.rs` | define semantic response node | semantic event spec | serializable response contract | reason/ui/node crates | contract module | bound |
| 10 | `ErrorErr01RuntimeClassified` | `crates/freehand-contracts/src/lib.rs` | define classified error node | error policy spec | serializable error contract | all owner crates | contract module | bound |
| 11 | `validate_reason_req01` | `crates/freehand-contracts/src/lib.rs` | validate non-empty user input | raw request contract | validated request contract | request builders | shared validator | bound |
| 12 | `validate_reason_req02` | `crates/freehand-contracts/src/lib.rs` | validate typed context-composed request | composed request contract | validated request contract | reason/planner | shared validator | bound |
| 13 | `validate_reason_req03` | `crates/freehand-contracts/src/lib.rs` | validate provider payload contract | provider payload contract | validated provider payload | provider semantic boundary | shared validator | bound |
| 14 | `FREEHAND_REMOTE_ACCESS_SCOPE_HEADER / FREEHAND_REMOTE_ACCESS_SCOPE_VALUE` | `crates/freehand-contracts/src/lib.rs` | define one shared Relay-to-WebUI remote access-scope header contract without adding the marker to business payload DTOs | typed transport boundary | stable header name/value | Relay Agent bridge and WebUI server | contract module | bound |
| 15 | `SearchEvidenceDelivery / SearchEvidenceTurnDelivery` | `crates/freehand-contracts/src/lib.rs` | define strict versioned search-evidence stage and turn contracts with unknown-field rejection | search delivery schema | replayable typed business delivery | provider/tool/reason/runtime owners | contract module | bound |

## Sync Status Against Code

- shared IDs, typed context segment contracts including task
  contract/snapshot/attention-resolution kinds, request nodes, tool contracts,
  preview contracts, semantic response nodes, and error contracts are bound in code
- request-side validation helpers remain single-owner contract guards and are reused across orchestrator boundaries
- remote access-scope constants are shared transport-control contracts and are absent from request/response payload node fields
- generated wiki must be regenerated from `docs/mainline-calls/contracts.core.json` when this function-map truth changes
