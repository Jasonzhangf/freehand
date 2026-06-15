# Function Map: `app.webui-smoke`

- feature_id: `app.webui-smoke`
- owner crate: `apps/freehand-server`
- owner module: `apps/freehand-server/src/main.rs`
- owner entry symbols:
  - `main`
  - `render_webui_smoke`

## Request Mainline

- app boundary receives a minimal WebUI smoke invocation
- app boundary consumes `freehand-ui-protocol` projection truth only
- app boundary may render query snapshot and separate slave-card projection without owning protocol semantics

## Response Mainline

- app boundary renders protocol-owned terminal text, query snapshot, and slave-card visibility into a minimal HTML/text smoke output
- CLI and WebUI divergence remains a rendering decision only, not a protocol decision

## Error Mainline

- invalid smoke input or missing projection returns explicit app error
- transport/render wiring failures are surfaced explicitly

## Shared Multi-Reference Functions

- `turn_projection_for_client`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: keep client-specific slave-card visibility inside the protocol owner
  - allowed callers: CLI/WebUI adapters, tests
  - related tests: slave turn subscription smoke
  - why shared: app boundary must not duplicate client-specific projection logic

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `render_webui_smoke` | `apps/freehand-server/src/main.rs` | render minimal WebUI smoke output from protocol truth | protocol query/projection truth | HTML/text smoke | app entrypoint | rendering helper | bound |
| 02 | `turn_projection_for_client` | `crates/freehand-ui-protocol/src/lib.rs` | gate slave-card visibility by client kind | turn projection + client kind | client-specific projection | app boundary | protocol owner | bound |

## Sync Status Against Code

- app boundary now renders a minimal WebUI smoke output from protocol truth
- protocol-owned client-specific projection helper exists and is now a shared owner boundary for the app smoke
