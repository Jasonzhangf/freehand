.PHONY: build fmt clippy test gates ci hooks

build:
	cargo build --workspace

fmt:
	cargo fmt --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

gates:
	cargo run -p xtask -- gates check

ci: build fmt clippy test gates

hooks:
	git config core.hooksPath .githooks
	chmod +x .githooks/pre-commit .githooks/pre-push

