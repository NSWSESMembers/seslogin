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

**The admin UI needs one more step.** The flag bypasses the *server's* token check, but the
admin SPA decides whether to show the login page by looking for a token in `localStorage`
— a purely client-side test that never asks the server. With nothing stored you get the
login form even though every request behind it would have succeeded. Seed any non-empty
value once per browser profile; with dev auth on, the server never reads it:

```js
// devtools console at http://localhost:5173, then reload
localStorage.setItem("admin_auth_token", "dev");
```

On the local stack you don't need the flag at all: `make local-seed` writes a **real** user
token you can paste into that same key, which exercises the ordinary authentication path
instead of bypassing it. See [§9](#logging-in). Prefer it — dev auth is for pointing a
server at a caller it has no token for, not for getting past this gate.

The kiosk keeps its session token the same way — as `scanAuthToken` inside its own
per-profile settings entry, in
[KioskEnvironment.tsx](web/src/kiosk/components/KioskEnvironment.tsx) — so expect the same
kind of gate there.

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
| Admin UI shows the login page despite `--dev-auth-user` | The gate is client-side — seed `admin_auth_token` in `localStorage` (see §7) |
| `make local-fetch` fails to download | Use the Maven Central route in §9, or the Docker one |

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
by primary key. Re-run it to undo whatever a test did.

The two files split by *what the data is*, which is also what keeps this safe in a public
repo:

- **`synthetic.json`** — everything that could describe a person or a place: locations,
  members, users, tokens and kiosk sessions. All invented, hand-written, and safe to edit.
  Keys beginning with `_` are notes, not attributes.
- **`from-prod.json`** — reference data only: the 220 categories and the 99 NITC groups they
  point at. Generated by `make local-seed-extract`, the only part of this stack that needs
  AWS access. It skips soft-deleted rows and **refuses to write anything carrying an `email`
  or `ses_api_person_id`**. Review the diff before committing.

Because no real location or member is copied any more, that refusal is now a backstop rather
than the thing standing between production data and a public repo.

| | |
| --- | --- |
| **Test A Unit** (`TestAUnit001`) | Where most testing happens. Members **Alice Anderson** (`10000001`) and **Bob Brown** (`10000002`) |
| **Test B Unit** (`TestBUnit001`) | Second unit, for cross-unit sign-in and for checking a grant is enforced. Member **Crossunit Tester** (`20000001`) |
| **Users** | `super@seslogin.test` (super) and `testunit@seslogin.test` (Test A Unit only) |
| **User tokens** | One ready-made login per user — see [Logging in](#logging-in) |
| **Kiosk sessions** | `TestAKiosk01` at Test A Unit, enrolment code `123456`; `TestAKiosk02`, the same unit but **key-enrolled** |
| **Categories** | All 220, plus the 99 NITC groups they point at |

Neither unit name contains the other, so a selector matching on name text can't hit both —
worth preserving if you rename them.

**Alice and Bob differ in one attribute, deliberately.** Alice has no `ses_api_person_id`,
so member sync doesn't own her record and the admin UI offers **Edit** and **Delete**; Bob
has one, so it hides both and shows the synced bullet
([MembersList.tsx](web/src/admin/pages/MembersList.tsx)). Use Alice for anything that
changes a member, and Bob to check the read-only half. A fixture where every member is
sync-owned looks fine and quietly makes the member edit form unreachable.

**The two kiosks cover the two enrolment styles**, which are mutually exclusive — `code`
and `key_fingerprint` both back GSIs, so a session carries one or the other:

- `TestAKiosk01` is **code-enrolled**: type `123456` at the kiosk. Enrolling consumes the
  code and swaps it for a JWT, so re-run `make local-seed` to put it back.
- `TestAKiosk02` is **key-enrolled**: it carries the public half of a fixed P-256 keypair
  and signs every request with the private half (the `SLKey` scheme, see
  [api/src/session_key.rs](api/src/session_key.rs)). The private half is committed in
  [local/seed/kiosk-signing-key.json](local/seed/kiosk-signing-key.json) and
  [`local/examples/kiosk-scan.mjs`](local/examples/kiosk-scan.mjs) installs it, so kiosk
  testing can begin on the scan screen instead of at enrolment. It is a throwaway key that
  authenticates nothing beyond a local database; if you regenerate it, update both files,
  since the fixture holds the public half and the JSON file the private one.

Rows are stored as raw DynamoDB items rather than going through `db::Handler`, because the
trait's `create_*` methods generate their own IDs, and the fixture needs to preserve them.

### Logging in

Three ways in, fastest first:

- **The seeded user token.** `make local-seed` writes a real `user_token` row for each seeded
  user, so you can authenticate as either one without a login round trip and without
  dev auth. The plaintext tokens are:

  | User | Token |
  | --- | --- |
  | `super@seslogin.test` (super) | `slu_localdev0000000000000000000super` |
  | `testunit@seslogin.test` (Test A Unit only) | `slu_localdev0000000000000000testunit` |

  In the browser, put one in `localStorage` under `admin_auth_token` and reload:

  ```js
  localStorage.setItem("admin_auth_token", "slu_localdev0000000000000000000super");
  ```

  For the API directly, send it as a bearer token:

  ```bash
  curl -s http://localhost:8000/ -H 'Content-Type: application/json' \
    -H 'Authorization: Bearer slu_localdev0000000000000000000super' \
    -d '{"query":"{ locations { id name } }"}'
  ```

  Only the sha256 of each token is stored, exactly as in production — the fixture holds the
  hash, and these plaintexts are in this document because they authenticate nothing beyond a
  database on your own machine. The second token is the useful one for authorization
  testing: it is refused super-only fields and can't reach Test B Unit.

- **Dev auth** — `--dev-auth-user super@seslogin.test` (see
  [§7](#7-bypassing-auth-for-local-ui-work)). Bypasses token verification entirely. Reach for
  it when you need to act as a caller you have no token for, such as a kiosk session.
- **The real email-code flow** — request a code and read it out of the API's own log, where
  `mockmail` prints the whole message. Nothing is sent anywhere.

### Running the stack detached

`make dev-local` holds the terminal, which is the wrong shape for a browser test or a CI
job. `make local-e2e` starts the same stack in the background and returns once both servers
actually answer:

```bash
make local-e2e           # database + tables + seed + API + web, detached
make local-e2e-status    # what's up, and where the logs are
make local-e2e-down      # stop the API and web; the database keeps running
```

Logs go to `local/.e2e/{api,web}.log`. The API runs without dev auth, so a script
authenticates with the seeded token above — the same code path a real login produces. The
build happens before anything is backgrounded, so a compile error fails the command rather
than leaving a server that never comes up.

**`make local-e2e` does not create `web/.env.local`**, and unlike `make dev-local` it
mostly doesn't need one: authenticating with a seeded token skips the login page, which is
the only thing `VITE_TURNSTILE_DISABLED` affects. Both example scripts work on a fresh
clone without it. You want it as soon as you use the **real email-code login**, since
Turnstile can't work against localhost:

```bash
cp web/.env.local.example web/.env.local     # only needed for the real login flow
```

### Driving the UI from a script

The local stack plus the seeded token gets you a browser check of a real change — not a unit
test's idea of one — in about a minute. Start it with `make local-e2e` so it keeps running
while the script drives it.

**Two ready-made scripts** put a browser into the state most work starts from, so you don't
write the login dance again each time — see [local/examples/](local/examples/):

```bash
node local/examples/admin-login.mjs     # admin dashboard, as a user, at a location
node local/examples/kiosk-scan.mjs      # kiosk scan screen, key-enrolled, ready to scan
```

Both take options as environment variables (`USER_EMAIL`, `LOCATION`, `GOTO`, `PROFILE`),
and `HEADED=1 KEEP_OPEN=1` leaves a visible browser open on the state they set up, which is
usually the point. The rest of this section is what they do and why, for when you write
your own. Any browser automation works; this is Playwright, which
needs no repo changes because it isn't a dependency of `web/`:

```bash
npm i playwright --no-save && npx playwright install chromium    # once, at the repo root
```

Install it at the **repo root**: the scripts are ES modules, and Node ignores `NODE_PATH`
for ESM, so it resolves `playwright` only from a `node_modules` beside them or above them.
`/node_modules/` is gitignored for this.

Point it at a Chromium you already have with `executablePath` if downloading one is
awkward (CI images and sandboxes usually ship one; `PLAYWRIGHT_BROWSERS_PATH` is the
other half of that, and `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1` stops the install fetching a
second copy).

```js
import { chromium } from "playwright";

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
page.on("pageerror", (e) => console.log(`[pageerror] ${e.message}`));

// 1. Log in with the seeded token (see above). The app gates the login page on
//    this key client-side, so it must be set before the app decides anything.
await page.goto("http://localhost:5173/admin");
await page.evaluate(() =>
  localStorage.setItem("admin_auth_token", "slu_localdev0000000000000000000super"));
await page.goto("http://localhost:5173/admin", { waitUntil: "networkidle" });

// 2. Dismiss the passkey enrolment prompt, which greets every fresh profile.
const later = page.getByRole("button", { name: /maybe later/i });
if (await later.count()) await later.click();

// 3. Choose a unit.
await page.getByText("Test A Unit", { exact: true }).click();
await page.waitForLoadState("networkidle");

// 4. Do the thing. Seeded ids are committed, so they're safe to hard-code.
//    Alice is the member sync does not own, so she is the editable one.
await page.goto("http://localhost:5173/admin/members/TestAMember1");
await page.locator("#givenname").fill("Alicia");
await page.getByRole("button", { name: "Save" }).click();

// 5. Wait on outcomes, never a timeout. These are two different events: the toast
//    means the mutation returned, the cell means the list re-queried and shows it.
await page.getByText("Member saved").waitFor();
await page.getByRole("cell", { name: "Alicia Anderson" }).waitFor();
await page.screenshot({ path: "/tmp/after.png", fullPage: true });
console.log(await page.locator("body").innerText());   // assert against text, not pixels
await browser.close();
```

Four things cost more time than they should the first time:

- **The token gate in step 1** is the big one — without it you get the login page and no
  clue that the server would have accepted every request.
- **The passkey prompt** covers the page on a fresh browser profile, so a click meant for
  the app silently lands on the overlay instead.
- **`exact: true` on the location.** The seeded unit names are chosen so neither contains
  the other, but keep it: it is what stops a rename from quietly matching two rows.
- **`page.locator("body").innerText()`** is worth more than the screenshot while iterating:
  it diffs cleanly, and it tells you what a blank-looking page actually rendered.

**Starting from a screen the scripts don't reach.** `kiosk-scan.mjs` leaves you ready to
sign a member *in*; the sign-out screens need someone already signed in. Write the open
period straight into the database rather than clicking a sign-in first:

```bash
# 3h ago: the ordinary sign-out flow. 13h ago crosses the 12h threshold and hits
# the forgot-to-sign-out interstitial instead.
make local-cli ARGS="period create-signin --person TestAMember1 \
  --location TestAUnit001 --session TestAKiosk02 --hours-ago 3 --dry-run false"
```

`make local-cli ARGS="..."` is `cli` pointed at the local database; on its own `cli` reads
`.env` and talks to the dev snapshot in AWS. It writes, so `--dry-run false` is what makes
it real, and the scanning kiosk must be at the same location or the period is invisible to
it. `session set-config-key` works the same way when you want to flip a kiosk setting
without going through the admin UI.

**Clean up between runs.** A script that stops halfway leaves its period open, and that
changes what the next run does — scanning a member who is already signed in signs them
*out*. `make local-seed` won't clear it (it only writes the fixtures back, and no fixture
describes a period); `make local-clear` will.

**On the kiosk specifically**, every scan screen stays mounted and parked off-side with a
CSS transform. So `body.innerText()` returns text from screens that aren't showing, and a
loose selector can resolve to an off-screen copy of a control. Scope to
`page.locator("div.translate-x-0")`, and use `settleKiosk(page)` rather than `settle(page)`
so the 500ms slide finishes before you touch anything.

Three independent ways to confirm a mutation really landed, in ascending order of trust:
the success toast, a full `page.reload()` (which proves it isn't just Relay store state),
and the row itself — see below. The API log is the fastest place to see a mutation fail:
each request prints one `api request` line carrying `operation_name`, `status`,
`graphql_error_count` and `mutation_failures`.

### Reading the local database directly

The AWS CLI is the obvious way in, but it isn't required — DynamoDB Local doesn't verify
signatures, so `curl` with a placeholder `Authorization` header is enough to read a row:

```bash
curl -s -X POST http://localhost:8100/ \
  -H 'Content-Type: application/x-amz-json-1.0' \
  -H 'X-Amz-Target: DynamoDB_20120810.GetItem' \
  -H 'Authorization: AWS4-HMAC-SHA256 Credential=local/x/ap-southeast-2/dynamodb/aws4_request, SignedHeaders=host, Signature=x' \
  -d '{"TableName":"seslogin_local_person","Key":{"id":{"S":"TestAMember1"}}}'
```

Swap `GetItem` for `Scan` (and drop `Key`) to dump a table. `npx dynamodb-admin` is the
friendlier option when you're browsing rather than scripting.

### Managing the stack

| Command | What it does |
| --- | --- |
| `make local-e2e` | Start the whole stack detached and wait until it answers |
| `make local-e2e-down` | Stop the detached API and web server |
| `make local-e2e-status` | Report what is up, and where its logs are |
| `make local-up` | Start DynamoDB Local (Java or Docker, whichever you have) |
| `make local-down` | Stop it, keeping the data |
| `make local-status` | Report whether it's running, and how |
| `make local-reset` | Stop it and **delete every local table and row** |
| `make local-fetch` | Install DynamoDB Local into `local/` (Amazon's tarball, else Maven Central) |
| `make local-tables` | Create any missing tables |
| `make local-tables-check` | Fail if the local database is missing a table this codebase expects |
| `make local-seed` | Write `local/seed/*.json` into the database |
| `make local-clear` | Delete the rows the app writes (periods, ephemeral state, passkeys), keeping the fixtures |
| `make local-cli ARGS="..."` | Run the `cli` inspector/editor against the local database |
| `node local/examples/*.mjs` | Put a browser into a state worth testing from (see [local/examples/](local/examples/)) |
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

### What guards the fixtures

The seed carries several *paired* values — a token's plaintext and its sha256, the kiosk's
private key and the public half on its session, an id in a fixture and the same id in these
docs. Change one side and nothing complained; the break surfaced later as a puzzling 401.
[api/tests/seed_fixtures.rs](api/tests/seed_fixtures.rs) now checks each pairing, that
references resolve, that every kiosk has exactly one enrolment style, and that the ids the
examples hard-code still exist. It reads JSON only — no database, no AWS — so it runs in
`cargo test` with everything else.

The rest of the stack is covered by [the local-stack workflow](.github/workflows/_check-local.yml),
which on any change under `local/` formats and parses the example scripts, shellchecks
`local/*.sh`, and then does the part that needs a real database: create the tables, run
`local-tables-check`, and seed the fixtures. That last group is what catches
[api/src/bin/local-tables.rs](api/src/bin/local-tables.rs) drifting from
[infra/dynamodb_test.tf](infra/dynamodb_test.tf), which is transcribed by hand.

`local/seed/*.json` is deliberately **not** under prettier: `local-seed extract` writes it
from Rust, so a formatter would fight the generator on every re-extract.

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
