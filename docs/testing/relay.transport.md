# Test Design: `relay.transport`

- feature_id: `relay.transport`
- owner: `crates/freehand-relay`
- resource map: `docs/resource-maps/core.json`
- module registry: `docs/module-registry/relay.transport.json`
- verification map: `docs/verification-maps/relay.transport.json`
- lifecycle path under test:
  - persisted store loads before listener startup
  - an operator explicitly initializes a versioned store before first startup
  - local-online verification starts the Agent WebUI process and proves its health before starting the Relay process
  - local-online verification fails immediately with the owning process log when either child exits before readiness
  - account registration/login issues account-scoped access
  - the standalone host requires an explicit cookie-security mode: HTTP mode omits `Secure`, while TLS mode emits it
  - authenticated Agent control identity persists role, status, active-session count, and freshness
  - directory query derives online state from freshness
  - an authenticated account directory WebSocket emits one initial snapshot and a new account-scoped snapshot after Agent admission, heartbeat state change, and disconnect
  - authenticated same-account control tunnels admit typed HTTP/ADP data pass-through
  - an authenticated caller opens `/relay/agents/{agent_id}/connect/{path}` and Relay forwards opaque WebSocket frames to the same-account Agent without defining the application protocol
  - every opened exchange has one terminal owner: success removes it on `ResponseEnd`, while request-send, request-body, response-open, protocol, client-disconnect, and Agent bridge failures remove it through the typed error chain
  - cancelling an active exchange wakes its receiver with the original failure, while cancelling an unknown or already-terminal exchange fails explicitly and cannot mutate another exchange
  - control identity acknowledgement precedes data/error admission; a mismatched, missing, or timed-out acknowledgement closes the Agent connection explicitly
  - process restart restores account/token/presence truth
  - canonical `make ci` executes both deployment and local-online smoke gates rather than only validating their command strings
- resource operations under test:
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
  - `relay_error_tunnel.correlate`

## Resource Operation Test Coverage

| resource operation | status | white-box coverage | module black-box coverage | project black-box coverage |
| --- | --- | --- | --- | --- |
| `relay_account.register` | bound | `cargo test -p freehand-relay store::tests::account_token_and_presence_survive_restart -- --nocapture` proves persisted hash-only account/token truth; `failed_persistence_does_not_publish_candidate_truth` proves failed durable writes cannot mutate live truth | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves registration, duplicate rejection, short-password rejection, explicit initialization, and strict corrupt/incomplete store loading | `scripts/verify-relay-deployment-smoke.sh` explicitly initializes, registers through the standalone binary, and inspects persisted secret isolation |
| `relay_account.authenticate` | bound | `cargo test -p freehand-relay store::tests::wrong_password_cross_account_and_expired_presence_are_rejected -- --nocapture` proves wrong-password and token isolation | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves Bearer/cookie success plus missing-token failure | `scripts/verify-relay-deployment-smoke.sh` logs in before and after restart with the same persisted account |
| Relay session cookie policy | bound | `cargo test -p freehand-relay config::tests::secure_cookie_accepts_only_explicit_boolean_values -- --nocapture` proves ambiguous values fail | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves HTTP cookie-only authentication and explicit TLS `Secure` emission | deployment smoke sets HTTP mode explicitly; production env example sets TLS mode explicitly |
| `agent_presence.heartbeat` | bound | `cargo test -p freehand-relay store::tests::account_token_and_presence_survive_restart -- --nocapture` proves persisted heartbeat recovery | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` posts authenticated heartbeat with role/status/count | `scripts/verify-relay-deployment-smoke.sh` posts a real-process heartbeat and verifies persistence after restart |
| `agent_presence.query_directory` | bound | `cargo test -p freehand-relay store::tests::wrong_password_cross_account_and_expired_presence_are_rejected -- --nocapture` proves same-account filter and offline lease projection | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves directory schema, status, count, and expiry | `scripts/verify-relay-deployment-smoke.sh` queries the standalone process directory before and after restart |
| `agent_presence.subscribe_directory` | bound | `cargo test -p freehand-relay --test relay_http_blackbox authenticated_directory_http_and_adp_proxy_are_account_isolated -- --nocapture` proves typed changed-snapshot projection | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves authenticated initial/online/offline snapshots, unauthenticated rejection, and cross-account silence | `scripts/verify-remote-relay-local-online.sh` verifies the deployed directory before generic protocol wiring |
| `agent_presence.admit_control` | bound | `cargo test -p freehand-relay -- --nocapture` proves control admission and disconnect projection | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves the outbound Agent becomes online only through control identity | `scripts/verify-relay-deployment-smoke.sh` proves authenticated Agent control admission |
| `relay_control_tunnel.connect` | bound | `cargo test -p freehand-relay -- --nocapture` proves the Agent client enters control identity admission before dependent data/error sockets and rejects mismatched admission | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves a real Agent-side outbound lifecycle establishes the authenticated control tunnel | `scripts/verify-relay-deployment-smoke.sh` starts the release Agent client and observes its live control-owned presence |
| `relay_control_tunnel.admit_data` | bound | `cargo test -p freehand-relay -- --nocapture` proves data admission is rejected before control | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves the client establishes data only after control | `scripts/verify-relay-deployment-smoke.sh` proves the real Agent client data channel |
| `relay_control_tunnel.admit_error` | bound | `cargo test -p freehand-relay tunnel::tests::error_attachment_requires_control_in_the_same_registry_mutation -- --nocapture` proves missing control rejects before error insertion and active control admits exactly one generation | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves the outbound client establishes its error return path only after control acknowledgement | `scripts/verify-relay-deployment-smoke.sh` proves the real Agent client keeps a control-owned error channel through proxied traffic |
| `relay_data_tunnel.proxy_http` | bound | `cargo test -p freehand-relay --test relay_http_blackbox authenticated_directory_http_and_adp_proxy_are_account_isolated -- --nocapture` proves Relay and local-ADP credential stripping in both directions, preservation of non-control request cookies, and byte-exact response streaming without content rewriting | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proxies real HTTP root/assets and invalid UTF-8 bytes, proves upstream `Set-Cookie` cannot cross into the Relay origin, and rejects cross-account access | `scripts/verify-relay-deployment-smoke.sh` proxies a real Agent HTTP surface through the release binary |
| `relay_data_tunnel.proxy_adp` | bound | `cargo test -p freehand-relay --test relay_http_blackbox authenticated_directory_http_and_adp_proxy_are_account_isolated -- --nocapture` proves frame conversion | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves a real bidirectional WebSocket echo and offline Agent rejection | `scripts/verify-relay-deployment-smoke.sh` validates deployment files and the module black-box ADP gate before promotion |
| `relay_data_tunnel.proxy_websocket` | bound | `cargo test -p freehand-relay -- --nocapture` proves generic target/query preservation, opaque binary forwarding, and backpressure beyond the 32-frame channel capacity | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves normal and abrupt caller close keep the Agent online for a second `/connect`, while cross-account, offline-Agent, and invalid-target access reject before upgrade | `scripts/verify-remote-relay-local-online.sh` verifies the deployed outbound data tunnel before distinct-network `/connect` smoke |
| `relay_error_tunnel.correlate` | bound | `cargo test -p freehand-relay tunnel::tests::failure_removes_only_the_active_exchange_and_unknown_failure_is_explicit -- --nocapture` proves exact exchange removal and explicit unknown failure; `duplicate_and_stale_error_attachments_cannot_replace_current_tunnel` proves duplicate admission and stale cleanup cannot replace/detach the current generation | `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture` proves Agent bridge failures reach the typed error attachment without becoming success | `scripts/verify-relay-deployment-smoke.sh` keeps the control, data, and error attachments live through a real proxied request |

## Positive And Negative Matrix

| state | positive lock | negative lock |
| --- | --- | --- |
| account | valid register/login survives restart | short password, duplicate username, wrong password, missing/invalid token fail |
| presence | heartbeat projects online role/status/count | cross-account read is empty; expired heartbeat projects offline and cannot proxy |
| presence stream | initial and changed snapshots remain account-scoped | unauthenticated/cross-account subscribers cannot observe another account's Agent changes |
| transport | same-account live HTTP/ADP round trip | unknown, cross-account, expired, malformed protocol, and unreachable upstream routes fail explicitly before success projection |
| generic WebSocket | same-account caller exchanges opaque text/binary frames through `/connect` | Relay never parses frames into session/task/provider truth and rejects invalid route/auth before upgrade |
| local WebSocket credential scope | ADP protocol injects the configured local ADP bearer only into `/adp` | generic `/connect/{path}` never receives the ADP bearer even when the Agent has one configured |
| verifier startup | upstream health admission precedes Relay process startup | an upstream or Relay process exit before readiness fails immediately instead of waiting out the full health timeout |
| persistence | explicit initialization and atomic valid store reloads | absent, incomplete, corrupt, duplicate-init, and failed-write stores do not create in-memory success truth |

## Exchange Lifecycle Positive/Negative Pairs

| direction | assertion | locked risk |
| --- | --- | --- |
| positive | `ResponseOpen -> ResponseChunk -> ResponseEnd` delivers the complete response and removes the pending exchange exactly once | successful traffic cannot be cleaned before terminal response truth |
| positive | an Agent error for an active exchange removes it and wakes the waiting HTTP/ADP receiver with that exact error | bridge failures cannot leave callers waiting forever |
| negative | a request-side send/body failure explicitly cancels the matching active exchange | client disconnects and broken data tunnels cannot leak pending state |
| negative | dropping a streamed HTTP response sends request-end and removes only that active exchange | browser/mobile SSE reconnects cannot leave the old Agent HTTP request or Relay pending truth alive |
| negative | downstream streamed-body closure is observed by the response pump, which marks the correlated exchange cancelled and awaits typed cancellation delivery plus Agent `ResponseEnd` | cancellation cleanup and error-channel failures cannot be silently converted to an inactive guard |
| negative | a cancelled HTTP exchange remains typed as cancelled until the Agent sends `ResponseEnd`; already-queued data is acknowledged without payload delivery | cross-socket cancellation races cannot turn a known terminal exchange into an unknown-frame failure that kills the shared Agent tunnel |
| negative | duplicate HTTP/ADP/generic WebSocket request-open, duplicate response-open, unknown response, and never-opened cancellation fail explicitly; cancellation racing after known local completion is idempotent | a duplicate Agent-side open cannot replace the active local exchange, while terminal cleanup races cannot kill the shared Agent tunnel |
| negative | data/error response frames must carry the same authenticated tunnel identity that opened the exchange | one account or Agent cannot inject terminal/data frames into another identity's predictable exchange id |
| negative | an HTTP response channel closing after `ResponseOpen` but before `ResponseEnd` fails the request | truncated upstream bytes cannot be projected as a successful HTTP response |
| negative | stale data-socket cleanup is conditioned on its attachment generation | an old socket cannot detach a newly reconnected Agent data tunnel |
| negative | duplicate control admission fails before replacement and rejected/stale control cleanup cannot detach the current generation | an unaffiliated control socket cannot disconnect the live Agent or its data/error tunnels |
| negative | duplicate error attachment fails before insertion and stale error-socket cleanup is generation-conditioned | a rejected or old error socket cannot replace or detach the current Agent error chain |
| negative | malformed, unknown-exchange, or undeliverable Agent data frames terminate that data attachment and fail its pending exchanges | protocol corruption cannot be ignored while remote clients remain waiting |
| negative | malformed, uncorrelated, unknown-exchange, or undeliverable Agent error frames terminate the error attachment with a concrete logged/typed terminal cause | error-chain mutation failures cannot be mistaken for a clean socket close |
| negative | unknown/late exchange frames are rejected without closing the shared data tunnel | one stale exchange cannot terminate unrelated concurrent exchanges |
| positive | every HTTP response streams opaque bytes with bounded channel backpressure, including non-UTF-8 bodies and UI content types | Relay cannot interpret business payloads or grow response memory without bound |
| positive | an Agent terminal error after response-open awaits bounded response capacity before delivery | slow consumers receive the real correlated terminal error instead of a truncated channel-close projection |
| positive | generic WebSocket close code/reason and raw encoded path/query bytes round-trip unchanged | opaque transport does not alter terminal or URI semantics |
| negative | malformed UTF-8 text/close-reason bytes and structurally incomplete close frames fail at both Relay and Agent decode boundaries | invalid payload bytes cannot be normalized with replacement characters and delivered as successful text |
| negative | WebSocket response errors after `ResponseOpen`, and response-channel closure before `ResponseEnd`, remain explicit error-chain failures | a failed or truncated WebSocket bridge cannot be projected as normal completion |
| positive | Relay client base URLs accept an origin, an arbitrary deployment prefix, or an already-mounted `/relay/` API root and build exactly one `relay/tunnel` suffix | accepted deployment URL shapes connect to the canonical tunnel route without duplicate path segments |
| negative | Relay client base URLs preserve path prefixes and reject query/fragment components | accepted configuration cannot silently connect to a different route |
| positive | explicit HTTP and TLS deployment modes emit their matching session-cookie attributes | direct HTTP tests remain usable while TLS production retains transport-only cookies |
| negative | cookie security is never inferred from forwarded headers or request payload | an untrusted caller cannot alter the server-owned authentication-cookie policy |
| negative | Relay and local-ADP authentication cookies are removed from proxied HTTP requests, and upstream `Set-Cookie` is removed from proxied responses | cloud-origin traffic cannot replay Relay credentials into an Agent or export an Agent-local bridge credential into the Relay origin |
| negative | data/error channels opened before a matching control identity acknowledgement are rejected | channel admission cannot race identity truth |
| negative | control disconnect racing error-channel admission is serialized by one registry mutation and cannot leave an error attachment without its control owner | the separate control-check/attach race cannot create an orphaned error chain |
| positive | raw percent-encoded Agent route segments are decoded and matched to the typed Agent identity before preserving the opaque target suffix | encoded Agent IDs cannot lose their intended HTTP/WebSocket target path |
| negative | malformed, mismatched, or structurally incomplete Agent route prefixes fail explicitly | route extraction cannot silently redirect a request to the Agent root |
| positive | a routable exchange is inserted only after the same identity has live data and error attachments, and returns both typed senders with pending response truth | every admitted exchange has its data path and error return path before `RequestOpen` |
| negative | data-only or error-only tunnel state rejects exchange admission without inserting pending truth | the data/error attachment race cannot strand an exchange after `ResponseOpen` |
| negative | raw pending insertion is private to the registry implementation and runtime callers plus architecture gates can enter only through `open_routable_exchange` | a later caller cannot bypass atomic data/error admission while maps and tests remain green |

## Known Gaps And Non-Goals

- This module does not own Agent startup config, daemon supervisor policy, WebUI Agent Dashboard, Android login, TLS certificate termination, session truth, or task/lifecycle truth.
- Product wiring begins only after all module checks, deployment smoke, architecture gates, and independent review pass.

## Outbound Tunnel Test Design

- Lifecycle manifest: `docs/lifecycles/relay-outbound-tunnel.json`.
- White-box: control/data/error types are distinct and contain no fields from the other channels; no type contains an upstream URL or caller-selected network destination.
- Positive module black-box: authenticated Agent control and data channels register one live identity; streamed HTTP request/response chunks preserve bytes and status; ADP frames remain opaque; restart reconnect replaces the old connection and closes pending work explicitly.
- Negative module black-box: wrong account token, mismatched Agent identity, cross-identity response/error injection, data-before-control, duplicate control without live-owner cleanup, expired/disconnected tunnel, malformed node order, unknown exchange, incomplete HTTP/WebSocket response, and Agent error all enter `RelayError*` and never project success.
- State assertions: disconnect makes presence offline; a stale connection cannot answer after replacement; request payload cannot change role/status/lease; control frames cannot become HTTP headers, body bytes, ADP frames, or metadata.
- Removal gate: `upstreamBaseUrl`, `agent_upstream`, `resolve_upstream`, `build_upstream_url`, `build_adp_url`, and Relay-originated connections to Agent-provided destinations are absent from source, maps, fixtures, and deployment scripts.
