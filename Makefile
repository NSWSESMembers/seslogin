dev:
	@set -e; \
	trap 'kill 0' INT TERM EXIT; \
	(cd api && RUST_LOG=info exec cargo run --bin poem -- --enable-mutations) & \
	(cd web && npm run relay -- --watch) & \
	(cd web && npm run dev)

lint:	gh-lint
	(cd api && cargo clippy)
	(cd web && npm run lint)

gha-lint:
	@command -v actionlint >/dev/null 2>&1 || { echo "actionlint not found. Install with: brew install actionlint"; exit 1; }
	@actionlint

format:
	(cd api && cargo fmt)
	(cd web && npm run format)
	(cd infra && terraform fmt -recursive)

check:	pre-commit-checks

pre-commit-checks:
	@echo "Running workflow checks..."
	@$(MAKE) gha-lint
	@echo "Running web checks..."
	@cd web && npm run relay
	@cd web && npx prettier --check .
	@cd web && npm run lint
	@cd web && npm run typecheck
	@cd web && npm run build
	@echo "Running infra checks..."
	@cd infra && terraform fmt -recursive -check -diff
	@echo "Running API checks..."
	@cd api && cargo fmt --check
	@cd api && cargo run --locked --bin export-schema > /tmp/schema.generated.graphql
	@cd api && diff -u schema.graphql /tmp/schema.generated.graphql
	@cd api && RUSTFLAGS='-Dwarnings' cargo clippy --locked --all-targets --all-features

install-githooks:
	@git config core.hooksPath .githooks
	@chmod +x .githooks/pre-commit
	@echo "Git hooks installed (core.hooksPath=.githooks)"

member-sync:
	cd api && RUST_LOG=info cargo run --bin sync-members --

sync-locations:
	cd api && RUST_LOG=info cargo run --bin sync-locations --

do-sync-locations:
	cd api && RUST_LOG=info cargo run --bin sync-locations -- --dry-run false

load-nitc-tags:
	cd api && RUST_LOG=info cargo run --bin load-nitc-tags --
