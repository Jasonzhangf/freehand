# Wiki: `relay.transport`

Generated from `docs/mainline-calls/relay.transport.json`. Do not edit by hand.

- owner crate: `crates/freehand-relay`
- owner module: `crates/freehand-relay/src/lib.rs`
- function map: `docs/function-maps/relay.transport.md`
- generated wiki: `docs/wiki/relay.transport.md`
- test design: `docs/testing/relay.transport.md`

## Resource Operation Backlinks

- relay_account.register
- relay_account.authenticate
- agent_presence.heartbeat
- agent_presence.query_directory
- agent_presence.subscribe_directory
- agent_presence.admit_control
- relay_control_tunnel.connect
- relay_control_tunnel.admit_data
- relay_control_tunnel.admit_error
- relay_data_tunnel.proxy_http
- relay_data_tunnel.proxy_adp
- relay_data_tunnel.proxy_websocket
- relay_data_tunnel.accept_generation
- relay_error_tunnel.correlate

## Request Mainline

- standalone process loads explicit bind, persisted-store, lease, and HTTP/TLS session-cookie policy before binding
- account register or login establishes one opaque account-scoped access token
- Agent control tunnel authenticates identity and persists account-scoped presence
- directory subscription and proxy requests authenticate before accessing presence
- transport opens typed data exchanges only through an authenticated control tunnel

## Response Mainline

- directory query and subscription project account-scoped online state, role, status, last seen, and Agent-reported active-session count
- HTTP, ADP, and generic WebSocket pass through opaque typed data frames without Relay ownership of Agent business truth
- restart restores persisted account, token hash, and Agent presence records

## Error Mainline

- invalid credentials and tokens fail explicitly
- cross-account, expired, and unknown Agent access fails explicitly
- corrupt store, failed persistence, and failed tunnel or local bridge IO fail explicitly

## Shared Multi-Reference Functions

- `RelayStore::authenticate`
  - owner: `crates/freehand-relay/src/store.rs`
  - purpose: provide one token-hash-to-account authority
  - allowed callers: crates/freehand-relay/src/service.rs
  - related tests: cargo test -p freehand-relay -- --nocapture
  - why shared: all authenticated Relay routes must resolve tokens through one account authority
- `RelayTunnelRegistry::open_routable_exchange`
  - owner: `crates/freehand-relay/src/tunnel.rs`
  - purpose: atomically require live data and error return attachments before creating one typed pending response channel
  - allowed callers: open_http_exchange, open_websocket_exchange
  - related tests: cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture
  - why shared: HTTP and WebSocket transports must share one account-scoped data/error admission owner without sharing payload semantics

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `register / RelayStore::register` | `crates/freehand-relay/src/service.rs / crates/freehand-relay/src/store.rs` | persist account password/token hashes and issue opaque token | username and password | authenticated account | register | RelayStore::register | relay_account | relay_account | relay_account.register | bound |
| 02 | `authenticated_account / RelayStore::authenticate` | `crates/freehand-relay/src/service.rs / crates/freehand-relay/src/store.rs` | resolve token hash to one account | request token | account id | authenticated_account | RelayStore::authenticate | relay_account | relay_account | relay_account.authenticate | bound |
| 03 | `heartbeat / RelayStore::heartbeat` | `crates/freehand-relay/src/service.rs / crates/freehand-relay/src/store.rs` | persist Agent presence | account and heartbeat | Agent presence | heartbeat | RelayStore::heartbeat | agent_presence | agent_presence | agent_presence.heartbeat | bound |
| 04 | `directory / RelayStore::directory` | `crates/freehand-relay/src/service.rs / crates/freehand-relay/src/store.rs` | project account Agent directory | account time and lease | Agent directory | directory | RelayStore::directory | agent_presence | agent_presence | agent_presence.query_directory | bound |
| 05 | `directory_subscription / project_directory` | `crates/freehand-relay/src/directory_socket.rs / crates/freehand-relay/src/service.rs` | authenticate and stream changed account-scoped Agent directory snapshots | account credential and presence revisions | typed directory snapshot or terminal frame | directory_subscription | project_directory | agent_presence | agent_presence | agent_presence.subscribe_directory | bound |
| 06 | `control_tunnel / attach_control` | `crates/freehand-relay/src/websocket_tunnel.rs` | validate and persist one authenticated Agent control identity, attach its generation-fenced control channel, then acknowledge identity admission before data/error channels can open | account and Agent identity | persisted generation-fenced control tunnel admission plus matching identity acknowledgement | control_tunnel | attach_control | agent_presence | relay_control_tunnel | agent_presence.admit_control | bound |
| 07 | `data_tunnel / attach_data / RelayTunnelRegistry::attach_data_for_control` | `crates/freehand-relay/src/websocket_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | admit one data channel only when its server-issued control generation still owns the authenticated Agent identity | authenticated Agent identity, server-issued control generation header, and typed data sender | control-generation-bound typed data tunnel or explicit stale-generation failure | data_tunnel | RelayTunnelRegistry::attach_data_for_control | relay_control_tunnel | relay_data_tunnel | relay_control_tunnel.admit_data | bound |
| 08 | `open_http_exchange / RelayTunnelRegistry::open_routable_exchange` | `crates/freehand-relay/src/http_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | open and stream one authenticated opaque HTTP exchange end to end, with one explicit pending-exchange failure owner | namespaced HTTP request | opaque HTTP response | open_http_exchange | RelayTunnelRegistry::open_routable_exchange | relay_data_tunnel | relay_data_tunnel | relay_data_tunnel.proxy_http | bound |
| 09 | `proxy_adp / proxy_websocket` | `crates/freehand-relay/src/websocket_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | open and stream one authenticated opaque ADP exchange | namespaced WebSocket upgrade | opaque bidirectional ADP frames | proxy_adp | proxy_websocket | relay_data_tunnel | relay_data_tunnel | relay_data_tunnel.proxy_adp | bound |
| 10 | `proxy_websocket_path / proxy_websocket` | `crates/freehand-relay/src/websocket_tunnel.rs` | validate a local-only target path and stream one authenticated opaque generic WebSocket exchange | namespaced generic WebSocket upgrade and local path | opaque bidirectional WebSocket frames | proxy_websocket_path | proxy_websocket | relay_data_tunnel | relay_data_tunnel | relay_data_tunnel.proxy_websocket | bound |
| 11 | `run_error_socket / RelayTunnelRegistry::fail_exchange_from_error_generation / RelayTunnelRegistry::fail_exchange` | `crates/freehand-relay/src/websocket_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | validate the authenticated error attachment generation before a typed Agent failure can remove the correlated exchange, deliver its exact failure, terminate malformed or stale error channels explicitly, and generation-fence cleanup | typed Agent error frame plus authenticated tunnel identity | correlated pending-exchange failure or explicit terminal error-channel failure | run_error_socket | RelayTunnelRegistry::fail_exchange_from_error_generation | relay_error_tunnel | relay_data_tunnel | relay_error_tunnel.correlate | bound |
| 12 | `main / RelayServerConfig::from_env / RelayService::serve` | `apps/freehand-relay-server/src/main.rs / crates/freehand-relay/src/config.rs / crates/freehand-relay/src/service.rs` | load explicit bind, store, lease, and cookie policy before serving standalone Relay | deployment environment | live Relay listener with fixed session-cookie policy | main | RelayService::serve |  |  |  | bound |
| 13 | `agent_tunnel_config_from_env / RelayAgentClient::run` | `apps/freehand-relay-server/src/main.rs / crates/freehand-relay/src/agent_client.rs` | load explicit Agent bridge configuration and enter the typed outbound tunnel lifecycle; its data-frame bridge requires remote scope for WebSocket and ADP opens, rejects scope on HTTP opens, and converts accepted scope to the local remote-access header | Agent deployment environment | live control, data, and error Agent tunnels | agent_tunnel_config_from_env | RelayAgentClient::run | relay_control_tunnel | relay_control_tunnel | relay_control_tunnel.connect | bound |
| 13a | `RelayAgentClient::run / RelayAgentClient::current_heartbeat` | `crates/freehand-relay/src/agent_client.rs` | read the typed Agent status and active-session count at control admission and every heartbeat; terminate the tunnel explicitly when the source fails | typed presence source closure | current authenticated Agent presence heartbeat or explicit source error | RelayAgentClient::run | RelayAgentClient::current_heartbeat | agent_presence | agent_presence | agent_presence.heartbeat | bound |
| 14 | `attach_error / RelayTunnelRegistry::admit_error_for_control` | `crates/freehand-relay/src/websocket_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | atomically admit one error channel only when its server-issued control generation still owns the authenticated Agent identity | authenticated Agent identity, server-issued control generation header, and typed error sender | control-generation-bound typed error tunnel or explicit stale-generation failure | attach_error | RelayTunnelRegistry::admit_error_for_control | relay_control_tunnel | relay_error_tunnel | relay_control_tunnel.admit_error | bound |
| 15 | `run_data_socket / RelayTunnelRegistry::accept_data_generation / RelayTunnelRegistry::accept_data` | `crates/freehand-relay/src/websocket_tunnel.rs / crates/freehand-relay/src/tunnel.rs` | validate the authenticated data attachment generation before delivering an inbound response frame to current pending-exchange truth | typed Agent response frame plus authenticated tunnel identity and attachment generation | current-generation correlated delivery or explicit stale-generation terminal failure | run_data_socket | RelayTunnelRegistry::accept_data_generation | relay_data_tunnel | relay_data_tunnel | relay_data_tunnel.accept_generation | bound |

## Sync Status Against Mainline Call

- account, token-hash persistence, Agent presence subscription, HTTP proxy, ADP proxy, generic WebSocket proxy, and standalone deployment host are code-bound
- legacy freehand-server Relay implementation is physically removed
- daemon remote-relay compatibility mode calls only the relay.transport public crate and reads bind/store/lease/cookie policy through RelayServerConfig env truth, not CLI bind defaults
- product config plus Master/Worker daemon outbound-client wiring and dynamic typed heartbeat projection are bound outside this module; Agent Dashboard and Android login remain incomplete
