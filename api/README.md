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
(`is_super`, `location_grants`, `location_read_only_grants`, session location), so authorization still applies
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

## OAuth authorization server

`poem`/`poem-local` also serve a small OAuth 2.1 + PKCE authorization server —
the plumbing an AI client (Claude Code, a claude.ai connector, …) uses to get a
token that acts as a specific user, with exactly their permissions. It's laid
out per the MCP authorization spec, though nothing here is MCP-specific yet
(see `oauth.rs`'s module docs and CLAUDE.md's plan for how a future MCP
endpoint and other credential types slot into the same tables/tokens):

- `GET /.well-known/oauth-authorization-server` — RFC 8414 metadata.
- `POST /oauth/register` — RFC 7591 dynamic client registration. Public
  clients only (`token_endpoint_auth_method: "none"`); nothing is stored
  server-side — the returned `client_id` is a self-verifying, HMAC-signed blob
  (see `oauth::encode_client_id`/`decode_client_id`).
- `POST /oauth/token` — RFC 6749 §3.2 token endpoint, form-encoded. Handles
  the `authorization_code` grant (PKCE S256 required) and `refresh_token`
  rotation (OAuth 2.1 reuse detection: presenting an already-rotated refresh
  token revokes the whole grant).

The consent screen a user approves/denies from is a web page
(`/admin/oauth/authorize`), backed by the `oauthAuthorizationRequest` query and
`approveOauthAuthorization` mutation — ordinary GraphQL, guarded like anything
else (`AuthRequirement::User`).

All three HTTP endpoints are implemented in `oauth_http.rs` as plain
`(app, method, path, headers, body) -> HttpReply` functions with no
framework dependency, so `server.rs` (poem) and `bin/lambda/handler.rs` (raw
`lambda_http`) each wrap them in a few lines rather than sharing a framework.

Config: `API_BASE_URL` (optional) fixes the issuer/endpoint URLs instead of
deriving them from the request's `Host` header — set it if the server sits
behind something that changes the host a client sees. `WEB_BASE_URL` (see
CLAUDE.md) doubles as the origin the `authorization_endpoint` metadata points
the browser at, via the shared `base_url::web_base_url()` helper.

### Connected apps (list + revoke)

Once a user has approved a client, `User.oauthGrants` (guarded the same way as
`user(id)` — the user themselves, or a super user) lists their authorized
grants: client name, redirect host, scope, created/last-used timestamps, and
`refreshExpiresAt` (when the grant goes dead if never used again). Token
hashes and the client id are never exposed. Expired grants are filtered out
client-side, since DynamoDB's TTL deletion lags real expiry.

`revokeOauthGrant(id)` deletes a grant outright — the caller's own, or (for a
super user) anyone's; anything else fails the same "not found" way a missing
grant would, so a caller can't probe for other users' grant IDs. There is no
"revoke on disable": disabling a user already blocks every credential kind
via `fetch_update_user_auth_info`, and re-enabling should restore access the
same way it does for other token types, rather than forcing every connected
app to be reconnected.

The web page is `/admin/settings/connected-apps` (linked from the Settings
submenu, beside Passkeys); a super user also sees and can revoke another
user's grants from that user's edit page.

## MCP interface

`poem`/`poem-local` also serve an [MCP](https://modelcontextprotocol.io)
endpoint at `POST <API base>/mcp` — the "Streamable HTTP" transport, run
stateless with plain JSON responses (no `Mcp-Session-Id`, no SSE; Lambda can't
hold sessions). It lets an AI client (Claude Code, a claude.ai custom
connector, Cursor, …) work with seslogin on your behalf, acting **as
you, with exactly your permissions** — see `mcp/mod.rs`'s module docs for why this
is a small hand-rolled JSON-RPC 2.0 dispatcher rather than the `rmcp` crate,
and for how every tool call still goes through the same GraphQL guards the
admin UI does.

**Connecting:**
- **Claude Code**: `claude mcp add --transport http seslogin <API base>/mcp`
  — it discovers the OAuth metadata, opens the consent page
  (`/admin/oauth/authorize`) in your browser, and stores the resulting token.
- **claude.ai**: add it as a custom connector with the URL `<API base>/mcp`.
- Any other MCP client that supports the authorization spec's discovery flow
  (RFC 9728 protected-resource metadata → RFC 8414 AS metadata → RFC 7591
  dynamic client registration → the consent page → the token endpoint) works
  the same way; see "OAuth authorization server" above for those endpoints.

**`<API base>` per environment:** `https://test.seslogin.com`,
`https://preprod.seslogin.com` and `https://seslogin.com` (prod), so the MCP URL
is e.g. `https://seslogin.com/mcp`. CloudFront routes `/mcp`, `/oauth/*` and
`/.well-known/oauth-*` to the API Lambda (as it does `/graphql`), and adds CORS
headers for browser-based MCP clients through a response headers policy
(`infra/web_test.tf`, `infra/web_preprod.tf`, `infra/web_prod.tf`). The handler
answers `OPTIONS` on those paths with a bare `204`
(`oauth_http::is_cors_preflight_path`). `API_BASE_URL` is set on all three
because CloudFront doesn't forward `Host`. Prod is also served at
`new.seslogin.com`, but the advertised MCP host is the apex: use
`https://seslogin.com/mcp`. Each Lambda's raw Function URL still works, with its
own CORS config allowing only the site origin.

Preprod and prod share a database and secrets, so an MCP client connected to
either acts on **live production users** with the connecting user's permissions.

**Auth:** only an OAuth `slat_` access token audience-bound to `<API
base>/mcp` is accepted (an `slu_` user token or a JWT is rejected here, same
as an `slat_` token is rejected on the GraphQL endpoint — see `oauth.rs`). A
missing or invalid token gets a `401` carrying `WWW-Authenticate: Bearer
resource_metadata="<API base>/.well-known/oauth-protected-resource/mcp"`, per
the MCP authorization spec's discovery flow.

**Tools:**

| tool | what it does |
|---|---|
| `whoami` | Who you're authenticated as: id, email, `isSuper`, the locations you can access, and your own settings (`emailSummaryLocationIds`, `disaggregateVirtualPeriods`). |
| `list_users` | List every admin user. Super user only. |
| `get_user` | Look up one user by id. A non-super caller may only look up themselves. |
| `create_user` | Create a user (email, `isSuper`, `locationGrants`, optional `readOnlyLocationGrants`). Super user only. |
| `update_user` | Change a user's email/`isSuper`/`isDev`/`locationGrants`/`readOnlyLocationGrants`; omitted fields keep their current value. Never touches `enabled`. Super user only. |
| `disable_user` / `enable_user` | Flip `enabled` — seslogin has no user delete, only disable. Super user only. |
| `list_locations` | The locations you can access (id, name, `enabled`, `viewerCanEdit`, `nitcEnabled`, `lastSuccessfulMemberSync`): every enabled one for a super user, otherwise your Admin and Read only grants. Start here for location ids. |
| `get_location` | One location's details (adds NITC and SES headquarters settings, created/updated). Needs a grant for it. |
| `list_members` | A location's active members, optionally filtered by `query` (case-insensitive substring of first/last/full name or member number). Deleted members are never listed. Each carries `syncManaged` (`sesApiPersonId` set). Admin, Read only or super user. |
| `get_member` | One member, with their `lastPeriod`. Needs a grant for their location. |
| `find_member_by_number` | As `get_member`, looked up by member number. "Not found" and "no grant" give the same error. |
| `create_member` / `update_member` / `delete_member` | Manage members. `update_member` changes only the fields passed. Admin (or super user) only; Read only is refused. Members owned by member sync (`syncManaged`) can be changed but the next sync may undo it, and the admin site doesn't offer it. |
| `sync_members` | Queue a member sync for a location. Admin (or super user) only. |
| `list_categories` / `get_category` | The activity categories a period is filed under (name, `enabled`, `isVirtual`, NITC group). Disabled ones are left out of the list unless `includeDisabled`. Global, so any signed-in user. |
| `list_periods` | A location's activity periods, newest first, with member name/number (or `guestName`), category, comment and sign-in/out kiosk names. `onlyActive` is who is signed in now (the Current page); `start` and `end` (together) select by when a period started; `categoryIds`, `first` and `after` filter and page. This also covers the Reports page's export. Admin, Read only or super user. |
| `list_member_periods` | One member's periods, same shape and paging, optionally by `categoryIds`. Needs a grant for the member's location. |
| `get_period` | One period. Needs a grant for its location. |
| `create_period` | Add a finished period for a member (member, location, category, start, end, optional comment). Admin (or super user) only. |
| `update_period` | Change times, comment and category (member periods, via `updatePeriodTimeCategory`) or guest name (guest periods, via `updateGuestPeriod`), as the admin edit page does. Only passed fields change; `comment: null` clears it, leaving it out keeps it. Admin (or super user) only. |
| `delete_period` | Delete a period. Admin (or super user) only. |
| `send_period_edit_link` | Email the member a link to correct a period. Sends real email; subject to a reminder cooldown; guests can't be sent one. Admin (or super user) only. |
| `get_dashboard` | The admin Home page for a location: member and check-in counts (24h / 7d / 30d, with the virtual split), total and average durations, kiosk counts, last member sync, the 7 daily buckets and the top categories. `asOf` moves the windows. Durations are seconds; the daily buckets are UTC days. Admin, Read only or super user. |
| `summarize_activity` | Time per member / category over a range, grouped by `groupBy`: `member`, `category`, `member_category`, `category_member`, `day_category_member` or `day_member` (the Totals, Breakdown, Daily breakdown and Heatmap pages). `start`/`end` select by when a period started. `categoryIds` is accepted only by `member` and `day_member`; other groupings reject it. Durations are seconds (completed periods only); a day's `date` is a Sydney `YYYY-MM-DD`. Admin, Read only or super user. |
| `members_last_seen` | Each active member of a location with when they were last seen (end of their latest period, or its start if still open, at any location; null if never) and `secondsAgo`; `sort` by `recent`, `least_recent` or `alphabetical` (the Last seen page). Admin, Read only or super user. |
| `list_kiosks` | A location's kiosks, most recent contact first: name, `lastContact`, client version, `keyFingerprint`, `keyExpiresAt`, `reactivatable`, `healthcheckUrl`, `config` and the kiosk's self-reported `clientInfo`. `keyReleasedAt` set marks a replaced leftover that no longer works. Admin, Read only or super user. |
| `get_kiosk` | One kiosk, same shape. Needs a grant for its location. |
| `update_kiosk` | Change `name`, `config` (a JSON object that replaces the whole config) or `healthcheckUrl`; only passed fields change, `healthcheckUrl: null` clears it, leaving it out keeps it. Admin (or super user) only. |
| `delete_kiosk` | Delete a kiosk (the device must be set up again by QR code). Also how to remove a replaced leftover. Admin (or super user) only. |
| `create_category` / `update_category` | Add or change an activity category (`name`, `isVirtual`, `enabled`, NITC group and participant type). `update_category` changes only the fields passed; `nitcGroupId` and `nitcParticipantType` can be cleared with `null`. Super user only. |
| `list_nitc_groups` / `get_nitc_group` | The NITC groups (SES non-incident type and tags) with the `categories` that export as each. Super user only. |
| `create_nitc_group` / `update_nitc_group` / `delete_nitc_group` | Manage NITC groups. `update_nitc_group` changes only the fields passed (`nitcTagIds` replaces the whole list). Deleting does not check categories still using the group. Super user only. |
| `list_ses_nonincident_tags` / `list_ses_reference_data` | The valid values for those forms: SES tag ids, `nonincidentTypes` (a group's `nitcType`) and `participantTypes` (a category's `nitcParticipantType`; fetched from SES, so can fail when SES is unreachable). Super user only. |
| `create_location` / `update_location` | Add or change a location: `name`, `enabled`, `nitcEnabled` (when NITC export starts; `null` turns it off) and `nitcCompleteOnExport`. `update_location` changes only the fields passed. No SES headquarters field: the site's pages don't set it (location sync does). Super user only. |
| `list_api_tokens` / `get_api_token` | API tokens (`slgn_`): name, `locationGrants`, `readOnly`, creator, `expiresAt`, `revokedAt`, `lastUsedAt`. Never a secret or hash. Super user only. |
| `update_api_token` / `revoke_api_token` | Change a token's name, locations, `readOnly` or `expiresAt` (only passed fields change; `expiresAt: null` = never expires), or revoke it for good. Super user only. |
| `get_audit_log` | The record of changes, newest first: `entries` (time, action, `entityType`, `entityLabel`, location, actor, field-level `changes`) and `pageInfo`. With `locationId` it is that location's log, open to Admin, Read only and super users; without it, every location plus global records (users, categories, tokens), super user only. `entityTypes` filters by kind; `first` (max 200) and `after` page. `ip` and `actor.via` are shown to super users only. |
| `update_my_settings` | Change your own `dailyEmailLocationIds` (the complete list of locations whose daily summary you get; locations you can access only) and/or `disaggregateVirtualPeriods`. Passing neither is an error. |
| `list_connected_apps` / `revoke_connected_app` | The AI clients and other apps you have authorised (the Connected apps page): client name, redirect host, created/last used, `refreshExpiresAt`, with `thisConnection` on the grant the call itself came in on. Revoking that one ends the session (`endedThisConnection`). `userId` (list) is super user only for anyone but yourself; a super user can revoke anyone's grant, anyone else only their own. |
| `list_passkeys` / `rename_passkey` / `delete_passkey` | Your passkeys: id, name, created/last used, never key material. Rename and delete work on your own passkeys only, even for a super user; `list_passkeys`'s `userId` is super user only for anyone but yourself. Adding a passkey is done in the browser. |
| `submit_feedback` | Send the seslogin team a help request as a Toolbox ticket from your own address (subject, message). Sends real email. If Toolbox isn't configured on the server the tool says so and points at support@seslogin.com, like the site's help dialog. |

**Not exposed, on purpose:**

- *Creating API tokens* (`createApiToken`). An MCP grant is revocable and expires, while an
  `slgn_` token is a long-lived credential that works on `/graphql` outside this tool set,
  so a grant must not be able to mint one. The API token tools never return a secret or
  hash.
- *Enrolling or reactivating kiosks, and the pending-enrolment lookup* (`enrollSession`,
  `reactivateSession`, `pendingEnrollmentKey`). A device is only bound to a location by a
  person scanning its QR code in the browser.
- *Login, logout and auth codes.* They mint or end the credentials this endpoint is
  already authenticated with.
- *Passkey registration and login.* The WebAuthn ceremonies need the user's authenticator
  in the browser. Passkeys can be listed, renamed and deleted.
- *OAuth consent* (`approveOauthAuthorization`, `oauthAuthorizationRequest`). Approving an
  app is the user's own decision on the site, not an AI client's. Connected apps can be
  listed and revoked.
- *`refreshToken`.* It re-issues a kiosk's session JWT (`Session`-guarded) and means
  nothing for a user.
- *Kiosk-only (`Session`-guarded) and edit-link-only (`PeriodLink`) fields.* They
  authenticate a device or a member's single link, neither of which can be this user.

**Times.** Any time argument takes Unix seconds (an integer) or a string: RFC 3339
with an offset (`2026-10-16T09:00:00+11:00`), a local `YYYY-MM-DDTHH:MM[:SS]` /
`YYYY-MM-DD HH:MM[:SS]`, or a date `YYYY-MM-DD` (midnight). Strings without an
offset are Australia/Sydney local time (a time that happens twice at the DST
fall-back takes the earlier one; one that doesn't exist is an error). Results
keep each Unix-seconds timestamp and add a `<name>Sydney` RFC 3339 sibling
(`createdAt` and `createdAtSydney`). Durations stay in seconds. Report day buckets come
in two kinds: `dailyPeriods7D[].dayStart` in `get_dashboard` is a Unix time at the start
of a *UTC* day, while the `date` in `summarize_activity` is a Sydney calendar date
string.

**Partial updates.** `update_*` tools fetch the record, overlay only the arguments
you pass and send the whole thing: an omitted argument keeps its value, and on
a nullable field an explicit `null` clears it (stated per tool).

**Structure.** `src/mcp/mod.rs` is the transport; `src/mcp/args.rs` the argument,
time and merge helpers; `src/mcp/tools/<domain>.rs` the tools, each with a
`catalogue()` and a `call()` that `tools/mod.rs` chains.

Every tool runs a **fixed** GraphQL document against the in-process schema
with your verified identity attached exactly as the GraphQL endpoint itself
builds a request, so a non-super token gets the same authorization errors
from `list_users`/`create_user`/etc. that the admin UI would; only `whoami`,
`get_user` (on your own id), the category reads, the own-account tools (settings, connected apps, passkeys, feedback), `get_audit_log` for your own locations and the location/member/period/report/kiosk
tools (for your own locations) work; the category/NITC/location/API-token administration
tools are `SuperUser` in GraphQL and refuse everyone else. Where a GraphQL read doesn't itself check the caller's grants
(`location`, `Location.people`, `person` and the six `Location.periodSummaryBy*` resolvers behind `summarize_activity`), the tool does, and the period and kiosk
tools check again even where the resolver does (`period`, `Location.periods`,
`Person.periods`, `Query.session`, `Location.sessions`, `Location.auditLog`), so a tool only ever narrows access. A tool failure (a guard rejection, a
validation error) comes back as an ordinary JSON-RPC *success* whose
`CallToolResult` carries `isError: true` and the message — not a transport
error — so a client can show it inline rather than treating the whole request
as failed.

**Revoking access:** under Settings → Connected apps (see above), or by
disabling the user, which blocks every credential kind immediately.

**Local testing:** `make dev-local` runs `poem-local` with a permissive CORS
policy, so the browser-based
[MCP Inspector](https://github.com/modelcontextprotocol/inspector)
(`npx @modelcontextprotocol/inspector`) works against
`http://localhost:8000/mcp` end-to-end, including the OAuth flow. The same
Inspector run against a *deployed* server will fail the discovery/registration
requests to CORS — production intentionally doesn't allow arbitrary
browser-origin JS to hit its OAuth endpoints; use `claude mcp add` or a
claude.ai connector there instead.

## Audit log

Every business write (people, periods, kiosk sessions, users, locations, categories, API tokens, passkeys, OAuth grants, …) is recorded in the `{DB_PREFIX}_audit_log` DynamoDB table: who did it, which location(s) it touched, and which fields changed from what to what. The table is defined in `infra/dynamodb*.tf` and documented in `SCHEMA.md`; it must exist (`terraform apply`) for entries to land. It is read through GraphQL (below); there is no UI for it yet.

It is implemented by `audit::AuditingHandler`, a `db::Handler` wrapper that the servers, sync jobs and CLI wrap their DynamoDB handler in (`poem`, `poem-local`, the API Lambda, `member_sync::run`, `location_sync::run`, `cli`, `load-nitc-tags`). The actor comes from the `AUDIT_CONTEXT` task-local each entry point sets: `user` (with `actor_via` `oauth_grant:<id>` / `user_token:<id>` / `oauth_client:<id>` when not a plain web login), `session` (kiosk), `api_token`, `period_link`, `system` (`actor_id` is the job: `member-sync`, `location-sync`, `cli`, `load-nitc-tags`, `oauth-refresh-reuse`), `unauthenticated`, or `unknown` when a write was made with no context set (a bug; it logs a warning).

Auditing is **best-effort**: if recording an entry fails it is logged at `error!` (with the entity type and id) and the write still succeeds. Dry runs and read-only servers record nothing, because the write they would describe is refused. Updates that change nothing record nothing.

### Reading the log (GraphQL)

Both views are Relay connections, **newest first**, default page 50 (max 200), and take `first`/`after` (towards older entries), `last`/`before` (back towards newer ones) and an optional `entityTypes` filter (any of the listed kinds; omitted or empty means every kind — the single `entityType` argument it replaces is deprecated but still accepted, and adds to the list). A cursor is the item's `sk` (`{ts:010}#{id}`); a malformed one is a `BAD_REQUEST` error. `first` with `before`, or `last` with `after`, is rejected.

| Field | Who may read it | Shows |
|---|---|---|
| `Location.auditLog` | A *user* with Admin **or** Read only access to the location, or a super user. Kiosk sessions, API tokens and edit links are refused even for their own location. | Entries at that location. A write touching two locations (a member moved between units) is in both logs. Global records (users, categories, …) are not here. |
| `Query.auditLog` | Super users only | Every event once, including global records. |

Each `AuditEntry` has `timestamp` (Unix seconds), `action`, `entityType`, `entityId`, `entityLabel`, `location` (the item's own location; the first one a multi-location write touched in the all-locations view; null for global records), `actor { kind id via label }` and `changes { field before after }`. `before`/`after` are text: a JSON string comes back raw, anything else as compact JSON. `actor.label` is looked up at read time (user email, kiosk name, API token name, job name, "Period edit link"); an unknown `actor_kind` reads as `UNKNOWN`. `ip` and `actor.via` are personal data and are returned to **super users only** (null otherwise).

An `entityTypes` filter is a `FilterExpression` (`entity_type IN (...)`), so a rare type in a busy log can read many rows to fill a page; a malformed row in the table is logged and skipped rather than failing the page.

To inspect entries locally after `make dev-local` / `make local-e2e`, scan the table in DynamoDB Local:

```bash
aws dynamodb scan --table-name seslogin_local_audit_log --endpoint-url http://localhost:8100 --region ap-southeast-2
```

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

## Converting setup-code kiosks to key auth

Kiosks enrolled with the removed 6-digit setup code authenticate with a session JWT. Each
one moves itself onto key auth, without an admin: while still on its JWT, the kiosk calls
`convertSessionToKey(publicKey, proof)`. `proof` is its ordinary `SLKey` request
signature (minus the scheme) over the body `slkey-convert:<session id>`, which shows it
holds the private half of the key. The server attaches the key to the caller's own session
(`SessionUpdateShape::AttachKey`, conditional on the row being live with no key and never
released) and the kiosk reloads into key mode.

When a kiosk converts is decided in the client (`web/src/kiosk/lib/keyConversion.ts`), not
the server. The mutation is idempotent for the same key, and returns `CONFLICT` if another
session holds the key; the kiosk then stays on its JWT and tries again later. JWTs keep
working for converted sessions until they are rejected outright.

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
