# Provider Edit Batch 3 Audit

- run_id: `2026-07-07-provider-edit-batch3-audit`
- objective source: `/Users/fanzhang/.codex/attachments/b6e96891-fd6f-4792-bc4e-88d719409faa/pasted-text-1.txt`
- scope: audit owner-backed provider/model edit completion against current code and evidence
- mode: implementation audit plus one owner-scoped UI leakage fix

## Requirement Audit

| Requirement | Evidence | Result |
| --- | --- | --- |
| Owner chain is `app.webui-smoke -> ui.protocol -> runtime.ui-command-dispatch -> config.core -> canonical config -> safe projection -> WebUI` | `docs/function-maps/config.core.md`, `docs/function-maps/ui.protocol.md`, `docs/function-maps/runtime.ui-command-dispatch.md`, `docs/function-maps/app.webui-smoke.md`; code symbols `UiCommand::UpdateProviderConfig`, `RuntimeCommandDispatcher::dispatch_update_provider_config`, `update_provider_config_in_path`, `submitProviderConfigUpdate` | Proven |
| WebUI does not parse/write TOML directly | `apps/freehand-server/assets/webui.js` uses `adpCommand({ UpdateProviderConfig })` and `adpQuery("QueryConfigStatus")`; server asset smoke rejects direct config helpers | Proven |
| Config write validates before persistence and preserves old config on invalid input | `update_provider_config_in_path` validates provider id/base URL/type/protocol/model/env, reparses selected config before `persist_config_atomically`; tests `freehand-config` and runtime invalid no-overwrite test passed | Proven |
| Persistence is atomic | `persist_config_atomically` writes temp file, syncs, then renames | Proven |
| Valid save projects restart-required and does not hot reload active runtime | runtime stores `pending_config_status` and returns `provider_config_saved_restart_required`; test `runtime_dispatch_updates_provider_config_without_hot_reloading_active_model` passed | Proven |
| Invalid save shows visible error | online proof `artifacts/webui-online/20260707-verify-4042-1783399147569/summary.json` has `settingsInvalidUpdateVisible=true` | Proven |
| Valid env-var save shows restart-required | same online proof has `settingsValidUpdateRestartRequired=true`; `settingsProviderSaveStatus` is `Saved. Restart required before active runtime changes.` | Proven |
| Credential/API key not projected or DOM-visible | online proof has `settingsUpdateNoSecretLeak=true`; WebUI has no password input and verifier scans Settings text for secret/API-key patterns | Proven for Settings surface |
| User-visible status must not leak internal owner/status tokens | Audit found prior leakage: `commandStatus` showed `provider_config_saved_restart_required -> config.core`; fixed in this audit by mapping the provider save receipt to `Provider config saved. Restart required.`, rejecting unexpected receipt statuses instead of rendering a generic success fallback, and adding verifier assertions | Fixed and proven |
| S-profile online proof on fixed `127.0.0.1:4042` | `make verify-webui-online` passed after temporary env-var setup and config/env restoration; latest artifact `artifacts/webui-online/20260707-verify-4042-1783399680000/summary.json` | Proven |
| Real config/env restored after validation | post-restore `freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp` returned `auth_source=inline` | Proven |

## Fix Applied During Audit

Problem:

- WebUI provider config save succeeded, but the command status line displayed internal runtime strings: `provider_config_saved_restart_required -> config.core`.
- This is not a credential leak, but it violates the product/UI rule that users should see service/config semantics, not internal owner/status ids.

Change:

- `apps/freehand-server/assets/webui.js` now maps provider config receipts through `providerConfigReceiptStatus`.
- Unknown provider config receipt statuses now raise `Config save returned an unexpected service status.` instead of being displayed as a successful save.
- `apps/freehand-server/src/lib.rs` asset smoke locks the helper and forbids the old provider-save status template.
- `scripts/webui_verify_online.mjs` asserts the valid provider update state does not include `provider_config_saved_restart_required` or `config.core` in `commandStatus`.

## Validation

- `node --check apps/freehand-server/assets/webui.js`
- `node --check scripts/webui_verify_online.mjs`
- `cargo test -p freehand-server root_and_asset_routes_return_webui_shell_files -- --nocapture`
- `cargo test -p freehand-config -- --nocapture`
- `cargo test -p freehand-ui-protocol -- --nocapture`
- `cargo test -p freehand-runtime runtime_dispatch_updates_provider_config_without_hot_reloading_active_model -- --nocapture`
- `cargo test -p freehand-runtime runtime_dispatch_rejects_invalid_provider_config_without_overwrite -- --nocapture`
- `cargo test -p freehand-cli -- --nocapture`
- `cargo fmt --check`
- `git diff --check`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- `scripts/install-launchd.sh restartS`
- `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
- `make verify-webui-online`

## Evidence

- Online proof: `artifacts/webui-online/20260707-verify-4042-1783399680000/summary.json`
- Valid save command status: `Provider config saved. Restart required.`
- Invalid save visible error: `Save failed: dispatch port failure: provider ... base_url must be an http(s) URL with a host`
- Post-restore config query: `auth_source=inline`

## Remaining Non-Batch-3 Gaps

- General WebUI command status still has other locations that can display raw dispatch status/feature ids for non-provider commands. This audit fixed the provider-settings path only.
- Release 4041 and Android true-device proof are separate Batch 4 surfaces, not required for the Batch 3 S-profile objective.
