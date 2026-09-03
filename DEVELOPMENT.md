# Development setup

How to get a local seslogin development environment running from scratch.

If you just want to know how to submit a change, see [CONTRIBUTING.md](CONTRIBUTING.md).

---

## 1. Get AWS access

The normal setup talks to DynamoDB in AWS, so you need AWS credentials before anything will
run locally.

> If you can't get AWS access — or you're working with someone you'd rather not hand
> credentials to — skip to [§9, Running without AWS](#9-running-without-aws). That path
> needs no AWS account at all, at the cost of starting from an empty database.

1. **Ask for an AWS IAM Identity Center account to be created.** Provide your preferred
   username and email address.
2. **Check your email for the account details.** Log in and set your password.
3. **Log in to the AWS access portal.** You should see the **`seslogin prod`** AWS account.
   Expand it and click **Access keys** next to **`SesloginAdmin`** — this shows the values
   you'll need in the next step (the SSO start URL and region).
4. **Configure the CLI profile:**

   ```bash
   aws configure sso
   ```

   Use these answers:

   | Prompt | Value |
   | --- | --- |
   | SSO session name | `seslogin` |
   | SSO start URL | (from the access keys panel) |
   | SSO region | `ap-southeast-2` |
   | SSO registration scopes | accept the default |
   | Account | `seslogin prod` (`641079927221`) |
   | Role | `SesloginAdmin` |
   | Default client region | `ap-southeast-2` |
   | Default output format | `json` |
   | **Profile name** | **`seslogin`** |

   The profile name matters — Terraform defaults to a profile called `seslogin` (the
   `aws_profile` variable).

Your SSO session expires periodically. When commands start failing with credential errors,
re-authenticate:

```bash
aws sso login --profile seslogin
```

> **Note:** `SesloginAdmin` is PowerUserAccess + `iam:*`.

---

## 2. Get the remaining secrets

Non-secret config is checked in at [.env](.env) — you don't need to create it.

Secrets live in `.env.secret`, which is gitignored. **Ask someone on the team to share the
contents of `.env.secret` with you**, then create the file at the repo root and paste them
in. [.env.secret.example](.env.secret.example) documents what each value is for.

> ⚠️ Never commit `.env.secret`. It's in [.gitignore](.gitignore) — keep it that way.

You also need frontend config:

```bash
cp web/.env.local.example web/.env.local
```

| Variable | Description |
| --- | --- |
| `VITE_BEACON_URL` | Base URL for the Beacon system, used to link NITC event IDs. Ask the team for the real value. |
| `VITE_TURNSTILE_DISABLED` | Leave at `1`. Turnstile (login CAPTCHA) can't work in local dev. |

---

## 3. Which database am I using?

**By default you are pointed at the dev tables**, not production. [.env](.env) sets:

```
DB_PREFIX=seslogin_test
```

The `seslogin_test_*` tables hold a **snapshot of a subset of production data**. That
snapshot is refreshed manually and infrequently, so treat it as **probably out of date** —
it's fine for building and testing UI, but don't be surprised when a member, location, or
period you expect isn't there, or when data looks stale.

You *can* point local dev at the production tables by setting `DB_PREFIX=seslogin_prod`,
and sometimes that's the only way to reproduce a real problem.

> ⚠️ **If you do, you are working against live production member data.** Prefer read-only
> work, avoid mutations, run sync commands in dry-run mode first, and double-check
> `DB_PREFIX` before anything that writes. Set it back to `seslogin_test` when you're done.

Note that the deployed `test`, `preprod`, and `prod` environments all point at the
production database — so this local default is *safer* than any deployed environment, not
representative of them.

There is a third option: `DB_PREFIX=seslogin_local`, a database in a container on your own
machine, holding nothing you didn't put there. See [§9](#9-running-without-aws).

---

## 4. Install the toolchain

### Rust

Install via [rustup](https://rustup.rs):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Restart your shell (or `source "$HOME/.cargo/env"`), then confirm:

```bash
rustc --version
```

The repo pins an **exact** Rust version in [api/rust-toolchain.toml](api/rust-toolchain.toml)
(currently `1.97.0`) so that CI and local dev run the same clippy lint set. rustup normally
installs it automatically the first time you build in `api/`. If it doesn't, `make check`
will tell you, and the fix is:

```bash
rustup toolchain install 1.97.0
```

### Node

Node.js **>= 22** is required. Install it however you normally do (`nvm`, `fnm`, Homebrew,
or [nodejs.org](https://nodejs.org)).

### Dependencies

```bash
cd web && npm i
```

Rust dependencies are fetched automatically on first build — no separate install step.

### Optional

- `actionlint` — required by `make check` for the GitHub Actions workflow lint:
  ```bash
  brew install actionlint
  ```
- **Terraform** — only needed if you're touching [infra/](infra/). `make check` runs
  `terraform fmt -check`.
- **VSCode** — open the repo and accept the recommended extensions prompt (see
  [.vscode/extensions.json](.vscode/extensions.json)): ESLint, Prettier, Relay, GraphQL
  syntax, rust-analyzer, Tailwind. Enable **format on save**.

---

## 5. Run it

```bash
make dev
```

That starts all three processes together:

| Process | What it does |
| --- | --- |
| `cargo run --bin poem -- --enable-mutations` | GraphQL API on **:8000** |
| `npm run relay -- --watch` | Relay compiler, regenerates TS types on query changes |
| `npm run dev` | Vite dev server on **:5173** |

Open <http://localhost:5173>.

The first `make dev` compiles the whole Rust API and will take a few minutes. Subsequent
runs are fast.

`make dev-local` is the same thing against a local DynamoDB container and mocked AWS
services, with no credentials at all — see [§9](#9-running-without-aws).

To run a piece on its own:

```bash
cd api && RUST_LOG=info cargo run --bin poem -- --enable-mutations
cd web && npm run relay -- --watch
cd web && npm run dev
```

---

## 6. Everyday commands

| Command | What it does |
| --- | --- |
| `make dev` | Run the whole stack (see above) |
| `make check` | **Full pre-commit suite — run this before every commit** |
| `make format` | Auto-fix formatting across Rust, web, and Terraform |
| `make test` | `cargo test` + web unit tests |

`make check` runs, in order: actionlint · Relay compile · Prettier check · ESLint ·
TypeScript typecheck · production web build · `terraform fmt -check` · Rust toolchain
version check · `cargo fmt --check` · GraphQL schema diff · Clippy with warnings as errors.

CI runs the same suite, so a green `make check` locally means a green CI.

**If `make check` fails on formatting**, don't fix it by hand:

```bash
make format
```

Then re-run `make check`.

### After changing the GraphQL API

If you touch queries or mutations in `api/src/graphql/`, regenerate the schema and Relay
types before the frontend will typecheck:

```bash
cd api && cargo run --locked --bin export-schema > schema.graphql
cd web && npm run relay
```

`make check` diffs the committed schema against a freshly generated one, so it will catch
this if you forget — but you'll save a round trip by doing it up front.

---

## 7. Bypassing auth for local UI work

To drive the web UI without logging in — screenshots, automated testing, or just moving
faster — start the API with a dev-auth flag. Token verification is bypassed and every
request is treated as the given caller, so the browser needs no `Authorization` header:

```bash
# Act as a kiosk/session
cd api && cargo run --bin poem -- --enable-mutations --dev-auth-session <SESSION_ID>

# Act as a user (record id or email)
cd api && cargo run --bin poem -- --enable-mutations --dev-auth-user <USER_ID_OR_EMAIL>
```

The impersonated caller keeps its real permissions (`is_super`, `location_grants`, session
location), so authorization still applies normally. A missing or inactive record yields
`401`.

> ⚠️ Dev only — this exists solely in the `poem` binary, not the deployed Lambda, and the
> server logs a loud warning at startup. If you've also set `DB_PREFIX=seslogin_prod`,
> you're impersonating a real caller against production data. Only impersonate records you
> own.

See [api/README.md](api/README.md) for more.

---

## 8. Troubleshooting

| Symptom | Fix |
| --- | --- |
| Credential / `ExpiredToken` errors from the API | `aws sso login --profile seslogin` |
| `make check` fails on Rust toolchain mismatch | `rustup toolchain install 1.97.0` |
| `make check` fails with `actionlint not found` | `brew install actionlint` |
| Frontend type errors on a query you just changed | `cd web && npm run relay` |
| Schema diff failure in `make check` | Regenerate `schema.graphql` (see §6) |
| Prettier / `cargo fmt` failures | `make format` |
| Data missing or looks stale | Expected — the dev tables are an old partial snapshot (see §3) |
| `make local-fetch` fails to download | Its download host is unreachable; the Maven Central fallback needs `mvn` on `PATH` (see §9) |

---

## 9. Running without AWS

`make dev-local` runs the whole app against a real DynamoDB Local plus in-process stand-ins
for the two other AWS services the API uses. No AWS account, no credentials, no
`.env.secret`.

It runs a **different binary**, `poem-local`, rather than `poem` with a flag. The mocks
aren't compiled into `poem` at all, so no environment variable or misconfiguration can talk
the real server into mocking its own email.

### One-time setup

DynamoDB Local is a Java program that Amazon also ships as a container image. You need one
of the two; **Java is the lighter choice** — a single process rather than a Linux VM, and
native on Apple silicon:

```bash
brew install --cask temurin dynamodb-local     # JRE 17+ and Amazon's launcher
```

If you would rather not install the `dynamodb-local` cask, a JRE alone is enough — fetch
Amazon's JAR into this repo instead (checksum-verified, gitignored):

```bash
brew install --cask temurin
make local-fetch
```

Or use containers, if you already run them for something else:

```bash
brew install colima docker docker-compose && colima start
```

`make local-up` picks whichever it finds, preferring Java. Force one with `LOCAL_DDB=java`
or `LOCAL_DDB=docker`. With neither installed it tells you what to install rather than
failing obscurely.

**No Homebrew, no Docker?** (A Linux box, a CI runner, a sandboxed agent.) `make
local-fetch` needs only a JRE 17+: it downloads Amazon's tarball, and if that host is
unreachable — it is behind a CDN that some networks refuse — it falls back to resolving the
same program from Maven Central, which needs `mvn` on `PATH`. Either way the result lands
in `local/dynamodb-local/` (gitignored) and `make local-up` picks it up with no further
setup. No Homebrew required.

```bash
make local-fetch     # tarball, else Maven Central
make local-up
```

### Running

```bash
make dev-local
```

That starts DynamoDB Local, creates the tables if they're missing, then starts the same
three processes `make dev` does. Open <http://localhost:5173> as usual.

You still need the frontend config from [§2](#2-get-the-remaining-secrets) — `cp
web/.env.local.example web/.env.local` — but not `.env.secret`.

### What's real and what isn't

| Service | Locally | Why |
| --- | --- | --- |
| DynamoDB | **Real** — [DynamoDB Local](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/DynamoDBLocal.html) on **:8100** | The data layer is written against real DynamoDB semantics (GSI key validation, empty sets, `BatchGetItem` unprocessed keys). A stand-in that merely stored rows would hide the bugs this codebase actually hits. |
| SQS | Mocked (`mockqueue`) | Every queue feeds a worker Lambda that needs real AWS regardless, so locally the honest behaviour is to log the message that would have been sent. |
| SES (email) | Mocked (`mockmail`) | The message is logged in full, which is how you read your own login code. Set `MOCK_MAIL_DIR` in [local/local.env](local/local.env) to also write each one to a file — easier for HTML emails. |
| SQS queue URLs | Not needed | `poem` requires all three `*_QUEUE_URL` variables; `poem-local` never reads them, which is why it needs no `.env.secret`. |
| SES headquarters API | Not available | `sync-members` / `sync-locations` / `nitc-export` need the real endpoint and credential. They aren't part of the local stack. |
| Turnstile | Disabled | Already the case for all local dev. |

The mocks are ordinary implementations of the [`queue::Handler`](api/src/queue.rs) and
[`mail::Handler`](api/src/mail.rs) traits — the same shape as `db::Handler` / `dynamodb` /
`mockdb`. `MyApp<DBH, Q, M>` is generic over all three, so each binary compiles exactly the
implementations it uses; the shared server code lives in [api/src/server.rs](api/src/server.rs)
and both `main`s are a dozen lines. Deployed environments are unaffected — the API Lambda
builds against SQS and SES like it always did.

Note that **DynamoDB is not mocked** in `poem-local`: `DB_PREFIX` and
`AWS_ENDPOINT_URL_DYNAMODB` still decide which database it talks to. Pointing it at a real
table gives you a server that provably cannot send email or enqueue work.

### The local database

`DB_PREFIX=seslogin_local`, a database of its own — it can't be confused with the
`seslogin_test` snapshot (which holds real member data), let alone production.

It starts empty; `make local-seed` fills it. That is part of `make dev-local`, so normally
you don't run it yourself.

### Seed data

`make local-seed` writes [local/seed/](local/seed/) into the local database. It needs no AWS
access — the fixtures are committed — and re-running it is safe, since every row is written
by primary key.

| | |
| --- | --- |
| **Test Unit** (`wBsJHYxy9snR`) | Real record and its two members, IDs unchanged, so ids you already use in testing still work |
| **Other Test Unit** (`OtherTestUn1`) | Invented, with one member (`OtherUnitMbr`, SES id `87654321`) for testing a sign-in by someone from another unit |
| **Categories** | All 220, plus the 99 NITC groups they point at |
| **Users** | `super@seslogin.test` (super) and `testunit@seslogin.test` (Test Unit only) |

No kiosk sessions — set those up per test.

Two files, with different rules:

- `from-prod.json` is generated by `make local-seed-extract`, the only part of this stack
  that needs AWS access. It skips soft-deleted rows, and **refuses to write anything
  carrying an `email` or `ses_api_person_id`** — this repo is public, so a future
  re-extract must not quietly publish a real member. Review the diff before committing.
- `synthetic.json` is hand-written and safe to edit. Keys beginning with `_` are notes,
  not attributes.

Rows are stored as raw DynamoDB items rather than going through `db::Handler`, because the
trait's `create_*` methods generate their own IDs, and the fixture needs to preserve them.

### Logging in

Either way in works:

- **Dev auth bypass** — `--dev-auth-user super@seslogin.test` (see
  [§7](#7-bypassing-auth-for-local-ui-work)). Fastest, and the impersonated user keeps its
  real permissions, so authorization still applies.
- **The real email-code flow** — request a code and read it out of the API's own log, where
  `mockmail` prints the whole message. Nothing is sent anywhere.

### Managing the stack

| Command | What it does |
| --- | --- |
| `make local-up` | Start DynamoDB Local (Java or Docker, whichever you have) |
| `make local-down` | Stop it, keeping the data |
| `make local-status` | Report whether it's running, and how |
| `make local-reset` | Stop it and **delete every local table and row** |
| `make local-fetch` | Install DynamoDB Local into `local/` (Amazon's tarball, else Maven Central) |
| `make local-tables` | Create any missing tables |
| `make local-tables-check` | Fail if the local database is missing a table this codebase expects |
| `make local-seed` | Write `local/seed/*.json` into the database |
| `make local-seed-extract` | Refresh `from-prod.json` from the real database (**needs AWS**) |

Environment knobs: `LOCAL_DDB=java|docker` forces a runtime, `LOCAL_DDB_PORT` moves the
port (match it in `local/local.env`), and `LOCAL_DDB_MEMORY=1` runs in memory so nothing
survives a restart — sensible in a throwaway sandbox, not on a laptop.

On the Docker route only, `local/docker-compose.yml` also brings up a DynamoDB browser UI
on <http://localhost:8101>. With the Java route, `npx dynamodb-admin` gives you the same
thing without a container.

Config lives in [local/local.env](local/local.env), which `make dev-local` exports before
starting anything. Exported variables beat `.env` — dotenvy never overrides a variable that
is already set — so that file, not `.env`, decides where local runs point. There is no
variable selecting the mocks; that is what the `poem-local` binary is.

### Schema drift

The table definitions live in [api/src/bin/local-tables.rs](api/src/bin/local-tables.rs),
transcribed by hand from [infra/dynamodb_test.tf](infra/dynamodb_test.tf). **That Terraform
file is the source of truth.** If you add a table or a GSI there, add it here too, or local
runs will fail with a `ValidationException` that doesn't obviously point at the cause.

`local-tables` refuses to run unless the DynamoDB endpoint is on localhost, so a stray
`DB_PREFIX` can't create tables in a real AWS account.

---

## Where to go next

- [README.md](README.md) — project overview, branches, deployments
- [CONTRIBUTING.md](CONTRIBUTING.md) — submitting a PR
- [SCHEMA.md](SCHEMA.md) — data model
- [MANUAL.md](MANUAL.md) — operator documentation
- [api/README.md](api/README.md) — API internals and member sync
- [web/README.md](web/README.md) — web-specific npm scripts
