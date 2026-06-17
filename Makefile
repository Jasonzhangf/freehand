.PHONY: build fmt clippy test mainlines gates ci hooks

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

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit .githooks/pre-push
