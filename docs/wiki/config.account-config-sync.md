# Wiki: `config.account-config-sync`

Generated from `docs/mainline-calls/config.account-config-sync.json`. Do not edit by hand.

- owner crate: `crates/freehand-account-config`
- owner module: `crates/freehand-account-config/src/lib.rs`
- function map: `docs/function-maps/config.account-config-sync.md`
- generated wiki: `docs/wiki/config.account-config-sync.md`
- test design: `docs/testing/config.account-config-sync.md`

## Resource Operation Backlinks

- account_config_document.validate
- account_config_document.get
- account_config_document.put
- account_config_document.project_safe
- account_config_document.pull
- account_config_document.push

## Request Mainline

- Relay host authenticates one account id before invoking the account-config router; no token or cookie enters the account config document owner
- GET resolves only the authenticated account document; PUT validates schemaVersion and typed non-secret content before mutation
- PUT requires If-Match for an existing document and computes the next revision plus canonical content etag inside the account-config owner
- runtime pull enters AccountConfigClient::pull with the authenticated Relay token, revalidates the returned document, and publishes an account-scoped device mirror under the runtime home
- runtime push exports the local non-secret surface through config.core::export_shared_account_config, sends the local mirror etag via AccountConfigClient::push, and maps 409 to an explicit conflict mirror carrying the server current document

## Response Mainline

- successful GET/PUT returns the typed document, revision, updatedAt, and matching HTTP/content etag
- safe projection returns only provider auth env names, model groups, relay URL plus token env names, and remote daemon entries without credentials
- durable atomic persistence completes before the in-memory latest document is published
- pull and push return only validated revisioned non-secret documents; the device mirror stores status, account id, revision, etag, updatedAt, and the non-secret document without credentials

## Error Mainline

- missing authentication, missing document, malformed schema, inline/secret values, unknown references, and I/O failures are explicit errors
- stale If-Match returns 409 with the server current document and does not overwrite newer truth
- account ids map to SHA-256 filenames so caller-controlled identities never become filesystem paths
- missing account documents are explicit not-configured client outcomes, stale pushes return explicit 409 conflict with the server document, and transport/validation failures never fall back to a fake synced mirror

## Shared Multi-Reference Functions

- `validate_config_document`
  - owner: `crates/freehand-account-config/src/lib.rs`
  - purpose: enforce one non-secret schema admission boundary for PUT, disk restore, and safe projection
  - allowed callers: AccountConfigStore::get, AccountConfigStore::put, project_safe_document
  - related tests: cargo test -p freehand-account-config -- --nocapture
  - why shared: all account config ingress and egress must enforce exactly the same typed schema and secret boundary

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `validate_config_document` | `crates/freehand-account-config/src/lib.rs` | validate typed content, references, unique ids, env-name auth, and secret-free values | typed account config content | validated non-secret account config content or explicit error | AccountConfigStore::put / project_safe_document | validate_config_document | account_config_document | account_config_document | account_config_document.validate | bound |
| 02 | `AccountConfigStore::get` | `crates/freehand-account-config/src/lib.rs` | load and validate the latest document for one authenticated account | authenticated account id | stored account config or explicit not-found/corrupt/I/O error | get_config | AccountConfigStore::get | account_config_document | account_config_document | account_config_document.get | bound |
| 03 | `AccountConfigStore::put` | `crates/freehand-account-config/src/lib.rs` | validate, compare If-Match, compute revision/etag, atomically persist, then publish latest truth | authenticated account id plus candidate content plus If-Match | next stored revision or explicit conflict/validation/I/O error | put_config | AccountConfigStore::put | account_config_document | account_config_document | account_config_document.put | bound |
| 04 | `project_safe_document` | `crates/freehand-account-config/src/lib.rs` | project exactly the validated non-secret schema surface | account config content | credential-free config projection | config consumers | project_safe_document | account_config_document | account_config_document | account_config_document.project_safe | bound |
| 05 | `AccountConfigClient::pull` | `crates/freehand-account-config/src/client.rs` | resolve the authenticated Relay account and pull the latest account document with strict response revalidation | Relay base URL plus bearer access token | validated revisioned non-secret account document or explicit not-found/transport/validation error | dispatch_pull_account_config | AccountConfigClient::pull | account_config_document | account_config_document | account_config_document.pull | bound |
| 06 | `AccountConfigClient::push` | `crates/freehand-account-config/src/client.rs` | push one validated non-secret candidate with the local mirror etag and map stale revision responses to an explicit conflict carrying the server document | optional If-Match etag plus validated non-secret config content | validated next revision document or explicit conflict/transport/validation error | dispatch_push_account_config | AccountConfigClient::push | account_config_document | account_config_document | account_config_document.push | bound |
| 07 | `dispatch_pull_account_config` | `crates/freehand-runtime/src/lib.rs` | route PullAccountConfig through the Relay client and persist the synced or explicit not-configured device mirror before returning the runtime receipt | PullAccountConfig command envelope plus live selected Relay connection | account_config_pulled receipt or explicit unsupported/dispatch failure with persisted mirror status | RuntimeCommandDispatcher::dispatch | AccountConfigClient::pull / AccountConfigMirror::synced / AccountConfigMirror::not_configured | account_config_document | account_config_document | account_config_document.pull | bound |
| 08 | `dispatch_push_account_config` | `crates/freehand-runtime/src/lib.rs` | route PushAccountConfig through config.core export plus the Relay client and persist the synced or conflict device mirror before returning the runtime receipt | PushAccountConfig command envelope plus live selected Relay connection and local config | account_config_pushed receipt, explicit conflict with server document, or explicit dispatch failure without mirror mutation on local config rejection | RuntimeCommandDispatcher::dispatch | export_shared_account_config / AccountConfigClient::push / AccountConfigMirror::synced / AccountConfigMirror::conflict | account_config_document | account_config_document | account_config_document.push | bound |

## Sync Status Against Mainline Call

- schema validation, account isolation, revision/etag, If-Match conflict, atomic persistence, and safe projection are code-bound
- Relay authentication remains an injected host boundary; freehand-account-config has no dependency on relay, runtime, UI, provider, or config truth crates
- device-side pull/push, mirror persistence, not-configured, conflict, and unsupported-without-Relay paths are code-bound
- effective-config application into config.core remains a later client phase and is not claimed by this server owner
