# GraphQL API server for seslogin

> ⚠️ **This connects to the production database by default.** `DB_PREFIX` defaults to
> `seslogin_prod` (that's what `.env` ships with), so running the dev server, sync commands,
> or any script locally operates on **live production member data** — there is currently no
> separate test database wired up. Be very careful: prefer dry-runs, avoid destructive
> mutations, and double-check `DB_PREFIX` before running anything that writes.

1. Install rust via [rustup](https://rustup.rs)
2. Setup AWS credentials such as: ~/.aws/credentials

```
[default]
aws_access_key_id = ...
aws_secret_access_key = ...
region = ap-southeast-2
```

3. `RUST_LOG=info cargo run --bin poem`

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
> enabled. Note that with the default `DB_PREFIX=seslogin_prod` you are impersonating a
> caller against **live production data**; if you also pass `--enable-mutations`, writes
> hit prod. Only impersonate records you own, and prefer read-only testing.

## SES member sync

Set `ses_api_headquarters_id` per location via the admin UI before syncing.

Run sync in dry-run mode (prints planned changes only):

```bash
SES_API_BASE_URL=https://your-ses-api.example.com \
SES_API_KEY=your-static-token \
DB_PREFIX=seslogin_test \
cargo run --bin sync-members -- --dry-run
```

Run sync in apply mode:

```bash
SES_API_BASE_URL=https://your-ses-api.example.com \
SES_API_KEY=your-static-token \
DB_PREFIX=seslogin_test \
cargo run --bin sync-members --
```

SQS Lambda binary (invoked per location by the dispatcher; reads config from env vars):

```bash
cargo run --bin sync-members-sqs-lambda
```

Standalone Lambda binary (reads config from env vars and executes one sync run directly):

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
