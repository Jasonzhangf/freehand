# Runtime Daemon Design

## Purpose

Define the first real host process that injects `freehand-runtime` into the UI HTTP/SSE transport without turning the transport app into a business owner.

## Owner

- feature_id: `app.runtime-daemon`
- owner crate: `apps/freehand-daemon`

## Core Boundary

- `apps/freehand-server` remains protocol-only transport owner
- `apps/freehand-daemon` is the runtime host owner
- `crates/freehand-runtime` remains the only runtime command-dispatch owner
- `freehand-server` must not depend on:
  - `freehand-runtime`
  - `freehand-reason`
  - `freehand-node`
  - `freehand-config`
  - provider crates

## First Baseline

- daemon process exposes the same first-version UI transport:
  - HTTP query
  - SSE subscribe
  - POST command ingress
- daemon injects a real `RuntimeCommandDispatcher`
- transport and runtime share one `UiProtocolState` handle
- daemon does not create a second command-routing truth
- app transport still relies on protocol-owned command ingress validation and owner-routing envelope
- daemon bootstrap goes through `freehand-runtime`, which consumes explicit peer-topology config truth from `config.core`

Status:

- first runtime-host baseline is now landed with shared transport plus runtime-owned dispatch wiring
- config-selected bootstrap is now landed for one agent from `~/.freehand/config.toml`
- config-selected bootstrap now uses configured local/paired node ids instead of synthetic peer ids
- provider-backed submit-user-input now runs inside `freehand-runtime` with Anthropic `messages`, completion schema, tool re-entry, persistence, and UI projection
- daemon restart now restores persisted turn projection into query/SSE state before the next command runs
- protocol-only async command ingress must call injected synchronous runtime dispatch through an explicit blocking boundary

## Shared Transport Rule

- HTTP/SSE/command handlers must live in one protocol-only transport implementation
- runtime host app may reuse that implementation, but must not fork it into a second behavior copy
- transport helpers may be shared from `apps/freehand-server` as long as they stay protocol-only
- transport may wrap injected synchronous runtime work with async-runtime-safe helpers such as `tokio::task::spawn_blocking`, but it must not own business dispatch semantics itself

## First Runtime Scope

- submit input -> `freehand-runtime` provider live bridge -> `reason.turn`
- cancel turn -> `reason.turn`
- direct message to slave -> `node.master-slave`
- resume turn -> explicit unsupported error
- provider-backed execution loop must cover Anthropic `messages`, completion schema, tool re-entry, persistence, and UI projection in the first live daemon baseline
- config-selected bootstrap chooses the active agent from `~/.freehand/config.toml` and uses its reciprocal paired-agent topology for local one-master-one-slave wiring

## Dependency Rule

- `apps/freehand-daemon` may depend on:
  - `freehand-runtime`
  - `freehand-server` transport library
  - `freehand-ui-protocol`
  - `axum` / `tokio`
- `apps/freehand-daemon` must not directly depend on:
  - `freehand-reason`
  - `freehand-node`
  - `freehand-config`
  - provider crates

- `crates/freehand-runtime` may depend on `freehand-config` for config-selected bootstrap

## Update Trigger

Update this doc when:

- the runtime host process shape changes
- daemon bootstrap moves from fixed bootstrap config to selected runtime config
- shared HTTP/SSE transport ownership changes
- runtime owner injection path changes
