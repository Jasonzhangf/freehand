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

### Event shape direction

Confirmed concept:

- provider/raw events
- semantic reasoning events
- UI projections / subscriber-facing events

Confirmed constraints:

- both semantic position and scene position must be capturable during debugging
- replay and ledger evidence should be first-class debug inputs

## Open Questions / TBD

- exact `ReasonRequest` type shape
- exact event enum and field names
- exact tool-call semantic model
- exact context orchestration algorithm
- exact turn/session persistence model
- exact provider capability negotiation model

## Design guardrails

- provider adapters must not become semantic truth
- reasoning orchestrator must not hide provider errors behind fallback behavior
- if shared semantic transform is reusable, it belongs in `freehand-blocks`

## Update trigger

Update this doc when:

- provider abstraction changes
- turn event model changes
- context orchestration model changes
- provider choice policy changes
