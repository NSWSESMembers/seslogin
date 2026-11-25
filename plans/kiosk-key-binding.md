# Kiosk key binding: one row per key

Status: **proposed**. Round 1 (phase 1) written as a stack on top of #590 (2026-11-24).

This plan replaces the way a kiosk's key is tied to its session. Today that link is
kept up by application code across several rows. Each new feature that touches it
(enrol, reactivate, and now replace in #590) adds another way for it to break. The
plan moves the link into a table keyed by the key itself, so DynamoDB enforces it.
It then removes the code paths that kept it up by hand.

## Rule for new code until this lands

So that nothing new is built on the old design:

- **Don't add callers of `get_session_id_by_key_fingerprint`** (the
  `key_fingerprint-index` GSI), or of the `kiosk_enroll_<fp>` `ephemeral_state`
  record. Sign-in and enrolment are the only users, and both move.
- **Every write that changes which session holds a key goes through
  `db::Handler::bind_session_key`** (#590), or through the unbind paths named in
  [Writers](#writers). Don't add a new `SessionUpdateShape` that sets or clears
  `key_fingerprint`.
- **Only `db::KioskKeyStore` writes `kiosk_key`** (round 1). Nothing outside the
  DynamoDB implementation writes that table directly.

## Why

### What goes wrong today

A kiosk proves who it is by signing each request with its private key. The server
finds the kiosk's session from the key's fingerprint. Three pieces of state decide
that link:

1. `session.key_fingerprint` and `session.public_key` on the session row.
2. `key_fingerprint-index`, a GSI over (1). It is the only way to go from a key to
   a session. It is **not unique** and **eventually consistent**.
3. `ephemeral_state` row `kiosk_enroll_<fp>`: "this device is on its QR screen and
   wants setting up". The kiosk re-creates it every 10 minutes and on every reload.
   It expires after 30 minutes.

The rule "one key, one session" lives only in application code. Six writers keep
it up, each in its own way: `enrollSession`, `reactivateSession`,
`replaceSessionDevice`, `ReleaseKey`, `Delete` and `Retire`. The races found
while reviewing #590:

| # | Race | Effect | Fixed by #590? |
| - | ---- | ------ | -------------- |
| 1 | Two admins enrol or replace with the same device at once | Two sessions hold one key. Sign-in rejects it as ambiguous, and the device is dead until someone deletes one | Mostly: the transaction uses up the enrolment record |
| 2 | Enrolment record re-created (kiosk re-publish or reload) within ~1s of a bind, while the GSI is behind | A second bind misses the new holder, so two sessions hold one key | **No**. It needs a strongly consistent key → session lookup |
| 3 | A second admin with the setup page still open binds a device someone just set up | It quietly moves the device off the kiosk just created | **No**. The page has no version for the device |
| 4 | Release, then rekey, as separate writes | Partial failure leaves kiosks keyless | Yes |
| 5 | Replace target changed after the admin chose it | Wrong kiosk cut off | Yes (`expectedVersion`) |
| 6 | Reactivate racing a move of the same kiosk | Old device told "reactivated"; new key extended | Yes (condition on fingerprint) |

#590 closes what can be closed without a schema change. Races 2 and 3 remain, and
every new key-related feature has to re-derive which reads and conditions keep the
rule true.

### Root causes

1. **The key → session link has no unique home.** It is an attribute of session
   rows, found through a GSI. DynamoDB can't enforce uniqueness on a GSI, and a GSI
   can't be read consistently.
2. **"Device is asking" is a row that is deleted and re-created.** Using up a
   request means deleting it. Anything that re-creates it (the kiosk's timer, a
   reload, an error-path restore) re-opens the request.
3. **Three mutations do the same job.** Enrol, reactivate and replace all mean
   "make this device that kiosk". Each has its own reads, conditions and messages.

## Design

### The `kiosk_key` table

`{prefix}kiosk_key`, hash key `fingerprint` (S), on-demand, no GSIs. One row per
device key that has ever asked to be set up or is bound to a session.

| Attribute | Type | Meaning |
| --------- | ---- | ------- |
| `fingerprint` | S | PK. Hex SHA-256 of the SPKI DER, as today |
| `public_key` | S | Base64 SPKI DER. Moves here from `session` |
| `session_id` | S | The session this key is bound to. **Present if and only if bound** |
| `bound_at` | N | When it was bound |
| `requested_at` | N | Last time the device published the key from its QR screen |
| `request_expires_at` | N | `requested_at` + 30 min. The device is "asking" while this is in the future |
| `ttl` | N | Native TTL. Set only on **unbound** rows (= `request_expires_at`), so abandoned requests disappear. Removed when bound |
| `v` | N | Version, bumped on every change of binding (not on a request refresh) |

The session row keeps `key_fingerprint`, now a pointer, unindexed once phase 4
drops the GSI. It also keeps `key_expires_at` (the sliding 14-day window, still
written only by the heartbeat) and `key_released_at`. `session.public_key` stays
until phase 4, for rollback.

**Invariant:** `kiosk_key(f).session_id = s` if and only if
`session(s).key_fingerprint = f` and `session(s)` is active. Every write that
changes either side writes both, in one `TransactWriteItems`, with each item
conditioned on the version it was read at.

DynamoDB now enforces both halves of "one key, one session": the key is the
primary key, and `session.key_fingerprint` holds a single value. No writer queries
a GSI to find holders, because the key row names its holder directly. That
removes race 2 entirely.

### Sign-in

1. `GetItem kiosk_key(f)`, strongly consistent. Not bound: 401.
2. `GetItem session(session_id)`, strongly consistent. Require
   `key_fingerprint = f`, `is_live()`, and an unexpired key. The pointer
   cross-check costs nothing and means a stale key row can never authenticate.
3. Verify the signature with `kiosk_key.public_key`.

That is two point reads, the same count as today (GSI query plus `BatchGetItem`),
but both are consistent. The "multiple active sessions share this key fingerprint"
branch goes away.

### Writers

All of these are one transaction. "cond" is the item's condition expression.

**Request** (`submitEnrollmentKey`, unauthenticated, with today's write-suppression):

- Unbound or missing row: `SET public_key, requested_at, request_expires_at, ttl`.
  cond `attribute_not_exists(fingerprint) OR attribute_not_exists(session_id)`.
- Bound row: `SET requested_at, request_expires_at` only.
  cond `attribute_exists(session_id)`.
- Try one, then the other on a condition failure. A request **never changes the
  binding**, so re-publishing can't re-open anything (root cause 2).

**Bind** (`bindKioskDevice`, which replaces enrol, reactivate and replace).
Server-side reads: `K = kiosk_key(f)` and the target `T`. The device must be asking
(`K.request_expires_at > now`), and `K.v` must equal the `expectedKeyVersion` the
admin's page loaded with (fixes race 3). The transaction:

1. `kiosk_key(f)`: cond `v = :kv AND request_expires_at > :now`. `SET session_id = T,
   bound_at`, `REMOVE ttl, requested_at, request_expires_at`, `ADD v`. Removing the
   request uses it up, and binding bumps `v`.
2. If `K.session_id = P` and `P ≠ T`, session `P`: cond `key_fingerprint = :f AND
   v = :pv`. Release it, as `ReleaseKey` does today.
3. Target. New: `Put`, cond `attribute_not_exists(id)`. Existing: cond
   `attribute_exists(active) AND v = :tv`. `SET key_fingerprint = f, key_expires_at`,
   `REMOVE key_released_at`, `ADD v`.
4. If `T.key_fingerprint = f0` and `f0 ≠ f`: `Delete kiosk_key(f0)`, cond
   `session_id = :t`. If the old device is still running, its next request gets a
   401, and it publishes again as an unbound request.

Reactivation is a bind to the existing `T` that already holds `f`: items 1 and 3
only. Item 3 sets the short reactivation window (`reactivated_key_expiry`) instead
of the full 14 days. A conflict in any item fails the whole write with nothing
changed. Which item failed is reported as today's messages: enrolment gone, device
moved, kiosk changed. A `TransactionConflict` is retried once, then reported as
"try again".

**Unbind** (`deleteSession`, `Retire`): the session update, plus
`Delete kiosk_key(f)` with cond `session_id = :s`.

**Heartbeat** (`Info`): session row only, unchanged. The 14-day window stays on
the session, so steady-state kiosk traffic never writes `kiosk_key`.

### API

- `bindKioskDevice(keyFingerprint, expectedKeyVersion, target)` → `Session`.
  `target` is a `@oneOf` input: `{ new: { locationId, name, config, healthcheckUrl } }`
  or `{ existing: { id, expectedVersion } }`. Same guards as today: `User`, Admin at
  the target's location, `require_not_oauth_grant`, not on MCP.
- `pendingEnrollmentKey(fingerprint)` adds `version` (fed back as
  `expectedKeyVersion`), and `currentKiosk { id name location { id name } }`. That
  field is filled only when the device is bound to a kiosk at a location the viewer
  can read, so the scan is still not a lookup tool for other units' kiosks.
- `enrollSession`, `reactivateSession` and `replaceSessionDevice` become thin
  wrappers over the same code, then are removed once the web app stops calling
  them. Nothing else uses them: MCP doesn't expose them, and kiosk and admin web
  deploy together.

### UI

The QR scan page becomes **Set up this computer**:

- The heading says what the computer is now: "This computer is the kiosk **Front
  desk** at **Test A Unit**", or "This computer isn't a kiosk yet".
- Two choices: **New kiosk**, or **An existing kiosk**. If the computer is already
  a kiosk, "An existing kiosk" is preselected with that kiosk chosen: that is
  reactivation, so the Kiosks page no longer needs its own Reactivate button. The
  kiosk list shows each kiosk's status: *online on another computer*, *offline
  since …*, *no computer*.
- Confirming against a kiosk that is online on another computer says that computer
  will be disconnected.
- A refusal (stale versions) reloads the page's data and says what changed.

The admin and kiosk guides and their screenshots change in the same PR as the UI.

## Rollout

Every PR is safe to deploy alone. The constraints that shape the order:

- **prod, preprod and qa share one database** and run different versions for
  days. Until every one of them writes `kiosk_key`, nothing may *read* it as
  authoritative.
- **Workers deploy from their own branch.** The checker Lambda retires sessions,
  so it must mirror unbinds before reads switch.
- **Terraform is applied by hand**, so a table has to exist before the code that
  writes it in a transaction deploys anywhere.
- **test** has its own tables and goes through the same steps.

### Phase 1: shadow table (round 1)

Nothing reads `kiosk_key` yet. Writes to it never block a real write: they are
unconditional inside existing transactions, or best-effort after a single-item
write.

| PR | What | Deploy gate |
| -- | ---- | ----------- |
| 1.1 | This plan | none |
| 1.2 | `kiosk_key` tables (prod + test) in Terraform, with backup, `local-tables.rs` and `SCHEMA.md`. No code uses them | `terraform apply` before 1.4 deploys anywhere |
| 1.3 | `db::KioskKeyStore` domain trait (row type and `record_kiosk_key_request`) with DynamoDB, audit pass-through, mock and fake impls. `submitEnrollmentKey` and `cli session reactivatable` record requests (best-effort) | 1.2 applied, or the best-effort write just logs |
| 1.4 | Mirror binds and unbinds. `bind_session_key` binds the key row and deletes the target's old one in the same transaction (unconditional). `Delete`/`Retire`/`ReleaseKey` read the old fingerprint back and best-effort delete its row if it still points at that session. Adds `get_kiosk_key` and an opt-in DynamoDB Local test of the transactions | **1.2 applied** (the bind transaction fails if the table is missing) |
| 1.5 | `cli kiosk-key reconcile` (`kiosk_key_reconcile` module): compares every session holding a key with `kiosk_key`, fixes missing and stale rows with conditional transactions, and refuses to touch a key two live sessions hold (reports it, exits non-zero). Adds `list_kiosk_keys` and the two repair writes. Global `--dry-run` previews | none |

**Gate to phase 2:** 1.3 and 1.4 deployed to prod, preprod, qa, test and workers.
Then `reconcile` on prod and test, re-run until it reports no drift over a few days.
Drift after the first clean run means a writer was missed.

### Phase 2: read `kiosk_key`

| PR | What |
| -- | ---- |
| 2.1 | Sign-in reads `kiosk_key` first, as [above](#sign-in), and falls back to the GSI on a missing row with a warning and a metric. Always cross-checks the session pointer |
| 2.2 | `pendingEnrollmentKey`, `enrollmentRealtimeToken` and `Session.reactivatable` read `request_expires_at`, falling back to the `ephemeral_state` record |
| 2.3 | `reconcile` runs daily in the checker Lambda, report-only, and alerts on drift |

**Gate:** no fallbacks and no drift for a week.

### Phase 3: `kiosk_key` decides

| PR | What |
| -- | ---- |
| 3.1 | `bind_session_key` conditions on the key row (`v`, request) instead of using up the `ephemeral_state` record and querying the GSI for holders. Holders come from `kiosk_key.session_id` |
| 3.2 | Unbind (`Delete`, `Retire`) becomes one transaction with the key row, conditioned on `session_id` |
| 3.3 | `bindKioskDevice` and `pendingEnrollmentKey.version`/`currentKiosk`. The three old mutations become wrappers |
| 3.4 | Web: **Set up this computer**, Reactivate button removed, guides and screenshots |

While prod lags preprod, prod still runs the phase 2 code. It keeps the key row up
to date (phase 1 mirroring), so the two versions agree. Its own binds have exactly
today's race profile, no worse.

### Phase 4: remove the old path

| PR | What |
| -- | ---- |
| 4.1 | Remove the sign-in fallback, `get_session_id_by_key_fingerprint`, and the `kiosk_enroll_*` `ephemeral_state` records |
| 4.2 | Remove `enrollSession`, `reactivateSession` and `replaceSessionDevice` |
| 4.3 | Stop writing `session.public_key`, and remove it in a backfill |
| 4.4 | Terraform: drop `key_fingerprint-index` (an online change). `SCHEMA.md` |

4.4 is the only step that is slow to undo (re-creating a GSI backfills it). It
goes last, after a soak.

### Rollback

- **Phases 1–2:** redeploy the previous version. The table is inert.
- **Phase 3:** the previous version still works, because every transaction writes
  the session side too.
- **Phase 4:** before 4.4, redeploy. After 4.4, re-add the GSI and wait for it to
  backfill.

## Testing

- **`FakeDb`** models `kiosk_key` and applies each transaction's conditions under
  one lock, so the GraphQL tests cover every conflict path.
- **DynamoDB Local** (added in 1.4): an `#[ignore]`d test drives the real
  `dynamodb::Handler` transactions against `make local-up`. `FakeDb` can't check
  condition expression syntax, reserved words or cancellation-reason ordering, and
  each of those fails silently in a fake. Run it with
  `cargo test --test kiosk_key_dynamodb_local -- --ignored`.
- **Phase 2:** the fallback metric is the test that phase 1 mirrored everything.

## Open questions

- **Device label.** Should the audit log record which device a kiosk moved to?
  The fingerprint isn't secret. A short prefix (`ab12cd34`) would let the log say
  "moved to another computer (ab12cd34)" instead of only `key_replaced_at`. Today
  `session_create_redacts_the_key` keeps it out.
- **Unbound row lifetime.** A released key's row is deleted, not kept unbound.
  Keeping it, as a history of a device's sessions, would need a GSI on
  `session_id`. Nothing needs that yet.
