# v2 Channel Registry Owner Binding

Status: source_implemented
Owner: `v2-channel-registry`
Governance: AppSDK `0.1.6`

## Scope

This module owns stateless bearer-token endpoint admission, endpoint
registration/discovery, capability and protocol reconciliation, route-change
publication, logical ChannelSession state, transport connection replacement,
suspend/reattach and ordered frame replay.

It does not own production sockets, TLS, Relay deployment, reasoning truth,
Session Log truth or UI rendering.

## Boundary

```text
endpoint registration
  -> token admission
  -> ChannelSession open
  -> Connection attach / replace / suspend / reattach
  -> Control / Payload / Error frame
  -> ordered replay
```

Allowed paths:

- `playground/experiments/v2/channel-registry/**`
- `tests/v2/channel-registry/**`
- `docs/design/v2-channel-registry-owner-binding.md`

Forbidden paths:

- `active/lib/**`, `protected/**`, `generated/**`
- `docs/v2/v2-ui-design.md`

## Gates

- `cargo test --manifest-path playground/experiments/v2/channel-registry/Cargo.toml --test v2_channel_registry_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/channel-registry/Cargo.toml --all-targets -- -D warnings`
- `appsdk verify --review-admission <project> --module v2-channel-registry`
