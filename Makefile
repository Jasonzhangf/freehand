.PHONY: provision-openminis-source build fmt clippy test mainlines gates relay-deployment-smoke relay-local-online relay-account-config-smoke launchd-guard-offline launchd-guard-online launchd-guards ci verify-webui-online verify-webui-surface-motion-online verify-webui-release-online release install-global install-symlink install-launchd install-launchdS install-worker-launchd install-worker-launchdS restart-launchd restart-launchdS restart-worker-launchd restart-worker-launchdS uninstall-launchd uninstall-launchdS uninstall-worker-launchd uninstall-worker-launchdS launchd-status launchd-statusS worker-launchd-status worker-launchd-statusS launchd-logs launchd-logsS worker-launchd-logs worker-launchd-logsS hooks
.PHONY: dev pre-push-fast nightly

# Build/test tiers (from fastest to slowest):
#   make dev           - fast inner loop: workspace build + fmt + targeted core
#                        tests + mainlines + gates. Use while iterating.
#   make pre-push-fast - personal-branch pre-push: dev + workspace clippy +
#                        relay-deployment-smoke. Skips full workspace test.
#   make ci            - full release-candidate gate: build + fmt + clippy +
#                        workspace test + mainlines + gates + relay smokes.
#                        This is what .githooks/pre-push runs by default.
#   make nightly       - ci + webui online verifiers. Run on the nightly cron.
#   make release       - full release: ci + Android JVM regression + release
#                        binaries + Android APK (invoked separately).

provision-openminis-source:
	scripts/provision-openminis-source.sh

build:
	cargo build --workspace

fmt:
	cargo fmt --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test: provision-openminis-source
	cargo test --workspace

mainlines:
	cargo run -p xtask -- mainlines check

gates: provision-openminis-source
	cargo run -p xtask -- gates check

relay-deployment-smoke:
	scripts/verify-relay-deployment-smoke.sh

relay-local-online:
	scripts/verify-remote-relay-local-online.sh

relay-account-config-smoke:
	scripts/verify-relay-account-config-smoke.sh

launchd-guard-offline:
	@if [ "$$(uname -s)" = "Darwin" ]; then \
		bash scripts/verify-launchd-restart-guard.sh; \
	else \
		printf '%s\n' 'launchd_restart_guard_offline_not_applicable platform='"$$(uname -s)"; \
	fi

launchd-guard-online:
	@if [ "$$(uname -s)" = "Darwin" ]; then \
		bash scripts/verify-launchd-restart-guard-online.sh; \
	else \
		printf '%s\n' 'launchd_restart_guard_online_not_applicable platform='"$$(uname -s)"; \
	fi

launchd-guards: launchd-guard-offline launchd-guard-online

dev: provision-openminis-source
	cargo build --workspace
	cargo fmt --check
	cargo test -p freehand-acp -p freehand-runtime -- --nocapture
	cargo run -p xtask -- mainlines check
	cargo run -p xtask -- gates check

pre-push-fast: provision-openminis-source
	cargo build --workspace
	cargo fmt --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test -p freehand-acp -p freehand-runtime
	cargo run -p xtask -- mainlines check
	cargo run -p xtask -- gates check
	scripts/verify-relay-deployment-smoke.sh

ci: provision-openminis-source build fmt clippy test mainlines gates relay-deployment-smoke relay-local-online relay-account-config-smoke launchd-guards

nightly: ci verify-webui-online verify-webui-surface-motion-online verify-webui-release-online

verify-webui-online:
	scripts/verify-webui-online.sh

verify-webui-surface-motion-online:
	node scripts/verify-webui-surface-motion-online.mjs

verify-webui-release-online:
	scripts/verify-webui-release-online.sh

release:
	scripts/release.sh

install-global:
	scripts/install-global.sh

install-symlink:
	scripts/install-symlink.sh

install-launchd:
	scripts/install-launchd.sh install

install-launchdS:
	scripts/install-launchd.sh installS

install-worker-launchd:
	scripts/install-launchd.sh installWorker

install-worker-launchdS:
	scripts/install-launchd.sh installWorkerS

restart-launchd:
	scripts/install-launchd.sh restart

restart-launchdS:
	scripts/install-launchd.sh restartS

restart-worker-launchd:
	scripts/install-launchd.sh restartWorker

restart-worker-launchdS:
	scripts/install-launchd.sh restartWorkerS

uninstall-launchd:
	scripts/uninstall-launchd.sh

uninstall-launchdS:
	scripts/uninstall-launchd.sh uninstallS

uninstall-worker-launchd:
	scripts/uninstall-launchd.sh uninstallWorker

uninstall-worker-launchdS:
	scripts/uninstall-launchd.sh uninstallWorkerS

launchd-status:
	launchctl print gui/$$(id -u)/com.freehand.daemon

launchd-statusS:
	launchctl print gui/$$(id -u)/com.freehand.daemonS

worker-launchd-status:
	launchctl print gui/$$(id -u)/com.freehand.worker

worker-launchd-statusS:
	launchctl print gui/$$(id -u)/com.freehand.workerS

launchd-logs:
	tail -n 80 "$$HOME/.freehand/logs/daemon.stdout.log" "$$HOME/.freehand/logs/daemon.stderr.log"

launchd-logsS:
	tail -n 80 "$$HOME/.freehand/logs/daemonS.stdout.log" "$$HOME/.freehand/logs/daemonS.stderr.log"

worker-launchd-logs:
	tail -n 80 "$$HOME/.freehand/logs/worker.stdout.log" "$$HOME/.freehand/logs/worker.stderr.log"

worker-launchd-logsS:
	tail -n 80 "$$HOME/.freehand/logs/workerS.stdout.log" "$$HOME/.freehand/logs/workerS.stderr.log"

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit .githooks/pre-push
