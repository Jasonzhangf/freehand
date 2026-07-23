# MiniMax Provider Snapshot

- snapshot_date: `2026-07-23`
- provider: `MiniMax`
- current Freehand path: Anthropic-compatible Messages endpoint configured under `providers.minimax`
- official_sources:
  - MiniMax native hosted search wire: not independently verified in this repo snapshot
  - Anthropic Messages hosted web_search server-tool reference:
    `https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/web-search-tool`
  - Freehand current config source: `~/.freehand/config.toml`
  - RCC/CC provider config evidence: `/Volumes/extension/.rcc/provider/cc/config.v2.toml`

## Current Freehand Truth

- The current Freehand MiniMax production baseline uses
  `https://api.minimaxi.com/anthropic` with protocol `messages` and model
  `MiniMax-M3`.
- That path is Anthropic-compatible Messages wire. Freehand therefore declares
  hosted search through the Anthropic Messages adapter when config
  `web_search=auto`.
- Freehand does not treat RCC/CC capability hints as independent runtime truth.
  The runtime truth is the configured provider/protocol plus adapter-owned
  render/parse support, followed by a live provider test that proves acceptance
  or returns the exact provider rejection.
- MiniMax native, non-Anthropic search wire remains unverified here and must not
  be hardcoded in runtime.

## Mapping Rule

- For the current `providers.minimax` `anthropic/messages` path,
  `web_search=auto` declares provider-hosted web_search as Anthropic Messages
  server-tool metadata:
  `{"type":"web_search_20250305","name":"web_search","max_uses":5}`.
- The live bridge may select only provider-neutral hosted tool metadata.
  Anthropic/MiniMax-compatible request bodies and hosted-search response block
  parsing stay in `crates/freehand-provider-anthropic`.
- `freehand-cliS adp-provider-web-search-test --url ws://127.0.0.1:4042/adp --provider minimax`
  is the configured S-profile live acceptance test. Success requires a
  provider-hosted web_search observation; provider rejection is a visible
  failure, not a reason to hide the configured capability.
- Do not fake broad search with `web_fetch`. `web_fetch` remains a concrete
  URL fetch tool only.
- If MiniMax native hosted search is later verified outside the Anthropic
  Messages-compatible endpoint, add a provider-specific adapter rendering/parsing
  path and update:
  - `docs/resource-maps/core.json`
  - `docs/function-maps/provider.semantic.md`
  - `docs/function-maps/provider.reason-live-bridge.md`
  - the MiniMax adapter function map/test design
  - focused request/response tests
  - S-profile online proof

## Watchpoints

- Capability declarations from external configs are hints; Freehand runtime
  truth is configured provider/protocol plus adapter-owned support plus live
  acceptance evidence.
- Provider wire DTOs must stay inside the provider adapter. Runtime may select
  capabilities from `ProviderDescriptor`, but must not hardcode MiniMax request
  or response bodies.
