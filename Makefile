.PHONY: build fmt clippy test mainlines gates ci verify-webui-online verify-webui-release-online release install-global install-symlink install-launchd install-launchdS install-worker-launchd install-worker-launchdS restart-launchd restart-launchdS restart-worker-launchd restart-worker-launchdS uninstall-launchd uninstall-launchdS uninstall-worker-launchd uninstall-worker-launchdS launchd-status launchd-statusS worker-launchd-status worker-launchd-statusS launchd-logs launchd-logsS worker-launchd-logs worker-launchd-logsS hooks

build:
	cargo build --workspace

fmt:
	cargo fmt --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

mainlines:
	cargo run -p xtask -- mainlines check

gates:
	cargo run -p xtask -- gates check

ci: build fmt clippy test mainlines gates

verify-webui-online:
	scripts/verify-webui-online.sh

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
