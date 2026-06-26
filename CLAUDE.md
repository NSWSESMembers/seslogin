# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

seslogin v2 is a member attendance tracking system for managing check-in/check-out sessions across locations. It replaces a legacy v1 system and syncs member data from an external SES API (headquarters system).

## Commands

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

### After GraphQL Schema Changes

When you modify the GraphQL API (queries or mutations in `api/src/graphql/`), you **must** regenerate the schema file and recompile the Relay types before the frontend will type-check correctly:

```bash
cd api && cargo run --locked --bin export-schema > schema.graphql   # regenerate api/schema.graphql
cd web && npm run relay                                               # regenerate Relay TS types
```

The `make pre-commit-checks` target runs a schema diff and Relay compilation, so it will catch this if skipped.

### Testing & Linting

```bash
cd api && cargo test                  # Run all Rust tests
cd api && cargo clippy                # Lint Rust code
cd web && npm run test:unit           # Web unit tests
make pre-commit-checks                # Full CI suite: relay, prettier, eslint, build, cargo fmt --check, schema diff, clippy
```

### Data Sync (local)

```bash
make sync      # Dry-run SES API sync (print changes only)
make do-sync   # Apply SES API sync to database
```

### Lambda Deployment

```bash
cd api && make deploy                   # Build & deploy API Lambda (seslogin-api)
cd api && make deploy-sync-lambda       # Build & deploy per-location sync Lambda
cd api && make deploy-dispatcher-lambda # Build & deploy SQS dispatcher Lambda
```

Auto-deployment is split by branch via `.github/workflows/deploy.yml`:

| Branch | Deploys |
|--------|---------|
| `prod` | Production API Lambda (`seslogin-api`) + web to `new.seslogin.com` |
| `preprod` | Preprod API Lambda (`seslogin-preprod-api`) + web to `preprod.seslogin.com` |
| `main` | Test API Lambda (`seslogin-test-api`) + sync/dispatcher/checker/nitc-export/healthcheck/activity-summary/sync-locations Lambdas + web to `test.seslogin.com` |

`preprod` is a production-like clone for staging: the `seslogin-preprod-api` Lambda intentionally shares prod's database (`DB_PREFIX=seslogin_prod`), SQS queues, and secrets (JWT/SES/Turnstile), so it operates on **live production data** with mutations enabled. It only differs from prod in its function name, IAM role, and WebAuthn/CORS origin (`preprod.seslogin.com`). Like `prod`, it deploys only the API Lambda + web (not the sync/utility Lambdas).

The following Lambdas are only deployed from `main`, not `prod` or `preprod`: sync (`seslogin-sync-members`), dispatcher (`seslogin-dispatcher`), checker (`seslogin-checker`), nitc-export (`seslogin-nitc-export`), healthcheck (`seslogin-healthcheck`), activity-summary (`seslogin-activity-summary`), and sync-locations (`seslogin-sync-locations`).

### Infrastructure (Terraform)

```bash
cd infra && terraform plan   # Preview infra changes
cd infra && terraform apply  # Apply infra changes
```

Terraform uses the `seslogin-new` AWS profile by default (var `aws_profile`) — an IAM Identity Center (SSO) profile for account `641079927221`. Run `aws sso login --profile seslogin-new` first. Admin access is the `SesloginAdmin` permission set (PowerUserAccess + `iam:*`); there is no separate `seslogin-terraform` managed policy.

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

Environment variables (loaded from `.env` and `.env.secret`):
- `DB_PREFIX` — DynamoDB table name prefix
- `JWT_SECRET` — JWT signing key
- `SES_API_BASE_URL` / `SES_API_KEY` — External member sync API
- `MEMBER_SYNC_QUEUE_URL` — SQS queue URL used by the dispatcher Lambda
- `RUST_LOG` — Log level (e.g., `info`, `debug`)
- `WEBAUTHN_RP_ID` / `WEBAUTHN_RP_ORIGIN` — Passkey relying-party ID and origin. Local dev defaults to `localhost` / `http://localhost:5173`; deployed envs use `seslogin.com` / the site origin (e.g. `https://new.seslogin.com`). A passkey is bound to the RP ID it was registered under, so local-dev passkeys won't work in prod.
