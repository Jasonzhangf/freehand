# Debug And Trace

Freehand debug must preserve two coordinates at once.

## Semantic Position

- `feature_id`
- `session_id`
- `turn_id`
- `provider_id`
- pipeline node id

## Scene Position

- crate
- file
- function
- line or span
- artifact path
- raw exchange id

## Required Artifact Shape

Every critical pipeline stage should eventually emit a trace envelope with:

- `trace_id`
- `semantic_ref`
- `source_ref`
- `input_hash`
- `output_hash`
- `artifact_path`
- `timestamp`

## Debug Rule

- logs are hints, not truth
- replay fixtures and event ledgers are truth
- snapshot truncation may remove debug-only noise, never semantic payload
- debug starts from function map owner and debug artifact path
- runtime scene evidence should be discoverable under `~/.freehand`
