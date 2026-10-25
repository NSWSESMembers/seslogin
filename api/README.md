# GraphQL API server for SESLogin

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
the plumbing an AI client (Claude Code, a claude.ai connector, …) or one of
SESLogin's own apps uses to get a token that acts as a specific user, with
exactly their permissions. It's laid out per the MCP authorization spec (see
`oauth.rs`'s module docs):

- `GET /.well-known/oauth-authorization-server` — RFC 8414 metadata.
- `POST /oauth/register` — RFC 7591 dynamic client registration. Public
  clients only (`token_endpoint_auth_method: "none"`); nothing is stored
  server-side — the returned `client_id` is a self-verifying, HMAC-signed blob
  (see `oauth::encode_client_id`/`decode_client_id`).
- `POST /oauth/token` — RFC 6749 §3.2 token endpoint, form-encoded. Handles
  the `authorization_code` grant (PKCE S256 required) and `refresh_token`
  rotation (OAuth 2.1 reuse detection: presenting an already-rotated refresh
  token revokes the whole grant).

**Clients.** There are two kinds (`oauth_clients.rs`), and which resource a
grant is bound to depends on which:

- **Dynamic** clients come from `POST /oauth/register`. Anyone can mint one,
  so their name is unverified and they are **MCP-only**: the token endpoint
  grants them `<API base>/mcp` and refuses any other `resource`
  (`invalid_target`).
- **Static** clients are SESLogin's own apps, listed in
  `oauth_clients::STATIC_CLIENTS`. Today that is Tally (`client_id` `tally`,
  public/PKCE, redirect `tally://oauth/callback`, resource `/graphql`). Each
  gets only the resource paths listed for it, appended to the API base; with
  no `resource` it gets the first. Redirect URIs are matched exactly and are
  not run through `validate_redirect_uri`, which is the only reason a custom
  scheme like `tally://` works — dynamic registration still requires
  `https://` or loopback `http://`. Static ids contain no `.`, so they can
  never collide with a dynamic id (`<payload>.<sig>`).

  **Expo Go development redirects.** A static client may also list
  `dev_expo_redirect_paths` (Tally: `/--/oauth/callback`), so a developer can
  sign in from Expo Go with `exp://<their LAN IP>:<port>/--/oauth/callback`,
  including against prod. Because whoever runs the Expo server at that address
  receives the code, it is fenced in (`oauth_clients::is_dev_expo_redirect`):
  only a user with `isDev` (set by a super user on the user's edit page) can
  see or approve one, so a phishing link with an `exp://` redirect fails for
  everyone else; the host must be loopback, RFC 1918 or link-local IPv4 (no
  public hosts, no `*.exp.direct` tunnels); the port, path and absence of a
  query/fragment are checked; and dynamic clients never get one. The consent
  page warns that it is a development build and names `ip:port`
  (`isDevelopmentBuild`), and Connected apps shows the IP rather than
  "SESLogin app".

  **Adding a static client** is a PR editing `STATIC_CLIENTS`: a new id with
  no `.`, the app's name, its exact redirect URIs, and its resource paths
  (`base_url::GRAPHQL_RESOURCE_PATH` and/or `MCP_RESOURCE_PATH`). The unit
  test over the list checks the shape. A static client listed for `/graphql`
  gets the whole site API as the user, so only list first-party apps there.
  A custom scheme can be claimed by any app on the device; PKCE stops a stolen
  code being redeemed, but not an impostor app starting its own flow, which is
  why the consent page still tells the user to approve only a sign-in they
  started.

  **Member sign-in clients.** A static client with `member_sign_in` (Tally)
  never uses the consent screen — `oauthAuthorizationRequest` and
  `approveOauthAuthorization` refuse it — and its grants always name a member;
  see "Tally sign-in" below. The token endpoint refuses a code naming a member
  for any other client, and one naming none for a member sign-in client.

`oauthAuthorizationRequest.isFirstParty` and `OAuthGrant.isFirstParty` say
whether the client is static; the consent page and Connected apps name such
an app as SESLogin's own instead of showing its custom scheme's meaningless
redirect "host".

**Using a token.** A grant works only on the endpoint its `resource` names,
matched exactly by `oauth::verify_access_token`: `POST /mcp` expects
`<API base>/mcp`, and the GraphQL endpoint (`auth::verify_token`) expects
`<API base>/graphql`. Both derive that from `base_url::api_resource`, so
behind CloudFront `API_BASE_URL` decides it and elsewhere the `Host` header
does. Locally GraphQL is served at `http://localhost:8000/`, but the resource
is still `http://localhost:8000/graphql`: it's an identifier, not a route.
`slrt_` refresh tokens are accepted only by `/oauth/token`.

**Credentials a grant can't mint.** A caller authenticated with an `slat_`
token (`AuthInfo::User { grant_id: Some(_), .. }`) is refused, by
`graphql::auth::require_not_oauth_grant`, anything that would hand out a
credential outliving or escaping the grant: `createApiToken`,
`beginPasskeyRegistration`/`finishPasskeyRegistration`,
`approveOauthAuthorization`, and `enrollSession`/`reactivateSession` (which
bind a kiosk key to a location). Revoking, deleting and renaming stay allowed:
they are within the user's own permissions. This applies to MCP and GraphQL
grants alike. Writes through either are audited with
`actor_via = oauth_grant:<id>`.

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

### Tally sign-in

Members sign in to Tally without a user record (`tally_sign_in.rs`). It is its
own flow, so it can never yield an admin session: the app opens
`<WEB_BASE_URL>/tally/sign-in` with an ordinary authorization request
(`client_id`, `redirect_uri`, PKCE `code_challenge`, `state`), the page asks
for an email, and the emailed 6-digit code is redeemed straight for an OAuth
authorization code — no `slu_` token is ever issued. Three unauthenticated
GraphQL fields back it:

- `tallySignInRequest(clientId, redirectUri)` checks the link: a member sign-in
  client and one of its exact redirects (or an Expo Go development redirect,
  flagged `isDevelopmentBuild`).
- `requestTallySignInCode(email, turnstileToken)` emails a code (Turnstile
  required, at most one per 30 seconds, valid 10 minutes, five guesses), but
  only if the email can sign in. It answers `true` either way.
- `completeTallySignIn(email, code, request)` redeems the code and returns the
  app's redirect URL with `code` and `state`. A wrong, expired or used-up code
  fails the same way. A development build needs the user signing in alongside
  to be a developer (`isDev`).

Who an email signs in as (`tally_sign_in::identify`): an enabled user linked to
a live member is that user and member; otherwise exactly one live member with
that email (compared trimmed and lowercased, through the person table's
`email_key-index`), plus the enabled user with that email if there is one;
otherwise nobody — a user with no member, or an email several members share.
A disabled user counts as no user. The grant names the member and that user,
and resolves fresh on every use to `AuthInfo::Member` (see CLAUDE.md), so the
member's unit is their home unit plus that user's units.

Codes share the admin login's `login_code` table. That is safe: a code proves
the inbox is theirs whichever flow sent it, and redeeming one here yields only
a Tally grant. A Tally request does replace a pending admin code for the same
email, and the 30-second limit is shared.

Locally (`make dev-local`) the seeded members have emails
(`alice@seslogin.test`, `bob@seslogin.test`, `crossunit@seslogin.test`), Turnstile is off (`local/local.env`), and the
code is in the API log (and in `MOCK_MAIL_DIR`, if you set it).
`testunit@seslogin.test` is a user with no member, so it is refused.

### Connected apps (list + revoke)

Once a user has approved a client, `User.oauthGrants` (guarded the same way as
`user(id)` — the user themselves, or a super user) lists their authorized
grants: client name, redirect host, `isFirstParty`, scope, created/last-used timestamps, and
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
connector, Cursor, …) work with SESLogin on your behalf, acting **as
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
base>/mcp` is accepted — which only dynamically registered clients can get. An
`slu_` user token, a JWT, or an `slat_` token bound to `/graphql` (a static
client's) is rejected here, just as an `/mcp`-bound token is rejected on the
GraphQL endpoint; see "Using a token" above. A
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
| `disable_user` / `enable_user` | Flip `enabled` — SESLogin has no user delete, only disable. Super user only. |
| `set_user_member` | Set (`personId`) or clear (`null`) the member a user acts as when signing in or out from the mobile app. Any user, the caller included; several users may share a member. Super user only. |
| `list_locations` | The locations you can access (id, name, `enabled`, `viewerCanEdit`, `nitcEnabled`, `lastSuccessfulMemberSync`): every enabled one for a super user, otherwise your Admin and Read only grants. Start here for location ids. |
| `get_location` | One location's details (adds NITC and SES headquarters settings, created/updated). Needs a grant for it. |
| `list_members` | A location's active members, optionally filtered by `query` (case-insensitive substring of first/last/full name or member number). Deleted members are never listed. Each carries `syncManaged` (`sesApiPersonId` set). Admin, Read only or super user. |
| `get_member` | One member, with their `lastPeriod`. Needs a grant for their location. |
| `find_member_by_number` | As `get_member`, looked up by member number. "Not found" and "no grant" give the same error. |
| `create_member` / `update_member` / `delete_member` | Manage members. `update_member` changes only the fields passed. Admin (or super user) only; Read only is refused. Members owned by member sync (`syncManaged`) can be changed but the next sync may undo it, and the admin site doesn't offer it. |
| `sync_members` | Queue a member sync for a location. Admin (or super user) only. |
| `list_categories` / `get_category` | The activity categories a period is filed under (name, `enabled`, `isVirtual`, NITC group, kiosk `groupName` / `shortName` / `icon`). Disabled ones are left out of the list unless `includeDisabled`. Global, so any signed-in user. |
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
| `create_category` / `update_category` | Add or change an activity category (`name`, `isVirtual`, `enabled`, NITC group and participant type, kiosk display `groupName` / `shortName` / `icon`). `update_category` changes only the fields passed; `nitcGroupId`, `nitcParticipantType`, `groupName`, `shortName` and `icon` can be cleared with `null`. Super user only. |
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

## Tally (members)

Tally is for **members**, not users: every Tally field below admits only an
`AuthInfo::Member`, which only Tally sign-in issues (see "Tally sign-in"). A
member can use their home unit, plus every unit the user signed in alongside
them holds either role at (every unit for a super user); `require_member_location`
in `graphql/auth.rs` is the one check. Any such member may read **and** write a
unit's shared tools — they are the unit's own, not admin data. Users on their
own (even super users), kiosks, API tokens and edit links are refused.

The apps start from two queries (`graphql/tally.rs`):

- `tallyViewer` — the member (`member`, `name`, `homeLocationId`), the email of
  the user signed in alongside if any (`userEmail`), and every unit they can use
  (`locations`, by name).
- `tallyLocation(id)` — one unit's tools: `tasks`, `taskLists`, `notes`,
  `noteFolders`, `equipment`, `equipmentReservations`,
  `equipmentReservationConflicts`, `mapPins`, `mapLayers`, `calendars`,
  `calendarEvents`, `locationShares` and `onCallRoles`. The core `Location` type
  carries none of them.

Everything a member writes records their **person ID** — `created_by`,
`updated_by`, `completed_by`, `reserved_for_person_id`, the location share's key
and the notification inbox's key — and `createdBy`/`updatedBy`/`reservedFor` and
the like show the member's name. Audited writes record `actor_kind: member`.

## Tasks

A location's shared to-do list, used by the Tally app (`src/graphql/tasks.rs`, rules in
`src/tasks.rs`, storage in SCHEMA.md's `task` and `task_list` sections).

- **Who:** any member signed in to Tally who can use the unit — see "Tally (members)" above. Users on their own, kiosk sessions, API tokens and edit links are refused.
- **Reading:** `TallyLocation.tasks(view: DUE | SCHEDULED, listId)` lists open tasks. `DUE` (the
  default) is everything showing today or earlier, overdue first; `SCHEDULED` is everything
  showing from a later day. Without `listId` every task at the location is listed, whatever
  list it is on. `TallyLocation.taskLists` and `Query.task(id)` complete the reads.
- **Dates** are Sydney-local days, `YYYY-MM-DD` in and out. A task shows from its `dueDate`,
  or from the day it was created when it has none. It is `overdue` from the day after its due
  date; a task that repeats in **weeks** gets a full week of showing first.
- **Recurring tasks** (`recurrence: { every, unit: DAYS | WEEKS }`) always have a due date
  (today if none was given). Completing one moves it to `every` days/weeks after the day it
  was completed, and it stays open; completing any other task closes it.
- **Concurrency:** `updateTask` and `completeTask` take `expectedVersion`; if the task was
  changed or completed since (say, two people tapping Complete at once), the second gets a
  `CONFLICT` error rather than skipping an occurrence.
- Deleting a list leaves its tasks on the location (`Task.list` becomes null).

```graphql
mutation {
  createTask(locationId: "TestAUnit001", title: "Wash the truck",
             recurrence: { every: 1, unit: WEEKS }) { id version dueDate }
}
```

## Contacts sync API

`GET /api/v1/contacts/sync?cursor=<opaque>&limit=<n>` serves the member directory as a
cursor-based change feed, for the Tally app's offline contact list
(`src/contacts.rs`). Every member, statewide, with only these fields:

```json
{
  "contacts": [
    { "id": "…", "firstName": "Ann", "lastName": "Smith", "memberNumber": "40064936",
      "locationId": "…", "unit": "Alpha Unit", "email": "…", "updatedAt": 1783655893 },
    { "id": "…", "deleted": true }
  ],
  "nextCursor": "…",
  "hasMore": false,
  "total": 12034
}
```

- **No cursor** is a full sync: every live member, page by page, with `total` on the
  first page. Tombstones are left out; the client drops anything it didn't receive.
- **With a cursor** it returns what changed since: upserts, plus `{id, deleted: true}`
  for members soft-deleted since. A member marked missing by member sync stays listed
  until they're actually deleted.
- `nextCursor` is always present, including on the last page. `limit` defaults to 1000
  and is clamped to 1–2000.
- `410 {"error":"cursor_expired"}` means the cursor is unusable (malformed, tampered
  with, an old format, or from another database); the client starts a full sync.
  Tombstones are never purged, so a cursor never goes stale on its own.
- `401` means no token, a bad one, or a credential that isn't a Tally member's. Callers
  are members signed in to Tally (their `/graphql`-bound Tally sign-in grant). Users on their
  own, kiosks, API tokens, edit links and MCP grants all get 401. It is never 403,
  which CloudFront would rewrite to the web app's `index.html`.
- Responses are gzip-compressed when the request sends `Accept-Encoding: gzip`.

**How the order works.** Each person row carries a `contact_key`
(`<unix µs, 20 digits>#<id>`), restamped by every write that changes a returned field
and by a rename of their location. A sparse GSI orders everyone by it (see SCHEMA.md),
and the cursor is the last key sent, HMAC-signed with a key derived from `JWT_SECRET`.
A read never serves keys from the last 30 seconds: a write takes its key from the clock
moments before it lands, so without that margin a client could move past a key that
had not been written yet. A change is therefore delivered on the first sync at least
30 seconds after it.

A full sync's first request (the one without a cursor) is logged at `info`
(`contacts: full sync page`, with the member's person id and row count); requests with a cursor
aren't.

**New environments** need the GSI (`terraform apply`) and a one-off backfill, so
existing rows are in it:

```bash
make local-cli ARGS="--dry-run person stamp-contacts"   # or `cargo run --bin cli -- …` against a real table
```

**Locally**, after `make dev-local`:

```bash
curl -s --compressed -H 'Authorization: Bearer slu_TokenReadOnly.localdev-readonly' \
  'http://localhost:8000/api/v1/contacts/sync?limit=2'
```

## Notes

A location's shared notebook, used by the Tally app (`src/graphql/notes.rs`, rules in
`src/notes.rs`, storage in SCHEMA.md's `note` and `note_folder` sections).

- **Who:** any member signed in to Tally who can use the unit — see "Tally (members)" above. Users on their own, kiosk sessions, API tokens and edit links are refused.
- **Reading:** `TallyLocation.notes(folderId, search)` lists the location's notes, most recently
  changed first. Without `folderId` every note at the location is listed, whatever folder it
  is in. `search` keeps notes whose title contains it, ignoring case. `TallyLocation.noteFolders`
  (by name) and `Query.note(id)` complete the reads.
- **Fields:** `title` is one trimmed line (max 200 characters). `content` is plain text, kept
  exactly as written (max 20,000 characters; only-whitespace counts as empty). It is expected
  to grow markdown-like formatting later.
- **Writes:** `createNote`, `updateNote` (omitted arguments unchanged, `folderId: null` takes
  the note out of its folder), `deleteNote`, `createNoteFolder`, `renameNoteFolder`,
  `deleteNoteFolder`. Deleting a folder leaves its notes on the location (`Note.folder`
  becomes null).
- **Concurrency:** `updateNote` takes `expectedVersion`; if someone else saved the note since
  it was read, the save fails with `CONFLICT` rather than overwriting their change.
- **Tables:** only the test tables (`seslogin_test_note`, `seslogin_test_note_folder`) exist
  so far, so a deployed API on the prod tables fails Notes calls, and nothing else.

```graphql
mutation {
  createNote(locationId: "TestAUnit001", title: "Radio channels",
             content: "Ops: 1\nAdmin: 2") { id version }
}
```

## Self sign-in (Tally)

A member signs themselves in and out from the Tally app, exactly as they would by typing
their member number at a kiosk at their home unit (`src/graphql/self_sign_in.rs`; the two
mutations are in `mutations.rs`).

- **Who:** only a member signed in to Tally. Users on their own, kiosk sessions, API tokens
  and edit links get `FORBIDDEN`. No location grant is needed: being the member is the whole
  authority, read fresh on every call, and the member must still exist and not be deleted.
  The MCP interface deliberately has no tool for this.
- **Home unit only:** only periods at the member's own `location_id` count, as the kiosk
  scopes by location. A period open at another unit (say, via that unit's kiosk) is ignored,
  and `selfSignIn` may open a home period alongside it.
- `Query.selfSignInStatus` returns `{ person, location, openPeriod, quickPick }`, or null
  when the caller has no linked member. `quickPick` is the kiosk's sign-out shortcuts
  (`build_quick_pick`), computed only when selected, and null if building them fails.
- `SelfSignInStatus.whosHere` is the kiosk's "Who's here" list for the member's home unit:
  `[{ id, name, guest, startTime, isMe }]` for every open, non-deleted period there (members
  and guests), oldest first (ties by id), capped at 500. `name` is "First Last", the guest's
  name ("Guest" if blank) or "Unknown member" for a missing or deleted member; `isMe` marks
  the caller's own period. It is deliberately narrow (no guest reason, member number or
  other period fields), needs no location grant, and is computed only when selected. The
  location is always the linked member's `location_id`, never caller-supplied.
- `selfSignIn` returns the newest open home period unchanged if there is one (a double tap
  never opens two), otherwise opens one with no kiosk session and publishes the realtime
  event. Without a linked member it fails `BAD_REQUEST`.
- `selfSignOut(id, expectedVersion, startTime, endTime, categoryId)` takes Unix seconds and
  refuses (messages are written for the member to read): `NOT_FOUND` if the period is
  missing or deleted; `BAD_REQUEST` for a guest period; `FORBIDDEN` if it is not the linked
  member's; `CONFLICT` if it is already signed out or `version != expectedVersion`;
  `BAD_REQUEST` unless `startTime < endTime`, the end is at most 60 seconds ahead of the
  server clock, and the category exists and is enabled. It leaves the comment alone, records
  no sign-out session, then enqueues the NITC export and publishes the realtime close, as
  `scanSignOut` does.
- **Race safety:** the write is `db::PeriodUpdateShape::TimeCategory` with
  `expected_version: Some(..)`, conditional on `v = :expected AND attribute_not_exists(end_time)`.
  A kiosk or admin that gets in between makes it fail with `db::Error::Conflict`, surfaced as
  `CONFLICT`, so an app sign-out never silently overwrites another. Every other caller passes
  `None`, which only requires the row to exist.
- Writes are audited with the member as the actor, like any other period write. Periods made
  this way have no `signedInSession` / `signedOutSession`, which the admin pages already
  render as plain times (and blank cells in report exports), like admin-created periods.

```graphql
query { selfSignInStatus { person { firstName } openPeriod { id version startTime }
  whosHere { id name guest startTime isMe } } }
mutation { selfSignIn { id version } }
```

## Equipment

A location's equipment register, used by the Tally app (`src/graphql/equipment.rs`, rules in
`src/equipment.rs`, storage in SCHEMA.md's `equipment` and `equipment_log` sections).

- **Who:** any member signed in to Tally who can use the unit — see "Tally (members)" above. Users on their own, kiosk sessions, API tokens and edit links are refused.
- **Reading:** `TallyLocation.equipment(search)` lists the location's equipment by name, as one flat
  list. `search` keeps items whose name or description contains it, ignoring case.
  `Query.equipment(id)` reads one; `Equipment.log` is its log, newest first.
- **Fields:** `type` is `VEHICLE`, `TRAILER`, `VESSEL`, `RADIO`, `DEVICE`, `BAG`, `BOX` or `TOOL`
  (more will come, so clients should cope with one they don't know). `name` is one trimmed
  line (max 200 characters); `description` is trimmed plain text (max 5,000). `status` is
  `IN_SERVICE`, `HAS_ISSUE` (still usable, but something needs fixing), `OUT_OF_SERVICE`,
  `MISSING` or `BORROWED`.
- **The log:** the status is never edited directly. `changeEquipmentStatus(id, status,
  comment)` changes it and logs the change with a comment saying why (required, max 1,000);
  `checkEquipment(id, comment)` logs that the caller checked it, comment optional, and leaves
  the status alone (it stamps `lastCheckedAt`). Each entry and its effect on the equipment are
  written in one DynamoDB transaction, conditional on the status it starts from, so the log
  always agrees with the status; a change that loses a race fails with `CONFLICT`.
  `createEquipment` (status `IN_SERVICE` unless given, optional `comment`) writes the first
  entry, with no `previousStatus`.
- **Edits:** `updateEquipment` (type, name and description; omitted arguments unchanged) takes
  `expectedVersion` and fails with `CONFLICT` if someone else edited it since it was read. Log
  entries don't bump the version. `deleteEquipment` hides the item; its log is kept.
- **Tables:** only the test tables (`seslogin_test_equipment`, `seslogin_test_equipment_log`)
  exist so far, so a deployed API on the prod tables fails Equipment calls, and nothing else.

```graphql
mutation {
  changeEquipmentStatus(id: "…", status: OUT_OF_SERVICE, comment: "Flat tyre") {
    status log { kind previousStatus status comment createdBy createdAt }
  }
}
```

## Equipment reservations

Bookings of one or more pieces of a location's equipment (usually a vehicle) for a time
range, used by the Tally app (`src/graphql/reservations.rs`, rules in `src/reservations.rs`,
storage in SCHEMA.md's `equipment_reservation` section).

- **Who:** any member signed in to Tally who can use the unit — see "Tally (members)" above. Users on their own, kiosk sessions, API tokens and edit links are refused.
- **Reading:** `TallyLocation.equipmentReservations(endingAfter, equipmentId)` lists the
  reservations that end after `endingAfter` (Unix seconds, default now) — upcoming and under
  way — by start time; `equipmentId` narrows to one piece of equipment. The index is keyed on
  the end time, so past reservations cost nothing to skip. `Query.equipmentReservation(id)`
  reads one.
- **Fields:** `equipmentIds` (at least one, max 20, repeats dropped; equipment deleted since
  stays) and `equipment` (the live ones, by name); `startTime` / `endTime` (Unix seconds; the
  end is after the start, at most 366 days later, and must be in the future when the range
  is set, though it may already have started); `description`, trimmed plain text (max 5,000).
- **Who it is for:** the caller, unless `reservedFor` names someone or something else —
  another person, an organisation, "Detailing / clean" — as one trimmed line (max 200; blank
  means the caller). Read back as `reservedFor` (the text, or the member's name),
  `reservedForMemberId` / `reservedForText` (exactly one is set) and
  `reservedForViewer` (whether it is the caller's).
- **Overlaps are allowed, and reported.** `EquipmentReservation.conflicts` lists the
  location's other reservations that overlap it in time and share a piece of equipment with
  it (times are half-open, so back-to-back reservations don't clash). It costs a query per
  reservation, so a client listing many should compare them itself.
  `TallyLocation.equipmentReservationConflicts(equipmentIds, startTime, endTime, excludeId)` asks
  the same question about a reservation not yet saved, so a client can warn before saving;
  `excludeId` leaves out the one being edited.
- **Writes:** `createEquipmentReservation(locationId, equipmentIds, startTime, endTime,
  reservedFor, description)`. `updateEquipmentReservation` changes only the arguments given
  (`reservedFor: null` makes it the caller's) and takes `expectedVersion`, failing with
  `CONFLICT` if someone else changed it since it was read; only a changed time range has to end
  in the future, so a finished one's description can still be fixed.
  `deleteEquipmentReservation` cancels it.
- **Tables:** only the test table (`seslogin_test_equipment_reservation`) exists so far, so a
  deployed API on the prod tables fails reservation calls, and nothing else.

```graphql
mutation {
  createEquipmentReservation(
    locationId: "…", equipmentIds: ["…"], startTime: 1793000000, endTime: 1793010800,
    reservedFor: "Detailing / clean"
  ) {
    id reservedFor conflicts { id reservedFor startTime endTime }
  }
}
```

## Calendar

A location's shared calendar of events, used by the Tally app (`src/graphql/calendar.rs`,
rules in `src/calendar.rs`, storage in SCHEMA.md's `calendar` and `calendar_event` sections).

- **Who:** any member signed in to Tally who can use the unit — see "Tally (members)" above. Users on their own, kiosk sessions, API tokens and edit links are refused.
- **Calendars:** `TallyLocation.calendars` (by name), `createCalendar(locationId, name, color)`,
  `updateCalendar(id, name, color)` (omitted arguments unchanged) and `deleteCalendar`. A colour
  is one of ten `CalendarColor`s (`RED`, `ORANGE`, `YELLOW`, `GREEN`, `TEAL`, `BLUE`, `INDIGO`,
  `PURPLE`, `PINK`, `GREY`); clients decide how each looks. An event is in one calendar or none.
  Deleting a calendar leaves its events, in no calendar (`calendar` and `calendarId` read null).
- **Timing:** an event is either **timed** — `startTime` / `endTime`, Unix seconds, end after
  start — or **all day** (`allDay: true`) — `startDate` / `endDate`, `YYYY-MM-DD`, the end
  inclusive and defaulting to the start. Dates have no time zone: an all-day event on the 5th is
  on the 5th wherever it is read. One occurrence lasts at most 366 days.
- **Repeats:** `repeat: DAILY` or `WEEKLY` (on the weekday it starts), through `repeatUntil`
  (`YYYY-MM-DD`, inclusive: the last date an occurrence may start on) or forever when that is
  null. A timed event repeats at the same Sydney wall-clock time, so a weekly 7:30pm meeting stays
  at 7:30pm across daylight saving (a time skipped when daylight saving starts moves an hour
  later). An occurrence is at most one repeat long (a day for daily, a week for weekly), so an
  event never overlaps its own next occurrence. Editing changes every occurrence: there are no
  per-occurrence exceptions.
- **Reading:** `TallyLocation.calendarEvents(from, to)` (Unix seconds, `to` after `from`, at most 400
  days) lists every occurrence that overlaps the window, repeats expanded, by start, at most
  5,000. Each `CalendarEventOccurrence` has its own `startTime` / `endTime` (timed) or
  `startDate` / `endDate` (all day) and the `event`. All-day occurrences are included when one of
  their dates overlaps the window in **any** time zone, so a client should check them against its
  own days. Every event is listed whatever calendar it is in; a client showing only some
  calendars filters on `event.calendarId`. Events are indexed on when their last occurrence
  ends, so finished ones cost nothing to skip. `Query.calendarEvent(id)` reads one.
- **Writes:** `createCalendarEvent`, `updateCalendarEvent` and `deleteCalendarEvent`. Update
  changes only the arguments given: `calendarId: null` takes it out of its calendar, `repeat:
  null` stops it repeating, `repeatUntil: null` makes it repeat forever; a new start with no end
  keeps the event's length, and switching `allDay` needs the new start (and end, for timed).
- **Concurrency:** `updateCalendarEvent` takes `expectedVersion`; if someone else saved the
  event since it was read, the save fails with `CONFLICT` rather than overwriting their change.
- **Tables:** only the test tables (`seslogin_test_calendar`, `seslogin_test_calendar_event`)
  exist so far, so a deployed API on the prod tables fails Calendar calls, and nothing else.
- **Equipment reservations** are not calendar events: Tally shows them in its calendar as a
  virtual "Reservations" calendar, read from `TallyLocation.equipmentReservations`.

```graphql
mutation {
  createCalendarEvent(locationId: "TestAUnit001", title: "Training night",
                      allDay: false, startTime: 1793000000, endTime: 1793007200,
                      repeat: WEEKLY, repeatUntil: "2027-06-30") { id version }
}
{
  tallyLocation(id: "TestAUnit001") {
    calendarEvents(from: 1793000000, to: 1795600000) {
      startTime endTime startDate endDate
      event { id title allDay calendarId calendar { name color } }
    }
  }
}
```

## Map

A location's shared map of pins, used by the Tally app (`src/graphql/map.rs`, rules in
`src/map.rs`, storage in SCHEMA.md's `map_pin` and `map_layer` sections).

- **Who:** any member signed in to Tally who can use the unit — see "Tally (members)" above. Users on their own, kiosk sessions, API tokens and edit links are refused.
- **Reading:** `TallyLocation.mapPins(layerId)` lists the location's pins by title. Without
  `layerId` every pin at the location is listed, whatever layer it is on.
  `TallyLocation.mapLayers` (by name) and `Query.mapPin(id)` complete the reads.
- **Fields:** `latitude` / `longitude` are WGS 84 degrees (within ±90 / ±180). `title` is one
  trimmed line (max 200 characters). `address` is free text, trimmed, not geocoded (max 500).
  `note` is plain text, kept exactly as written (max 20,000; only-whitespace counts as empty).
  An absent address or note reads as `""`. `type` (`ACCESS`, `ENTRY`, `FLOOD` or `HAZARD`)
  says what the pin marks, which the app shows as an icon; null for a plain pin, and for a
  stored type this server doesn't know, so clients should cope with types being added.
- **Writes:** `createMapPin`, `updateMapPin` (omitted arguments unchanged, `layerId: null`
  takes the pin off its layer, `type: null` makes it a plain pin), `deleteMapPin`, `createMapLayer`, `renameMapLayer`,
  `deleteMapLayer`. Deleting a layer leaves its pins on the location's map (`MapPin.layer`
  becomes null).
- **Concurrency:** `updateMapPin` takes `expectedVersion`; if someone else saved the pin since
  it was read, the save fails with `CONFLICT` rather than overwriting their change.
- **Tables:** only the test tables (`seslogin_test_map_pin`, `seslogin_test_map_layer`) exist
  so far, so a deployed API on the prod tables fails Map calls, and nothing else.

```graphql
mutation {
  createMapPin(locationId: "TestAUnit001", title: "Staging area",
               latitude: -33.8688, longitude: 151.2093, type: ACCESS,
               address: "1 Example St", note: "Gate code at the shed") { id version }
}
```

## On Call

A location's on-call roles (Duty Officer, Duty Incident Controller, …) and who holds each one
now, used by the Tally app (`src/graphql/on_call.rs`, rules in `src/on_call.rs`, storage in
SCHEMA.md's `on_call_role` section). Scheduling (who holds a role in a given week) will come
later; for now a role just has the people in it now.

- **Who:** any member signed in to Tally who can use the unit — see "Tally (members)" above. Users on their own, kiosk sessions, API tokens and edit links are refused.
- **Reading:** `TallyLocation.onCallRoles` lists the location's roles by name; `Query.onCallRole(id)`
  reads one. Each has `primary` (who is on call, or null) and `secondaries` (anyone else in it,
  by name), both `Person`s, and `membersChangedAt`, when the people last changed.
- **Fields:** `name` is one trimmed line (max 100 characters). `description` is plain text kept
  as written (max 2,000; only-whitespace counts as empty). `phone` is optional, for a number
  that reaches whoever holds the role (e.g. a duty phone that diverts to them): digits,
  spaces, `()-` and a leading `+`, 3–15 digits, kept as written.
- **People:** anyone in a role is its one primary or a secondary; secondaries need a primary
  (`BAD_REQUEST` otherwise). Duplicates, and the primary, are dropped from the secondaries
  (at most 20). People may be members of any location, so a neighbouring unit's member can
  cover a duty. Newly added people must exist and not be deleted (`NOT_FOUND`); someone
  deleted after being put in a role drops out of `primary`/`secondaries` but doesn't stop the
  role being edited.
- **Writes:** `createOnCallRole`, `updateOnCallRole` (omitted arguments unchanged, `phone: null`
  removes the number, `primaryPersonId: null, secondaryPersonIds: []` empties the role),
  `deleteOnCallRole`.
- **Concurrency:** `updateOnCallRole` takes `expectedVersion`; if someone else saved the role
  since it was read, the save fails with `CONFLICT`.
- **Tables:** only the test table (`seslogin_test_on_call_role`) exists so far, so a deployed
  API on the prod tables fails On Call calls, and nothing else.

```graphql
mutation {
  createOnCallRole(locationId: "TestAUnit001", name: "Duty Officer",
                   phone: "0400 000 000", primaryPersonId: "TestAMember1") { id version }
}
```

## Locate

Members sharing their phone's position with a location's members, for the Tally app's Locate
tool and Unit Map (`src/graphql/location_share.rs`, rules in `src/location_share.rs`, storage
in SCHEMA.md's `location_share` section).

- **Who:** any member signed in to Tally who can use the unit — see "Tally (members)" above. Users on their own, kiosk sessions, API tokens and edit links are refused. A share is always the caller's own: one per member, keyed by their person ID, and only they can change or stop it.
- **Sharing:** `startLocationShare(locationId, durationMinutes, intervalSeconds, position)`
  shares for 5, 15, 30 or 60 minutes, with the phone promising a position every
  `intervalSeconds` (60 to 300). It replaces any share the caller already has, at any
  location. `updateLocationShare(position)` sends a newer position, and fails with
  `NOT_FOUND` once the share has ended (`endsAt`) or been stopped, which is the phone's cue
  to stop. `stopLocationShare` removes it at once (false if there was none).
- **Positions:** WGS 84 `latitude`/`longitude`, plus optional `accuracy` (metres, at most
  100 km), `altitude`, `altitudeAccuracy`, `heading` and `speed`. Negative heading and speed
  mean unknown, as phones report them, and are dropped. `recordedAt` (the fix time, Unix
  seconds) defaults to now and is never later than now. Only the latest position is kept.
- **Showing:** `TallyLocation.locationShares` lists every share still showing, most recently
  updated first; `Query.myLocationShare` is the caller's own. A share shows until
  `visibleUntil`, 7 minutes after its latest position, even once it has ended (`active` is
  then false), so a phone that loses signal fades out rather than vanishing at once. Each
  carries `name` (the member's name), `person`, `isMine`,
  `intervalSeconds`, `startedAt`, `endsAt`, `updatedAt` and `recordedAt`.
- **Audit:** starting and stopping are audited against the location (start, end and
  interval, never the position); position updates are not.
- **Tables:** only the test table (`seslogin_test_location_share`) exists so far, so a
  deployed API on the prod tables fails Locate calls, and nothing else.

```graphql
mutation {
  startLocationShare(locationId: "TestAUnit001", durationMinutes: 15, intervalSeconds: 120,
                     position: { latitude: -33.70, longitude: 151.10, accuracy: 8 }) { endsAt }
}
```

## Notifications

Each member's in-app notification inbox, read by the Tally app (`src/graphql/notifications.rs`,
producers' API in `src/notifications.rs`, storage in SCHEMA.md's `notification` section).

- **Model:** denormalised and fanned out on write. An event that notifies several members stores
  a full copy per recipient (they share a `notification_id`), so the inbox is one query with no
  join. A notification has a `source` (the producing app, e.g. `reservations`), a `kind`
  (`reservation.for_you`), a `title`, an optional `body`, an optional `locationId` and an
  optional `targetType`/`targetId` for what tapping it opens. The table is keyed by recipient
  member (person ID) and a time-ordered `sk`, with a sparse `unread-index` for counting and listing unread.
  Rows are removed by the TTL 90 days after creation, read or not.
- **Producing one:** call `notifications::notify(app, NewNotification, Audience)` *after* the
  business write has succeeded. `NewNotification::new(source, kind, title)` takes `.location()`,
  `.body()` and `.target(type, id)`. An `Audience` is `Members(person_ids)`. Only live (not
  deleted) members are notified, the acting member (the current audit-context actor, if it is
  a member) is left out, and nobody is notified twice.
- **Producers:** each fires after its write succeeds, in the GraphQL resolver (so the MCP
  tools, which run the same documents, are covered; no other path makes these writes), always
  with `locationId` set. Tally maps `source`/`kind`/`targetType` to icons and screens, so the
  strings are fixed:

  | source | kind | targetType | When | Audience | Title / body |
  |---|---|---|---|---|---|
  | `reservations` | `reservation.for_you` | `reservation` | `createEquipmentReservation`, or `updateEquipmentReservation`, leaves `reserved_for_person_id` a member who is not the actor and was not the previous one | that member | "Equipment reserved for you" / "<equipment names, sorted> · <Sun 25 Oct 09:00–11:00, Sydney time>" ("Equipment" if the names can't be read) |
  | `oncall` | `oncall.assigned` | `on_call_role` | `createOnCallRole`, or `updateOnCallRole`, makes a person the primary when they were not, or adds them to the secondaries when they were not already there (moving primary to secondary counts; removals and reorderings do not) | that member, once per newly assigned member | "You're now primary for <role>" / "You're now a secondary for <role>" |

  Equipment going into HAS_ISSUE, OUT_OF_SERVICE or MISSING used to tell the unit's users; it
  notifies nobody for now, while who among a unit's members should hear is rethought
  (`EquipmentStatus::attention_title` still holds the titles).

  The acting member is never told (so a reservation you make for yourself, or an on-call role you
  put yourself in, sends nothing). Today `reservation.for_you` cannot fire through the API: a
  reservation is for the caller or for free text, never another member, so the member it is for is
  always the actor. The producer is in place (`reservations::notify_reserved_for`) for when
  reserving for someone else is added.
- **Best-effort:** like the audit log, `notify` returns `()`: a failure is logged at `error!`
  and swallowed, because the thing it announces has already happened. The notification methods
  of `db::Handler` pass through `AuditingHandler` unaudited: the business write is audited where
  it is made, and read state is a personal marker.
- **API:** `Query.myNotifications(first, after, unreadOnly)` is a Relay connection, newest
  first, 50 by default and 200 at most, whose cursor and node `id` are the row's `sk`. Each
  node has `source`, `kind`, `title`, `body`, `locationId`, `targetType`, `targetId`,
  `createdAt`, `readAt` and `read`. `Query.myUnreadNotificationCount` is capped at 100 (show
  "99+" above 99). `markNotificationsRead(ids)` (at most 200) and `markAllNotificationsRead`
  return `{ unreadCount }`. Marking is idempotent: an ID that is already read, or that is not
  the caller's or does not exist, is ignored.
- **Who:** any member signed in to Tally; users on their own, kiosk sessions, API tokens and
  edit links are refused. Everything is the caller's own, because the table is keyed by their
  person ID.
- **Access lost:** a notification with a `locationId` the member can no longer use in Tally
  is left out of `myNotifications`, so stored text does not
  reach someone who lost access. `myUnreadNotificationCount` reads an index without the rows
  and so can still count such a notification, and the count can lag a write by a moment.
- **Tables:** only the test table (`seslogin_test_notification`) exists so far, so a deployed
  API on the prod tables fails these calls, and nothing else (a failed `notify` is only
  logged).

```graphql
{
  myNotifications(first: 20, unreadOnly: true) {
    edges { cursor node { id title body targetType targetId createdAt } }
    pageInfo { hasNextPage endCursor }
  }
  myUnreadNotificationCount
}
```

## WiFi networks

The WiFi networks Tally's WiFi app shows signed-in members, with their passwords
(`src/wifi.rs`, `src/graphql/wifi.rs`).

- **Never committed or stored:** they come from the `WIFI_NETWORKS` environment variable, a
  JSON array. Deployed, it is `var.wifi_networks`, set only in the git-ignored
  `infra/terraform.tfvars` and shared by prod, preprod and the test API; `jsonencode` saves
  escaping there: `wifi_networks = jsonencode([{ title = "Shed", ssid = "SES-Shed", password =
  "..." }])`. Locally, put it in the git-ignored `.env.secret`. Unset or blank means no
  networks.
- **Format:** `[{"title": "Shed", "ssid": "SES-Shed", "password": "..."}]`. `title` defaults to
  the SSID; `security` is `WPA` (WPA/WPA2/WPA3 personal; the default with a password), `WEP` or
  `nopass` (open; the default without one); `hidden` defaults to false. An entry with no SSID,
  no password when it needs one, or an unknown field makes the whole value invalid: the query
  then fails rather than quietly showing fewer networks, and the error is logged without
  quoting the value.
- **API:** `Query.wifiNetworks(knownVersion)` returns `{ version, networks }`, each network
  `{ title, ssid, password, security (WPA | WEP | NONE), hidden }`. `version` is a hash of
  the list, so it changes whenever any network does without anyone bumping it. When
  `knownVersion` is already current, `networks` is null and the passwords are not sent again.
- **Who:** any member signed in to Tally; users on their own, kiosk sessions, API tokens and
  edit links are refused.

```graphql
{ wifiNetworks(knownVersion: "3f2a9c0d1b7e4a65") { version networks { title ssid password security hidden } } }
```

## Unit directory

Every enabled location, statewide, with the details someone looking a unit up wants, for the
Tally app's Units tool (`src/graphql/unit_directory.rs`): `Query.unitDirectory`, by name.

- **Who:** any member signed in to Tally, like the contacts sync. It is statewide reference
  data, so it needs no role at any location. Users on their own, kiosk sessions, API tokens
  and edit links are refused.
- **Fields:** `id`, `name`, `code` (three capital letters), `address` (one line), `latitude` /
  `longitude`, `parentHeadquarters`, `phoneNumbers` and `emailAddresses` (each a list of
  `{ label, number }` / `{ label, address }`), and `placeholder`.
- **Placeholders:** only `id` and `name` are real so far. The rest are invented from the
  location's id and name until the real details are stored, and every unit has
  `placeholder: true` so clients can mark them as sample data. They are the same on every
  read. Phone numbers are in ACMA's ranges for fiction (`(02) 5550 xxxx`, `(02) 7010 xxxx`),
  email addresses are at `example.com`, positions are spread over inland NSW and the
  headquarters is the zone that position falls in. The one real number is the flood and
  storm line, 132 500, which is the same for every unit.

```graphql
{ unitDirectory { id name code address latitude longitude parentHeadquarters
                  phoneNumbers { label number } emailAddresses { label address } placeholder } }
```

## Audit log

Every business write (people, periods, kiosk sessions, users, locations, categories, API tokens, passkeys, OAuth grants, …) is recorded in the `{DB_PREFIX}_audit_log` DynamoDB table: who did it, which location(s) it touched, and which fields changed from what to what. The table is defined in `infra/dynamodb*.tf` and documented in `SCHEMA.md`; it must exist (`terraform apply`) for entries to land. It is read through GraphQL (below); there is no UI for it yet.

It is implemented by `audit::AuditingHandler`, a `db::Handler` wrapper that the servers, sync jobs and CLI wrap their DynamoDB handler in (`poem`, `poem-local`, the API Lambda, `member_sync::run`, `location_sync::run`, `cli`, `load-nitc-tags`, `load-category-display`). The actor comes from the `AUDIT_CONTEXT` task-local each entry point sets: `user` (with `actor_via` `oauth_grant:<id>` / `user_token:<id>` / `oauth_client:<id>` when not a plain web login), `member` (a member signed in to Tally, by person ID, `actor_via` `oauth_grant:<id>` or `oauth_client:<id>`), `session` (kiosk), `api_token`, `period_link`, `system` (`actor_id` is the job: `member-sync`, `location-sync`, `cli`, `load-nitc-tags`, `load-category-display`, `oauth-refresh-reuse`), `unauthenticated`, or `unknown` when a write was made with no context set (a bug; it logs a warning).

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

## Category display metadata

A category row can carry the kiosk's presentation of it: `group_name` (the kiosk
group, e.g. "Training"), `short_name` ("AIIMS") and `icon` (the icon name,
"aiims"), exposed as `Category.groupName` / `shortName` / `icon`. It is a
deliberate copy of the hard-coded tree in `web/src/lib/categories.ts`, which the
kiosk still reads; the copy exists for the mobile app. The copy lives in
`api/assets/category-display.json` (one entry per subcategory), and a web test
(`categories.display.test.ts`) fails when it drifts from the kiosk tree.

`updateCategory`'s `groupName`, `shortName` and `icon` are three-state: omitted
keeps the stored value (the admin edit form does not send them), `null` or an empty
string clears it, a string sets it. `createCategory` takes them as optional.

`load-category-display` backfills the file into the database. It is a dry run
unless `--apply`, only touches categories whose three fields differ, skips ids with
no live category, and leaves categories not in the file alone (audited as
`system: load-category-display`). It is not deployed as a Lambda.

```bash
cd api && RUST_LOG=info cargo run --bin load-category-display --          # plan
cd api && RUST_LOG=info cargo run --bin load-category-display -- --apply  # write
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
