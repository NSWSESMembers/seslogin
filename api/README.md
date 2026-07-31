# GraphQL API server for seslogin

> **First-time setup — including AWS access and secrets — is in
> [DEVELOPMENT.md](../DEVELOPMENT.md).** This file covers API-specific details.

> ⚠️ **Check which database you're on.** `.env` sets `DB_PREFIX=seslogin_test`, so by default
> the dev server and local scripts use the `seslogin_test_*` tables — an out-of-date partial
> snapshot of production. If you have switched to `DB_PREFIX=seslogin_prod` you are operating
> on **live production member data**: prefer dry-runs, avoid destructive mutations, and
> double-check `DB_PREFIX` before running anything that writes.

Prerequisites: Rust via [rustup](https://rustup.rs) (the exact version is pinned in
`rust-toolchain.toml`) and AWS credentials via the `seslogin` SSO profile —
`aws sso login --profile seslogin`. See [DEVELOPMENT.md](../DEVELOPMENT.md) for how to set
that profile up.

Then: `RUST_LOG=info cargo run --bin poem`

## Run tests

```
cargo test
```

## Run dev API server

```
RUST_LOG=info cargo run --bin poem
```

Listens on port 8000.

Add `--enable-mutations` to allow writes.

### Bypassing auth for local testing

For driving the web UI locally without logging in (e.g. automated testing or
screenshots), the `poem` server can bypass token verification and treat **every**
request as a fixed caller. Pass exactly one of:

```bash
# Act as a kiosk/session — provide a session record id
RUST_LOG=info cargo run --bin poem -- --enable-mutations --dev-auth-session <SESSION_ID>

# Act as a user — provide a user record id or email
RUST_LOG=info cargo run --bin poem -- --enable-mutations --dev-auth-user <USER_ID_OR_EMAIL>
```

With either flag set, the `Authorization` header is ignored and no token is
needed from the browser. The impersonated caller keeps its real permissions
(`is_super`, `location_grants`, session location), so authorization still applies
normally. If the given record isn't found (or the session is inactive), requests
return `401`.

> ⚠️ Dev only. This lives solely in the `poem` binary — it is **not** available in
> the deployed Lambda — and the server logs a loud warning at startup when it's
> enabled. Under the default `DB_PREFIX=seslogin_test` you are impersonating a caller in the
> dev snapshot; if you have switched to `DB_PREFIX=seslogin_prod` you are impersonating a
> **real** caller against live production data, and `--enable-mutations` means writes hit
> prod. Only impersonate records you own, and prefer read-only testing.

## SES member sync

Set `ses_api_headquarters_id` per location via the admin UI before syncing.

Sync defaults to dry-run mode (prints planned changes only):

```bash
SES_API_BASE_URL=https://your-ses-api.example.com \
SES_API_KEY=your-static-token \
DB_PREFIX=seslogin_test \
cargo run --bin sync-members --
```

Run sync in apply mode by passing `--dry-run false`:

```bash
SES_API_BASE_URL=https://your-ses-api.example.com \
SES_API_KEY=your-static-token \
DB_PREFIX=seslogin_test \
cargo run --bin sync-members -- --dry-run false
```

Lambda binary (consumes one SQS message per location from the dispatcher; reads config from env vars):

```bash
cargo run --bin sync-members-lambda
```

Optional flags:

- `--location-id L10 --location-id L22` limits syncing to specific locations.
- `--page-limit 100` overrides SES API page size.
- `--max-retries 3` controls retries for transient SES API failures.
- `--max-mutations 500` aborts apply mode if planned writes exceed threshold.

Behavior notes:

- Imported SES members are tagged with `members.ses_api_person_id`.
- Updates/deletes only apply to local rows whose `ses_api_person_id` matches SES `person.id`.
- If a row exists with matching `serialnumber` but no `ses_api_person_id`, sync prints a skip message and does not modify that row.
