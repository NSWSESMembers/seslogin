# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

seslogin v2 is a member attendance tracking system for managing check-in/check-out sessions across locations. It replaces a legacy v1 system and syncs member data from an external SES API (headquarters system).

## Commands

> **Full local setup guide: [DEVELOPMENT.md](DEVELOPMENT.md)** — AWS access, secrets,
> toolchain, and troubleshooting.

> ⚠️ **Databases: local dev is on the dev tables; every deployed environment is on prod.**
> `.env` sets `DB_PREFIX=seslogin_test`, so `make dev` and locally-run scripts hit the
> `seslogin_test_*` tables — a manually refreshed, **out-of-date partial snapshot** of
> production. Expect missing or stale records; that's the snapshot, not a bug.
>
> All *deployed* environments use `seslogin_prod`: prod, preprod, `seslogin-test-api`, and
> all sync/utility Lambdas (`var.db_prefix` defaults to `seslogin_prod`). Anything you push
> to a deployment branch runs against **live production member data** with mutations enabled.
>
> Local dev can also be pointed at prod by setting `DB_PREFIX=seslogin_prod`. If you do:
> prefer read-only work, use dry-runs, avoid destructive mutations, and double-check
> `DB_PREFIX` before running anything that writes.

### Development

```bash
make dev                    # Start everything: API + Relay compiler watch + web dev server
```

Or individually:
```bash
cd api && RUST_LOG=info cargo run --bin poem -- --enable-mutations    # API server (port 8000)
cd web && npm run relay -- --watch                                    # Relay GraphQL compiler
cd web && npm run dev                                                  # Web dev server
```

#### Bypassing auth for local UI testing

To drive the web UI locally without logging in (e.g. taking screenshots or automated
testing), start the `poem` server with one of these flags. Token verification is then
bypassed and every request is treated as the given caller — no `Authorization` header
needed from the browser:

```bash
cargo run --bin poem -- --enable-mutations --dev-auth-session <SESSION_ID>       # act as a kiosk/session
cargo run --bin poem -- --enable-mutations --dev-auth-user <USER_ID_OR_EMAIL>    # act as a user (id or email)
```

The impersonated caller keeps its real permissions, so authorization still applies. This
is dev-only (not present in the deployed Lambda) and logs a warning at startup; a missing
or inactive record yields `401`. See `api/README.md` for details.

> ⚠️ Under the default `DB_PREFIX=seslogin_test` this impersonates a caller in the dev
> snapshot, which is safe. If you have switched to `DB_PREFIX=seslogin_prod`, it acts as a
> **real** user or kiosk against live production data, and `--enable-mutations` means writes
> hit prod. Only impersonate records you own, and prefer read-only testing.

### After GraphQL Schema Changes

When you modify the GraphQL API (queries or mutations in `api/src/graphql/`), you **must** regenerate the schema file and recompile the Relay types before the frontend will type-check correctly:

```bash
cd api && cargo run --locked --bin export-schema > schema.graphql   # regenerate api/schema.graphql
cd web && npm run relay                                               # regenerate Relay TS types
```

The `make check` target runs a schema diff and Relay compilation, so it will catch this if skipped.

### Testing & Linting

```bash
cd api && cargo test                  # Run all Rust tests
cd api && cargo clippy                # Lint Rust code
cd web && npm run test:unit           # Web unit tests (type-checks first, then runs vitest)
make test                             # Both test suites: cargo test + web unit tests
make check                            # Static checks — see below. Does NOT run tests.
make format                           # Auto-fix formatting: cargo fmt, prettier, terraform fmt
```

**`make check` does not run any tests.** It is the static-analysis half of CI only:

| | |
|---|---|
| workflows | `actionlint` |
| web | relay compile, `prettier --check`, eslint, `tsc -b`, vite build |
| infra | `terraform fmt -check` |
| api | toolchain version check, `cargo fmt --check`, `export-schema` diff against `schema.graphql`, `clippy -Dwarnings` |

It needs `actionlint` and Terraform on PATH. When it fails on formatting, run `make format`
rather than fixing by hand.

CI additionally runs `npm test` and `cargo test --locked` (`.github/workflows/_check-web.yml`,
`_check-api.yml`). So **a green `make check` does not mean CI is green** — run `make test` too
before pushing, or `make check && make test` for the full equivalent.

Type-checking covers test files: `web/tsconfig.app.json` includes `src` (so every `*.test.tsx`)
plus `setupTests.ts`. Vitest itself only strips types via esbuild and never checks them, which is
why `test:unit` runs `tsc -b` first. The bare `npm test` script stays type-check-free because CI
already runs `npm run typecheck` as its own step.

### Data Sync (local)

Run the binaries directly with `cargo run`. All of them default to `--dry-run true`; pass
`--dry-run false` to actually write.

```bash
cd api && RUST_LOG=info cargo run --bin sync-members --                     # Dry-run SES API member sync (print changes only)
cd api && RUST_LOG=info cargo run --bin sync-members -- --dry-run false     # Apply member sync to database
cd api && RUST_LOG=info cargo run --bin sync-locations --                   # Dry-run location sync
cd api && RUST_LOG=info cargo run --bin sync-locations -- --dry-run false   # Apply location sync
cd api && RUST_LOG=info cargo run --bin load-nitc-tags --                   # Load NITC tags
```

> ⚠️ Always review the dry-run output before applying. If you have switched to
> `DB_PREFIX=seslogin_prod`, an apply writes to the **production** database.

### Lambda Deployment

Deployment is automated via GitHub Actions; there is no supported manual/local deploy path.
Push to the relevant branch below rather than running `cargo lambda deploy` by hand.

Auto-deployment is split by branch, one workflow per branch under `.github/workflows/` (`deploy-prod.yml`, `deploy-preprod.yml`, `deploy-test.yml`, `deploy-workers.yml`):

| Branch | Deploys |
|--------|---------|
| `prod` | Production API Lambda (`seslogin-api`) + web to `new.seslogin.com` |
| `preprod` | Preprod API Lambda (`seslogin-preprod-api`) + web to `preprod.seslogin.com` |
| `test` | Test API Lambda (`seslogin-test-api`) + web to `test.seslogin.com` |
| `workers` | Background/worker Lambdas: sync/dispatcher/checker/nitc-export/healthcheck/activity-summary/sync-locations/open-period-notice |

`preprod` is a production-like clone for staging: the `seslogin-preprod-api` Lambda intentionally shares prod's database (`DB_PREFIX=seslogin_prod`), SQS queues, and secrets (JWT/SES/Turnstile), so it operates on **live production data** with mutations enabled. It only differs from prod in its function name, IAM role, and WebAuthn/CORS origin (`preprod.seslogin.com`). Like `prod`, it deploys only the API Lambda + web (not the sync/utility Lambdas).

The following Lambdas are only deployed from the `workers` branch, not from `test`, `prod`, or `preprod`: sync (`seslogin-sync-members`), dispatcher (`seslogin-dispatcher`), checker (`seslogin-checker`), nitc-export (`seslogin-nitc-export`), healthcheck (`seslogin-healthcheck`), activity-summary (`seslogin-activity-summary`), sync-locations (`seslogin-sync-locations`), and open-period-notice (`seslogin-open-period-notice`). `test` deploys only the API Lambda + web.

#### Branch model and history rewriting

`test`, `workers`, `preprod`, and `prod` are **deployment branches** — each one pushes to its respective environment. They are not protected against history rewrites: expect force-pushes / rewritten history on all of them, and especially on `test`, which is frequently rewritten as experimental work is pushed to it.

> ⚠️ **Shared production database.** All these environments are usually configured to point at the same production database (see the `preprod` note above; `test` typically does too). A deploy to *any* of these branches — including experimental pushes to `test` — runs against **live production data** with mutations enabled. Take care accordingly.

`main` is the stable integration branch and is **not** force-pushed. It may be ahead of or behind the deployment branches at any given time. Always fork PR branches from `main` (never from a deployment branch) so your work sits on top of stable, non-rewritten history.

### Infrastructure (Terraform)

```bash
cd infra && terraform plan   # Preview infra changes
cd infra && terraform apply  # Apply infra changes
```

Terraform uses the `seslogin` AWS profile by default (var `aws_profile`) — an IAM Identity Center (SSO) profile for account `641079927221`. Run `aws sso login --profile seslogin` first. Admin access is the `SesloginAdmin` permission set (PowerUserAccess + `iam:*`); there is no separate `seslogin-terraform` managed policy. (The migration's old account `303170530482` is the `sdunster` profile.)

## Architecture

### Structure

- `api/` — Rust GraphQL backend (primary codebase); also builds all Lambda binaries
- `web/` — React/Relay frontend
- `infra/` — Terraform for AWS infrastructure (Lambdas, SQS, IAM, EventBridge scheduler)

### API Architecture

**Entry point**: `api/src/bin/poem.rs` — Poem HTTP server on port 8000, mounts GraphQL endpoint

**GraphQL**: `api/src/graphql.rs` — All queries and mutations (~69KB). Mutations require `--enable-mutations` CLI flag.

**Database abstraction**: `api/src/db.rs` defines traits; `api/src/dynamodb.rs` is the DynamoDB implementation. A `mockdb` implementation exists for tests.

> **Optional attributes: omit, don't write `Null`.** When an optional field is absent, leave the attribute off the item entirely — on `put_item` skip the `.item(...)` call; on `update_item` put it in a `REMOVE` clause rather than `SET`ting it to `AttributeValue::Null`. This is mandatory for any attribute that backs a GSI key (DynamoDB rejects a `Null` GSI key with a `ValidationException` — this was the cause of the category-creation bug) and is also required for String/Number Sets (which cannot be stored empty). Apply it uniformly to all optional attributes for consistency; hydration in `dynamodb.rs` already treats a missing attribute and `Null` identically.

**Auth**: `api/src/auth.rs` — token verification dispatches on prefix:
1. API tokens (`slgn_` prefix) — opaque hashed secrets for programmatic access
2. User tokens (`slu_` prefix) — opaque hashed secrets issued via email-code auth
3. JWT (no prefix) — session JWTs (single-use numeric kiosk codes → 14-day JWT) and user JWTs

Authorization uses an `AuthRequirement` guard enum per field: `Session`, `UserOrSession`, `User`, `SuperUser`.

**DataLoader**: `api/src/dataloader.rs` — Batches DB lookups to avoid N+1 in GraphQL resolvers.

**Member sync**: `api/src/member_sync.rs` — Fetches paginated member list from SES API, diffs against local DB, plans and optionally applies changes (adopt IDs, create, update, soft-delete). In production, sync runs as two Lambdas: `dispatcher-lambda` (triggered by EventBridge every hour, `cron(0 * * * ? *)` UTC) hashes each location ID into one of 24 hour buckets and enqueues only the locations whose bucket matches the current UTC hour; `sync-members-lambda` consumes each SQS message and runs the sync for that location. Net effect: each location is synced once per 24-hour cycle at a consistent but distributed UTC hour. The SQS queue has a DLQ with 3 retries.

**Deleting departed members (the "absence pass")**: SES signals a departure by dropping the person from the unit's payload, not by a `deleted` flag — a `deleted: true` row is unexpected and only logs a warning. Deletion is therefore two-phase, so that a member who transfers to a *different* unit is never deleted:

1. A member absent from their location's payload gets `Person.missing_since` stamped, not deleted.
2. Any sync at any location that sees them again clears the marker. A transferring member is marked by the old unit, then matched by `ses_api_person_id` at the new unit within the same 24-hour cycle, moved by the normal update path, and cleared.
3. Only once the marker is older than `SES_SYNC_ABSENCE_GRACE_SECS` (default 7 days) is the member soft-deleted.

**Off by default** — set `SES_SYNC_ABSENCE_ENABLED=true` per environment after reviewing a dry run. Guards, all per-location so one bad unit cannot abort a run: a payload whose rows all fail to parse skips the pass entirely (an *empty* payload does not — plenty of units are legitimately empty in SES, and the cap below covers the rest); candidates are capped at `max(SES_SYNC_ABSENCE_MIN, SES_SYNC_ABSENCE_PERCENT% of the synced roster)`; and deletions are suppressed unless the location's previous successful sync is within `SES_SYNC_MAX_SYNC_STALENESS_SECS` (default 36h), so a location recovering from a DLQ outage cannot delete its roster on a single sighting. Absence writes are deliberately excluded from `max_mutations` — that abort would DLQ the location and stop its legitimate creates and updates too.

**Open period notices ("you forgot to sign out")**: `api/src/open_period_notice.rs`, run by `open-period-notice-lambda` on an hourly schedule from 07:30 to 20:30 Sydney (`cron(30 7-20 * * ? *)`; the job re-checks the hour itself so a manual invocation can't mail someone at 3am). It finds periods with a `start_time` and no `end_time` and emails the member the same `slp_` edit link the admin **Remind** button sends, so they can enter their own finish time.

Three waves at 12h / 24h / 48h after the period started, then the period is left alone; nothing is considered past 7 days. Two rules shape everything else:

- **The next threshold is indexed by how many waves a period has already had**, not by elapsed time. So a wave delayed by quiet hours, the person gap or a truncated run just happens on a later run instead of doubling up — which is what makes truncation and per-location failure isolation safe.
- **The unit of politeness is the person, not the period.** One failed kiosk can leave a member with several open entries, so candidates are grouped by person: a person emailed within the last 12h is skipped entirely (one cheap read, no per-period lookups), and otherwise exactly one email goes out about their *oldest* due period. An admin reminder counts towards that gap too.

Marker rows live in `ephemeral_state` (`open_period_notice_person_*` for the 12h gap, `open_period_notice_period_*` for the wave counter) and **fail closed** — a corrupt or unreadable row blocks the send. That is the deliberate opposite of the admin cooldown's fail-open behaviour: a human pressing a button shouldn't be stopped by a bad row, but an unattended mailer that fails open is one bad row away from a loop. Write ordering is split for the same reason: the person gap is stamped *before* sending (so a failed send can't become a retry storm) and the wave counter *after* (so a transient SES failure retries the wave rather than consuming it).

A period first seen older than 36h never enters the funnel at all. This is the switch-on protection: widening the allow list mails only entries that crossed 12h in about the last day, not the whole backlog.

`api/src/bin/open-period-notice.rs` is the CLI twin — **defaults to `--dry-run true`**, and takes `--person-id` / `--location-id` to widen the allow list ad hoc, `--now <unix>` to exercise the waves without waiting, and `--all-locations` (dry run only) to size the org-wide blast radius before enabling a scope.

**SES API client**: `api/src/ses_api.rs` — HTTP client with retry logic for the external headquarters system.

**JWT**: `api/src/jwt.rs` — HMAC-SHA256 tokens with claims `{ user_id, exp }` or `{ session_id, exp }`.

### Core Data Model

| Entity | Key fields | Notes |
|--------|-----------|-------|
| `Location` | `id`, `name`, `ses_api_headquarters_id` | Maps to SES HQ for sync |
| `Person` | `id`, `location_id`, `member_number`, `ses_api_person_id` | Members; synced from SES |
| `Period` | `id`, `person_id`, `location_id`, `category_id`, `start_time`, `end_time` | Attendance events |
| `Session` | `id`, `name`, `location_id`, `code`, `healthcheck_url` | Kiosk/device sessions |
| `User` | `id`, `email`, `is_super`, `location_grants` | System admins |
| `Category` | `id`, `name` | Activity types for periods |

All entities use soft deletes (`deleted` flag).

### Configuration

Environment variables (loaded from `.env` and `.env.secret`; see
[.env.secret.example](.env.secret.example) for the full secret list):
- `DB_PREFIX` — DynamoDB table name prefix. `.env` sets `seslogin_test` (dev snapshot); deployed envs use `seslogin_prod`.
- `JWT_SECRET` — JWT signing key
- `SES_API_BASE_URL` / `SES_API_KEY` — External member sync API
- `SES_INTRANET_SEARCH_API_BASE_URL` / `SES_INTRANET_SEARCH_API_KEY` — SES intranet contact-directory search, used to sync member emails. Separate credential from `SES_API_KEY` (uses the `Ocp-Apim-Subscription-Key` header).
- `MEMBER_SYNC_QUEUE_URL` / `NITC_EXPORT_QUEUE_URL` / `HEALTHCHECK_QUEUE_URL` — SQS queue URLs
- `TURNSTILE_SECRET_KEY` — Cloudflare Turnstile secret for verifying login CAPTCHA tokens
- `TURNSTILE_DISABLED` — Set to `1` locally to bypass Turnstile (it can't work in local dev). Pair with `VITE_TURNSTILE_DISABLED=1` in `web/.env.local`.
- `RUST_LOG` — Log level (e.g., `info`, `debug`)
- `MAIL_OVERRIDE_TO` — Redirect **all** outgoing email to this address instead of its real recipient, logging a warning each time. Set it locally before touching anything that mails a member: `seslogin_test` is a snapshot of production and carries real member addresses, so the admin "Remind" button would otherwise email a real volunteer from your laptop. Never set in a deployed environment.
- `WEB_BASE_URL` — Public site origin used to build member-facing period edit links (`<base>/period#<token>`). Optional: falls back to the first `WEBAUTHN_RP_ORIGIN`, which is already the site origin in every environment, so no infra change is needed to deploy.
- `OPEN_PERIOD_NOTICE_PERSON_IDS` / `OPEN_PERIOD_NOTICE_LOCATION_IDS` — Comma-separated allow lists for the open-period notice job (see below). **Both empty by default, which makes the job a no-op** — it returns without a single DB call. A period qualifies if its person id *or* its location id is listed.
- `OPEN_PERIOD_NOTICE_MAX_PER_RUN` (default 200) — Soft cap; a run over it sends the oldest N and leaves the rest for the next run. `OPEN_PERIOD_NOTICE_MAX_CANDIDATES` (default 2000) — Circuit breaker; a run over it sends *nothing* and alerts to SNS, because that many open periods means kiosks have stopped signing people out rather than that there is a backlog to work through.
- `WEBAUTHN_RP_ID` / `WEBAUTHN_RP_ORIGIN` — Passkey relying-party ID and origin. Local dev defaults to `localhost` / `http://localhost:5173`; deployed envs use `seslogin.com` / the site origin (e.g. `https://new.seslogin.com`). A passkey is bound to the RP ID it was registered under, so local-dev passkeys won't work in prod.
