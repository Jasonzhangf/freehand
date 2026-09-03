# v2 Session Canvas Plugin Owner Binding

Status: source_implemented
Owner: `v2-session-canvas-plugin`
Governance: AppSDK `0.1.6`

## Scope

This module owns Session Log-derived active/recent/history canvas nodes/edges,
focus and filter projections.

It does not own Session Log truth, visual graph layout or browser history.

## Boundary

```text
Session Log relationships
  -> derive
  -> publish
  -> focus / filter
```

Allowed paths:

- `playground/experiments/v2/session-canvas-plugin/**`
- `tests/v2/session-canvas-plugin/**`
- `docs/design/v2-session-canvas-plugin-owner-binding.md`

Forbidden paths:

- `active/lib/**`, `protected/**`, `generated/**`
- `docs/v2/v2-ui-design.md`

## Gates

- `cargo test --manifest-path playground/experiments/v2/session-canvas-plugin/Cargo.toml --test v2_session_canvas_plugin_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/session-canvas-plugin/Cargo.toml --all-targets -- -D warnings`
- `appsdk verify --review-admission <project> --module v2-session-canvas-plugin`
