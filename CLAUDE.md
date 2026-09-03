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

#### Injecting resolver errors

`SESLOGIN_FORCE_FIELD_ERRORS` makes a specific GraphQL field resolution fail, for
testing how the frontend handles resolver errors:

```bash
cd api && SESLOGIN_FORCE_FIELD_ERRORS='location.periods.edges.1.node.person' \
  RUST_LOG=info cargo run --bin poem -- --enable-mutations
```

Comma-separated targets, each an *exact* GraphQL response path — the same dotted
string (`field.field.<index>.field...`) a real resolver error's `path` would carry,
and the one this tool's own "injecting error at ..." log line prints, so a path
copied from either place pins that exact field on that exact row. A `*` segment
matches any single segment at that position (typically an array index), for a field
across every row instead of one specific one — pair it with `@<rate>` (0.0–1.0,
default 1.0) to select roughly that fraction, decided by hashing each row's own
resolved path so the *same* rows fail on every refetch (a rate on an exact,
non-wildcarded path just makes that one path always or never fire — not something
you can tune from the outside, so leave it off there). Failing a **nullable** field
(`...node.person`) yields a *partial* response — HTTP 200 with `data` populated plus
an `errors` entry — which is the shape clients most often mishandle; failing a
**non-null** field (`...node.location`) collapses the response to `data: null`.
`SESLOGIN_FORCE_FIELD_ERRORS_BUDGET` caps total injections for the life of the
process, so setting it to `1` lets you verify that a retry actually refetches.

Dev only and compiled out of release builds (`#[cfg(debug_assertions)]`), so it cannot
be enabled in any deployed environment. See `api/README.md` for details.

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

Run the binaries directly with `cargo run`. Every binary **except `cli`** is a dry run
unless you pass `--apply`; their other defaults match how the corresponding Lambda is
configured in production (see [infra/lambda_sync.tf](infra/lambda_sync.tf)), so a local
run plans the same changes the deployed job would.

```bash
cd api && RUST_LOG=info cargo run --bin sync-members --                # Dry-run SES API member sync (print changes only)
cd api && RUST_LOG=info cargo run --bin sync-members -- --apply        # Apply member sync to database
cd api && RUST_LOG=info cargo run --bin sync-locations --              # Dry-run location sync
cd api && RUST_LOG=info cargo run --bin sync-locations -- --apply      # Apply location sync
cd api && RUST_LOG=info cargo run --bin load-nitc-tags --              # Dry-run NITC tag load
cd api && RUST_LOG=info cargo run --bin nitc-export -- backfill        # Dry-run NITC export backfill
cd api && RUST_LOG=info cargo run --bin activity-summary --            # Print summary emails instead of sending
```

`cli` is the exception: it writes immediately, and takes an optional global `--dry-run`
to preview a write command instead.

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
| `workers` | Background/worker Lambdas: sync/dispatcher/checker/nitc-export/healthcheck/activity-summary/sync-locations |

`preprod` is a production-like clone for staging: the `seslogin-preprod-api` Lambda intentionally shares prod's database (`DB_PREFIX=seslogin_prod`), SQS queues, and secrets (JWT/SES/Turnstile), so it operates on **live production data** with mutations enabled. It only differs from prod in its function name, IAM role, and WebAuthn/CORS origin (`preprod.seslogin.com`). Like `prod`, it deploys only the API Lambda + web (not the sync/utility Lambdas).

The following Lambdas are only deployed from the `workers` branch, not from `test`, `prod`, or `preprod`: sync (`seslogin-sync-members`), dispatcher (`seslogin-dispatcher`), checker (`seslogin-checker`), nitc-export (`seslogin-nitc-export`), healthcheck (`seslogin-healthcheck`), activity-summary (`seslogin-activity-summary`), and sync-locations (`seslogin-sync-locations`). `test` deploys only the API Lambda + web.

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

**Database abstraction**: `api/src/db.rs` defines traits; `api/src/dynamodb.rs` is the DynamoDB implementation. A `mockdb` implementation exists for tests — it fails every call, so its job is exercising error paths, not standing in for a database.

**Queue and mail abstraction**: the same trait/impl/mock split.

| Concern | Trait | AWS impl | Mock |
|---|---|---|---|
| DynamoDB | `db.rs` | `dynamodb.rs` | `mockdb.rs` (fails everything) |
| SQS | `queue.rs` | `sqs.rs` | `mockqueue.rs` (records) |
| SES email | `mail.rs` | `sesmail.rs` | `mockmail.rs` (logs) |

Unlike `mockdb`, the queue and mail mocks *succeed* — they exist so the API can run with no AWS account. The app reaches all three through `app::HasDb` / `app::HasQueues` / `app::HasMail`, and `MyApp<DBH, Q, M>` is generic over each.

**There is no runtime switch between them.** Which implementations exist is decided at compile time, by which binary you build: `bin/poem.rs` is DynamoDB + SQS + SES, `bin/poem-local.rs` is DynamoDB + the mocks. The shared server (handler, routes, CLI) lives in `server.rs` so the two can't drift; each binary is a ~15-line `main`. A cargo feature was rejected because `make check` runs `clippy --all-features`, which would enable it — the real SQS and SES paths would stop being linted, and any `--all-features` build would quietly produce a mocked server.

`sqs.rs` also keeps free `enqueue_*` functions holding each message's wire format. The worker binaries (`dispatcher-lambda`, `nitc-export`) call those directly: each owns exactly one queue, so the three-queue `Queues` handle would be the wrong shape, and they need real AWS anyway.

> **Optional attributes: omit, don't write `Null`.** When an optional field is absent, leave the attribute off the item entirely — on `put_item` skip the `.item(...)` call; on `update_item` put it in a `REMOVE` clause rather than `SET`ting it to `AttributeValue::Null`. This is mandatory for any attribute that backs a GSI key (DynamoDB rejects a `Null` GSI key with a `ValidationException` — this was the cause of the category-creation bug) and is also required for String/Number Sets (which cannot be stored empty). Apply it uniformly to all optional attributes for consistency; hydration in `dynamodb.rs` already treats a missing attribute and `Null` identically.

> **Optional attributes: omit, don't write `Null`.** When an optional field is absent, leave the attribute off the item entirely — on `put_item` skip the `.item(...)` call; on `update_item` put it in a `REMOVE` clause rather than `SET`ting it to `AttributeValue::Null`. This is mandatory for any attribute that backs a GSI key (DynamoDB rejects a `Null` GSI key with a `ValidationException` — this was the cause of the category-creation bug) and is also required for String/Number Sets (which cannot be stored empty). Apply it uniformly to all optional attributes for consistency; hydration in `dynamodb.rs` already treats a missing attribute and `Null` identically.

**Auth**: `api/src/auth.rs` — token verification dispatches on prefix:
1. API tokens (`slgn_` prefix) — opaque hashed secrets for programmatic access
2. User tokens (`slu_` prefix) — opaque hashed secrets issued via email-code auth
3. JWT (no prefix) — session JWTs (single-use numeric kiosk codes → 14-day JWT) and user JWTs

Authorization uses an `AuthRequirement` guard enum per field: `Session`, `UserOrSession`, `User`, `SuperUser`.

**DataLoader**: `api/src/dataloader.rs` — Batches DB lookups to avoid N+1 in GraphQL resolvers.

**Member sync**: `api/src/member_sync.rs` — Fetches paginated member list from SES API, diffs against local DB, plans and optionally applies changes (adopt IDs, create, update, soft-delete). Email sync (via the separate SES intranet search API) deliberately runs **last**, after the member changes are applied and `last_successful_member_sync` is stamped: it has its own endpoint and credential, so an outage there still fails the location into the DLQ but can no longer stop every location's primary member sync — which would both bury a real single-location failure in a fleet-wide DLQ flood and falsely trip the checker lambda's stale-location digest. In production, sync runs as two Lambdas: `dispatcher-lambda` (triggered by EventBridge every hour, `cron(0 * * * ? *)` UTC) hashes each location ID into one of 24 hour buckets and enqueues only the locations whose bucket matches the current UTC hour; `sync-members-lambda` consumes each SQS message and runs the sync for that location. Net effect: each location is synced once per 24-hour cycle at a consistent but distributed UTC hour. The SQS queue has a DLQ with 3 retries.

**Deleting departed members (the "absence pass")**: SES signals a departure by dropping the person from the unit's payload, not by a `deleted` flag — a `deleted: true` row is unexpected and only logs a warning. Deletion is therefore two-phase, so that a member who transfers to a *different* unit is never deleted:

1. A member absent from their location's payload gets `Person.missing_since` stamped, not deleted.
2. Any sync at any location that sees them again clears the marker. A transferring member is marked by the old unit, then matched by `ses_api_person_id` at the new unit within the same 24-hour cycle, moved by the normal update path, and cleared.
3. Only once the marker is older than `SES_SYNC_ABSENCE_GRACE_SECS` (default 7 days) is the member soft-deleted.

**Enabled in production** (`SES_SYNC_ABSENCE_ENABLED=true` in `lambda_sync.tf`), and on by default in `sync-members` to match; pass `--absence-enabled false` to turn it off for a run. The `AbsencePolicy::default()` used by library callers stays disabled, so the lambda's own fallback is off if the env var is ever unset. Guards:

- A payload whose rows all fail to parse skips the pass for that location (an *empty* payload does not — plenty of units are legitimately empty in SES, and the cap below covers the rest).
- Candidates are capped at `max(SES_SYNC_ABSENCE_MIN, SES_SYNC_ABSENCE_PERCENT% of the synced roster)`. **Exceeding the cap is fatal**: the sync aborts the location before applying anything — no creates, no updates, no `last_successful_member_sync` stamp — so in the lambda the message retries and lands in the DLQ. Being over the cap means the local roster and SES have diverged too far to tell a genuine mass departure from a bad payload, so a human decides. Dry runs log the abort instead of taking it, so a review pass still covers every remaining location.
- Deletions are suppressed unless the location's previous successful sync is within `SES_SYNC_MAX_SYNC_STALENESS_SECS` (default 36h), so a location recovering from a DLQ outage cannot delete its roster on a single sighting.

Absence writes are excluded from `max_mutations`: a location with a large but legal backlog of departures shouldn't trip a global tripwire and abort every *other* location's creates and updates. The per-location cap above is what polices them.

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
- `MEMBER_SYNC_QUEUE_URL` / `NITC_EXPORT_QUEUE_URL` / `HEALTHCHECK_QUEUE_URL` — SQS queue URLs. All three are required by `poem` and by the API Lambda. `poem-local` never reads them.
- `MOCK_MAIL_DIR` — `poem-local` only: also write each "sent" message to a file in this directory. Optional; messages are logged either way.
- `TURNSTILE_SECRET_KEY` — Cloudflare Turnstile secret for verifying login CAPTCHA tokens
- `TURNSTILE_DISABLED` — Set to `1` locally to bypass Turnstile (it can't work in local dev). Pair with `VITE_TURNSTILE_DISABLED=1` in `web/.env.local`.
- `RUST_LOG` — Log level (e.g., `info`, `debug`)
- `MAIL_OVERRIDE_TO` — Redirect **all** outgoing email to this address instead of its real recipient, logging a warning each time. Set it locally before touching anything that mails a member: `seslogin_test` is a snapshot of production and carries real member addresses, so the admin "Remind" button would otherwise email a real volunteer from your laptop. Never set in a deployed environment.
- `WEB_BASE_URL` — Public site origin used to build member-facing period edit links (`<base>/period#<token>`). Optional: falls back to the first `WEBAUTHN_RP_ORIGIN`, which is already the site origin in every environment, so no infra change is needed to deploy.
- `WEBAUTHN_RP_ID` / `WEBAUTHN_RP_ORIGIN` — Passkey relying-party ID and origin. Local dev defaults to `localhost` / `http://localhost:5173`; deployed envs use `seslogin.com` / the site origin (e.g. `https://new.seslogin.com`). A passkey is bound to the RP ID it was registered under, so local-dev passkeys won't work in prod.
