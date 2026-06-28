.PHONY: build fmt clippy test mainlines gates ci release install-global install-launchd restart-launchd uninstall-launchd launchd-status launchd-logs hooks

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

release:
	scripts/release.sh

install-global:
	scripts/install-global.sh

install-launchd:
	scripts/install-launchd.sh install

restart-launchd:
	scripts/install-launchd.sh restart

uninstall-launchd:
	scripts/uninstall-launchd.sh

launchd-status:
	launchctl print gui/$$(id -u)/com.freehand.daemon

launchd-logs:
	tail -n 80 "$$HOME/.freehand/logs/daemon.stdout.log" "$$HOME/.freehand/logs/daemon.stderr.log"

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit .githooks/pre-push
