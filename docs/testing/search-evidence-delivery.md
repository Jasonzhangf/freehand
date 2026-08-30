# Test Design: Search Evidence Delivery

- design_id: `search-evidence-schema-delivery-pipeline-20260815-v2`
- lifecycle: `docs/lifecycles/search-evidence-delivery.json`
- truth owner: `reason.turn`
- producer owners: `provider.semantic`, `tool.registry`
- projection owners: `ui.protocol`, `app.webui-smoke`

## Lifecycle Coverage

- Domain plan validates domain policy and minimum verified source count.
- Hosted discovery admits usable candidates only when an original HTTP/HTTPS URL exists.
- Every usable hosted candidate enters camo verification before final delivery.
- Supplement decisions enforce Weibo-first for news and Xiaohongshu-first for tutorial/operations.
- Social candidates re-enter the same camo verification stage.
- Final claims reference persisted verified source ids only.
- Success and blocked terminal paths are explicit and mutually exclusive.

## White-Box Gates

- `freehand-contracts`: six delivery schemas round-trip; `deny_unknown_fields` rejects unknown keys.
- `freehand-blocks`: field validators and adjacent state transitions have paired positive/negative tests.
- `freehand-provider-core` plus adapters: hosted wire results become typed discovery candidates; missing URL remains unusable.
- `freehand-tools`: camo search and page verification parse typed outputs; only actual camo access may emit `verified_by=camo`.
- `freehand-reason`: turn truth persists deliveries and rejects complete claims with unknown, failed, or non-camo sources.
- `freehand-runtime`: `sourced_search` selects hosted search first and exposes camo for verification/supplement only.
- `freehand-ui-protocol`: typed evidence projection preserves source ids, links, attempts, claims, and unconfirmed rows.
- `app.webui-smoke`: DOM rendering uses only `UiSearchEvidenceProjection` fields and never parses provider/camo raw text.

## Positive Paths

- hosted candidates -> camo verification -> no supplement -> final complete.
- hosted candidates -> camo verification -> required social discovery -> camo verification -> final complete.
- no usable or verified source -> explicit blocked final.

## Negative Paths

- hosted discovery -> final is rejected.
- hosted snippet or `web_fetch` output cannot become verified evidence.
- missing/invalid URL cannot enter verification.
- `verified` without `verified_by=camo` or non-empty excerpt is rejected.
- news plan/decision missing Weibo priority is rejected.
- tutorial/operations plan/decision missing Xiaohongshu priority is rejected.
- unknown source id, failed source id, or model-supplied new URL cannot support a final claim.
- unsupported Weibo/X camo platform is explicit failure, never ordinary web-search fallback.

## Schema Conformance Corpus

The search delivery schemas use a repository-owned conformance corpus under
`fixtures/search-evidence/`, modeled on the fixture/rule/verdict binding used by
`sno-ai/mda`. The corpus is machine readable and must run through the same
`freehand-blocks` parser, validators, final builder, and stage matcher consumed by
runtime. `xtask gates check` is the required build/CI entry. A documentation-only
fixture list or a second validator inside `xtask` is insufficient.

Each fixture has:

- stable `id`
- `path`
- `schema`
- `verdict`: `accept` or `reject`
- `rules`: one or more stable rule ids
- `expected_error_category` for rejected fixtures
- `expected_field_path` for rejected fixtures where one field owns the failure
- `operation`: tagged delivery parse, turn validation, final-turn build, or
  model-stage admission

Required fixture families:

- valid round-trip for each of the six delivery schemas
- unknown top-level and nested fields
- missing required fields and wrong primitive/array types
- wrong discriminator/schema version and wrong stage delivery
- unusable hosted candidate with no original URL
- verified candidate with non-camo producer, empty excerpt, or absent access attempt
- final claim with unknown, failed, conflicted, or non-verified source ids
- domain plans missing Weibo for news or Xiaohongshu for tutorial/operations
- explicit blocked final with no verified source

Every new validator rule adds at least one valid and one invalid fixture. The
runner must report fixture id, rule id, verdict, error category, and field path;
it must fail when the observed verdict, category, or field path differs from the
manifest, and when a JSON fixture under the corpus is not referenced exactly once.

Required command gates:

- `cargo test -p freehand-blocks search_evidence -- --nocapture`
- `cargo test -p xtask search_evidence_conformance -- --nocapture`
- `cargo run -p xtask -- search-schema check`
- `cargo run -p xtask -- gates check`

## Natural-Language Routing Evals

The routing corpus must use realistic prompts that do not name internal schemas,
delivery tags, or tools. Each scenario records the provider family, domain, and
fixture response sequence, then asserts protocol/runtime truth rather than
matching a prose answer.

Positive assertions:

- sourced-search profile is selected for a research request
- hosted discovery occurs before any camo operation
- every usable hosted URL produces a camo verification delivery
- news supplementation selects Weibo first; tutorial/operations selects XHS first
- final claims resolve only to persisted verified source ids
- complete is emitted only after final delivery validation

Negative assertions:

- no schema/tool name is required in the user prompt
- a hosted snippet without an original URL cannot become a source
- the model cannot bypass verification by emitting a URL or evidence in final text
- `web_fetch` cannot produce `verified_by=camo`
- no verified source cannot produce complete or a normal summary
- unsupported social platform is explicit failure, not ordinary web-search fallback
- direct summary, general-knowledge fallback, unknown-tool output, and raw
  provider/tool execution errors are not accepted as a successful delivery

Each scenario is run against the current schema pipeline and a no-schema
baseline. A PASS requires concrete evidence from delivery records, stage/tool
trace, and persisted source-id resolution. Human grading may supplement these
checks but cannot replace machine assertions. Keep a fixed validation split so
description/prompt changes are not tuned only against the examples used for
development.

The first reliability slice narrows provider experiments to model-authored
delivery stages: domain plan, supplement decision, and final delivery. For each
OpenAI Responses and Anthropic Messages scenario, run at least five repetitions
for one-shot valid, malformed-then-valid, unknown-field-then-valid,
wrong-stage-then-valid, and three-invalid exhaustion. Prompts are natural
language and must not contain internal Rust type names, delivery tags, or tool
names. Provider/tool-owned hosted discovery and camo verification retain their
existing deterministic owner tests and are not reclassified as model-authored
schema output.

## Provider / Domain / Failure Matrix

| Provider | Domain | Required positive path | Required negative path |
| --- | --- | --- | --- |
| OpenAI Responses | news | hosted URL -> camo verification -> Weibo supplement when needed -> final citations | hosted result without URL; direct final before camo |
| OpenAI Responses | tutorial / operations | hosted URL -> camo verification -> XHS supplement when needed -> final citations | missing XHS priority; `web_fetch` marked verified |
| Anthropic Messages | news | server `web_search` observation -> typed hosted discovery -> camo verification | hosted snippet treated as body evidence |
| Anthropic Messages | general / technical | typed hosted discovery -> verification -> final or explicit blocked | unknown source id; model-supplied URL not in discovery |

For each provider/domain pair, the fixture matrix must cover malformed JSON,
unknown field, wrong schema stage, missing original URL, unknown source,
conflicting evidence, insufficient verified source count, and explicit platform
unsupported. Success, blocked, and schema-retry exhaustion are separate expected
verdicts.

## Structured Rejection Metrics

The parser/validator may expose counters keyed only by:

- schema id/version
- stage
- top-level or nested field path
- rejection category

Metrics must not contain raw prompt text, provider request/response payloads,
tool arguments/results, URL query content, or cookie values. Metrics are
observation-side data and never participate in search business truth or state
transitions.

## Project Black-Box Evidence

- S-profile online run records hosted discovery before camo access.
- Every delivered source has original URL, access timestamp, status, and evidence excerpt.
- Real Xiaohongshu profile is exercised when credentials/profile are available.
- Weibo and X are reported unsupported until current local camo capability and real profile access are both proven.
- WebUI source links and citations match persisted reason-turn source ids.

## Known Pre-Implementation Gaps

- Lifecycle edges are `pending` until code symbols and tests land.
- Current local `camo search --help` confirms only `xhs`; Weibo/X platform adapters are not part of the approved implementation without separate capability evidence.
