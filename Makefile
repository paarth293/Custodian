.PHONY: test check build clean

# Dependencies are installed by the documented setup flow or Dockerfile.
# This target deliberately performs no network access, so it is safe to run
# offline after the project dependencies have been provisioned.
test:
	npm run typecheck
	npm test
	cargo test --workspace --locked
	python -m pytest
	python tests/security/scan_secrets.py

check:
	npm run format:check
	npm run lint
	npm run typecheck
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --locked -- -D warnings
	ruff check .
	ruff format --check .
	mypy .
	$(MAKE) test

build:
	npm run build
	cargo build --workspace --locked

clean:
	npm run clean
	cargo clean
