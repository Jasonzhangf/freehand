# Test Design: `config.account-config-sync`

- feature_id: `config.account-config-sync`
- owner crate: `crates/freehand-account-config`
- resource map: `docs/resource-maps/core.json`
- function map: `docs/function-maps/config.account-config-sync.md`
- mainline call map: `docs/mainline-calls/config.account-config-sync.json`
- verification map: `docs/verification-maps/config.account-config-sync.json`

## Lifecycle And Logic Path

- Relay host authenticates one account and injects only the account id.
- Account config owner validates strict typed non-secret content.
- Existing document writes require exact If-Match; first write has no matcher.
- Candidate revision and etag are written atomically and fsynced before publication.
- GET and safe projection revalidate stored content before response.

## Resource Operation Test Coverage

| resource operation | status | white-box coverage | module black-box coverage | project black-box coverage |
| --- | --- | --- | --- | --- |
| `account_config_document.validate` | bound | `cargo test -p freehand-account-config valid_document_projects_safely -- --nocapture` plus secret/unknown-reference unit cases prove the typed non-secret boundary | `cargo test -p freehand-account-config --test account_config_http_blackbox -- --nocapture` rejects unknown fields, inline auth, secret values, and missing references through HTTP | `scripts/verify-relay-account-config-smoke.sh` submits a valid document and proves a secret-bearing document fails |
| `account_config_document.get` | bound | `cargo test -p freehand-account-config store_restart_restores_revision_and_etag -- --nocapture` proves disk restore and corrupt-file rejection | `cargo test -p freehand-account-config --test account_config_http_blackbox -- --nocapture` proves same-account GET and cross-account 404 | `scripts/verify-relay-account-config-smoke.sh` restarts the standalone host and re-reads the persisted revision |
| `account_config_document.put` | bound | `cargo test -p freehand-account-config revision_etag_and_account_isolation_round_trip -- --nocapture` proves first write, guarded update, stale conflict, and isolation | `cargo test -p freehand-account-config --test account_config_http_blackbox -- --nocapture` proves PUT/ETag/If-Match, concurrent stale 409 with server document, and unauthorized rejection | `scripts/verify-relay-account-config-smoke.sh` proves process-level PUT/restart/GET and stale conflict behavior |
| `account_config_document.project_safe` | bound | `cargo test -p freehand-account-config valid_document_projects_safely -- --nocapture` proves projection equals only the strict typed schema | `cargo test -p freehand-account-config --test account_config_http_blackbox -- --nocapture` scans successful JSON responses for forbidden secret keys and values | `scripts/verify-relay-account-config-smoke.sh` verifies the process response contains env names but no credential values |
| `account_config_document.pull` | bound | `cargo test -p freehand-account-config client -- --nocapture` validates auth/me, pull response validation, and explicit not-found mapping | `cargo test -p freehand-runtime --lib runtime_account_config_sync_pull -- --nocapture` proves bearer pull, synced mirror persistence, and not-configured projection | `scripts/verify-relay-account-config-smoke.sh` plus Claw online client smoke must prove deployed same-account pull |
| `account_config_document.push` | bound | `cargo test -p freehand-account-config client -- --nocapture` validates If-Match push and conflict document decoding | `cargo test -p freehand-runtime --lib runtime_account_config_sync_push_conflict -- --nocapture` proves export, bearer push, conflict mirror persistence, and explicit failure | `scripts/verify-relay-account-config-smoke.sh` plus Claw online client smoke must prove deployed same-account push |

## White-Box Coverage

- Valid provider registry, endpoint, daemon, and model group references.
- Duplicate ids, unknown references, unsupported schema version, inline auth, secret-shaped values, absolute paths, and unknown fields.
- Revision starts at one and increments exactly once after persistence.
- Etag is canonical content SHA-256 and survives restart.
- Existing update without If-Match and stale If-Match fail without mutation.
- Corrupt stored JSON or mismatched stored etag fails explicitly.

## Module Black-Box Coverage

- Authenticated first PUT, GET, guarded update, stale 409 with current document.
- Missing authentication 401.
- Different account cannot observe another account document.
- Secret/whole-config payload rejection.
- Concurrent same-base PUT permits one revision and rejects the stale peer.

## Project Black-Box Impact

- Standalone `freehand-relay-server` composes Relay authentication plus account config router.
- Restart preserves Relay account store and account config document revision independently.
- Claw online validation is required before claiming deployed same-account sharing.

## Known Gaps And Non-Goals

- This server owner does not apply config into `config.core`, runtime, WebUI, or Android.
- API keys, tokens, passwords, pair credentials, environment values, absolute local paths, and whole `config.toml` files are never accepted.
- No last-write-wins, merge fallback, endpoint fallback, or silent conflict resolution is implemented.
- Pull/push does not apply server content into `config.toml`; effective-config import is a separate explicit client phase.
