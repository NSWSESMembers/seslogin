DOCKER_COMPOSE := $(shell if docker compose version >/dev/null 2>&1; then echo "docker compose"; elif command -v docker-compose >/dev/null 2>&1; then echo "docker-compose"; else echo "docker compose"; fi)
HOST_HOME := $(if $(SUDO_USER),/home/$(SUDO_USER),$(HOME))
AWS_DIR ?= $(HOST_HOME)/.aws
COMPOSE_ENV := AWS_DIR=$(AWS_DIR)

dev:
	$(COMPOSE_ENV) $(DOCKER_COMPOSE) up dev

dev-down:
	-$(COMPOSE_ENV) $(DOCKER_COMPOSE) stop dev

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

docker-image:
	docker build -t seslogin-build-env .

docker-shell:
	$(COMPOSE_ENV) $(DOCKER_COMPOSE) exec dev bash

docker-build:
	docker run --rm \
		-v $(CURDIR):/workspace \
		-v seslogin-cargo-registry:/usr/local/cargo/registry \
		-v seslogin-cargo-git:/usr/local/cargo/git \
		-v seslogin-npm-cache:/root/.npm \
		-w /workspace \
		seslogin-build-env \
		bash -lc "npm ci --prefix web && npm run build --prefix web && cargo build --manifest-path api/Cargo.toml --locked --bins"

docker-up:
	$(COMPOSE_ENV) $(DOCKER_COMPOSE) up -d dev

docker-down:
	$(COMPOSE_ENV) $(DOCKER_COMPOSE) down

docker-workflow:
	$(COMPOSE_ENV) $(DOCKER_COMPOSE) exec dev \
		bash -lc "npm run build --prefix web && cargo build --manifest-path api/Cargo.toml --locked --bins"
