# Freehand Release And Global Install

## Release Truth

`scripts/release.sh` is the local release truth.

It runs:

1. `make ci`
2. `apps/freehand-android/gradlew testDebugUnitTest`
3. `cargo build --release -p freehand-cli -p freehand-server -p freehand-daemon`
4. `apps/freehand-android/gradlew assembleRelease`
5. artifact staging under `dist/`

Android release packaging disables Android release lint checks in `apps/freehand-android/app/build.gradle.kts`.
The release regression truth remains `make ci` plus Android JVM unit tests; APK packaging must produce the unsigned artifact and must not depend on the currently failing Android Lint Vital task.

Release artifacts:

- `dist/bin/freehand-cli`
- `dist/bin/freehand-server`
- `dist/bin/freehand-daemon`
- `dist/android/freehand-android-release-unsigned.apk`

Android update serving truth:

- the daemon serves `GET /android/update.json`
- the daemon serves `GET /android/freehand-android.apk`
- default APK source path is `dist/android/freehand-android-release-unsigned.apk`
- override APK path with runtime env `FREEHAND_ANDROID_APK_PATH`
- override manifest version fields with runtime env `FREEHAND_ANDROID_VERSION_CODE` and `FREEHAND_ANDROID_VERSION_NAME`
- Android clients may auto-check and auto-download this APK, but installation still goes through the Android system installer and requires user confirmation and any one-time unknown-sources permission

The GitHub release workflow calls the same script after the full gate, then uploads `dist/bin/*` and `dist/android/*`.

## Alpha Closeout Gate

Alpha promotion requires the release build plus one fixed-port WebUI online proof against the S-profile daemon. The S-profile is the default development/alpha validation surface and runs on `127.0.0.1:4042` through `freehand-*S` commands. The release profile stays on `127.0.0.1:4041` and is verified only by the explicit release target.

Run the alpha closeout sequence from the repo root:

```bash
scripts/install-launchd.sh installS
scripts/install-launchd.sh restartS
make verify-webui-online
```

`make verify-webui-online` is intentionally separate from `make ci` because it requires a running local S-profile daemon on `127.0.0.1:4042` and a local Chrome binary for real browser evidence. It is mandatory for alpha promotion and for WebUI/ADP/session lifecycle changes, but it is not a CI-safe static gate.

Release profile verification is explicit and must not be used as the default development WebUI gate:

```bash
scripts/install-global.sh
scripts/install-launchd.sh restart
make verify-webui-release-online
```

The default online gate performs a real browser flow through `http://127.0.0.1:4042/` and compares it with ADP truth through `freehand-cliS`:

1. creates a new WebUI conversation
2. submits a success sample and verifies the composer clears while the user input remains visible
3. waits for terminal success
4. submits a failed `read_file` sample that must continue through a second model round
5. verifies only the current live card animates and historical turns are static
6. waits for terminal success with `runtime-turn-N-r2`
7. refreshes the page and verifies both prompts remain visible
8. runs `freehand-cliS adp-session-query` for the same session
9. writes screenshots and `summary.json` under `artifacts/webui-online/<run-id>/`

Alpha cannot be claimed from `node --check`, unit tests, or static screenshots alone. The required evidence is the final S-profile `summary.json`, screenshots, fixed `4042` health, ADP truth, and matching served WebUI asset hash when code changed. Release `4041` proof is additional release-closeout evidence, not the default WebUI development gate.

Current alpha blockers that are explicitly outside the single-agent WebUI alpha scope must remain documented in `docs/architecture/architecture-gaps.md`:

- `control.center` / `error.center` / `task.orchestration` full framework orchestration
- `metadata.core` broader provider/debug producer coverage

If alpha scope expands to master/worker task delegation, `task.orchestration` stops being a documented gap and becomes a release blocker.

## Global Install

`scripts/install-global.sh` is the host global-install truth.

Default install prefix:

```bash
~/.local
```

Install:

```bash
scripts/install-global.sh
```

Override install prefix:

```bash
FREEHAND_PREFIX=/usr/local scripts/install-global.sh
```

Installed commands:

- `freehand-cli`
- `freehand-server`
- `freehand-daemon`
- `freehand-daemon-launchd`

Ensure the install bin dir is on `PATH`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

## Development Symlink Install

`scripts/install-symlink.sh` is the development install truth.

It builds debug host binaries and installs S-suffixed symlinks into the same prefix:

- `freehand-cliS` -> `target/debug/freehand-cli`
- `freehand-serverS` -> `target/debug/freehand-server`
- `freehand-daemonS` -> `target/debug/freehand-daemon`
- `freehand-daemon-launchdS` is a copied launchd wrapper under the install prefix

Install or refresh symlinks:

```bash
scripts/install-symlink.sh
```

This mode is for local development and unattended verification. Rebuilding the repo updates the symlink targets without replacing global release commands. Use the global install path only when promoting a stage to release.

## macOS Background Service

`scripts/install-launchd.sh install` performs first-time launchd setup: it installs the host binaries, writes a user LaunchAgent, and starts the daemon in the background.

`scripts/install-launchd.sh restart` restarts the existing LaunchAgent with `launchctl kickstart -k`, then waits for `/health` on the fixed bind to become ready before reporting success. It does not rewrite the install state.

Default service truth:

- label: `com.freehand.daemon`
- fixed WebUI URL: `http://127.0.0.1:4041/`
- plist: `~/Library/LaunchAgents/com.freehand.daemon.plist`
- daemon env: `~/.freehand/daemon.env`
- stdout log: `~/.freehand/logs/daemon.stdout.log`
- stderr log: `~/.freehand/logs/daemon.stderr.log`
- launchd policy: `RunAtLoad=true`, `KeepAlive=true`

Development symlink service truth:

- label: `com.freehand.daemonS`
- fixed WebUI URL: `http://127.0.0.1:4042/`
- plist: `~/Library/LaunchAgents/com.freehand.daemonS.plist`
- daemon env: `~/.freehand/daemonS.env`
- stdout log: `~/.freehand/logs/daemonS.stdout.log`
- stderr log: `~/.freehand/logs/daemonS.stderr.log`
- daemon binary: `$HOME/.local/bin/freehand-daemonS`

First-time install and start:

```bash
scripts/install-launchd.sh install
```

First-time development symlink service install and start:

```bash
scripts/install-launchd.sh installS
```

Restart without reinstall:

```bash
scripts/install-launchd.sh restart
```

Restart development symlink service without reinstall:

```bash
scripts/install-launchd.sh restartS
```

Both `install` and `restart` wait for the daemon to answer `GET /health` on the configured fixed bind before they exit successfully.

Status:

```bash
make launchd-status
```

Logs:

```bash
make launchd-logs
```

Uninstall:

```bash
scripts/uninstall-launchd.sh
```

Uninstall development symlink service:

```bash
scripts/uninstall-launchd.sh uninstallS
```

The LaunchAgent runs `freehand-daemon-launchd`, which loads `~/.freehand/daemon.env` before execing:

```bash
freehand-daemon serve --agent "$FREEHAND_DAEMON_AGENT" --bind "$FREEHAND_DAEMON_BIND"
```

Default `~/.freehand/daemon.env` values created on first install:

```bash
FREEHAND_DAEMON_AGENT="master"
FREEHAND_DAEMON_BIND="127.0.0.1:4041"
FREEHAND_DAEMON_WORKDIR="<repo root at install time>"
FREEHAND_DAEMON_BIN="$HOME/.local/bin/freehand-daemon"
FREEHAND_PAIR_TOKEN_SHARED="<generated or existing value>"
```

Default `~/.freehand/daemonS.env` values created on first symlink install:

```bash
FREEHAND_DAEMON_AGENT="master"
FREEHAND_DAEMON_BIND="127.0.0.1:4042"
FREEHAND_DAEMON_WORKDIR="<repo root at install time>"
FREEHAND_DAEMON_BIN="$HOME/.local/bin/freehand-daemonS"
FREEHAND_PAIR_TOKEN_SHARED="<generated or existing value>"
```

macOS does not require extra accessibility or full-disk permissions for the localhost WebUI service. If the bind address is later changed to `0.0.0.0:4041` or a LAN/Tailscale address, macOS firewall may ask for inbound-network approval once for the daemon binary.

If an existing `daemon.env` points `FREEHAND_DAEMON_BIN` at a different install prefix, `scripts/install-launchd.sh install` fails explicitly instead of silently running an old daemon binary.

## Daemon Startup

Runtime config truth remains:

```bash
~/.freehand/config.toml
```

Minimal local master/worker config:

```toml
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "chat_completions"
baseURL = "http://127.0.0.1:8000"
defaultModel = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
apiKey = "sk-local"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agent = "worker"
pair_token = "FREEHAND_PAIR_TOKEN_SHARED"
provider = "mini27"

[agents.worker]
name = "worker"
mode = "slave"
node_id = "worker-node"
paired_agent = "master"
pair_token = "FREEHAND_PAIR_TOKEN_SHARED"
provider = "mini27"
```

For the first local topology, paired agents must resolve the same pair token value:

```bash
export FREEHAND_PAIR_TOKEN_SHARED="dev-shared-token"
```

Start one configured agent:

```bash
freehand-daemon serve --agent master --bind 127.0.0.1:4041
```

Then open:

```text
http://127.0.0.1:4041/
```

The daemon serves WebUI and `/ui/*` routes from the same origin, so WebUI automatically talks to the daemon process that served it.

The daemon reads config-selected startup mode from `~/.freehand/config.toml`; config changes require restart.
