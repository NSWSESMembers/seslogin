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

- A **nullable** target (`...node.person`, `...node.category`) produces a _partial_
  response: HTTP 200, `data` populated, plus an `errors` entry. This is the case
  clients tend to mishandle, and it is otherwise hard to provoke.
- A **non-null** target (`...node.location`) propagates up to the nearest nullable
  ancestor, usually collapsing the whole response to `data: null`.

Two details worth knowing:

- **Failures are deterministic.** With a `*` in the path, the rate is decided by
  hashing each matched row's own full response path, so the _same_ rows fail on every
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
behind something that changes the host a client sees. The
`authorization_endpoint` metadata is on the issuer's origin too
(`<issuer>/admin/oauth/authorize`): deployed, the same CloudFront distribution
serves the web app, so a client always lands on its own environment's consent
page. It deliberately isn't `WEB_BASE_URL`, which preprod and qa point at prod.
Locally `poem` redirects `/admin/oauth/authorize` and `/tally/sign-in` to the
same path on `base_url::web_base_url()` (the web dev server).

### Tally sign-in

Members sign in to Tally without a user record (`tally_sign_in.rs`). It is its
own flow, so it can never yield an admin session: the app opens
`<WEB_BASE_URL>/tally/sign-in` with an ordinary authorization request
(`client_id`, `redirect_uri`, PKCE `code_challenge`, `state`), the page asks
for an email, and the emailed 6-digit code is redeemed straight for an OAuth
authorization code — no `slu_` token is ever issued. The page also takes a
passkey instead of the code (below). Four unauthenticated GraphQL fields back
it, plus `beginPasskeyLogin`, shared with the admin login:

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
- `completeTallyPasskeySignIn(challengeId, credentialJson, request)` verifies a
  passkey assertion for a `beginPasskeyLogin` challenge (single use, whatever
  the outcome) and returns the redirect URL the same way. An unknown passkey,
  a bad signature or an expired challenge fails with "That passkey wasn't
  recognised"; a passkey whose owner signs in as nobody, with a message saying
  the account isn't linked to an SES member.

Who an email signs in as (`tally_sign_in::identify`): an enabled user linked to
a live member is that user and member; otherwise exactly one live member with
that email (compared trimmed and lowercased, through the person table's
`email_key-index`), plus the enabled user with that email if there is one;
otherwise nobody — a user with no member, or an email several members share.
A disabled user counts as no user. The grant names the member and that user,
and resolves fresh on every use to `AuthInfo::Member` (see CLAUDE.md), so the
member's units are every unit they belong to (primary or secondary) plus that user's units.

**Passkeys.** Every deployed environment's relying party is `seslogin.com`,
and the sign-in page is on the site's own origin, so a passkey a user added on
the admin site works here too. The page has a "Sign in with a passkey" button
and also offers saved passkeys on the email field (conditional mediation). A
passkey signs in as its owner (`tally_sign_in::identify_passkey_owner`):

- a user's passkey as `identify` would for the user's email (`identify_user`),
  skipping the lookup by email, and as nobody once the user is disabled: a
  passkey proves the user, not the inbox;
- a member's own passkey as that member while they are live, alongside the user
  their email signs in with, if it still signs in as them.

The admin login (`finishPasskeyLogin`) refuses a member's passkey with a
`FORBIDDEN` error saying it is for Tally. Both finish mutations verify through
`passkey::verify_login`; both registrations go through
`passkey::start_registration`/`finish_registration`, and a member's passkey
has its own user handle (`passkey::user_handle`).

**Saving a passkey after a code.** The page redeems a code with
`redeemTallySignInCode(email, code, request, canSavePasskey)`, passing whether
the device has a platform authenticator and "Not now" hasn't been chosen for
that email on it in the last 30 days. When it can, and the person has no
passkey yet (the user's or the member's), the answer is a `passkeyTicket`
instead of the `redirectUrl`: a single-use, 10-minute `ephemeral_state` row
(kind `tally_signin`, stored by hash) holding who signed in and the request.
The page offers "Save a passkey" (`beginTallyPasskeyRegistration(ticket)`, then
`finishTallyPasskeyRegistration(ticket, challengeId, credentialJson)`, whose
challenge is bound to that ticket) or "Not now", and either way exchanges the
ticket with `continueTallySignIn(ticket)`, which re-checks the member is live
and the user enabled, then mints the authorization code (so it can't expire
while they decide). The passkey is the user's when a user signed in alongside
the member, so it works on the admin site too — an email code to that inbox
already signs in there, so this hands out nothing new — and the member's
otherwise. It is called "Saved at Tally sign-in". `completeTallySignIn` still
works, for a page loaded before the offer existed.

**Managing passkeys in Tally.** Tally's settings list the passkeys that sign the
member in (`TallyViewer.passkeys`: the member's own and the riding-along user's,
newest first, each with `signsInToSeslogin` true for the user's, which work on
the admin site too) and rename or delete them (`renameTallyPasskey(id, name)`,
name trimmed, 1–100 characters; `deleteTallyPasskey(id)`). Tally members only
(`graphql/tally_passkeys.rs`); anyone else's passkey is `NOT_FOUND`, like a
missing one. Both writes are audited, as every passkey write is.

Adding one happens in a browser, since a passkey belongs to the site's relying
party, not the app. `startTallyPasskeySetup` (Tally members only) returns a
single-use ticket: an `ephemeral_state` row (kind `tally_addpk`, stored by
hash, 10 minutes) naming the member and their grant. Tally opens
`/tally/passkey#<ticket>` on its SESLogin site (the fragment never reaches a
server). The page first proves an inbox: `sendTallyPasskeySetupCode(ticket,
email)` emails an ordinary Tally sign-in code (same `login_code` table, limits
and expiry), but only if that email signs in as the ticket's member, answering
`true` either way. `verifyTallyPasskeySetupCode(ticket, email, code)` redeems it
and marks the ticket confirmed. Only then do `beginTallyPasskeySetup(ticket)` and
`finishTallyPasskeySetup(ticket, challengeId, credentialJson)` work (`BAD_REQUEST`
before). Finishing uses the ticket up and returns the grant's redirect URI with
`passkey=saved`, which sends the browser back to the app. The ticket is refused
(`BAD_REQUEST`) once expired, used, or its member or grant is gone, so revoking
a sign-in also kills a ticket it asked for.

The passkey, called "Added in Tally", is the **user's** when the confirmed
email is an enabled SESLogin user's (it then signs in to the admin site too),
and the **member's** own otherwise. That is the same rule as saving one after a
sign-in code: the code proves the inbox, and a code to that inbox already signs
in to the admin site, so the Tally grant alone never mints a credential.

**App Store review.** `TALLY_REVIEW_SIGN_IN=<email>:<6 digits>` makes that one
email always be sent that code instead of a random one, so a reviewer can sign
in without the inbox. It is set on the prod and test APIs, to the same value
(`var.tally_review_sign_in` in the git-ignored `terraform.tfvars`), and both
databases hold the review account; preprod and qa leave it unset. The code
still expires, is used up and allows five guesses like any other, and a
request still has to send it first. It is a secret, never
committed, because the repository is public. Each successful sign-in as that
email is reported by email (member, time and client IP, never the code) to
`TALLY_REVIEW_NOTIFY_TO`, so a leaked code is noticed; unset or blank sends none.

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

### Tally sign-ins (list + sign out)

Tally's settings list the devices a member is signed in on and can sign any of
them out (`graphql/tally_sign_ins.rs`, Tally members only). Each Tally sign-in is
its own grant, so one per device. `TallyViewer.signIns` lists the live grants
naming the member (`oauth_grant`'s `person_id-index`) plus, when a user signed in
alongside them, that user's grants that name a member (Tally ones, through
`user_id-index`; that also covers a user since linked to another member). The
user's connected apps (MCP clients) aren't listed, since those are managed here
on the admin site. Each entry has `clientName`, `createdAt`, `lastUsedAt`,
`refreshExpiresAt` and `isCurrent` (the grant making the request), with this
device first, then most recently used. `revokeTallySignIn(id)` deletes the
grant, this device's included, and returns its ID. Anyone else's grant, a
connected app, or one past its expiry is `NOT_FOUND`.

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
`https://qa.seslogin.com`, `https://preprod.seslogin.com` and
`https://seslogin.com` (prod), so the MCP URL
is e.g. `https://seslogin.com/mcp`. CloudFront routes `/mcp`, `/oauth/*` and
`/.well-known/oauth-*` to the API Lambda (as it does `/graphql`), and adds CORS
headers for browser-based MCP clients through a response headers policy
(`infra/web_test.tf`, `infra/web_qa.tf`, `infra/web_preprod.tf`,
`infra/web_prod.tf`). The handler
answers `OPTIONS` on those paths with a bare `204`
(`oauth_http::is_cors_preflight_path`). `API_BASE_URL` is set on all four
because CloudFront doesn't forward `Host`. Prod is also served at
`new.seslogin.com`, but the advertised MCP host is the apex: use
`https://seslogin.com/mcp`. Each Lambda's raw Function URL still works, with its
own CORS config allowing only the site origin.

QA, preprod and prod share a database and secrets, so an MCP client connected
to any of them acts on **live production users** with the connecting user's permissions.
Test runs against the `seslogin_test` tables, a snapshot that still holds real
member data.

**Auth:** only an OAuth `slat_` access token audience-bound to `<API
base>/mcp` is accepted — which only dynamically registered clients can get. An
`slu_` user token, a JWT, or an `slat_` token bound to `/graphql` (a static
client's) is rejected here, just as an `/mcp`-bound token is rejected on the
GraphQL endpoint; see "Using a token" above. A
missing or invalid token gets a `401` carrying `WWW-Authenticate: Bearer
resource_metadata="<API base>/.well-known/oauth-protected-resource/mcp"`, per
the MCP authorization spec's discovery flow.

**Tools:**

| tool                                                            | what it does                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| --------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `whoami`                                                        | Who you're authenticated as: id, email, `isSuper`, the locations you can access, and your own settings (`emailSummaryLocationIds`, `disaggregateVirtualPeriods`).                                                                                                                                                                                                                                                                                                                            |
| `list_users`                                                    | List every admin user. Super user only.                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `get_user`                                                      | Look up one user by id. A non-super caller may only look up themselves.                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `create_user`                                                   | Create a user (email, `isSuper`, `locationGrants`, optional `readOnlyLocationGrants`). Super user only.                                                                                                                                                                                                                                                                                                                                                                                      |
| `update_user`                                                   | Change a user's email/`isSuper`/`isDev`/`locationGrants`/`readOnlyLocationGrants`; omitted fields keep their current value. Never touches `enabled`. Super user only.                                                                                                                                                                                                                                                                                                                        |
| `disable_user` / `enable_user`                                  | Flip `enabled` — SESLogin has no user delete, only disable. Super user only.                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `set_user_member`                                               | Set (`personId`) or clear (`null`) the member a user acts as when signing in or out from the mobile app. Any user, the caller included; several users may share a member. Super user only.                                                                                                                                                                                                                                                                                                   |
| `list_locations`                                                | The locations you can access (id, name, `enabled`, `viewerCanEdit`, `nitcEnabled`, `lastSuccessfulMemberSync`): every enabled one for a super user, otherwise your Admin and Read only grants. Start here for location ids.                                                                                                                                                                                                                                                                  |
| `get_location`                                                  | One location's details (adds NITC and SES headquarters settings, created/updated). Needs a grant for it.                                                                                                                                                                                                                                                                                                                                                                                     |
| `list_members`                                                  | A location's active members, optionally filtered by `query` (case-insensitive substring of first/last/full name or member number). Deleted members are never listed. Each carries `syncManaged` (`sesApiPersonId` set). Admin, Read only or super user.                                                                                                                                                                                                                                      |
| `get_member`                                                    | One member, with their `lastPeriod`. Needs a grant for their location.                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `find_member_by_number`                                         | As `get_member`, looked up by member number. "Not found" and "no grant" give the same error.                                                                                                                                                                                                                                                                                                                                                                                                 |
| `create_member` / `update_member` / `delete_member`             | Manage members. `update_member` changes only the fields passed. Admin (or super user) only; Read only is refused. Members owned by member sync (`syncManaged`) can be changed but the next sync may undo it, and the admin site doesn't offer it.                                                                                                                                                                                                                                            |
| `sync_members`                                                  | Queue a member sync for a location. Admin (or super user) only.                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `list_categories` / `get_category`                              | The activity categories a period is filed under (name, `enabled`, `isVirtual`, NITC group, kiosk `groupName` / `shortName` / `icon`). Disabled ones are left out of the list unless `includeDisabled`. Global, so any signed-in user.                                                                                                                                                                                                                                                        |
| `list_periods`                                                  | A location's activity periods, newest first, with member name/number (or `guestName`), category, comment and sign-in/out kiosk names. `onlyActive` is who is signed in now (the Current page); `start` and `end` (together) select by when a period started; `categoryIds`, `first` and `after` filter and page. This also covers the Reports page's export. Admin, Read only or super user.                                                                                                 |
| `list_member_periods`                                           | One member's periods, same shape and paging, optionally by `categoryIds`. Needs a grant for the member's location.                                                                                                                                                                                                                                                                                                                                                                           |
| `get_period`                                                    | One period. Needs a grant for its location.                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `create_period`                                                 | Add a finished period for a member (member, location, category, start, end, optional comment). Admin (or super user) only.                                                                                                                                                                                                                                                                                                                                                                   |
| `update_period`                                                 | Change times, comment and category (member periods, via `updatePeriodTimeCategory`) or guest name (guest periods, via `updateGuestPeriod`), as the admin edit page does. Only passed fields change; `comment: null` clears it, leaving it out keeps it. Admin (or super user) only.                                                                                                                                                                                                          |
| `delete_period`                                                 | Delete a period. Admin (or super user) only.                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `send_period_edit_link`                                         | Email the member a link to correct a period. Sends real email; subject to a reminder cooldown; guests can't be sent one. Admin (or super user) only.                                                                                                                                                                                                                                                                                                                                         |
| `get_dashboard`                                                 | The admin Home page for a location: member and check-in counts (24h / 7d / 30d, with the virtual split), total and average durations, kiosk counts, last member sync, the 7 daily buckets and the top categories. `asOf` moves the windows. Durations are seconds; the daily buckets are UTC days. Admin, Read only or super user.                                                                                                                                                           |
| `summarize_activity`                                            | Time per member / category over a range, grouped by `groupBy`: `member`, `category`, `member_category`, `category_member`, `day_category_member` or `day_member` (the Totals, Breakdown, Daily breakdown and Heatmap pages). `start`/`end` select by when a period started. `categoryIds` is accepted only by `member` and `day_member`; other groupings reject it. Durations are seconds (completed periods only); a day's `date` is a Sydney `YYYY-MM-DD`. Admin, Read only or super user. |
| `members_last_seen`                                             | Each active member of a location with when they were last seen (end of their latest period, or its start if still open, at any location; null if never) and `secondsAgo`; `sort` by `recent`, `least_recent` or `alphabetical` (the Last seen page). Admin, Read only or super user.                                                                                                                                                                                                         |
| `list_kiosks`                                                   | A location's kiosks, most recent contact first: name, `lastContact`, client version, `keyFingerprint`, `keyExpiresAt`, `reactivatable`, `healthcheckUrl`, `config` and the kiosk's self-reported `clientInfo`. `keyReleasedAt` set marks a replaced leftover that no longer works. Admin, Read only or super user.                                                                                                                                                                           |
| `get_kiosk`                                                     | One kiosk, same shape. Needs a grant for its location.                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `update_kiosk`                                                  | Change `name`, `config` (a JSON object that replaces the whole config) or `healthcheckUrl`; only passed fields change, `healthcheckUrl: null` clears it, leaving it out keeps it. Admin (or super user) only.                                                                                                                                                                                                                                                                                |
| `delete_kiosk`                                                  | Delete a kiosk (the device must be set up again by QR code). Also how to remove a replaced leftover. Admin (or super user) only.                                                                                                                                                                                                                                                                                                                                                             |
| `create_category` / `update_category`                           | Add or change an activity category (`name`, `isVirtual`, `enabled`, NITC group and participant type, kiosk display `groupName` / `shortName` / `icon`). `update_category` changes only the fields passed; `nitcGroupId`, `nitcParticipantType`, `groupName`, `shortName` and `icon` can be cleared with `null`. Super user only.                                                                                                                                                             |
| `list_nitc_groups` / `get_nitc_group`                           | The NITC groups (SES non-incident type and tags) with the `categories` that export as each. Super user only.                                                                                                                                                                                                                                                                                                                                                                                 |
| `create_nitc_group` / `update_nitc_group` / `delete_nitc_group` | Manage NITC groups. `update_nitc_group` changes only the fields passed (`nitcTagIds` replaces the whole list). Deleting does not check categories still using the group. Super user only.                                                                                                                                                                                                                                                                                                    |
| `list_ses_nonincident_tags` / `list_ses_reference_data`         | The valid values for those forms: SES tag ids, `nonincidentTypes` (a group's `nitcType`) and `participantTypes` (a category's `nitcParticipantType`; fetched from SES, so can fail when SES is unreachable). Super user only.                                                                                                                                                                                                                                                                |
| `create_location` / `update_location`                           | Add or change a location: `name`, `enabled`, `nitcEnabled` (when NITC export starts; `null` turns it off) and `nitcCompleteOnExport`. `update_location` changes only the fields passed. No SES headquarters field: the site's pages don't set it (location sync does). Super user only.                                                                                                                                                                                                      |
| `list_api_tokens` / `get_api_token`                             | API tokens (`slgn_`): name, `locationGrants`, `readOnly`, creator, `expiresAt`, `revokedAt`, `lastUsedAt`. Never a secret or hash. Super user only.                                                                                                                                                                                                                                                                                                                                          |
| `update_api_token` / `revoke_api_token`                         | Change a token's name, locations, `readOnly` or `expiresAt` (only passed fields change; `expiresAt: null` = never expires), or revoke it for good. Super user only.                                                                                                                                                                                                                                                                                                                          |
| `get_audit_log`                                                 | The record of changes, newest first: `entries` (time, action, `entityType`, `entityLabel`, location, actor, field-level `changes`) and `pageInfo`. With `locationId` it is that location's log, open to Admin, Read only and super users; without it, every location plus global records (users, categories, tokens), super user only. `entityTypes` filters by kind; `first` (max 200) and `after` page. `ip` and `actor.via` are shown to super users only.                                |
| `update_my_settings`                                            | Change your own `dailyEmailLocationIds` (the complete list of locations whose daily summary you get; locations you can access only) and/or `disaggregateVirtualPeriods`. Passing neither is an error.                                                                                                                                                                                                                                                                                        |
| `list_connected_apps` / `revoke_connected_app`                  | The AI clients and other apps you have authorised (the Connected apps page): client name, redirect host, created/last used, `refreshExpiresAt`, with `thisConnection` on the grant the call itself came in on. Revoking that one ends the session (`endedThisConnection`). `userId` (list) is super user only for anyone but yourself; a super user can revoke anyone's grant, anyone else only their own.                                                                                   |
| `list_passkeys` / `rename_passkey` / `delete_passkey`           | Your passkeys: id, name, created/last used, never key material. Rename and delete work on your own passkeys only, even for a super user; `list_passkeys`'s `userId` is super user only for anyone but yourself. Adding a passkey is done in the browser.                                                                                                                                                                                                                                     |
| `submit_feedback`                                               | Send the seslogin team a help request as a Toolbox ticket from your own address (subject, message). Sends real email. If Toolbox isn't configured on the server the tool says so and points at support@seslogin.com, like the site's help dialog.                                                                                                                                                                                                                                            |

**Not exposed, on purpose:**

- _Creating API tokens_ (`createApiToken`). An MCP grant is revocable and expires, while an
  `slgn_` token is a long-lived credential that works on `/graphql` outside this tool set,
  so a grant must not be able to mint one. The API token tools never return a secret or
  hash.
- _Enrolling or reactivating kiosks, and the pending-enrolment lookup_ (`enrollSession`,
  `reactivateSession`, `pendingEnrollmentKey`). A device is only bound to a location by a
  person scanning its QR code in the browser.
- _Login, logout and auth codes._ They mint or end the credentials this endpoint is
  already authenticated with.
- _Passkey registration and login._ The WebAuthn ceremonies need the user's authenticator
  in the browser. Passkeys can be listed, renamed and deleted.
- _OAuth consent_ (`approveOauthAuthorization`, `oauthAuthorizationRequest`). Approving an
  app is the user's own decision on the site, not an AI client's. Connected apps can be
  listed and revoked.
- _`refreshToken`._ It re-issues a kiosk's session JWT (`Session`-guarded) and means
  nothing for a user.
- _Kiosk-only (`Session`-guarded) and edit-link-only (`PeriodLink`) fields._ They
  authenticate a device or a member's single link, neither of which can be this user.

**Times.** Any time argument takes Unix seconds (an integer) or a string: RFC 3339
with an offset (`2026-10-16T09:00:00+11:00`), a local `YYYY-MM-DDTHH:MM[:SS]` /
`YYYY-MM-DD HH:MM[:SS]`, or a date `YYYY-MM-DD` (midnight). Strings without an
offset are Australia/Sydney local time (a time that happens twice at the DST
fall-back takes the earlier one; one that doesn't exist is an error). Results
keep each Unix-seconds timestamp and add a `<name>Sydney` RFC 3339 sibling
(`createdAt` and `createdAtSydney`). Durations stay in seconds. Report day buckets come
in two kinds: `dailyPeriods7D[].dayStart` in `get_dashboard` is a Unix time at the start
of a _UTC_ day, while the `date` in `summarize_activity` is a Sydney calendar date
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
(`location`, `Location.people` and the six `Location.periodSummaryBy*` resolvers behind `summarize_activity`), the tool does, and the period and kiosk
tools check again even where the resolver does (`period`, `Location.periods`, `Query.session`, `Location.sessions`, `Location.auditLog`), so a tool only ever narrows access. A single member (`get_member`, `find_member_by_number`, `list_member_periods`) is read through `Query.person`, which allows an Admin or Read only grant at any of the member's units (primary or secondary); `update_member` and `delete_member` still need Admin at the primary unit. A tool failure (a guard rejection, a
validation error) comes back as an ordinary JSON-RPC _success_ whose
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
Inspector run against a _deployed_ server will fail the discovery/registration
requests to CORS — production intentionally doesn't allow arbitrary
browser-origin JS to hit its OAuth endpoints; use `claude mcp add` or a
claude.ai connector there instead.

## Members at several units

A member's primary unit is `Person.location_id`, the unit member sync lists them at in
the SES API; the other units they volunteer at are
`secondary_location_ids` (directory sync). `Location.people` (and so the MCP
`list_members`), `dashboardSummary`, `badgeLeaderboard` and the weekly badge digest use
`list_members_for_location`, which lists both. `Person.locationId` is always the primary
unit, `Person.isMemberOf(locationId)` says whether a unit is one of theirs, and
`Person.viewerCanEdit` is write access at the primary unit, matching `updatePerson` and
`deletePerson`. `cli person list --include-secondary` lists a unit the same way; its
default is the primary-only list.

A member is a _visitor_ only at a unit that is none of theirs: `!person.is_member_of(location)`,
never `location_id != location.id`. So `scanRegister2(confirmVisitor: true)` signs a member of
the kiosk's unit (primary or secondary) in without `CONFIRM_VISITOR`, and a secondary unit counts
like the primary unit for badges: it earns no "visiting" credit there (away check-ins, distance badges).
Badges are worked out when gamification is on at the kiosk's unit and at _any_ of the member's
units (kiosk scans, directly-created periods and `badge-nightly` alike); distance badges
measure from the nearest of the member's units that has coordinates. The admin Activity pages
label a visitor "Visiting from <unit>" and a member whose primary unit is elsewhere "Primary: <unit>".

## Perpetual trophies

Thirteen badges that belong to a **unit**, not a member, each with exactly one holder at a
time (`src/trophies.rs`). Whoever beats the holder takes it, and every change of hands is
kept as a reign in the `trophy` table (see SCHEMA.md). Two kinds:

- **Standing records** (The Boomerang: longest gap between sign-ins at the unit; The
  Odometer: a member's furthest sign-in at a unit that is none of theirs, from the
  nearest of their own units). A claim counts for 365 days; a strictly bigger one takes
  the trophy. When the holder's claim ages out, the best claim still inside the window
  takes over, or the holder keeps it if there is none.
- **Rolling leads** (the other eleven), scored nightly over a trailing window (The
  Chain counts a current weekly run, and The Sash every badge the member has earned at
  the unit, with no window). A challenger
  must strictly beat the holder's current score and meet the minimum; a quiet holder
  keeps it until passed, or until their own score falls below the trophy's
  `keep_minimum` (`lapsed`; the same as its minimum except The Kettle and The Padlock,
  which take 3 busy days in 60 to win but are kept while at least 1),
  when it passes to whoever leads or goes vacant if nobody qualifies.

A reign records the best score the holder reached while holding it (the holder row's
`best_value`) and the score the next holder took it with (`ended_by_value`), so the
history reads sensibly even when a rolling holder's score had fallen by the time they lost
it.

Ties go to the holder, and between challengers on the same score, whoever reached it
first. The Sash, most badges at the unit, breaks a tie on the count like a medal table
before that: more gold badges, then silver, then bronze (`Standing::tiebreak`; starter,
wildcard and passport stamp badges count towards the total only). A challenger level on
the count takes it from the holder only with a better medal count; level on both, the
holder keeps it. It counts what the Badge Report's leaderboard counts
(`badges::awards_in_range` at the unit), so it is decided after the night's personal
badges. The badge leaderboard (`Location.badgeLeaderboard`, on the dashboard and the
kiosk) and the Badge Report's members table rank by the same rule, through
`trophies::medal_count`: badges, then medals, then whoever reached their count first
(their latest badge); `rank` is shared only by members level on badges and medals.

Only a unit's own members (primary or secondary) can hold its trophies. Every
hours-based trophy counts a sign-in at most 12 hours, so nothing rewards a long shift.

`trophy_nightly::run`, at the end of `badge-nightly` (`seslogin-badge-nightly`), is the
source of truth: for every unit with gamification on it reads 372 days of sign-ins,
scores each trophy, and writes what changed with `write_trophy`, which is conditional on
the holder row's version. It also catches what a kiosk scan can't: a record whose period
was deleted or edited reverts to the best claim still standing (or goes vacant), a claim
that has aged out passes on, and a holder who is no longer a member hands the trophy to
the leader (or leaves it vacant). Each change writes a reign for the old holder and
notices for the member who took it and, if they were beaten, the one who lost it. A
dry run (`badge-nightly` without `--apply`) logs the changes instead. A unit that fails
is logged and skipped. `cli trophy list --location <id>` shows a unit's cabinet.

On a kiosk scan (`trophy_scan.rs`, under the same switches as badges: `kiosk_badge_member`)
a sign-in can take a standing record on the spot: The Boomerang when it is at one of the
member's units (the gap since their previous sign-in there), or The Odometer at each of
their units with gamification on when it is at a unit that is none of theirs
(`away_distance_km`). A take writes the reign and a `lost` notice for the old holder, and
a version conflict is left for the nightly run. `scanRegister2` (on a sign-in) and
`scanSignOutWithBadges` return `trophyChanges`: what this scan took, plus the member's
unseen notices at each of their units, which are then deleted. A kiosk that doesn't
select `trophyChanges` still makes claims but leaves the notices for one that does.

### Badge congratulations emails

`badge_congrats::run` (`seslogin-badge-congrats`, 19:00 Sydney daily; `badge-congrats`
by hand, a dry run unless `--apply`, with `--person-id` and `--override-to` for testing)
emails each member what they won: every badge awarded to them in the last 3 days,
wherever they earned it, and every trophy they took at a unit of theirs in that time and
still hold, with who they took it from. A member is mailed when any of their units
(primary or secondary) has gamification and `badge_emails_enabled` on, which is off by
default, at the email directory sync stored for them; no email, no message. The 3-day
look-back covers `badge-nightly` dating a badge back to the sign-in that earned it and a
missed run, and an `ephemeral_state` record per member (`badge-congrats:<person id>`,
kept 7 days) lists what has been mailed, so each win is mailed once. The record is written
after the send: if it fails the member may be mailed a win twice, but never misses one.
Badge icons are the PNGs in `web/public/image/badges/png/`, since Gmail and Outlook
don't show SVG.

## Tally (members)

Tally is for **members**, not users: every Tally field below admits only an
`AuthInfo::Member`, which only Tally sign-in issues (see "Tally sign-in"). A
member can use every unit they belong to (their primary unit and their secondary
units, `AuthInfo::Member::member_location_ids`), plus every unit the user signed in alongside
them holds either role at (every unit for a super user); `require_member_location`
in `graphql/auth.rs` is the one check. Any such member may read **and** write a
unit's shared tools — they are the unit's own, not admin data. Users on their
own (even super users), kiosks, API tokens and edit links are refused.

The apps start from two queries (`graphql/tally.rs`):

- `tallyViewer` — the member (`member`, `name`, `homeLocationId`, and
  `memberLocationIds`: every unit they belong to, primary first, which tells "my units"
  apart from units reached only through the user's grants), the email of
  the user signed in alongside if any (`userEmail`), whether that user is a super
  user (`isSuper`) or a developer (`isDev`, which shows Tally's environment
  picker), and every unit they can use (`locations`, by name: the ones they belong to plus
  the user's).
- `tallyLocation(id)` — one unit's tools: `tasks`, `taskLists`, `notes`,
  `noteFolders`, `equipment`, `equipmentReservations`,
  `equipmentReservationConflicts`, `mapPins`, `mapLayers`, `calendars`,
  `calendarEvents`, `locationShares` and `onCallRoles`. The core `Location` type
  carries none of them. Its `zone` is the unit's SES zone, another
  `TallyLocation` whose `id` and `name` anyone who sees the unit may read; its
  tools keep their own checks.

Everything a member writes records their **person ID** — `created_by`,
`updated_by`, `completed_by`, `reserved_for_person_ids`, the location share's key
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
- **Assignees:** a task may be assigned to up to 20 members (`assigneeIds` on `createTask`
  and `updateTask`, by person ID, from any unit, like an on-call role). With none it is the
  whole unit's. Assigning changes nothing about who may see, edit or complete it: anyone at
  the unit still can. `updateTask` without `assigneeIds` leaves them alone; given, it
  replaces them (`[]` unassigns everyone), and a member already assigned may stay after
  being deleted, but only live members can be added. `Task.assigneeIds` lists them all,
  `Task.assignees` the live ones by name, and `Task.assignedToViewer` says whether the caller
  is one. Members newly assigned get a notification (see "Notifications").
- **Photos:** up to 10, through `photoAssetIds` on `createTask` and `updateTask`; see "Attached photos".
- **Linked notes:** up to 10, through `noteIds` on `createTask` and `updateTask`; see "Linked notes".
- **Concurrency:** `updateTask` and `completeTask` take `expectedVersion`; if the task was
  changed or completed since (say, two people tapping Complete at once), the second gets a
  `CONFLICT` error rather than skipping an occurrence.
- Deleting a list leaves its tasks on the location (`Task.list` becomes null).

```graphql
mutation {
  createTask(
    locationId: "TestAUnit001"
    title: "Wash the truck"
    recurrence: { every: 1, unit: WEEKS }
  ) {
    id
    version
    dueDate
  }
}
```

## Offline sync API

`GET /api/v1/sync?cursor=<opaque>&limit=<n>` is the Tally app's offline sync
(`src/offline_sync.rs`): one cursor-based change feed for the data the app keeps on the
device, so there is a single sync to maintain: the member directory (`contacts`), the
unit directory (`units`) and the caller's shared contacts (`sharedContacts`). Contacts are every member, statewide, with only these
fields:

```json
{
  "contacts": [
    {
      "id": "…",
      "firstName": "Ann",
      "lastName": "Smith",
      "preferredName": "Annie",
      "memberNumber": "40064936",
      "locationId": "…",
      "unit": "Alpha Unit",
      "email": "…",
      "mobile": "+61400000000",
      "phones": [
        { "number": "+61400000000", "types": ["Primary Telephone", "SMS"] }
      ],
      "capabilityTags": [{ "id": 12, "name": "Chainsaw", "description": "…" }],
      "units": [
        {
          "entityId": "…",
          "name": "Alpha Unit",
          "code": "ALP",
          "locationId": "…",
          "zone": "Metro Zone",
          "membershipType": "Member",
          "ranks": ["Unit Commander"],
          "roles": ["Storm Team Leader"],
          "positions": [],
          "isPrimary": true
        }
      ],
      "updatedAt": 1783655893
    },
    { "id": "…", "deleted": true }
  ],
  "units": [
    {
      "id": "…",
      "name": "Hornsby Unit",
      "code": "HOR",
      "zoneId": "…",
      "address": {
        "line1": "…",
        "line2": "…",
        "suburb": "Hornsby",
        "state": "NSW",
        "postcode": "2077",
        "formatted": "…"
      },
      "postalAddress": { "formatted": "…" },
      "latitude": -33.7,
      "longitude": 151.1,
      "phone": "…",
      "email": "…",
      "accreditations": [
        {
          "type": "Flood Rescue",
          "status": "Accredited",
          "number": "…",
          "note": "…"
        }
      ]
    }
  ],
  "sharedContacts": [
    {
      "id": "…",
      "locationId": "…",
      "name": "Duty Officer phone",
      "description": "…",
      "phone": "0400 000 000",
      "phoneCanSms": true,
      "email": "…",
      "updatedAt": 1783655893
    },
    { "id": "…", "name": "SOC", "phoneCanSms": false, "updatedAt": 1783655893 }
  ],
  "nextCursor": "…",
  "hasMore": false,
  "total": 12034
}
```

`preferredName`, `email`, `mobile` and `phones` come from directory sync, `capabilityTags`
from member sync, and `units` are the member's `person_unit` rows (their units plus any
clusters, Community Action Teams and staff sections), primary first and then by name.
An optional field, and an empty list, is left out, and an upsert is always the whole
contact, so the client replaces what it holds rather than merging.

**Units** are not a change feed: there are only ~280 and they rarely change, so a
response carries the whole list, every enabled location by name, whenever it differs
from the list the cursor last delivered, and leaves `units` out otherwise. The cursor
records a hash of the list, so a full sync's first page always has it and its later
pages don't. When `units` is present the client replaces its whole list (a unit missing
from it has closed). Each unit is what location sync stored: the code, coordinates and
zone from the SES API's headquarters record (`zoneId` is the zone's own `id` in the
list, and only units have one), the addresses, phone and email from the
location's SES intranet directory entity, and the accreditations (sorted by type,
"Not Applicable" rows kept) from the SES API. Any of those may be missing: 29 units have
no coordinates, and State Headquarters and a few pseudo-units have no directory entity.
Zones and State Headquarters are locations too, so they are listed. Each response reads
the location table (a Scan of ~280 rows) to compute the hash.

**Shared contacts** (see "Shared contacts" below) work the same way as units: the whole
list, by name, whenever it differs from the one the cursor last delivered, and nothing
otherwise; the client replaces what it holds, so a contact missing from it was deleted.
Unlike units, the list is the **caller's own**: every statewide contact (no
`locationId`), plus the contacts of each location they may use in Tally (every unit they belong to
and their user's units, either role, or every location for a super user) — exactly what
GraphQL lets them read. So a member who gains or loses a unit gets the new list on their
next sync. `description` and `email`, and `phone` when absent, are left out; whether the
caller may _edit_ one isn't sent (a location's always, a statewide one only for a super
user, which `tallyViewer.isSuper` says). Each response scans the `shared_contact` table
(a few rows per location) to compute the hash. A cursor from before shared contacts were
synced has no hash for them, so its next response carries the list.

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
(`<unix µs, 20 digits>#<id>`), restamped by every write that changes a returned field,
by a rename of their location, and by a change to their `person_unit` rows. A sparse
GSI orders everyone by it (see SCHEMA.md), and the cursor is the last key sent,
HMAC-signed with a key derived from `JWT_SECRET`.
A read never serves keys from the last 30 seconds: a write takes its key from the clock
moments before it lands, so without that margin a client could move past a key that
had not been written yet. A change is therefore delivered on the first sync at least
30 seconds after it.

A full sync's first request (the one without a cursor) is logged at `info`
(`sync: full sync page`, with the member's person id and row count); requests with a cursor
aren't.

**New environments** need the GSI (`terraform apply`) and a one-off backfill, so
existing rows are in it:

```bash
make local-cli ARGS="--dry-run person stamp-contacts"   # or `cargo run --bin cli -- …` against a real table
```

**Locally**, after `make dev-local`:

```bash
curl -s --compressed -H 'Authorization: Bearer slu_TokenReadOnly.localdev-readonly' \
  'http://localhost:8000/api/v1/sync?limit=2'
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

```graphql
mutation {
  createNote(
    locationId: "TestAUnit001"
    title: "Radio channels"
    content: "Ops: 1\nAdmin: 2"
  ) {
    id
    version
  }
}
```

## Self sign-in (Tally)

A member signs themselves in and out from the Tally app, exactly as they would by typing
their member number at a kiosk at their primary unit, or at another unit they can use in Tally (a secondary unit included)
(`src/graphql/self_sign_in.rs`; the two mutations are in `mutations.rs`).

- **Who:** only a member signed in to Tally. Users on their own, kiosk sessions, API tokens
  and edit links get `FORBIDDEN`. No location grant is needed: being the member is the whole
  authority, read fresh on every call, and the member must still exist and not be deleted.
  The MCP interface deliberately has no tool for this.
- **Which unit:** `selfSignInStatus` and `selfSignIn` take an optional `locationId`. Without
  it they act at the primary unit. Any unit given must be one the member can use in Tally — one
  they belong to (primary or secondary, no user grant needed), a unit the user signed in alongside them holds either
  role at, or any unit for a super user (`require_member_location`, as for Tasks) — else
  `FORBIDDEN`; one that doesn't exist is `NOT_FOUND`. This is the app's version of a member
  signing in at another unit's kiosk.
- **One unit at a time:** only periods at the chosen unit count, as the kiosk scopes by
  location. A period open at another unit (say, via that unit's kiosk) is ignored, and
  `selfSignIn` may open a period alongside it.
- `Query.selfSignInStatus(locationId)` returns `{ person, location, openPeriod, quickPick }`,
  or null when the caller has no linked member. `quickPick` is the kiosk's sign-out shortcuts
  (`build_quick_pick`), computed only when selected, and null if building them fails.
- `SelfSignInStatus.location` is the unit the status is for. `whosHere` is the kiosk's
  "Who's here" list for that unit: `[{ id, name, guest, startTime, isMe }]` for every open,
  non-deleted period there (members and guests), oldest first (ties by id), capped at 500. `name` is "First Last", the guest's
  name ("Guest" if blank) or "Unknown member" for a missing or deleted member; `isMe` marks
  the caller's own period. It is deliberately narrow (no guest reason, member number or
  other period fields), needs no location grant at any unit the member belongs to, and is computed only when selected.
- `selfSignIn(locationId)` returns the newest open period at that unit unchanged if there is
  one (a double tap never opens two), otherwise opens one with no kiosk session and publishes
  the realtime event. Without a linked member it fails `BAD_REQUEST`.
- `selfSignOut(id, expectedVersion, startTime, endTime, categoryId)` closes one of the
  member's own periods at whichever unit. It takes Unix seconds and refuses (messages are written for the member to read): `NOT_FOUND` if the period is
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
query {
  selfSignInStatus {
    person {
      firstName
    }
    openPeriod {
      id
      version
      startTime
    }
    whosHere {
      id
      name
      guest
      startTime
      isMe
    }
  }
}
mutation {
  selfSignIn {
    id
    version
  }
}
# At another unit the member can use:
mutation {
  selfSignIn(locationId: "TestBUnit001") {
    id
    version
  }
}
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
- **Photos:** up to 10, through `photoAssetIds` on `createEquipment` and `updateEquipment`; see "Attached photos".
- **Linked notes:** up to 10, through `noteIds` on `createEquipment` and `updateEquipment`; see "Linked notes".
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

```graphql
mutation {
  changeEquipmentStatus(id: "…", status: OUT_OF_SERVICE, comment: "Flat tyre") {
    status
    log {
      kind
      previousStatus
      status
      comment
      createdBy
      createdAt
    }
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
- **Who it is for:** one or more members, or free text — never both, which is refused.
  `reservedForMemberIds` names the members by person ID (max 20, repeats dropped; any unit's,
  as for an on-call role, the caller among them or not; each must exist and not be deleted,
  though one already on the reservation may stay after they are). Otherwise `reservedFor`
  names someone or something else — someone outside the organisation, an organisation,
  "Detailing / clean" — as one trimmed line (max 200). With neither (blank text, no members)
  it is the caller's. Read back as `reservedFor` (the text, or the members' names by name,
  comma-separated), `reservedForMemberIds` / `reservedForMembers` (the `Person`s, live ones by
  name) or `reservedForText` (members and text are never both set) and `reservedForViewer`
  (whether the caller is among the members).
- **Overlaps are allowed, and reported.** `EquipmentReservation.conflicts` lists the
  location's other reservations that overlap it in time and share a piece of equipment with
  it (times are half-open, so back-to-back reservations don't clash). It costs a query per
  reservation, so a client listing many should compare them itself.
  `TallyLocation.equipmentReservationConflicts(equipmentIds, startTime, endTime, excludeId)` asks
  the same question about a reservation not yet saved, so a client can warn before saving;
  `excludeId` leaves out the one being edited.
- **Writes:** `createEquipmentReservation(locationId, equipmentIds, startTime, endTime,
reservedFor, reservedForMemberIds, description)`. `updateEquipmentReservation` changes only
  the arguments given — who it is for changes when `reservedFor` or `reservedForMemberIds` is
  given, decided by the two as on create, so `reservedFor: null` makes it the caller's — and
  takes `expectedVersion`, failing with
  `CONFLICT` if someone else changed it since it was read; only a changed time range has to end
  in the future, so a finished one's description can still be fixed.
  `deleteEquipmentReservation` cancels it.

```graphql
mutation {
  createEquipmentReservation(
    locationId: "…"
    equipmentIds: ["…"]
    startTime: 1793000000
    endTime: 1793010800
    reservedFor: "Detailing / clean"
  ) {
    id
    reservedFor
    conflicts {
      id
      reservedFor
      startTime
      endTime
    }
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
- **Equipment reservations** are not calendar events: Tally shows them in its calendar as a
  virtual "Reservations" calendar, read from `TallyLocation.equipmentReservations`.

```graphql
mutation {
  createCalendarEvent(
    locationId: "TestAUnit001"
    title: "Training night"
    allDay: false
    startTime: 1793000000
    endTime: 1793007200
    repeat: WEEKLY
    repeatUntil: "2027-06-30"
  ) {
    id
    version
  }
}
{
  tallyLocation(id: "TestAUnit001") {
    calendarEvents(from: 1793000000, to: 1795600000) {
      startTime
      endTime
      startDate
      endDate
      event {
        id
        title
        allDay
        calendarId
        calendar {
          name
          color
        }
      }
    }
  }
}
```

## Agendas

Meeting agendas and minutes for Tally's Agendas app (`src/graphql/meetings.rs` and
`src/graphql/agenda_items.rs`; storage in SCHEMA.md's `meeting` and `agenda_item` sections). This
section covers **meetings**, their **topics** (agenda items) and the topics' links to tasks,
equipment, on-call roles, photos and notes, and the meeting's exported **minutes**.

- **Who:** any member signed in to Tally who can use the unit (`require_meeting_access`, through
  `require_member_location`: see "Tally (members)" above) may read and write every meeting of it,
  finished ones included. Users on their own, kiosk sessions, API tokens and edit links are refused.
  Authors are recorded by person ID (`createdBy` / `updatedBy` read as the member's name).
- **Reading:** `TallyLocation.meetings(startsAfter, startsBefore, newestFirst, limit)` lists the
  unit's live meetings scheduled in `[startsAfter, startsBefore)` (Unix seconds, either optional),
  oldest first or newest first, at most `limit` (default 50, capped at 200). `Query.meeting(id)`
  reads one (null when missing or deleted).
- **Status:** `PLANNED` (never started), `IN_PROGRESS` (`startedAt` set, `finishedAt` not) or
  `FINISHED` (`finishedAt` set), derived from the two timestamps.
- **Writes:** `createMeeting(locationId, title, description, startsAt)`,
  `updateMeeting(id, expectedVersion, title, description, startsAt)` (omitted
  arguments unchanged), `deleteMeeting(id)`,
  `startMeeting(id, expectedVersion)` and `finishMeeting(id, expectedVersion)`. The title is one
  line of at most 200 characters; the description is at most 5,000.
- **Start and finish:** `startMeeting` stamps `startedAt` now. On a `FINISHED` meeting it
  **reopens** it (clears `finishedAt`, keeps the original `startedAt`); on one already
  `IN_PROGRESS` it fails with `BAD_REQUEST`. `finishMeeting` works only from `IN_PROGRESS`
  (otherwise `BAD_REQUEST`).
- **Concurrency:** every update, start and finish takes `expectedVersion`; if someone else changed
  the meeting since it was read the call fails with `CONFLICT` rather than overwriting.
- **Audit:** each write is recorded against the meeting (see CLAUDE.md's audit log).

### Topics (agenda items)

Topics have the same access as meetings (`require_meeting_access` on the topic's location).

- **Reading:** `Meeting.items` lists a meeting's live topics ordered by `position`, then when
  added, then ID. `Query.agendaItem(id)` reads one (null when missing or deleted). `AgendaItem`
  has `meetingId`/`meeting`, `locationId`, `text`, `presenter` (the `Person`, null once deleted)
  and `presenterId`, `position` (1-based), `version`, and `createdBy` (the member's name) /
  `createdById` (their person ID, for "added by") / `updatedBy`.
- **One text box:** a topic has no title or separate minutes, just `text`: Markdown as in Notes,
  kept as written, at most 20,000 characters (only-whitespace counts as empty). Whatever needs a
  short name for a topic uses the first line of its text as plain text (cut to 100 characters
  and "…"; `src/agenda_text.rs`), the same rule as Tally's preview. A topic must have text
  or at least one link (`BAD_REQUEST` otherwise); `text` can be omitted on create when it links
  something.
- **Creating:** `createAgendaItem(meetingId, text, presenterId, …links)`. The presenter defaults
  to the caller and may be a live member of any unit (`NOT_FOUND` otherwise). The topic goes at the
  end (`position` = highest + 1), whatever the meeting's status: there is no difference between a
  topic added before and during the meeting. A missing or deleted meeting is `NOT_FOUND`.
- **Updating:** `updateAgendaItem(id, expectedVersion, text, presenterId, …links)`: omitted
  arguments are unchanged, and the presenter cannot be cleared (the current presenter may stay even
  after being deleted). Stale `expectedVersion` is `CONFLICT`. `deleteAgendaItem(id)` returns the
  ID.
- **Presenter notifications:** making another member the presenter, on `createAgendaItem` or by
  changing it in `updateAgendaItem`, tells them in their Tally inbox (see "Notifications": source
  `agendas`, kind `agenda.presenter_assigned`, target `agenda_item`; the body names the topic by
  the first line of its text, else its first link's name, else "Topic n"). Best-effort: a failed
  notification never fails the mutation, and presenting your own topic sends nothing.
- **Reordering** (at any meeting status): `reorderAgendaItems(meetingId, itemIds)` takes exactly
  the meeting's live topic
  IDs, each once, in the wanted order (otherwise `BAD_REQUEST`), renumbers them 1..n, writes only
  topics whose position changed (each conditional on the version just read; a topic changed in the
  meantime is `CONFLICT`) and returns them in the new order.
- **Audit:** create, field changes, moves and deletes are each recorded against the topic
  (link changes included, as `task_ids`, `equipment_ids`, `on_call_role_ids`, `photo_asset_ids`
  and `note_ids`).
- **Links:** a topic links to up to 10 each of the unit's tasks, equipment, on-call roles, photos
  and notes. `AgendaItem` has `tasks`/`taskIds`, `equipment`/`equipmentIds`,
  `onCallRoles`/`onCallRoleIds`, `photos`/`photoAssetIds` and `notes`/`noteIds`: the object lists
  are in order and leave out anything deleted or unreadable, the ID lists are as stored. Both
  `createAgendaItem` and `updateAgendaItem` take `taskIds`, `equipmentIds`, `onCallRoleIds`,
  `photoAssetIds` and `noteIds` (all optional `[ID!]`), so a topic and its first link can be made
  in one call. On an update, omitted leaves a list alone and a list replaces it (`[]` clears it).
  A task, piece of equipment or role (`src/graphql/linked_items.rs`) must exist, be live and belong
  to the topic's unit when it is linked; more than 10, or the same one twice, is `BAD_REQUEST`; one
  already linked may stay whatever has become of it. They have no reference rows. Photos follow
  "Attached photos" and notes "Linked notes", with owner type `agenda_item`: refs are written
  before the topic names the asset or note (the topic's ID is chosen first for this) and removed
  after it stops naming it or is deleted. Linking never changes the thing linked.

```graphql
mutation {
  createAgendaItem(meetingId: "…", text: "Budget\n\nAgreed to defer.") {
    id
    position
    text
    presenter {
      id
    }
  }
}
```

### Minutes

`Meeting.minutes: String!` is the meeting's minutes as Markdown, for Tally to show, share, copy
and save as a note (`src/graphql/meeting_minutes.rs` gathers the data, `src/agendas.rs` renders
it: a pure function with its own unit tests). It needs the same access as the meeting and is
built fresh on every read; a deleted task, equipment, role, note or person is left out, as on
`AgendaItem`. It uses only what Tally's note renderer shows (`#` and `##` headings, paragraphs,
`- ` lists, `**bold**`, backslash escapes) and also reads as plain text pasted into an email:

```text
# Monthly unit meeting

Test A Unit · Wed 18 Nov 2026, 7:32–8:41 pm

## 1.
Presented by Bob Brown

Vehicle servicing schedule

Annual service of KMA56. KMA56 to go in on 21 Nov.

- Task: Book KMA56 service — Bob Brown, due 21 Nov 2026
- Equipment: KMA56 — Has an issue
- On call: Duty officer — Dana Dualunit (+ Al Adams)
- Note: Service checklist
- Photos: 2 attached

## 2.
Presented by Bob Brown

Christmas BBQ date

Sat 13 Dec, at the shed.
```

- **Header:** `# <title>`, then `<unit> · <when>` in Sydney time: "Planned for Wed 18 Nov 2026, 7:30 pm"
  (the scheduled start) until it is started, "In progress since …" (the actual start) while it
  runs, and "Wed 18 Nov 2026, 7:32–8:41 pm" once finished (both dates if it ran past midnight).
  The meeting's description follows as a paragraph.
- **Topics:** every topic, in agenda order, under a bare `## n.` heading, whatever the meeting's
  status when it was added (there is no "other business" group). A topic has no title and its text
  is shown in full, so the heading carries no name. Each topic has "Presented by <name>" (left out
  if the presenter was deleted), its text, then its links as a bullet list. A meeting with no
  topics says so.
- **Text** is inserted exactly as the members wrote it (only line endings and surrounding blank
  space are normalised), so its own Markdown shows as written, first line included. Everything
  else a member typed (names, the meeting's title and description) is escaped so it cannot make
  headings, lists, quotes or formatting of its own (for example, `# Budget` as a title shows as
  typed).
- **Links:** "Task: <title> — <assignees>, due <date> (done)", "Equipment: <name> — <status>"
  (Tally's wording, e.g. "Has an issue"), "On call: <role> — <primary> (+ <secondaries>)" (or
  "nobody"), "Note: <title>" and "Photos: n attached".

```graphql
mutation {
  createMeeting(
    locationId: "TestAUnit001"
    title: "Monthly unit meeting"
    description: "Unit hall"
    startsAt: 1793000000
  ) {
    id
    version
    status
  }
}
{
  tallyLocation(id: "TestAUnit001") {
    meetings(startsAfter: 1793000000, newestFirst: false, limit: 20) {
      id
      title
      status
      startsAt
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
- **Photos:** up to 10, through `photoAssetIds` on `createMapPin` and `updateMapPin`; see "Attached photos".
- **Linked notes:** up to 10, through `noteIds` on `createMapPin` and `updateMapPin`; see "Linked notes".
- **Writes:** `createMapPin`, `updateMapPin` (omitted arguments unchanged, `layerId: null`
  takes the pin off its layer, `type: null` makes it a plain pin), `deleteMapPin`, `createMapLayer`, `renameMapLayer`,
  `deleteMapLayer`. Deleting a layer leaves its pins on the location's map (`MapPin.layer`
  becomes null).
- **Concurrency:** `updateMapPin` takes `expectedVersion`; if someone else saved the pin since
  it was read, the save fails with `CONFLICT` rather than overwriting their change.

```graphql
mutation {
  createMapPin(
    locationId: "TestAUnit001"
    title: "Staging area"
    latitude: -33.8688
    longitude: 151.2093
    type: ACCESS
    address: "1 Example St"
    note: "Gate code at the shed"
  ) {
    id
    version
  }
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
- **Linked notes:** up to 10, through `noteIds` on `createOnCallRole` and `updateOnCallRole`; see "Linked notes".
- **Concurrency:** `updateOnCallRole` takes `expectedVersion`; if someone else saved the role
  since it was read, the save fails with `CONFLICT`.

```graphql
mutation {
  createOnCallRole(
    locationId: "TestAUnit001"
    name: "Duty Officer"
    phone: "0400 000 000"
    primaryPersonId: "TestAMember1"
  ) {
    id
    version
  }
}
```

## Shared contacts

Contacts added by hand to the Tally app's Contacts, beside the synced member directory: a
location's own (its Duty Officer phone, a shed landline, …) or statewide ones (the SOC, the EAP,
peer support, …) (`src/graphql/shared_contacts.rs`, rules in `src/shared_contact.rs`, storage in
SCHEMA.md's `shared_contact` section).

- **Who:** a location's contacts are like its other shared tools: any member signed in to Tally
  who can use the unit may read, add, change and delete them — see "Tally (members)" above.
  Statewide contacts can be read by every member signed in to Tally, but only added, changed or
  deleted when the user signed in alongside them is a super user (`FORBIDDEN` otherwise).
  `SharedContact.viewerCanEdit` says which. Users on their own, kiosk sessions, API tokens and
  edit links are refused.
- **Reading:** `TallyLocation.sharedContacts` lists a location's contacts by name;
  `Query.stateContacts` lists the statewide ones by name; `Query.sharedContact(id)` reads one.
  `statewide` and `locationId` (null for statewide) say which list a contact is on.
- **Fields:** `name` is one trimmed line (max 100 characters). `description` is plain text kept
  as written (max 2,000; only-whitespace counts as empty). `phone` is optional, with the same
  rule as an on-call role's (digits, spaces, `()-` and a leading `+`, 3–15 digits, kept as
  written). `phoneCanSms` says whether the phone takes text messages, and is always false
  without a phone. `email` is optional, trimmed, and only checked for a basic `name@domain.tld`
  shape (max 254).
- **Writes:** `createSharedContact` (with `locationId` for a location's contact, without it for
  a statewide one), `updateSharedContact` (omitted arguments unchanged, `phone: null` and
  `email: null` remove them, and removing the phone turns `phoneCanSms` off), and
  `deleteSharedContact`. A contact never moves between a location and statewide.
- **Concurrency:** `updateSharedContact` takes `expectedVersion`; if someone else saved the
  contact since it was read, the save fails with `CONFLICT`.
- **Offline:** the offline sync API (above) also sends each member their shared contacts, so
  Tally can show them without a connection. Writes still go through GraphQL.

```graphql
mutation {
  createSharedContact(
    locationId: "TestAUnit001"
    name: "Duty Officer"
    phone: "0400 000 000"
    phoneCanSms: true
  ) {
    id
    version
  }
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

```graphql
mutation {
  startLocationShare(
    locationId: "TestAUnit001"
    durationMinutes: 15
    intervalSeconds: 120
    position: { latitude: -33.70, longitude: 151.10, accuracy: 8 }
  ) {
    endsAt
  }
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
- **Producing one:** call `notifications::notify(app, NewNotification, Audience)` _after_ the
  business write has succeeded. `NewNotification::new(source, kind, title)` takes `.location()`,
  `.body()` and `.target(type, id)`. An `Audience` is `Members(person_ids)`. Only live (not
  deleted) members are notified, the acting member (the current audit-context actor, if it is
  a member) is left out, and nobody is notified twice.
- **Producers:** each fires after its write succeeds, in the GraphQL resolver (so the MCP
  tools, which run the same documents, are covered; no other path makes these writes), always
  with `locationId` set. Tally maps `source`/`kind`/`targetType` to icons and screens, so the
  strings are fixed:

  | source         | kind                  | targetType     | When                                                                                                                                                                                                                             | Audience                                    | Title / body                                                                                                                                |
  | -------------- | --------------------- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
  | `reservations` | `reservation.for_you` | `reservation`  | `createEquipmentReservation`, or `updateEquipmentReservation`, adds members to `reserved_for_person_ids` (every member it is for, on create)                                                                                     | each added member other than the actor      | "Equipment reserved for you" / "<equipment names, sorted> · <Sun 25 Oct 09:00–11:00, Sydney time>" ("Equipment" if the names can't be read) |
  | `tasks`        | `task.assigned`       | `task`         | `createTask`, or `updateTask`, adds members to `assignee_ids` (every assignee, on create)                                                                                                                                         | each added member other than the actor      | "Task assigned to you" / "<title>" or "<title> · Due <Mon 4 Mar>"                                                                          |
  | `oncall`       | `oncall.assigned`     | `on_call_role` | `createOnCallRole`, or `updateOnCallRole`, makes a person the primary when they were not, or adds them to the secondaries when they were not already there (moving primary to secondary counts; removals and reorderings do not) | that member, once per newly assigned member | "You're now primary for <role>" / "You're now a secondary for <role>"                                                                       |
  | `agendas`      | `agenda.presenter_assigned` | `agenda_item` | `createAgendaItem` with a `presenterId` other than the caller, or `updateAgendaItem` changing the presenter to someone other than the caller (no notification when the presenter is the caller, is unchanged, or the update leaves the presenter alone) | the new presenter | "You're presenting at <meeting title>" / "<topic> · <Wed 18 Nov 2026, 7:30 pm, Sydney time of the meeting's `startsAt`> · added by <caller name>" (`<topic>` is the first line of the topic's text as plain text, cut to 100 characters, else its first link's name, else "Topic n"; "assigned by" on an update; the " · … by …" part is left out if the caller's name can't be read) |

  Equipment going into HAS_ISSUE, OUT_OF_SERVICE or MISSING used to tell the unit's users; it
  notifies nobody for now, while who among a unit's members should hear is rethought
  (`EquipmentStatus::attention_title` still holds the titles).

  The acting member is never told (so a reservation you make for yourself, a task you assign to
  yourself, or an on-call role you put yourself in, sends nothing).

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

```graphql
{
  myNotifications(first: 20, unreadOnly: true) {
    edges {
      cursor
      node {
        id
        title
        body
        targetType
        targetId
        createdAt
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
  myUnreadNotificationCount
}
```

## Storage

Uploaded files live in an S3 bucket, reached through `storage::Handler` (`src/storage.rs`)
with the same trait / AWS impl / mock split as the queue: `s3::Handler` in `poem` and the API
Lambda, `mockstorage::Handler` in `poem-local` and tests. The bucket is `ASSET_BUCKET`; unset or
blank turns uploads off (`is_enabled()` is false, every call returns `storage::Error::Unavailable`)
without affecting anything else.

- **Keys:** `pending/{id}` (where an upload lands, expired by a lifecycle rule),
  `originals/{id}` and `derived/{id}/{variant}.webp` (`storage::pending_key`,
  `original_key`, `derived_key`).
- **Uploads go straight to storage**, never through the API: `presign_post` returns a
  `PresignedPost { url, fields, expires_at }`, a form the client posts (fields in order, then
  the file as a part named `file`, last). S3 itself enforces the exact key, content type and
  size. `aws-sdk-s3` cannot presign a POST, so `s3::sign_post` signs the policy by hand with
  SigV4; its unit tests pin the signing key against AWS's published example and a full signature.
- **Locally**, `mockstorage` returns the relative URL `/dev/storage/upload` and the fields
  `key`, `Content-Type` and `policy` (an HMAC-signed token binding key, type, exact size and
  expiry). `poem-local` serves that route: 204 on success, 400 with a short message otherwise.
  Files go to `MOCK_STORAGE_DIR` (default `local/storage/`).

## Assets and uploads

How a Tally member uploads an image (`src/assets.rs` for the rules, `src/graphql/assets.rs` for
the API; storage is described under "Storage" and the served images under "Media"). Only
members signed in to Tally may, at any unit they can use (`require_asset_access`, via
`require_member_location`); users, kiosks and API tokens are refused.

1. `createAssetUpload(input: { locationId, contentType, byteSize, filename, width, height,
   takenAt, cameraMake, cameraModel })` validates the request, counts it against the member's
   limit, creates a **pending** asset (`created_by` is the member's person ID) and returns
   `{ asset, uploadUrl, fields, expiresAt }`.
2. The client POSTs a `multipart/form-data` request to `uploadUrl` with each of `fields` **in
   order** and the file as a part named `file` **last**. `uploadUrl` may be relative: resolve
   it against the API origin (the origin of the GraphQL URL). The form works for 15 minutes
   and storage itself enforces the key, content type and exact size. It answers 204.
3. `completeAssetUpload(id)` checks the object arrived, is exactly `byteSize` bytes and starts
   with the magic bytes of `contentType`, copies it to `originals/{id}`, deletes
   `pending/{id}` and marks the asset **ready**. Completing a ready asset returns it again. A
   missing file, wrong size or wrong magic bytes is `BAD_REQUEST` (the last two also delete the
   pending file); the asset stays pending.

`Asset.url(variant: THUMB | DISPLAY | FULL)` is a relative signed `/media/...` URL (see
"Media"), null until the asset is ready. `Asset.createdBy` is the uploader's person ID.

- **Accepted:** `image/jpeg` and `image/webp`, 1 byte to 30 MiB (31,457,280). Tally converts
  HEIC to JPEG on the phone. `width`/`height` (1 to 20000), `takenAt` (not more than a day
  ahead, not older than five years), `filename` (path stripped, 255 characters) and camera
  make/model (100 characters) are hints: ones that don't fit are dropped, not refused. No GPS
  is ever sent or stored.
- **Limit:** 200 uploads per member per Sydney day, counted when `createAssetUpload` succeeds
  validation (`assets::upload_counter_id`, a daily counter in `ephemeral_state`). The 201st
  fails `RATE_LIMITED`: "You've uploaded the most files allowed today". `assets::daily_counter_id`
  builds the id for any other daily counter.
- **Errors:** `BAD_REQUEST` for an unaccepted type or size, an upload that hasn't arrived or
  doesn't match, and storage not configured ("Uploads are not available on this environment",
  when `ASSET_BUCKET` is unset); `FORBIDDEN` for the wrong kind of caller or unit; `NOT_FOUND`
  for an unknown location or asset; `RATE_LIMITED` as above; `INTERNAL` if storage fails.
- **Cleanup:** a pending asset's row expires after a day (DynamoDB TTL) and `pending/` objects
  after a day (lifecycle rule). Nothing deletes a ready asset yet.
- **Locally:** `make dev-local` (`poem-local`) uses `mockstorage`: `uploadUrl` is
  `/dev/storage/upload`, served by `poem-local`, and files land in `MOCK_STORAGE_DIR`.

## Photos

Tally's Photos app: a unit's shared photo stream (`src/photos.rs` for the rules and the sweep,
`src/graphql/photos.rs` for the API). A photo is a member's ready asset (see "Assets and
uploads") shown to everyone who can use the unit in Tally (`require_photo_access`, via
`require_member_location`); users, kiosks and API tokens are refused.

- **Reading:** `TallyLocation.photos(first: Int = 60, after: String)` lists the unit's visible,
  unexpired photos newest first by `sortTime` (`takenAt`, else `createdAt`), at most 200 a page
  (`first` is clamped to 1..200). `endCursor` is opaque (the last photo's `sort_key`, base64);
  a cursor we did not issue is `BAD_REQUEST`. `Photo.asset.url(variant:)` is the image.
  `TallyLocation.photoUploadsRemainingToday` is 100 minus today's count, never below 0.
- **Adding:** `createPhoto(locationId, assetId)`. The asset must exist (`NOT_FOUND`), be ready
  and at that unit (`BAD_REQUEST`) and have been uploaded by the caller (`FORBIDDEN`). The unit's
  daily counter (`counter#photo#<location>#<Sydney date>`, in `ephemeral_state`) is incremented
  first; the 101st photo of a Sydney day is `RATE_LIMITED` ("This unit has added 100 photos
  today. Try again tomorrow."). The photo and its `photo#<id>` asset reference are written in
  one transaction; if the asset changed under us that is `BAD_REQUEST`.
- **Hiding:** `hidePhoto(id)`, by any member who can use the photo's unit. The photo leaves the
  stream for everyone immediately and its `expires_at` becomes at most 7 days after it was
  hidden. There is no undo. A photo that is unknown, swept or already hidden is `NOT_FOUND`.
- **Lifetime:** a photo is shown for 30 days after it was added (`db::PHOTO_LIFETIME_SECS`).
- **The sweep:** `photos::sweep` soft-deletes every photo whose `expires_at` has passed and
  removes its asset reference, so the future garbage collector can free the asset. It runs
  daily in the checker Lambda (06:00 Sydney) after session retirement, audited as `system:
  photo-sweep`; a failure is logged and neither stops session retirement nor the stale-sync
  alert, and fails the invocation at the end. By hand: `cli photo sweep` (global `--dry-run`
  to preview). The stored image objects are not deleted by the sweep.

## Attached photos

Tasks, equipment, map pins and agenda items can each carry up to 10 photos (`src/graphql/attached_photos.rs`;
storage is each row's `photo_asset_ids` in SCHEMA.md). Access is the owner's own (`require_task_access`,
`require_equipment_access`, `require_map_access`, `require_meeting_access`).

- **Reading:** `Task.photos`, `Equipment.photos`, `MapPin.photos` and `AgendaItem.photos` are the attached assets in
  order (`url(variant:)` is the image; one that can no longer be read is left out), and
  `photoAssetIds` their IDs.
- **Writing:** `photoAssetIds` on `createTask`/`updateTask`, `createEquipment`/`updateEquipment`
  `createMapPin`/`updateMapPin` and `createAgendaItem`/`updateAgendaItem`. On an update, omitted leaves the photos alone and a list
  replaces them, in that order (`[]` removes them all).
- **What may be attached:** any ready image asset at the owner's unit: one the member just
  uploaded (see "Assets and uploads"), or one from the unit's photo stream (`Photo.asset.id`),
  whoever uploaded it. More than 10, the same asset twice, or an asset that is missing, not
  ready or at another unit is `BAD_REQUEST`. A photo already attached may stay attached
  whatever has become of its asset.
- **References:** each attached asset has an `asset_ref` owned by `task#<id>`, `equipment#<id>`
  `map_pin#<id>` or `agenda_item#<id>`, written (checking the asset is ready at the unit) before the owner first
  names it and removed after the owner stops naming it or is deleted. So a photo taken from
  the stream stays attached after the stream's sweep drops its own reference. A reference left
  behind by a failed write only delays the future garbage collector.

## Linked notes

Tasks, map pins, equipment, calendar events, on-call roles and agenda items can each link to up to 10 of
their unit's [notes](#notes) (`src/graphql/linked_notes.rs`; storage is each row's `note_ids`
and the `note_ref` table in SCHEMA.md). Access is the owner's own (`require_task_access`,
`require_map_access`, `require_equipment_access`, `require_calendar_access`,
`require_on_call_access`, `require_meeting_access`). It follows "Attached photos" closely.

- **Reading:** `notes` on `Task`, `MapPin`, `Equipment`, `CalendarEvent`, `OnCallRole` and `AgendaItem` is
  the linked notes in order (loaded through the `Note` dataloader; one that was deleted is left
  out), and `noteIds` their IDs, deleted ones included.
- **Writing:** `noteIds` on `createTask`/`updateTask`, `createMapPin`/`updateMapPin`,
  `createEquipment`/`updateEquipment`, `createCalendarEvent`/`updateCalendarEvent` and
  `createOnCallRole`/`updateOnCallRole` and `createAgendaItem`/`updateAgendaItem`. On an update, omitted leaves the links alone and a list
  replaces them, in that order (`[]` removes them all). `expectedVersion` and `CONFLICT` behave
  as before.
- **What may be linked:** a live (not deleted) note at the owner's unit. More than 10, the same
  note twice, or a note that is missing, deleted or at another unit is `BAD_REQUEST`. A note
  already linked may stay linked whatever has become of it (deleting a note does not unlink it;
  it just stops showing in `notes`).
- **The note is never touched:** linking, unlinking and deleting the owner leave the note, and
  its version, as they were.
- **References:** each link has a `note_ref` row, `(note_id, owner)` with `owner` one of
  `task#<id>`, `equipment#<id>`, `map_pin#<id>`, `calendar_event#<id>`, `on_call_role#<id>` or `agenda_item#<id>`,
  written before the owner first names the note and removed after the owner stops naming it or
  is deleted (a failed removal is logged, not fatal). `db::Handler::list_note_refs(note_id)`
  answers "which items link to this note"; because a failed write can leave a stray row, a reader
  must check the owner still names the note. Nothing exposes that in GraphQL yet (a later
  `Note.linkedFrom`). Not audited, like `asset_ref`; the owner's `note_ids` change is.

## SOP notes

A unit can link one of its [notes](#notes) per Tally screen as its standard operating
procedure for that tool; Tally shows the note's title and first line at the top of the screen
with an info icon (`src/graphql/sop_notes.rs`). The places (`SopPlace`) are `ON_CALL`,
`EQUIPMENT` (the list), `EQUIPMENT_BORROWING` (shown when a member marks equipment as
borrowed), `TASKS`, `CALENDAR`, `MAP`, `RESERVATIONS`, `NOTES` and `PHOTOS`.

- **Storage:** one optional attribute per place on the unit's `location` row (`sop_note_on_call`,
  `sop_note_equipment`, …, holding a note ID; see SCHEMA.md), `Location.sop_notes` in Rust. Absent
  means nothing linked. It is written only by `LocationUpdateShape::SopNote`, which sets or
  removes that one attribute and leaves `updated_at` (and so sync) alone.
- **Reading:** `TallyLocation.sopNotes: [SopNote!]!`, one `{ place, noteId, note }` per place with a
  linked note that still exists (the `Note` is loaded through its dataloader), in the order above.
  A deleted note drops out; it is not unlinked.
- **Writing:** `setSopNote(locationId: ID!, place: SopPlace!, noteId: ID): TallyLocation!` links
  `noteId` at `place`, replacing any note linked there, or unlinks the place when `noteId` is null;
  read `sopNotes` off the returned unit. Linking the note already linked, or unlinking an empty
  place, is a successful no-op that writes nothing.
- **Access:** `require_note_access` on the unit, for both: only an `AuthInfo::Member` at that unit
  (users alone, kiosks, API tokens and edit links get `FORBIDDEN`). Any such member may link, not
  just admins. There is no MCP tool.
- **Errors:** a note that does not exist, is deleted or belongs to another unit is `BAD_REQUEST`
  ("That note can't be linked. It may have been deleted."); a unit that does not exist is `NOT_FOUND`.
- **References:** each link has a `note_ref` owned by `location_sop#<location_id>:<PLACE>`, written
  before the location names the note and removed after it stops (a failed removal is logged, not
  fatal), as in "Linked notes".
- **Audit:** an update to the location with a `sopNote.<PLACE>` field diff, filed at that unit.

## Media

Members only ever receive **derived** images: `src/media.rs` turns an original in the asset
bucket into a normalised WebP at one of three sizes, on the first request for it.

- **Variants** (`media::ImageVariant`, GraphQL `ImageVariant { THUMB DISPLAY FULL }`): `thumb`
  400, `display` 1440, `full` 2560 pixels on the long edge. Never upscaled.
- **Pipeline** (`media::derive`): decode JPEG or WebP under a 100-megapixel / 768 MiB limit,
  resize (a cheap pre-shrink first for big sources, then Catmull-Rom), apply the EXIF
  orientation, convert an embedded ICC profile to sRGB (`qcms`), encode lossy WebP at quality
  80. The output has no metadata at all, so no GPS. It runs in `spawn_blocking`.
- **Storage:** the result is written to `derived/{id}/{variant}.webp` and served from there
  next time. Originals are never served. A failed write is logged and the image still returned.
- **URL** (`media::media_path(asset_id, variant, now, &secret)`): relative,
  `/media/{asset_id}/{variant}?e={expires}&s={sig}`; clients resolve it against the API origin.
  `expires = (floor(now / 86400) + 2) * 86400`, so the URL is identical for a whole UTC day
  (CDN and app caches hit) and valid for 1 to 2 days from when it was minted. `sig` is the first
  16 bytes of HMAC-SHA256 over `"{id}/{variant}/{expires}"` (base64url, no padding), keyed by
  `ServerSecret::media_url_key()` (its own derivation of `JWT_SECRET`).
- **Responses** (`media::serve`): 200 `image/webp` with `Cache-Control: public, max-age=31536000,
  immutable`; 400 unknown variant or malformed path; 401 bad, missing or expired signature; 410
  no original; 422 the original cannot be decoded; 500 storage or encoding failed; 503 no
  `ASSET_BUCKET`. Every error is `no-store`. **Never 403 or 404**: every CloudFront distribution
  rewrites those to `/index.html` with a 200.
- **Where it runs:** `poem` and `poem-local` serve `/media/*` in-process from the app's storage
  and secret (local dev has no CloudFront). Deployed, the API Lambda does not; CloudFront routes
  `/media/*` to the media Lambda (`bin/media-lambda.rs`: `ASSET_BUCKET` + `JWT_SECRET`, no
  DynamoDB, 2048 MB): `seslogin-media` for prod, preprod and qa (prod bucket, deployed from
  `workers`) and `seslogin-test-media` for test (deployed from `test`). The behaviour's cache
  policy puts all query strings in the key (`aws_cloudfront_cache_policy.media`).

## WiFi networks

The WiFi networks Tally's WiFi app shows signed-in members, with their passwords
(`src/wifi.rs`, `src/graphql/wifi.rs`).

- **Never committed or stored:** they come from the `WIFI_NETWORKS` environment variable, a
  JSON array. Deployed, it is `var.wifi_networks`, set only in the git-ignored
  `infra/terraform.tfvars` and shared by prod, preprod, qa and the test API; `jsonencode` saves
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
{
  wifiNetworks(knownVersion: "3f2a9c0d1b7e4a65") {
    version
    networks {
      title
      ssid
      password
      security
      hidden
    }
  }
}
```

## Audit log

Every business write (people, periods, kiosk sessions, users, locations, categories, API tokens, passkeys, OAuth grants, …) is recorded in the `{DB_PREFIX}_audit_log` DynamoDB table: who did it, which location(s) it touched, and which fields changed from what to what. The table is defined in `infra/dynamodb*.tf` and documented in `SCHEMA.md`; it must exist (`terraform apply`) for entries to land. It is read through GraphQL (below); there is no UI for it yet.

It is implemented by `audit::AuditingHandler`, a `db::Handler` wrapper that the servers, sync jobs and CLI wrap their DynamoDB handler in (`poem`, `poem-local`, the API Lambda, `member_sync::run`, `location_sync::run`, `cli`, `load-nitc-tags`, `load-category-display`). The actor comes from the `AUDIT_CONTEXT` task-local each entry point sets: `user` (with `actor_via` `oauth_grant:<id>` / `user_token:<id>` / `oauth_client:<id>` when not a plain web login), `member` (a member signed in to Tally, by person ID, `actor_via` `oauth_grant:<id>` or `oauth_client:<id>`), `session` (kiosk), `api_token`, `period_link`, `system` (`actor_id` is the job: `member-sync`, `location-sync`, `cli`, `load-nitc-tags`, `load-category-display`, `oauth-refresh-reuse`), `unauthenticated`, or `unknown` when a write was made with no context set (a bug; it logs a warning).

Auditing is **best-effort**: if recording an entry fails it is logged at `error!` (with the entity type and id) and the write still succeeds. Dry runs and read-only servers record nothing, because the write they would describe is refused. Updates that change nothing record nothing.

### Reading the log (GraphQL)

Both views are Relay connections, **newest first**, default page 50 (max 200), and take `first`/`after` (towards older entries), `last`/`before` (back towards newer ones) and an optional `entityTypes` filter (any of the listed kinds; omitted or empty means every kind — the single `entityType` argument it replaces is deprecated but still accepted, and adds to the list). A cursor is the item's `sk` (`{ts:010}#{id}`); a malformed one is a `BAD_REQUEST` error. `first` with `before`, or `last` with `after`, is rejected.

| Field               | Who may read it                                                                                                                                                  | Shows                                                                                                                                                                                                                                                                       |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Location.auditLog` | A _user_ with Admin **or** Read only access to the location, or a super user. Kiosk sessions, API tokens and edit links are refused even for their own location. | Entries at that location. A write touching several locations is in each of their logs: a member moved between units, and every change to a person, which is filed at every unit they belong to (primary and secondary). Global records (users, categories, …) are not here. |
| `Query.auditLog`    | Super users only                                                                                                                                                 | Every event once, including global records.                                                                                                                                                                                                                                 |

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

The most operationally useful field is `env`/`origin`: the `preprod` and `qa` front-ends
talk to the **production** database, so a kiosk running one of those builds is signing
real members in and out — and nothing else in the admin UI would reveal it.
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

## Kiosk authentication

Kiosks authenticate only with a signed key. An admin enrols a kiosk by scanning its QR
code (`submitEnrollmentKey` / `enrollSession`). From then on, the kiosk sends
`Authorization: SLKey <fingerprint>.<timestamp>.<signature>` with every request, signing
the request body with its non-extractable P-256 key (`api/src/session_key.rs`,
`auth::verify_signed_key`).

Kiosks once set up with a 6-digit setup code authenticated with a session JWT. They
converted themselves to key auth through a temporary `convertSessionToKey` mutation, and
the leftovers were purged. JWTs are no longer issued or accepted, and any bearer token
without a recognised prefix is refused. `JWT_SECRET` keeps its name but now only derives
the OAuth client-id signing key (`api/src/server_secret.rs`).

## SES member sync

Set `ses_api_headquarters_id` per location via the admin UI before syncing.

Sync reconciles against `db::list_people_for_location`, the people whose **primary** unit
is the location. Never swap in `list_members_for_location`, which also returns members
of other units who volunteer here (the admin Members page, dashboard and badges use
it): they are absent from this unit's SES payload, so the absence pass would mark and
then soft-delete them.

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
