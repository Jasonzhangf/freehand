# Provider And Reasoning Design

## Status

Confirmed discussion only. Unconfirmed details remain `TBD`.

## Confirmed

### Provider strategy

- prefer Rust-native implementation
- do not bind core semantics to Vercel TypeScript SDK
- OpenAI-compatible provider may use Rust library or raw HTTP/SSE
- Anthropic side may use raw HTTP/SSE until stable Rust SDK choice is justified
- provider core should own transport-neutral semantics
- provider adapters should map wire format into one shared reasoning semantic

### Reasoning semantic goal

- different providers should map into one semantic model
- reasoning module should abstract provider differences behind one contract
- each turn must emit reasoning progress/status events
- other modules consume those events through subscription, not direct coupling
- reasoning module should compile independently as CLI bin
- configured master may dispatch reasoning work to local sub-agents or paired remote slave agents, but reasoning semantics should remain one contract surface
- startup configuration may decide whether an agent participates as master or slave, but reasoning semantic contract should remain unchanged across those modes

### Reasonix alignment status

Current Freehand baseline now has typed segment request planning in code, but full runtime rewrite orchestration is still pending.

Locked direction:

- preserve stable context segments across turns to improve provider cache hit rate
- separate stable reusable context from per-turn volatile input
- make context composition a typed pipeline, not ad hoc rendered text
- record cache-sensitive segment boundaries in debug/replay evidence
- prefer subagent search final-report projection over injecting raw exploration transcripts
- admit subagent context into parent turns only as typed conclusion segments
- keep stable prefix immutable on ordinary turns; rewrites happen only through explicit rewrite gates

Reference evidence already extracted:

- Reasonix keeps cache-friendly prompt head stable and uses separate subagent sessions
- Reasonix parent sees subagent final answer, not child transcript internals
- Codex requires typed bounded fragments and keeps prompt input separate from telemetry/header/trace state

Current implementation baseline:

- current code plans typed request segments before provider rendering
- current code keeps planner diagnostics outside request content
- current code exposes stable-segment hash and token-cost diagnostics for cache analysis
- current code now sources rewrite mode/version from session-history truth instead of hardcoded turn-local constants
- current code now has a dedicated `reason.rewrite-policy` owner path in `freehand-blocks` for compaction and recovery trigger decisions
- current code now has `ReasonRewriteRuntime` as the single baseline consumer that can call session-history rewrite gates from policy decisions
- current code now has an Anthropic-only live bridge in `freehand-testkit` that routes selected config into one real reason turn without adding provider adapter dependencies to `freehand-reason`

Current implementation gap:

- current code does not yet wire tool-schema fingerprint into planner diagnostics from runtime tool truth
- production server-grade multi-provider runtime loop wiring for real usage metrics and recovery payloads remains pending

`freehand-reason` and provider crates must stay independently owned and independently testable.

Hard boundary:

- `freehand-reason` owns turn truth, context orchestration, completion-schema control, and semantic event broadcast
- `freehand-blocks` owns pure rewrite trigger policy and abnormal-state classification
- provider crates own protocol rendering/parsing and provider semantic output normalization
- provider crates must not write turn/session truth
- `freehand-reason` must not depend on provider adapter crates
- provider adapter crates must not depend on `freehand-reason`
- the only allowed runtime bridge is provider-neutral contracts and `freehand-provider-core` semantic outputs

### Event shape direction

Confirmed concept:

- provider/raw events
- semantic reasoning events
- UI projections / subscriber-facing events

Confirmed constraints:

- both semantic position and scene position must be capturable during debugging
- replay and ledger evidence should be first-class debug inputs

### Metadata / request pipeline hard isolation

Cross-module metadata and request data must be type-isolated.

Hard rule:

- request chain nodes carry user/task/context/provider-input data only
- metadata carries routing, provenance, debug, trace, provider/model, cache accounting, and scene position only
- metadata must not be embedded into request payload strings to drive behavior
- request content must not be read from metadata fields
- metadata and request nodes must use distinct types and distinct builders
- only adjacent pipeline builders may combine metadata with request data for envelope emission or debug ledger writes
- provider wire renderers must receive an explicit request node plus explicit metadata/config, not a mixed catch-all DTO

## Open Questions / TBD

- exact `ReasonRequest` type shape
- exact event enum and field names
- exact tool-call semantic model
- exact turn/session persistence model
- exact provider capability negotiation model
- exact metadata/request envelope type names and builder ownership

## Design guardrails

- provider adapters must not become semantic truth
- reasoning orchestrator must not hide provider errors behind fallback behavior
- if shared semantic transform is reusable, it belongs in `freehand-blocks`
- metadata must not be mixed into request data pipeline types
- request data must not be recovered from metadata/debug fields

## Update trigger

Update this doc when:

- provider abstraction changes
- turn event model changes
- context orchestration model changes
- provider choice policy changes
