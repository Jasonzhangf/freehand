# Function Map: `metadata.core`

- feature_id: `metadata.core`
- owner crate: `crates/freehand-metadata`
- owner module: `crates/freehand-metadata/src/lib.rs`
- owner entry symbols:
  - `MetadataId`
  - `MetadataKind`
  - `MetadataWriteOwner`
  - `MetadataWriteNode`
  - `MetadataSubject`
  - `MetadataEntry`
  - `MetadataEnvelope`
  - `MetadataEnvelope::new`
  - `MetadataCenter`
  - `MetadataCenter::write`
  - `MetadataCenter::by_trace`
  - `validate_metadata_envelope`

## Request Mainline

- owner module constructs metadata with one explicit `MetadataWriteOwner`
- owner module constructs metadata with one explicit `MetadataWriteNode`
- owner module attaches a `MetadataSubject` carrying trace/session/turn identity without carrying request text
- metadata entry keys are validated before admission
- metadata center stores only validated internal control/provenance metadata

## Response Mainline

- metadata center returns accepted metadata records as internal provenance/control truth
- metadata center can query records by `trace_id` for debugging and audit correlation
- serialized metadata remains replay-safe without becoming request-chain data

## Error Mainline

- missing metadata id, writer owner, writer node, trace id, or entries is rejected explicitly
- request-data-like keys such as `request.*`, `payload.*`, `prompt.*`, `input.*`, `content`, or `text` are rejected explicitly
- invalid metadata is not stored and is not rewritten into debug/session/request truth
- no fallback path exists for recovering request content from metadata

## Shared Multi-Reference Functions

- `validate_metadata_envelope`
  - owner: `crates/freehand-metadata/src/lib.rs`
  - purpose: enforce metadata owner, write-node, subject, and request-data key separation before metadata can be stored or forwarded
  - allowed callers: module metadata writers, metadata center, tests
  - related tests: metadata validation tests, metadata/request isolation smoke
  - why shared: avoids each module inventing its own metadata provenance and request-isolation checks
- `MetadataCenter::write`
  - owner: `crates/freehand-metadata/src/lib.rs`
  - purpose: central validated metadata admission point
  - allowed callers: runtime/reason/provider/node/debug integration layers after they construct metadata envelopes
  - related tests: metadata center write/query smoke
  - why shared: keeps metadata admission and validation in one owner instead of distributed module-local maps

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `MetadataWriteOwner` | `crates/freehand-metadata/src/lib.rs` | identify the feature/symbol that wrote metadata | owner feature/crate/module/symbol | writer owner contract | metadata writers | metadata contract | bound |
| 02 | `MetadataWriteNode` | `crates/freehand-metadata/src/lib.rs` | identify the pipeline node where metadata was written | pipeline node and optional runtime node id | write-node contract | metadata writers | metadata contract | bound |
| 03 | `MetadataSubject` | `crates/freehand-metadata/src/lib.rs` | identify trace/session/turn subject without request text | trace/session/turn ids | metadata subject contract | metadata writers | metadata contract | bound |
| 04 | `MetadataEnvelope::new` | `crates/freehand-metadata/src/lib.rs` | construct validated metadata envelope | id/kind/owner/node/subject/entries | accepted metadata envelope or explicit error | metadata writers | `validate_metadata_envelope` | bound |
| 05 | `validate_metadata_envelope` | `crates/freehand-metadata/src/lib.rs` | enforce owner, write-node, subject, entry, and request-key rules | metadata envelope | pass/fail | `MetadataEnvelope::new` and `MetadataCenter::write` | validation helpers | bound |
| 06 | `MetadataCenter::write` | `crates/freehand-metadata/src/lib.rs` | admit validated metadata into the center | metadata envelope | stored metadata or explicit error | metadata writers | validator + in-memory store | bound |
| 07 | `MetadataCenter::by_trace` | `crates/freehand-metadata/src/lib.rs` | query metadata records by trace id for audit/debug correlation | trace id | metadata record references | debug/audit tools | in-memory store | bound |

## Metadata / Request Isolation Notes

- metadata center owns internal control/provenance metadata only
- request-chain content remains in request node types such as `ReasonReq01UserRawInput`, `ReasonReq02ContextComposedInput`, and `ReasonReq03ProviderPayload`
- metadata entries must not use request-like keys and must not be treated as a fallback source for prompt, input, or message content
- debug envelopes may reference metadata later, but debug remains observation-only and does not own metadata admission

## Sync Status Against Code

- metadata envelope, writer owner, write node, subject, validation, and in-memory center are bound in code
- first integration slice does not yet wire runtime/reason/provider producers into the metadata center
- generated wiki must be regenerated from `docs/mainline-calls/metadata.core.json` when this function-map truth changes
