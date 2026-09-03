# v2 Notification Plugin Owner Binding

Status: source_implemented
Owner: `v2-notification-plugin`
Governance: AppSDK `0.1.6`

## Scope

This module owns typed notification admission, deterministic importance/time
ranking, acknowledgement, snooze, archive and projection publication.

It does not own task/session/Agent truth, business payload content or UI
rendering.

## Boundary

```text
typed source event
  -> admit
  -> rank
  -> publish
  -> acknowledge / snooze / archive
```

Allowed paths:

- `playground/experiments/v2/notification-plugin/**`
- `tests/v2/notification-plugin/**`
- `docs/design/v2-notification-plugin-owner-binding.md`

Forbidden paths:

- `active/lib/**`, `protected/**`, `generated/**`
- `docs/v2/v2-ui-design.md`

## Gates

- `cargo test --manifest-path playground/experiments/v2/notification-plugin/Cargo.toml --test v2_notification_plugin_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/notification-plugin/Cargo.toml --all-targets -- -D warnings`
- `appsdk verify --review-admission <project> --module v2-notification-plugin`
