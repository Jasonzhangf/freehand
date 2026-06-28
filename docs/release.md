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

The GitHub release workflow calls the same script after the full gate, then uploads `dist/bin/*` and `dist/android/*`.

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

First-time install and start:

```bash
scripts/install-launchd.sh install
```

Restart without reinstall:

```bash
scripts/install-launchd.sh restart
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
