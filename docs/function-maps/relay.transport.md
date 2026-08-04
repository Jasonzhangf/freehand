# Function Map: `relay.transport`

- feature_id: `relay.transport`
- owner crate: `crates/freehand-relay`
- owner module: `crates/freehand-relay/src/lib.rs`
- process host: `apps/freehand-relay-server/src/main.rs`
- mainline call source: `docs/mainline-calls/relay.transport.json`
- generated wiki: `docs/wiki/relay.transport.md`
- test design: `docs/testing/relay.transport.md`
- module registry: `docs/module-registry/relay.transport.json`
- verification map: `docs/verification-maps/relay.transport.json`
- entry symbols:
  - `RelayStore::load`
  - `RelayStore::register`
  - `RelayStore::login`
  - `RelayStore::authenticate`
  - `RelayStore::heartbeat`
  - `RelayStore::directory`
  - `directory_subscription`
  - `RelayService::new`
  - `RelayService::router`
  - `RelayService::serve`
  - `RelayAgentClient::new_with_presence_source`
  - `RelayAgentClient::current_heartbeat`
  - `RelayAgentClient::run`
  - `control_tunnel`
  - `data_tunnel`
  - `error_tunnel`
  - `proxy_http_inner`
  - `proxy_adp`
  - `proxy_websocket_path`
  - `proxy_websocket`
  - `run_error_socket`
  - `RelayTunnelRegistry::open_routable_exchange`
  - `RelayTunnelRegistry::fail_exchange`
  - `RelayTunnelRegistry::accept_data_generation`
  - `RelayTunnelRegistry::fail_exchange_from_error_generation`
  - `RelayTunnelRegistry::attach_data_for_control`
  - `RelayTunnelRegistry::admit_error_for_control`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `relay_account`
  - `agent_presence`
  - `relay_control_tunnel`
  - `relay_data_tunnel`
  - `relay_error_tunnel`
- touched resources:
  - `relay_account`
  - `agent_presence`
  - `relay_control_tunnel`
  - `relay_data_tunnel`
  - `relay_error_tunnel`
- resource operations:
  - `relay_account.register`
  - `relay_account.authenticate`
  - `agent_presence.heartbeat`
  - `agent_presence.query_directory`
  - `agent_presence.subscribe_directory`
  - `agent_presence.admit_control`
  - `relay_control_tunnel.connect`
  - `relay_control_tunnel.admit_data`
  - `relay_control_tunnel.admit_error`
  - `relay_data_tunnel.proxy_http`
  - `relay_data_tunnel.proxy_adp`
  - `relay_data_tunnel.proxy_websocket`
  - `relay_data_tunnel.accept_generation`
  - `relay_error_tunnel.correlate`
- forbidden shortcuts:
  - config, daemon, WebUI, and Android must not own password hashing, token persistence, Agent presence, or route authorization
  - Relay must not read, infer, or mutate Agent session, task, lifecycle, provider, or model truth
  - control, data, and error semantics must remain on their typed channels; direct client-selected upstream URLs are forbidden

## Request Mainline

- standalone process requires an explicit persisted-store path and explicit HTTP/TLS session-cookie mode before binding one Relay listener
- registration validates username/password, stores Argon2 password hash plus token hash, and returns the raw opaque token once
- login verifies the Argon2 hash and issues a new opaque token whose hash is persisted
- API and proxy requests authenticate Bearer or HttpOnly-cookie credentials to one account id; the deployment-owned cookie mode controls only the `Secure` response attribute
- Agent control identity validates Agent identity, role, work status, and active-session count projection before atomic presence persistence
- directory query and subscription scope records to the authenticated account and derive online state from heartbeat lease freshness
- control admission returns a server-issued control generation; the Agent must echo it only in the typed data/error channel handshake header, and Relay admits those channels only while that exact control generation still owns the account/Agent identity
- control, data, and error attachments are generation-fenced, so stale control owners cannot attach new dependent channels, stale data frames cannot mutate current exchanges, stale error frames cannot fail current exchanges, and stale socket cleanup cannot detach the current channel
- HTTP/ADP/generic WebSocket requests open typed exchanges to the Agent's outbound data tunnel; Relay never connects to an Agent-provided destination
- control identity is acknowledged before the Agent opens data/error channels, and the Agent streams HTTP request chunks directly into the local request body instead of assembling a full request in memory
- Agent-side local WebSocket request construction retains the typed ADP/generic discriminator; only ADP may receive the configured local bearer
- Relay channel URL construction preserves arbitrary deployment prefixes and mounts an already-present `/relay/` API root exactly once

## Response Mainline

- account APIs return account id, normalized username, and opaque token without password/hash fields
- Agent directory query and subscription return one row per registered Agent with online state, role, status, last seen, and Agent-reported active-session count
- HTTP proxy strips Relay and Agent-local authentication cookies at the trust boundary, preserves non-control request semantics, blocks upstream `Set-Cookie` from entering the Relay origin, and streams every response body as opaque bytes without content-type inspection or path rewriting
- ADP and generic WebSocket proxies forward opaque frame kinds and bytes bidirectionally without parsing payload semantics
- standalone deployment package contains the binary host, systemd unit, and non-secret env schema

## Error Mainline

- invalid credentials, duplicate username, missing/invalid token, malformed heartbeat, cross-account Agent access, expired presence, and unknown Agent return explicit HTTP errors
- corrupt store fails process startup; store writes, tunnel disconnects, and local bridge errors fail explicitly through their owning chain
- routable exchange admission atomically requires both the same identity's data attachment and typed error return attachment before pending truth is inserted; every later request-side send/body/protocol/client-disconnect failure closes the matching pending exchange through `RelayTunnelRegistry::fail_exchange` or explicit streamed-response cancellation
- terminal error delivery awaits bounded capacity, and unknown/already-terminal exchange errors fail explicitly
- Agent tunnel failures enter `run_error_socket`, which binds the authenticated error identity and attachment generation through `RelayTunnelRegistry::fail_exchange_from_error_generation` before the unique `RelayTunnelRegistry::fail_exchange` owner mutates pending truth; malformed, stale, uncorrelated, unknown, or undeliverable error frames emit/log one concrete terminal cause before generation-conditioned error attachment cleanup
- a WebSocket response error after `ResponseOpen` remains an explicit error-chain termination; channel closure before `ResponseEnd` cannot be projected as successful completion
- malformed UTF-8 text or close-reason bytes fail at the receiving WebSocket decode boundary and enter the correlated typed error chain; Relay never normalizes invalid bytes into a successful text payload
- expired presence remains visible as offline in directory but cannot be proxied
- Relay errors never become Agent session/task success truth
- streamed HTTP client disconnect is observed by the async response pump, which marks only the correlated exchange cancelled, sends `RelayErrorOut04ClientCancellation`, and retains typed cancellation truth until the Agent's data-channel `ResponseEnd`; cancellation never enters HTTP/ADP/WebSocket payload bytes

## Shared Multi-Reference Functions

- `RelayStore::authenticate`
  - owner: `crates/freehand-relay/src/store.rs`
  - callers: account identity query, heartbeat, directory, HTTP proxy, ADP proxy
  - reason: one token-hash-to-account authority prevents route-specific authentication copies
- `RelayTunnelRegistry::open_routable_exchange`
  - owner: `crates/freehand-relay/src/tunnel.rs`
  - callers: `open_http_exchange` and `open_websocket_exchange`
  - reason: both transports require one account-scoped exchange registry with live data and error return attachments before pending insertion, while retaining distinct payload contracts
- `RelayTunnelRegistry::fail_exchange`
  - owner: `crates/freehand-relay/src/tunnel.rs`
  - callers: HTTP request owner, ADP request owner, Agent error tunnel, and data/control disconnect cleanup
  - reason: one terminal owner removes pending truth and wakes the matching receiver without route-local cleanup copies

## Call Table

Deployment uses two explicit commands: `freehand-relay-server init-store` initializes the versioned store once, then `freehand-relay-server serve` requires bind, store, and lease environment values and refuses to create missing truth.

| step | symbol | path | responsibility | input | output | caller | callee | binding |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `register / RelayStore::register` | `crates/freehand-relay/src/service.rs / crates/freehand-relay/src/store.rs` | persist password/token hashes and issue opaque token | username/password | authenticated account | `register` | `RelayStore::register` | bound |
| 02 | `authenticated_account / RelayStore::authenticate` | `crates/freehand-relay/src/service.rs / crates/freehand-relay/src/store.rs` | resolve token hash to account | raw request token | account id | `authenticated_account` | `RelayStore::authenticate` | bound |
| 03 | `heartbeat / RelayStore::heartbeat` | `crates/freehand-relay/src/service.rs / crates/freehand-relay/src/store.rs` | persist account-scoped Agent presence | authenticated account plus heartbeat | Agent presence | `heartbeat` | `RelayStore::heartbeat` | bound |
| 04 | `directory / RelayStore::directory` | `crates/freehand-relay/src/service.rs / crates/freehand-relay/src/store.rs` | project sorted online/offline Agent rows | account/time/lease | Agent directory | `directory` | `RelayStore::directory` | bound |
| 05 | `directory_subscription / project_directory` | `crates/freehand-relay/src/directory_socket.rs / crates/freehand-relay/src/service.rs` | stream account-scoped directory revisions | authenticated account and presence revision | current Agent directory projection | `directory_subscription` | `project_directory` | bound |
| 06 | `control_tunnel / attach_control` | `crates/freehand-relay/src/websocket_tunnel.rs` | admit generation-fenced authenticated Agent control identity | account/Agent identity | generation-fenced control admission | `control_tunnel` | `attach_control` | bound |
| 07 | `data_tunnel / attach_data / RelayTunnelRegistry::attach_data_for_control` | `crates/freehand-relay/src/websocket_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | admit data only while the server-issued control generation still owns the authenticated Agent identity | authenticated Agent identity, control generation header, and typed data sender | control-generation-bound typed data tunnel or explicit stale-generation failure | `data_tunnel` | `RelayTunnelRegistry::attach_data_for_control` | bound |
| 08 | `open_http_exchange / RelayTunnelRegistry::open_routable_exchange` | `crates/freehand-relay/src/http_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | atomically require data/error return paths before streaming one opaque HTTP exchange | authenticated namespaced request | routable HTTP exchange | `open_http_exchange` | `RelayTunnelRegistry::open_routable_exchange` | bound |
| 09 | `proxy_adp / proxy_websocket` | `crates/freehand-relay/src/websocket_tunnel.rs` | validate the ADP target and enter the shared WebSocket proxy owner | namespaced ADP upgrade | opaque bidirectional WebSocket exchange | `proxy_adp` | `proxy_websocket` | bound |
| 10 | `proxy_websocket_path / proxy_websocket` | `crates/freehand-relay/src/websocket_tunnel.rs` | validate a local-only generic WebSocket target and enter the shared proxy owner | namespaced generic WebSocket upgrade and local path | opaque bidirectional WebSocket exchange | `proxy_websocket_path` | `proxy_websocket` | bound |
| 11 | `run_error_socket / RelayTunnelRegistry::fail_exchange_from_error_generation / RelayTunnelRegistry::fail_exchange` | `crates/freehand-relay/src/websocket_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | generation-fence an authenticated Agent error before correlating it to exactly one pending data exchange | typed Agent error frame, tunnel identity, and attachment generation | exact pending-exchange failure or explicit stale/terminal channel failure | `run_error_socket` | `RelayTunnelRegistry::fail_exchange_from_error_generation` | bound |
| 12 | `main / RelayServerConfig::from_env / RelayService::serve` | `apps/freehand-relay-server/src/main.rs / crates/freehand-relay/src/config.rs / crates/freehand-relay/src/service.rs` | load explicit bind/store/lease/cookie-policy env and serve | deployment env | live listener with fixed cookie policy | `main` | `RelayService::serve` | bound |
| 13 | `agent_tunnel_config_from_env / RelayAgentClient::run` | `apps/freehand-relay-server/src/main.rs / crates/freehand-relay/src/agent_client.rs` | load explicit Agent bridge config and enter the typed outbound tunnel lifecycle through authenticated control admission | Agent deployment env | live control-owned lifecycle with dependent data/error Agent tunnels | `agent_tunnel_config_from_env` | `RelayAgentClient::run` | bound (`relay_control_tunnel.connect`: `relay_control_tunnel` -> `relay_control_tunnel`) |
| 13a | `RelayAgentClient::run / RelayAgentClient::current_heartbeat` | `crates/freehand-relay/src/agent_client.rs` | read typed status/count at identity admission and every control heartbeat; source failure terminates the tunnel instead of reusing stale projection | typed presence source closure | current authenticated Agent heartbeat or explicit source error | `RelayAgentClient::run` | `RelayAgentClient::current_heartbeat` | bound (`agent_presence.heartbeat`: `agent_presence` -> `agent_presence`) |
| 14 | `attach_error / RelayTunnelRegistry::admit_error_for_control` | `crates/freehand-relay/src/websocket_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | admit error only while the server-issued control generation still owns the authenticated Agent identity | authenticated Agent identity, control generation header, and typed error sender | control-generation-bound typed error tunnel or explicit stale-generation failure | `attach_error` | `RelayTunnelRegistry::admit_error_for_control` | bound (`relay_control_tunnel.admit_error`: `relay_control_tunnel` -> `relay_error_tunnel`) |
| 15 | `run_data_socket / RelayTunnelRegistry::accept_data_generation / RelayTunnelRegistry::accept_data` | `crates/freehand-relay/src/websocket_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | generation-fence an authenticated Agent response before it can mutate current pending-exchange truth | typed Agent response frame, tunnel identity, and attachment generation | current-generation correlated delivery or explicit stale terminal failure | `run_data_socket` | `RelayTunnelRegistry::accept_data_generation` | bound |
