# v2 Search Plugin Owner Binding

Status: source_implemented
Owner: `v2-search-plugin`
Governance: AppSDK `0.1.6`

## Scope

This module owns typed search index, keyword query, result classification,
cache invalidation and rebuild lifecycle.

It does not own source records, UI filtering or distributed indexing.

## Boundary

```text
source record
  -> index
  -> query
  -> classify
  -> cache / invalidate / rebuild
```

Allowed paths:

- `playground/experiments/v2/search-plugin/**`
- `tests/v2/search-plugin/**`
- `docs/design/v2-search-plugin-owner-binding.md`

Forbidden paths:

- `active/lib/**`, `protected/**`, `generated/**`
- `docs/v2/v2-ui-design.md`

## Gates

- `cargo test --manifest-path playground/experiments/v2/search-plugin/Cargo.toml --test v2_search_plugin_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/search-plugin/Cargo.toml --all-targets -- -D warnings`
- `appsdk verify --review-admission <project> --module v2-search-plugin`
