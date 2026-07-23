# MiniMax Provider Snapshot

- snapshot_date: `2026-07-23`
- provider: `MiniMax`
- current Freehand path: Anthropic-compatible Messages endpoint configured under `providers.minimax`
- official_sources:
  - MiniMax native hosted search wire: not verified in this repo snapshot
  - Freehand current config source: `~/.freehand/config.toml`
  - RCC/CC provider config evidence: `/Volumes/extension/.rcc/provider/cc/config.v2.toml`

## Current Freehand Truth

- The current Freehand MiniMax production baseline uses
  `https://api.minimaxi.com/anthropic` with protocol `messages` and model
  `MiniMax-M3`.
- That path is Anthropic-compatible Messages wire. It is not a verified MiniMax
  native web-search wire.
- RCC/CC config may declare a `web_search` capability, but that is not enough
  to enable Freehand hosted search. Freehand needs an exact provider/protocol
  wire contract before declaring provider-hosted search support.

## Mapping Rule

- Do not enable `ProviderWebSearchCapability::Hosted` for MiniMax until the
  exact selected Freehand provider protocol has a verified native hosted-search
  request and response shape.
- Do not fake broad search with `web_fetch`. `web_fetch` remains a concrete
  URL fetch tool only.
- If MiniMax hosted search is later verified, add a provider-specific adapter
  rendering/parsing path and update:
  - `docs/resource-maps/core.json`
  - `docs/function-maps/provider.semantic.md`
  - `docs/function-maps/provider.reason-live-bridge.md`
  - the MiniMax adapter function map/test design
  - focused request/response tests
  - S-profile online proof

## Watchpoints

- Capability declarations from external configs are hints, not Freehand runtime
  truth.
- Provider wire DTOs must stay inside the provider adapter. Runtime may select
  capabilities from `ProviderDescriptor`, but must not hardcode MiniMax request
  or response bodies.
