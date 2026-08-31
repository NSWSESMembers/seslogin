# Vite serves in about a second, but `cargo run` has to compile the API first, so
# starting all three at once means the browser can reach the web app minutes before
# :8000 answers — every GraphQL call in that window fails. Build the API up front,
# then hold vite until it is actually listening.
dev:
	@set -e; \
	trap 'kill 0' INT TERM EXIT; \
	(cd web && npm run relay -- --watch) & \
	echo "==> Building API (first build may take a few minutes)..."; \
	(cd api && cargo build --bin poem); \
	(cd api && RUST_LOG=info exec cargo run --bin poem -- --enable-mutations) & \
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

lint:	gha-lint
	(cd api && cargo clippy)
	(cd web && npm run lint)

gha-lint:
	@command -v actionlint >/dev/null 2>&1 || { echo "actionlint not found. Install with: brew install actionlint"; exit 1; }
	@actionlint

format:
	(cd api && cargo fmt)
	(cd web && npm run format)
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
