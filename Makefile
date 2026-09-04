# Vite serves in about a second, but `cargo run` has to compile the API first, so
# starting all three at once means the browser can reach the web app minutes before
# :8000 answers — every GraphQL call in that window fails. Build the API up front,
# then hold vite until it is actually listening.
#
# $(1) is the server binary to run; $(2) is a shell prelude, used by dev-local to
# export local/local.env first.
define run_dev
	@set -e; \
	$(2) \
	trap 'kill 0' INT TERM EXIT; \
	(cd web && npm run relay -- --watch) & \
	echo "==> Building API (first build may take a few minutes)..."; \
	(cd api && cargo build --bin $(1)); \
	(cd api && RUST_LOG=info exec cargo run --bin $(1) -- --enable-mutations) & \
	api_pid=$$!; \
	printf '==> Waiting for API on :8000'; \
	ready=; \
	for _ in $$(seq 1 120); do \
		if curl -sf -o /dev/null http://localhost:8000/; then ready=1; break; fi; \
		if ! kill -0 $$api_pid 2>/dev/null; then \
			echo; echo "==> API exited before it became ready — see its output above."; \
			exit 1; \
		fi; \
		printf '.'; sleep 0.5; \
	done; \
	if [ -z "$$ready" ]; then \
		echo; echo "==> Timed out after 60s waiting for the API on :8000."; \
		exit 1; \
	fi; \
	echo " ready"; \
	(cd web && npm run dev)
endef

dev:
	$(call run_dev,poem,)

# ── AWS-free local stack ──────────────────────────────────────────────────────
# The `poem-local` binary (DynamoDB + mocked SQS/SES) against DynamoDB Local,
# run by Java or Docker depending on what the machine has, with a database of its
# own (seslogin_local). Needs no AWS credentials and touches no AWS account.
# See DEVELOPMENT.md.

# Exported vars beat .env — dotenvy never overrides an already-set variable.
LOCAL_ENV = set -a; . ./local/local.env; set +a;
# Runs DynamoDB Local via Java or Docker, whichever this machine has. Force one
# with LOCAL_DDB=java / LOCAL_DDB=docker.
LOCAL_DDB_SH = ./local/dynamodb.sh
# Same stack as dev-local, started in the background. See local/e2e.sh.
LOCAL_E2E_SH = ./local/e2e.sh

dev-local: local-up local-tables local-seed
	$(call run_dev,poem-local,$(LOCAL_ENV))

local-up:
	@$(LOCAL_DDB_SH) start

# ── Detached stack, for scripts ───────────────────────────────────────────────
# `dev-local` holds the terminal; these return once everything is answering, so a
# browser test or CI job can drive the app. See local/e2e.sh.
local-e2e:
	@$(LOCAL_E2E_SH) up

local-e2e-down:
	@$(LOCAL_E2E_SH) down

local-e2e-status:
	@$(LOCAL_E2E_SH) status

local-down:
	@$(LOCAL_DDB_SH) stop

local-status:
	@$(LOCAL_DDB_SH) status

# Download Amazon's DynamoDB Local JAR into local/, for the Java route without
# the `dynamodb-local` brew cask. Checksum-verified against Amazon's published sum.
local-fetch:
	@$(LOCAL_DDB_SH) fetch

# Also discards the stored data — every local table and row goes with it.
local-reset:
	@$(LOCAL_DDB_SH) reset

# Write local/seed/*.json into the local database. Needs no AWS access.
local-seed:
	@$(LOCAL_ENV) cd api && cargo run --quiet --bin local-seed -- apply

# Refresh local/seed/from-prod.json from the real database. Needs AWS access, and
# is the only part of the local stack that does — run it rarely, review the diff.
local-seed-extract:
	@cd api && cargo run --quiet --bin local-seed -- extract

local-tables:
	@$(LOCAL_ENV) cd api && cargo run --quiet --bin local-tables

# Fails if the local database is missing any table this codebase expects.
local-tables-check:
	@$(LOCAL_ENV) cd api && cargo run --quiet --bin local-tables -- --check

lint:	gha-lint
	(cd api && cargo clippy)
	(cd web && npm run lint)

gha-lint:
	@command -v actionlint >/dev/null 2>&1 || { echo "actionlint not found. Install with: brew install actionlint"; exit 1; }
	@actionlint

format:
	(cd api && cargo fmt)
	(cd web && npm run format)
	(cd web && npx prettier --write ../local/examples)
	(cd infra && terraform fmt -recursive)

test:
	(cd api && cargo test)
	(cd web && npm run test:unit)

check:
	@echo "Running workflow checks..."
	@$(MAKE) gha-lint
	@echo "Running web checks..."
	@cd web && npm run relay
	@cd web && npx prettier --check .
	@cd web && npm run lint
	@cd web && npm run typecheck
	@cd web && npm run build
	@echo "Running local stack checks..."
	@cd web && npx prettier --check ../local/examples
	@for f in local/examples/*.mjs; do node --check "$$f" || exit 1; done
	@if command -v shellcheck >/dev/null 2>&1; then shellcheck local/*.sh; \
	else echo "  (shellcheck not installed; skipping local/*.sh)"; fi
	@cd api && cargo test --locked --test seed_fixtures
	@echo "Running infra checks..."
	@cd infra && terraform fmt -recursive -check -diff
	@echo "Running API checks..."
	@$(MAKE) check-toolchain
	@cd api && cargo fmt --check
	@cd api && cargo run --locked --bin export-schema > /tmp/schema.generated.graphql
	@cd api && diff -u schema.graphql /tmp/schema.generated.graphql
	@cd api && RUSTFLAGS='-Dwarnings' cargo clippy --locked --all-targets --all-features

check-toolchain:
	@expected=$$(sed -nE 's/^channel *= *"(.*)"/\1/p' api/rust-toolchain.toml); \
	case "$$expected" in \
		stable|beta|nightly|"") \
			echo "rust-toolchain.toml channel is '$$expected' (floating); skipping exact-version check."; \
			exit 0;; \
	esac; \
	actual=$$(cd api && rustc --version | awk '{print $$2}'); \
	if [ "$$expected" != "$$actual" ]; then \
		echo "ERROR: Rust toolchain mismatch — CI may flag clippy lints that don't reproduce locally."; \
		echo "  api/rust-toolchain.toml pins: $$expected"; \
		echo "  active rustc:                 $$actual"; \
		echo "  Fix: rustup toolchain install $$expected   (rustup normally auto-installs it; run this if it didn't)"; \
		exit 1; \
	fi; \
	echo "Rust toolchain OK ($$actual)"
