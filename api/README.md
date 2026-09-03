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

### Injecting resolver errors

To exercise the frontend's GraphQL error handling, `SESLOGIN_FORCE_FIELD_ERRORS`
makes a specific field resolution fail on demand:

```bash
# Fail exactly this row's person lookup — copy the path straight out of a previous
# run's "injecting error at ..." log line, or a real resolver error's `path`
SESLOGIN_FORCE_FIELD_ERRORS='location.periods.edges.1.node.person' \
  RUST_LOG=info cargo run --bin poem -- --enable-mutations

# Fail ~5% of every row's person lookup, and only the first 1 of them
SESLOGIN_FORCE_FIELD_ERRORS='location.periods.edges.*.node.person@0.05' \
  SESLOGIN_FORCE_FIELD_ERRORS_BUDGET=1 \
  RUST_LOG=info cargo run --bin poem -- --enable-mutations
```

Each target is an exact GraphQL response path, comma-separated — the same dotted
shape (`field.field.<index>.field...`) a real resolver error's `path` carries. A `*`
segment matches any single segment at that position (typically an array index), for
targeting a field across every row instead of one specific one; pair it with
`@<rate>` (a probability in `0.0..=1.0`, default `1.0`) to fail roughly that fraction
of matches. A rate on an exact, non-wildcarded path just makes that one path always
or never fire — not something you can tune from the outside — so leave it off there.

The failure is raised from an async-graphql extension, which is indistinguishable
from the resolver itself returning `Err` — so async-graphql applies its normal
null-propagation. That is what makes this useful:

- A **nullable** target (`...node.person`, `...node.category`) produces a *partial*
  response: HTTP 200, `data` populated, plus an `errors` entry. This is the case
  clients tend to mishandle, and it is otherwise hard to provoke.
- A **non-null** target (`...node.location`) propagates up to the nearest nullable
  ancestor, usually collapsing the whole response to `data: null`.

Two details worth knowing:

- **Failures are deterministic.** With a `*` in the path, the rate is decided by
  hashing each matched row's own full response path, so the *same* rows fail on every
  refetch. A random rate would make retry behaviour untestable — you couldn't tell a
  fix from a lucky reroll.
- **`_BUDGET` caps total injections** for the life of the process. Set it to `1` and
  the first attempt fails while a retry succeeds, which is how you verify that a
  "try again" path actually refetches.

> ⚠️ Dev only, and structurally so: the module is behind `#[cfg(debug_assertions)]`,
> and every deployed Lambda is a release build, so the code is absent from them rather
> than merely disabled. The server logs a loud warning at startup when injection is
> active.

## Client self-reporting (`X-Client-Info`)

Every request from the web client carries two diagnostic headers, which the server
stores on the caller's session record during the throttled `last_contact` refresh in
`auth::touch_session` — so they cost no writes beyond the one already happening:

- `X-Client-Version` — the build the client is running (unchanged; predates the below).
- `X-Client-Info` — a compact JSON object describing the client, parsed by
  `client_info::ClientReport::from_headers`.

The server folds in two facts of its own: the request's `User-Agent`, and the clock skew
implied by the client's reported `clockMs`. The result is exposed as
`Session.clientInfo` and rendered in the admin kiosk list (environment column) and the
kiosk edit page (full panel).

The most operationally useful field is `env`/`origin`: the `test` and `preprod`
front-ends talk to the **production** database, so a kiosk running one of those builds is
signing real members in and out — and nothing else in the admin UI would reveal it.
`env` comes from `VITE_ENVIRONMENT`, set per deploy workflow; a build without it reports
`dev`.

Two rules govern the parsing, both enforced by tests in `client_info.rs`:

1. **A bad header never fails a request.** Missing, oversized, malformed, or wrong-typed
   input degrades to "nothing reported". Diagnostics must not be able to lock a kiosk out
   of service.
2. **A parsed snapshot is authoritative.** Fields the client omits are `REMOVE`d from the
   record rather than left in place, so a value on screen is always something the kiosk
   reported at the timestamp in `clientInfo.updatedAt` — never a stale leftover.

Sizes are bounded (2 KB per header, 128 characters per field, 256 for the user agent).

## SES member sync

Set `ses_api_headquarters_id` per location via the admin UI before syncing.

Sync defaults to dry-run mode (prints planned changes only):

```bash
SES_API_BASE_URL=https://your-ses-api.example.com \
SES_API_KEY=your-static-token \
DB_PREFIX=seslogin_test \
cargo run --bin sync-members --
```

Run sync in apply mode by passing `--apply`:

```bash
SES_API_BASE_URL=https://your-ses-api.example.com \
SES_API_KEY=your-static-token \
DB_PREFIX=seslogin_test \
cargo run --bin sync-members -- --apply
```

Lambda binary (consumes one SQS message per location from the dispatcher; reads config from env vars):

```bash
cargo run --bin sync-members-lambda
```

Optional flags:

- `--location-id L10 --location-id L22` limits syncing to specific locations.
- `--page-limit 100` overrides SES API page size.
- `--max-retries 3` controls retries for transient SES API failures.
- `--max-mutations 500` aborts apply mode if planned writes exceed threshold (default 100).
- `--no-adopt` disables SES ID adoption, which is on by default.
- `--absence-enabled false` disables the departed-member pass, which is on by default.

The defaults for adoption, the absence pass, page size, retries and the mutation cap all
match the `seslogin-sync-members` Lambda's environment in
[`infra/lambda_sync.tf`](../infra/lambda_sync.tf), so a local dry run plans the same
changes the deployed job would. Any of them can still be overridden by the same env var
the Lambda uses (`SES_SYNC_ADOPT`, `SES_SYNC_ABSENCE_ENABLED`, …); an explicit flag wins
over the env var.

Behavior notes:

- Imported SES members are tagged with `members.ses_api_person_id`.
- Updates/deletes only apply to local rows whose `ses_api_person_id` matches SES `person.id`.
- If a row exists with matching `serialnumber` but no `ses_api_person_id`, sync prints a skip message and does not modify that row.
