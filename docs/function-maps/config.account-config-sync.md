# Function Map: `config.account-config-sync`

- feature_id: `config.account-config-sync`
- owner crate: `crates/freehand-account-config`
- owner module: `crates/freehand-account-config/src/lib.rs`
- resource map: `docs/resource-maps/core.json`
- mainline call source: `docs/mainline-calls/config.account-config-sync.json`
- generated wiki: `docs/wiki/config.account-config-sync.md`
- owner entry symbols:
  - `AccountConfigStore::new`
  - `AccountConfigStore::get`
  - `AccountConfigStore::put`
- `validate_config_document`
- `project_safe_document`
- `config_router`
- `AccountConfigClient::pull`
- `AccountConfigClient::push`
- `AccountConfigMirror::save_to_runtime_home`
- `dispatch_pull_account_config`
- `dispatch_push_account_config`

## Resource Map Binding

- owned resources: `account_config_document`
- touched resources: `account_config_document`
- resource operations: `account_config_document.validate`, `account_config_document.get`, `account_config_document.put`, `account_config_document.project_safe`, `account_config_document.pull`, `account_config_document.push`
- operation source/target: `account_config_document` -> `account_config_document`
- forbidden shortcuts: config documents must not enter Relay account/tunnel/presence stores, whole-machine config uploads, ADP/business payloads, WebUI local state, provider credentials, or Android native config truth.
- host boundary: `apps/freehand-relay-server` authenticates one Relay account and passes only its account id through `AccountAuthenticator`; raw Relay tokens/cookies never enter this crate's document or persistence API.

## Request Mainline

- Relay host authenticates one account before invoking `/relay/api/config`.
- GET resolves only that account's SHA-256-addressed document file.
- PUT decodes a strict typed schema, rejects unknown/secret-bearing fields, and validates all provider/model/endpoint references.
- Existing documents require exact `If-Match`; first write accepts no matcher.
- The owner computes the next revision, canonical content etag, and updated timestamp.

## Response Mainline

- GET and PUT return schemaVersion, revision, etag, updatedAt, and typed non-secret content.
- HTTP `ETag` equals the document etag.
- A successful write fsyncs and atomically renames before in-memory latest truth changes.
- Safe projection returns only credential-free schema fields.

## Error Mainline

- Missing authentication is 401; missing account document is 404.
- Stale `If-Match` is 409 with the complete current server document.
- Unknown fields, inline credentials, secret-shaped values, absolute paths, duplicate ids, and unknown references fail before persistence.
- Corrupt stored JSON, mismatched stored etag, and persistence errors remain explicit and never publish candidate truth.

## Shared Multi-Reference Functions

- `validate_config_document`
  - owner: `crates/freehand-account-config/src/lib.rs`
  - callers: PUT, disk restore, safe projection
  - purpose: one schema and secret-boundary admission path

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `validate_config_document` | `crates/freehand-account-config/src/lib.rs` | validate schema and secret boundary | typed document content | validated content or error | `AccountConfigStore::put` / `project_safe_document` | `validate_config_document` | `account_config_document` | `account_config_document` | `account_config_document.validate` | bound |
| 02 | `AccountConfigStore::get` | `crates/freehand-account-config/src/lib.rs` | load one account document | account id | stored document or error | `get_config` | `AccountConfigStore::get` | `account_config_document` | `account_config_document` | `account_config_document.get` | bound |
| 03 | `AccountConfigStore::put` | `crates/freehand-account-config/src/lib.rs` | validate, compare, persist, publish | account id + content + If-Match | next revision or error | `put_config` | `AccountConfigStore::put` | `account_config_document` | `account_config_document` | `account_config_document.put` | bound |
| 04 | `project_safe_document` | `crates/freehand-account-config/src/lib.rs` | project non-secret content | typed content | safe content | config consumers | `project_safe_document` | `account_config_document` | `account_config_document` | `account_config_document.project_safe` | bound |
| 05 | `AccountConfigClient::pull` | `crates/freehand-account-config/src/client.rs` | pull and revalidate the authenticated account document | Relay URL plus bearer token | validated server document or explicit not-found/error | `dispatch_pull_account_config` | `AccountConfigClient::pull` | `account_config_document` | `account_config_document` | `account_config_document.pull` | bound |
| 06 | `AccountConfigClient::push` | `crates/freehand-account-config/src/client.rs` | push validated non-secret content with mirror etag and preserve explicit conflicts | candidate content plus optional If-Match | next document or conflict/error | `dispatch_push_account_config` | `AccountConfigClient::push` | `account_config_document` | `account_config_document` | `account_config_document.push` | bound |
| 07 | `AccountConfigMirror::save_to_runtime_home` | `crates/freehand-account-config/src/mirror.rs` | atomically persist device-side sync status and non-secret document mirror | mirror plus runtime home | durable mirror or explicit I/O error | runtime dispatch | `AccountConfigMirror::save_to_runtime_home` | `account_config_document` | `account_config_document` | `account_config_document.pull` | bound |
| 08 | `dispatch_pull_account_config` | `crates/freehand-runtime/src/lib.rs` | route explicit pull and project its mirror status | PullAccountConfig envelope plus live Relay connection | pull receipt or explicit dispatch failure | `RuntimeCommandDispatcher::dispatch` | `AccountConfigClient::pull / AccountConfigMirror::*` | `account_config_document` | `account_config_document` | `account_config_document.pull` | bound |
| 09 | `dispatch_push_account_config` | `crates/freehand-runtime/src/lib.rs` | export local non-secret config, push with mirror etag, and persist success/conflict | PushAccountConfig envelope plus local config | push receipt or explicit conflict/failure | `RuntimeCommandDispatcher::dispatch` | `export_shared_account_config / AccountConfigClient::push / AccountConfigMirror::*` | `account_config_document` | `account_config_document` | `account_config_document.push` | bound |

## Sync Status Against Mainline Call

- Code symbols and resource operations are bound to `docs/mainline-calls/config.account-config-sync.json`.
- Client-side effective-config application is not owned or claimed here.
