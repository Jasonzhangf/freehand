.PHONY: build fmt clippy test mainlines gates ci release install-global hooks

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

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit .githooks/pre-push
