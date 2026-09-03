# v2 Memory Plugin Owner Binding

Status: source_implemented
Owner: `v2-memory-plugin`
Governance: AppSDK `0.1.6`

## Scope

This module owns session-scoped memory attach/detach, summarize, record,
save/load/search/export lifecycle and projections.

It does not own original Session Log facts, provider payloads or browser
persistence.

## Boundary

```text
attach
  -> summarize
  -> record
  -> save / load / search / export
  -> detach
```

Allowed paths:

- `playground/experiments/v2/memory-plugin/**`
- `tests/v2/memory-plugin/**`
- `docs/design/v2-memory-plugin-owner-binding.md`

Forbidden paths:

- `active/lib/**`, `protected/**`, `generated/**`
- `docs/v2/v2-ui-design.md`

## Gates

- `cargo test --manifest-path playground/experiments/v2/memory-plugin/Cargo.toml --test v2_memory_plugin_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/memory-plugin/Cargo.toml --all-targets -- -D warnings`
- `appsdk verify --review-admission <project> --module v2-memory-plugin`
