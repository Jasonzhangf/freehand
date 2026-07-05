.PHONY: build fmt clippy test mainlines gates ci verify-webui-online release install-global install-symlink install-launchd install-launchdS restart-launchd restart-launchdS uninstall-launchd uninstall-launchdS launchd-status launchd-statusS launchd-logs launchd-logsS hooks

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

restart-launchd:
	scripts/install-launchd.sh restart

restart-launchdS:
	scripts/install-launchd.sh restartS

uninstall-launchd:
	scripts/uninstall-launchd.sh

uninstall-launchdS:
	scripts/uninstall-launchd.sh uninstallS

launchd-status:
	launchctl print gui/$$(id -u)/com.freehand.daemon

launchd-statusS:
	launchctl print gui/$$(id -u)/com.freehand.daemonS

launchd-logs:
	tail -n 80 "$$HOME/.freehand/logs/daemon.stdout.log" "$$HOME/.freehand/logs/daemon.stderr.log"

launchd-logsS:
	tail -n 80 "$$HOME/.freehand/logs/daemonS.stdout.log" "$$HOME/.freehand/logs/daemonS.stderr.log"

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit .githooks/pre-push
