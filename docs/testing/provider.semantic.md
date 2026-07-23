# Test Design: `provider.semantic`

- feature_id: `provider.semantic`
- owner: `crates/freehand-provider-core`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `provider_hosted_search.declare`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `provider_hosted_search.declare` | bound | `cargo test -p freehand-provider-core hosted_tool_metadata -- --nocapture` covers provider-neutral hosted tool metadata on `ProviderSemanticRequest`; `cargo test -p freehand-runtime live_bridge_derives_hosted_web_search_only_for_supported_openai_responses -- --nocapture --test-threads=1` covers capability-driven declaration | `cargo test -p freehand-provider-openai web_search -- --nocapture` proves the adapter consumes the provider-neutral declaration without a core-owned wire DTO | `node scripts/verify-provider-hosted-web-search-online.mjs` proves S-profile OpenAI Responses request truth declares hosted `web_search` and does not expose a local function tool named `web_search` |

- lifecycle path under test:
  - provider request enters semantic adapter
  - typed provider payload is validated before adapter rendering
  - provider-neutral tool schema/choice/exchange metadata stays outside request text
  - provider-neutral hosted tool metadata stays outside request text and outside local tool execution
  - stream or single-shot output becomes unified semantic events
  - debug/raw retention policy stays separated from normal path
  - recovery classification remains explicit
- white-box plan:
  - event mapping, capability declaration, recovery classification, retention rules
  - hosted tool declaration is provider-neutral metadata on `ProviderSemanticRequest.hosted_tools`
  - OpenAI `responses` protocol mapping
  - OpenAI `chat completions` protocol admission in semantic request layer
  - provider-neutral tool metadata round trip on the semantic request object
- module black-box plan:
  - provider semantic boundary emits expected unified events for stream and single-shot flows
- project black-box impact:
  - reason layer can consume provider semantic output without provider-specific leakage
- fixtures / replay inputs / runtime evidence paths:
  - provider payload fixtures
  - `~/.freehand/ledgers/providers`
  - `~/.freehand/replays/providers`
- known gaps:
  - adapter-specific fixture catalog not yet defined
- sync status between design and implementation:
  - semantic request builder, typed payload validation, event mapping, and error classification baseline landed
  - provider-neutral tool metadata is now part of the request contract
  - provider-neutral hosted tool metadata is now part of the request contract
- mainline/wiki sync:
  - wiki generated from mainline call must stay in sync with provider semantic owner code and function map updates
