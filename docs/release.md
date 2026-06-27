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

Ensure the install bin dir is on `PATH`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

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
