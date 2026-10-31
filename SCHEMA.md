# Database Schema Reference

seslogin uses DynamoDB as its database backend. All tables are defined in `infra/dynamodb.tf`. Tables come in pairs — a production set with prefix `var.db_prefix` and a test set with prefix `var.db_prefix_test`. All tables use `PAY_PER_REQUEST` billing (on-demand capacity); there is no provisioned throughput to tune.

DynamoDB only enforces uniqueness on the primary key. All other uniqueness requirements (e.g. one registration_number per person) are enforced at the application layer.

In DynamoDB, only attributes that are part of a key or GSI key must be declared in the table definition. All other fields are schema-free per item.

All IDs are exposed to the API layer as opaque UUID strings. Conversion happens inside `dynamodb.rs`, never in callers.

---

## Schema

### `{prefix}user`

| Attribute | Type | Role                 |
| --------- | ---- | -------------------- |
| `id`      | S    | Hash key (PK) — UUID |
| `email`   | S    | GSI hash key         |

**GSIs:**

| GSI           | Hash key | Sort key | Projection | Purpose                                                                 |
| ------------- | -------- | -------- | ---------- | ----------------------------------------------------------------------- |
| `email-index` | `email`  | —        | KEYS_ONLY  | Email-code login: resolve email → user ID without reading the full item |

`username-index` is intentionally absent — username login is not supported in v2.

`KEYS_ONLY` projection is deliberately minimal: the login path only needs the user ID; fetching the full user record happens in a separate `GetItem` call, so paying to replicate all attributes into the index is wasteful.

**Non-obvious attributes (not in table definition):**

- `is_super` (Bool) — superuser flag
- `location_grants` (SS) — string set of location UUIDs where this user is an **Admin** (read and write)
- `location_read_only_grants` (SS) — string set of location UUIDs where this user is **Read only** (can view, cannot change; disjoint from `location_grants`). Omitted when empty, like `location_grants`
- `person_id` (S) — the member record (`person.id`) this user acts as when signing in or out from the mobile app. Set only by a super user (`setUserMember`); several users may share one. Omitted when unset
- `deleted` (Bool)
- `access_time` (N) — Unix timestamp

---

### `{prefix}category`

| Attribute       | Type | Role                 |
| --------------- | ---- | -------------------- |
| `id`            | S    | Hash key (PK) — UUID |
| `nitc_group_id` | S    | GSI hash key         |

**GSIs:**

| GSI                   | Hash key        | Sort key | Projection | Purpose                                                                                |
| --------------------- | --------------- | -------- | ---------- | -------------------------------------------------------------------------------------- |
| `nitc_group_id-index` | `nitc_group_id` | —        | ALL        | `get_nitc_group`: find all categories sharing a topic group without scanning the table |

**Non-obvious attributes:**

- `parent_name` (S) — the parent category's display name, denormalized into each item during data import. No join is needed to build the full display name; the item already contains `"{parent_name} - {name}"` or equivalent.
- `group_name` (S, optional) — the kiosk group the category sits under (`"Training"`). Display metadata duplicated from the kiosk's hard-coded tree (`web/src/lib/categories.ts`) for the mobile app; backfilled by `load-category-display` from `api/assets/category-display.json`. Omitted, never `Null`, when unset.
- `short_name` (S, optional) — the short name the kiosk shows (`"AIIMS"`). Same provenance as `group_name`.
- `icon` (S, optional) — the kiosk icon's file stem in `web/src/assets/categories/` (`"aiims"`). Only the name is stored; no image is served. Same provenance as `group_name`.
- `nitc_enabled` (Bool)
- `nitc_type` (S) — `'Training'`, `'Other'`, or `'Community Engagement'`
- `nitc_participant_type` (S) — `'Attendee'`, `'Trainer'`, or `'Assessor'`
- `nitc_tag_ids` (SS) — native DynamoDB string set of tag ID strings (e.g. `{"316", "317", "318"}`)
- `enabled` (Bool)

---

### `{prefix}location`

| Attribute | Type | Role                 |
| --------- | ---- | -------------------- |
| `id`      | S    | Hash key (PK) — UUID |

No GSIs. Locations are always fetched by ID or via a full-table Scan (the table has ~279 items; a Scan is acceptable at this scale and avoids the cost of maintaining a GSI).

**Non-obvious attributes:**

- `ses_api_headquarters_id` (S) — the HQ system ID; absent means not linked
- `last_successful_member_sync` (N) — Unix timestamp
- `nitc_enabled` (N) — Unix timestamp NITC export starts from; `0`/absent means off (legacy rows may still hold a Bool, which reads as off)
- `nitc_complete_on_export` (Bool) — only written as `false`, for locations that want their exported NITCs left incomplete in SES; absent (the default) means they are marked completed
- `enabled` (Bool)

Written only by location sync (`api/src/location_sync.rs`), each omitted when SES has no value:

- `code` (S), `latitude` / `longitude` (N) — from the SES API's headquarters record. 29 units have no coordinates
- `ses_directory_entity_id` (S) — the SES intranet directory `Entity` this location resolved to, by `code` and then by name (codes are shared across entity kinds, e.g. `CFS`, `COR`, `SEZ`). Absent for State Headquarters, two units missing from the directory, and a few pseudo-units
- `address` / `postal_address` (M) — the entity's "Main address" / "Postal Address": `line1`, `line2`, `suburb`, `state`, `postcode`, `formatted`, each present only when set
- `email` (S) — the entity's "Internet Mail"; `phone` (S) — its "Phone", else its "Mobile"
- `accreditations` (S) — JSON text of the unit's `GET /headquarters/{id}/accreditations` rows, `[{"type", "status", "number"?, "note"?}]` sorted by type. Every returned row is kept, "Not Applicable" included, so "not applicable" and "no record" stay distinct. Units only

---

### `{prefix}person_unit`

One row per (member, SES intranet directory entity) the member is assigned to: their units, plus clusters, Community Action Teams, zone capability units and staff sections. Written only by directory sync, which replaces a member's rows to exactly what their directory record lists. Rows exist only for live members of our `person` table; a member's rows are removed once they are soft-deleted.

| Attribute     | Type | Role                                                     |
| ------------- | ---- | -------------------------------------------------------- |
| `person_id`   | S    | Hash key                                                 |
| `entity_id`   | S    | Range key — the directory `EntityId`                     |
| `location_id` | S    | Sparse GSI hash key — present only when the entity is one of our locations (`location.ses_directory_entity_id`) |

| GSI                 | Hash key      | Sort key | Projection | Purpose                                         |
| ------------------- | ------------- | -------- | ---------- | ----------------------------------------------- |
| `location_id-index` | `location_id` | —        | ALL        | A location's members, primary or not. Not read yet |

**Other attributes** (each omitted when absent or empty):

- `entity_name`, `entity_code`, `zone` (S) — from the assignment and the entity
- `membership_type` (S) — the Volunteer assignment's detail (Member, Probationary, Life, …); absent on a staff-only row
- `ranks`, `roles`, `positions` (SS)
- `is_primary` (Bool) — only written as `true`, when `location_id` is the member's `person.location_id`
- `updated_at` (N)

Every Rank and Role sits at an entity where the member also has a Volunteer assignment, and nobody has two Volunteer assignments at one entity, so one row per (member, entity) loses nothing.

---

### `{prefix}period`

| Attribute       | Type | Role                                                                                              |
| --------------- | ---- | ------------------------------------------------------------------------------------------------- |
| `id`            | S    | Hash key (PK) — UUID                                                                              |
| `location_id`   | S    | Item data only (not a GSI key)                                                                    |
| `person_id`     | S    | GSI hash key                                                                                      |
| `start_time`    | N    | GSI sort key                                                                                      |
| `nitc_event_id` | S    | GSI hash key                                                                                      |
| `location_open` | S    | Sparse GSI hash key — present only on open (no `end_time`), non-deleted periods; absent otherwise |
| `location_live` | S    | Sparse GSI hash key — present only on non-deleted periods; absent on deleted periods              |

**GSIs:**

| GSI                              | Hash key        | Sort key     | Projection | Purpose                                                                                                  |
| -------------------------------- | --------------- | ------------ | ---------- | -------------------------------------------------------------------------------------------------------- |
| `location_open-start_time-index` | `location_open` | `start_time` | ALL        | List open non-deleted periods for a location (`onlyActive=true`). Sparse — only open periods are indexed |
| `location_live-start_time-index` | `location_live` | `start_time` | ALL        | List all non-deleted periods for a location (`onlyActive=false`). Sparse — deleted periods are excluded  |
| `person_id-start_time-index`     | `person_id`     | `start_time` | ALL        | List periods for a person, ordered by time                                                               |
| `nitc_event_id-index`            | `nitc_event_id` | —            | ALL        | List all periods assigned to a given NITC event                                                          |

The sparse location indexes (`location_open`, `location_live`) equal `location_id` when present and are REMOVED (not nulled) when a period closes or is deleted. DynamoDB only indexes items where the GSI hash key attribute exists, so the index contains exactly the periods of interest — no filter expression required. Both attributes are maintained by every write path (`start_period_for_person_location`, `create_period`, `end_period`, and all `update_period` variants).

The `start_time` sort key across the location/person GSIs means DynamoDB returns results in ascending time order natively. Descending order requires `ScanIndexForward: false`. Time range filtering (`start_time BETWEEN :low AND :high`) uses `KeyConditionExpression` on the sort key — pushing the filter into the key condition is significantly more efficient than a `FilterExpression`, which would apply after reading all pages.

`ALL` projection is used on every period GSI because period rows are frequently read in full (the calling resolver needs all fields). A `KEYS_ONLY` or custom projection would require a second `BatchGetItem` per result, trading read capacity for storage savings — not worthwhile given the access pattern.

**Non-obvious attributes:**

- `version` (N) — optimistic concurrency counter for NITC export
- `nitc_event_id` (S) — absent until Phase 1 assigns the period
- `nitc_participant_id` (N) — absent until Phase 2 exports the period
- `nitc_exported_version` (N) — absent means never exported
- `signed_in_session_id` (S) — kiosk session that signed the period in; absent for admin-created periods
- `signed_out_session_id` (S) — kiosk session that signed the period out; absent until a kiosk sign-out (admin edits leave it untouched)
- `deleted` (Bool)

---

### `{prefix}person`

| Attribute             | Type | Role                 |
| --------------------- | ---- | -------------------- |
| `id`                  | S    | Hash key (PK) — UUID |
| `location_id`         | S    | GSI hash key         |
| `registration_number` | S    | GSI hash key         |
| `ses_api_person_id`   | S    | GSI hash key         |
| `contact_feed`        | S    | GSI hash key         |
| `contact_key`         | S    | GSI sort key         |
| `email_key`           | S    | GSI hash key         |

**GSIs:**

| GSI                         | Hash key              | Sort key | Projection | Purpose                                                            |
| --------------------------- | --------------------- | -------- | ---------- | ------------------------------------------------------------------ |
| `location_id-index`         | `location_id`         | —        | ALL        | List all members at a location for admin views and member sync     |
| `registration_number-index` | `registration_number` | —        | KEYS_ONLY  | Batch lookup during kiosk scan-in: registration number → person ID |
| `ses_api_person_id-index`   | `ses_api_person_id`   | —        | KEYS_ONLY  | Member sync reconciliation: HQ person ID → local person ID         |
| `contact_feed-contact_key-index` | `contact_feed` | `contact_key` | ALL | Contacts feed: every person, ordered by when a field the contacts API returns last changed |
| `email_key-index`           | `email_key`           | —        | KEYS_ONLY  | Tally sign-in: email → the member(s) it belongs to (sparse)        |

`KEYS_ONLY` on the scan-in and sync indexes avoids storing a full copy of each person item per index, since these lookups only need the primary key to drive a subsequent `BatchGetItem` or direct update.

`ses_api_person_id-index` is defined in Terraform. DynamoDB supports adding a GSI to an existing table online via `UpdateTable`; it backfills from existing items automatically while the table stays live, so `terraform apply` will add it in-place with no table recreation required — queries against the GSI will fail only during the brief `CREATING` backfill window.

**Non-obvious attributes:**

- `registration_number` (S) — the SES member registration number (zero-padded)
- `ses_api_person_id` (S) — absent until member sync links the record
- `email` (S) — agency address first, else "Volunteers Primary E-Mail". Written only by directory sync, which never clears it
- `mobile` (S), `phones` (S), `preferred_name` (S) — written only by directory sync (`api/src/directory_sync.rs`). `mobile` is the first `+614…` number among Primary Telephone, Personal Mobile Phone, SMS and Work Mobile Phone. `phones` is JSON text, `[{"number", "types": [..]}]`, one entry per distinct number (faxes excluded), the mobile first. A directory record with no phones keeps the stored `mobile`/`phones`. `preferred_name` is set only when the directory has one
- `capability_tags` (S) — JSON text of the member's SES API `capabilityTags` (qualifications such as Job Ready, flood rescue levels, driver), `[{"id", "name", "description"}]` sorted by id. Written only by member sync; absent when SES lists none
- `deleted` (Bool)
- `missing_since` (N) — unix seconds at which member sync first saw this person absent from
  their location's SES payload. Absent on a healthy record; cleared by any sync that sees
  them again, and removed on soft-delete so a marker only ever exists on a live row. Once
  it ages past the grace window (default 7 days) the person is soft-deleted. Note the
  absence pass reads the roster through `location_id-index`, which is `ALL`-projected — a
  `KEYS_ONLY` projection there would hydrate every row with `missing_since: None` and
  silently prevent all deletions.
- `contact_feed` (S) — always `"all"`: a constant hash key, so every person sits in one
  ordered partition of `contact_feed-contact_key-index`. The write rate is member sync plus
  admin edits, far below a partition's limits.
- `contact_key` (S) — `<Unix microseconds, zero-padded to 20 digits>#<id>`, restamped by
  every write that changes something the contacts API returns: `create_person`, and
  `update_person`'s `Fields`, `Location`, `Directory`, `CapabilityTags`, `Undelete` and
  `Delete` shapes (not `SesApiPersonId`, `MissingSince` or `BadgeState`). Renaming a location
  restamps every live member there, since their unit name changed, and
  `replace_person_units` restamps the member when any of their `person_unit` rows changed. Sorting by it orders people by last change; the id suffix
  keeps it unique when two writes share a microsecond. Soft-deleted rows keep their key, so
  they double as the feed's tombstones. Rows written before the index existed have neither
  attribute until `cli person stamp-contacts` backfills them.
- `email_key` (S) — `email` trimmed and lowercased (`db::email_key`), present exactly when
  `email` is. Written and removed with `email` by `update_person`'s `Email` shape. Several
  people may share one, and soft-deleted rows keep theirs, so a lookup returns every match
  and the caller filters. Rows whose email was set before the index existed have no key
  until `cli person stamp-email-keys` backfills them. It is only used by Tally sign-in.

---

### `{prefix}session`

Kiosk sessions. Every kiosk enrols by QR code and holds a key: `public_key`, `key_fingerprint` and `key_expires_at`. It authenticates each request by signature (`SLKey`, see `api/src/session_key.rs`). Kiosks once set up with the removed 6-digit setup code ran on a session JWT. They converted themselves to key auth before JWTs were refused, and `cli session purge-legacy` (since removed) soft-deleted the rest.

| Attribute         | Type | Role                                                                  |
| ----------------- | ---- | --------------------------------------------------------------------- |
| `id`              | S    | Hash key (PK)                                                         |
| `active`          | N    | GSI hash key — present (`1`) on live sessions, absent on deleted ones |
| `location_id`     | S    | GSI sort key                                                          |
| `key_fingerprint` | S    | GSI hash key — present only while a session holds a key               |

**GSIs:**

| GSI                        | Hash key          | Sort key      | Projection | Purpose                                                          |
| -------------------------- | ----------------- | ------------- | ---------- | ---------------------------------------------------------------- |
| `active-location_id-index` | `active`          | `location_id` | ALL        | Admin UI and CLI: list a location's sessions                     |
| `key_fingerprint-index`    | `key_fingerprint` | —             | KEYS_ONLY  | Signed kiosk requests and enrollment: find the session for a key |

Both are sparse: a row is in an index only while it has that index's hash key attribute.

`active` is set to `1` on creation and **removed** on soft-delete, so a deleted session disappears from `active-location_id-index` with no filter expression. Membership in that index is _not_ the same as being usable, though. Rows that can no longer authenticate keep `active` (see the lifecycle table below), so every caller that means "a working kiosk" must also check `Session::is_live` and `key_expires_at`.

`key_fingerprint-index` is `KEYS_ONLY`: the caller only needs the session ID, and fetches the full row with `GetItem`.

There is no `code-index`, `active-super-index` or `legacy_id-index`. Setup codes and super-sessions are gone, and v1 IDs are no longer looked up. `legacy_id` is still _stored_ on rows migrated from v1, but nothing indexes or reads it. A `code` attribute may linger on old rows, but nothing reads it.

**Non-obvious attributes:**

- `active` (N) — `1` on live rows; absent on deleted ones
- `public_key` (S) — base64 SPKI DER of the kiosk's P-256 key; dropped when the key is released or the session deleted
- `key_fingerprint` (S) — hex SHA-256 of `public_key`; dropped with it
- `key_expires_at` (N) — Unix seconds after which signed requests stop authenticating. Slid forward to now + 14 days by the throttled `last_contact` refresh, and set to a short window by `reactivateSession`
- `key_released_at` (N) — set when the session's key was handed to a new session enrolled from the same device (`ReleaseKey`). Such a row can never come back online
- `retired_at` (N) — set when `SessionUpdateShape::Retire` hid a session that had been unusable for more than 14 days (see `api/src/session_retire.rs`). The row is otherwise like an admin delete — no `active` or key fields — and this is what tells the two apart
- `last_contact` (N) — Unix timestamp of the last authenticated request, refreshed at most once per `LAST_CONTACT_REFRESH_SECS`
- `client_version` (S), `client_info_updated_at` (N) and the `client_*` attributes — the kiosk's self-reported diagnostics
- `config` (S) — JSON-serialised map of UI config key/value pairs
- `healthcheck_url` (S) — absent if not configured
- `created_at` (N) — absent on rows migrated from v1
- `updated_at` (N) — touched by every write except the `last_contact` heartbeat. **Cannot be used to date when a session died:** 115 active rows were bulk-rewritten on 2026-09-08
- `v` (N) — version, bumped by writes that change what a kiosk shows or whether it can authenticate; absent (read as 0) on rows that predate it
- `legacy_id` (S) — v1 session ID; unindexed and unused

**Lifecycle and index membership:**

| State        | How it gets there                                               | `active-location_id-index` | `key_fingerprint-index` | Can authenticate                            | Unusable since                            |
| ------------ | --------------------------------------------------------------- | -------------------------- | ----------------------- | ------------------------------------------- | ----------------------------------------- |
| Live key     | Enrolled, and the kiosk checked in within 14 days               | yes                        | yes                     | yes                                         | —                                         |
| Expired key  | No contact for 14 days                                          | yes                        | yes                     | no, but an admin can `reactivateSession` it | `key_expires_at`                          |
| Released key | Its device re-enrolled as another session (`ReleaseKey`)        | yes                        | —                       | no, never again                             | `key_released_at`                         |
| Deleted      | An admin deleted it (`Delete` removes `active` and keys)        | —                          | —                       | no                                          | `updated_at`                              |
| Retired      | Unusable for more than 14 days (`Retire`, see `session_retire`) | —                          | —                       | no                                          | (`retired_at` records when it was hidden) |

**Known gaps (from a prod scan on 2026-10-15, before the setup-code kiosks were converted or purged):**

- **Nothing retired unusable rows.** Released and expired key sessions, and lapsed setup-code kiosks, kept `active` forever, so they stayed in `active-location_id-index` (and expired keys stayed in `key_fingerprint-index`). `SessionUpdateShape::Retire` (`api/src/session_retire.rs`) now hides them 14 days after they become unusable, run daily by the checker Lambda (and by hand with `cli session retire-stale`). On 2026-10-15, 108 of the 150 rows in `active-location_id-index` could never authenticate again, and 33 had been dead for more than a year. Most of those were lapsed setup-code kiosks, which the legacy purge has since deleted.
- **Disabling a location does not touch its sessions.** 2 rows sat at a disabled location.

---

### `{prefix}nitc_event`

| Attribute     | Type | Role                 |
| ------------- | ---- | -------------------- |
| `id`          | S    | Hash key (PK) — UUID |
| `location_id` | S    | GSI hash key         |
| `topic_date`  | S    | GSI sort key         |

**GSIs:**

| GSI                            | Hash key      | Sort key     | Projection | Purpose                                                              |
| ------------------------------ | ------------- | ------------ | ---------- | -------------------------------------------------------------------- |
| `location_id-topic_date-index` | `location_id` | `topic_date` | ALL        | Find or create the event for a given location, topic group, and date |

`topic_date` is a composite sort key: `"{nitc_group_id}#{event_date}"` — for example `"42#2026-05-01"`. This design allows:

- **Exact match** (`topic_date = "42#2026-05-01"`) — used by `get_nitc_event_for_day`
- **Prefix query** (`begins_with(topic_date, "42#")`) — potential future use to list all events for a topic group at a location

Encoding both dimensions into a single string sort key avoids the need for a composite key attribute or a second GSI. The `#` separator is safe because neither `nitc_group_id` nor dates contain `#`.

`ALL` projection is used because Phase 2 always needs the full event record (version, ses_api_nitc_id, etc.) immediately after the lookup.

**Non-obvious attributes:**

- `nitc_group_id` (S) — the NITC topic group ID; same value as the prefix of `topic_date`
- `ses_api_nitc_id` (N) — absent until Phase 2 creates the event in the SES API
- `version` (N)
- `synced_version` (N) — absent means never synced

---

### `{prefix}nitc_group`

| Attribute      | Type | Role                                                                                                      |
| -------------- | ---- | --------------------------------------------------------------------------------------------------------- |
| `id`           | S    | Hash key (PK)                                                                                             |
| `nitc_type`    | S    | The NITC role this group represents (e.g. attendee, trainer, assessor)                                    |
| `nitc_tag_ids` | SS   | String set of integer tag ID strings — the SES API tags applied when exporting a participant of this type |

No GSIs. The only access pattern is `GetItem` by `id` (called from `get_nitc_group`). This is a reference/lookup table; the application never writes to it. Data is imported once at table creation time.

---

### `{prefix}task`

A location's shared to-do list ("Tasks"). Dates are **Sydney-local calendar days** stored as `YYYY-MM-DD`, which sort lexically in date order.

| Attribute          | Type | Role                                                                                            |
| ------------------ | ---- | ----------------------------------------------------------------------------------------------- |
| `id`               | S    | Hash key (PK)                                                                                   |
| `open_location_id` | S    | GSI hash key (`open_location_id-show_date-index`) — a copy of `location_id` while the task is open, otherwise absent |
| `show_date`        | S    | GSI sort key — the day the task starts showing                                                  |

**GSIs:**

| GSI                                | Hash key           | Sort key    | Projection | Purpose                                                                   |
| ---------------------------------- | ------------------ | ----------- | ---------- | ------------------------------------------------------------------------- |
| `open_location_id-show_date-index` | `open_location_id` | `show_date` | ALL        | A location's open tasks: due now (`show_date <= today`) or scheduled (`> today`) |

The index is **sparse**: completing a non-recurring task, or deleting any task, REMOVEs `open_location_id`, so the row leaves it. Completed and deleted tasks stay in the base table, reachable by `id` only. A recurring task is never completed for good — completing it moves `due_date` and `show_date` forward and it stays open.

**Non-obvious attributes (not in table definition):**

- `location_id` (S) — the task's location, kept after it stops being open
- `list_id` (S) — optional `task_list` the task is on; it may name a deleted list
- `title` (S), `description` (S, optional)
- `due_date` (S, optional) — the day the task was scheduled for. `show_date` is this, or the creation day when there is none
- `recurrence_every` (N), `recurrence_unit` (S: `days` or `weeks`) — both present or both absent
- `v` (N) — version, bumped by every write; edits and completions are conditional on it, so two people completing the same task cannot both succeed
- `completed_at` (N), `completed_by` (S) — a non-recurring task's completion, and the person ID of the member who completed it
- `last_completed_at` (N), `last_completed_by` (S) — a recurring task's latest completion, and who by (person ID)
- `created_by` / `updated_by` (S) — person IDs of the members who made and last changed it
- `deleted` (N) — Unix seconds when soft-deleted; absent otherwise
- `created_at` / `updated_at` (N)

---

### `{prefix}task_list`

Optional named lists a location's tasks can be grouped on.

| Attribute     | Type | Role                                  |
| ------------- | ---- | ------------------------------------- |
| `id`          | S    | Hash key (PK)                         |
| `location_id` | S    | GSI hash key (`location_id-index`)    |

**GSIs:**

| GSI                 | Hash key      | Sort key | Projection | Purpose                  |
| ------------------- | ------------- | -------- | ---------- | ------------------------ |
| `location_id-index` | `location_id` | —        | ALL        | A location's lists; deleted ones are filtered out on read |

**Non-obvious attributes (not in table definition):** `name` (S), `created_by` (S), `deleted` (N, soft delete — the list's tasks are left alone), `created_at` / `updated_at` (N).

---

### `{prefix}note`

A location's shared notebook ("Notes").

| Attribute     | Type | Role                                  |
| ------------- | ---- | ------------------------------------- |
| `id`          | S    | Hash key (PK)                         |
| `location_id` | S    | GSI hash key (`location_id-index`)    |

**GSIs:**

| GSI                 | Hash key      | Sort key | Projection | Purpose                  |
| ------------------- | ------------- | -------- | ---------- | ------------------------ |
| `location_id-index` | `location_id` | —        | ALL        | A location's notes; deleted ones are filtered out on read, and the API sorts and searches them in memory |

**Non-obvious attributes (not in table definition):**

- `folder_id` (S) — optional `note_folder` the note is in; it may name a deleted folder
- `title` (S) — one line of plain text
- `content` (S, optional) — plain text; absent when empty
- `v` (N) — version, bumped by every write; saves are conditional on it, so two people editing the same note cannot silently overwrite each other
- `created_by` / `updated_by` (S) — person IDs of the members who made and last changed it
- `deleted` (N) — Unix seconds when soft-deleted; absent otherwise
- `created_at` / `updated_at` (N)

---

### `{prefix}note_folder`

Optional named folders a location's notes can be filed in.

| Attribute     | Type | Role                                  |
| ------------- | ---- | ------------------------------------- |
| `id`          | S    | Hash key (PK)                         |
| `location_id` | S    | GSI hash key (`location_id-index`)    |

**GSIs:**

| GSI                 | Hash key      | Sort key | Projection | Purpose                  |
| ------------------- | ------------- | -------- | ---------- | ------------------------ |
| `location_id-index` | `location_id` | —        | ALL        | A location's folders; deleted ones are filtered out on read |

**Non-obvious attributes (not in table definition):** `name` (S), `created_by` (S), `deleted` (N, soft delete — the folder's notes are left alone), `created_at` / `updated_at` (N).

---

### `{prefix}map_pin`

The pins on a location's shared map ("Map").

| Attribute     | Type | Role                                  |
| ------------- | ---- | ------------------------------------- |
| `id`          | S    | Hash key (PK)                         |
| `location_id` | S    | GSI hash key (`location_id-index`)    |

**GSIs:**

| GSI                 | Hash key      | Sort key | Projection | Purpose                  |
| ------------------- | ------------- | -------- | ---------- | ------------------------ |
| `location_id-index` | `location_id` | —        | ALL        | A location's pins; deleted ones are filtered out on read, and the API filters and sorts them in memory |

**Non-obvious attributes (not in table definition):**

- `layer_id` (S) — optional `map_layer` the pin is on; it may name a deleted layer
- `pin_type` (S, optional) — what the pin marks, shown as an icon: `access`, `entry`, `flood` or `hazard`; absent for a plain pin. More are expected, so readers tolerate a value they don't know
- `title` (S) — one line of plain text
- `latitude` / `longitude` (N) — WGS 84 degrees
- `address` (S, optional) — free text, not geocoded; absent when empty
- `note` (S, optional) — plain text; absent when empty
- `v` (N) — version, bumped by every write; saves are conditional on it, so two people editing the same pin cannot silently overwrite each other
- `created_by` / `updated_by` (S) — person IDs of the members who made and last changed it
- `deleted` (N) — Unix seconds when soft-deleted; absent otherwise
- `created_at` / `updated_at` (N)

---

### `{prefix}map_layer`

Optional named layers a location's map pins can be put on.

| Attribute     | Type | Role                                  |
| ------------- | ---- | ------------------------------------- |
| `id`          | S    | Hash key (PK)                         |
| `location_id` | S    | GSI hash key (`location_id-index`)    |

**GSIs:**

| GSI                 | Hash key      | Sort key | Projection | Purpose                  |
| ------------------- | ------------- | -------- | ---------- | ------------------------ |
| `location_id-index` | `location_id` | —        | ALL        | A location's layers; deleted ones are filtered out on read |

**Non-obvious attributes (not in table definition):** `name` (S), `created_by` (S), `deleted` (N, soft delete — the layer's pins are left alone), `created_at` / `updated_at` (N).

---

### `{prefix}on_call_role`

A location's on-call roles ("On Call"), such as Duty Officer, and who holds each one now.

| Attribute     | Type | Role                                  |
| ------------- | ---- | ------------------------------------- |
| `id`          | S    | Hash key (PK)                         |
| `location_id` | S    | GSI hash key (`location_id-index`)    |

**GSIs:**

| GSI                 | Hash key      | Sort key | Projection | Purpose                  |
| ------------------- | ------------- | -------- | ---------- | ------------------------ |
| `location_id-index` | `location_id` | —        | ALL        | A location's roles; deleted ones are filtered out on read, and the API sorts them by name |

**Non-obvious attributes (not in table definition):**

- `name` (S) — one line of plain text
- `description` (S, optional) — plain text; absent when empty
- `phone` (S, optional) — a number that reaches whoever holds the role, kept as written
- `primary_person_id` (S, optional) — the `person` on call; absent while the role is empty
- `secondary_person_ids` (SS, optional) — everyone else in the role; absent when there is nobody else, and never present without a primary. People may be from any location, and may since have been deleted (the API leaves them out)
- `members_changed_at` (N, optional) — Unix seconds when the people in the role last changed; absent while it is empty
- `v` (N) — version, bumped by every write; saves are conditional on it
- `created_by` / `updated_by` (S) — person IDs of the members who made and last changed it
- `deleted` (N) — Unix seconds when soft-deleted; absent otherwise
- `created_at` / `updated_at` (N)

---

### `{prefix}equipment`

A location's equipment register ("Equipment").

| Attribute     | Type | Role                                  |
| ------------- | ---- | ------------------------------------- |
| `id`          | S    | Hash key (PK)                         |
| `location_id` | S    | GSI hash key (`location_id-index`)    |

**GSIs:**

| GSI                 | Hash key      | Sort key | Projection | Purpose                  |
| ------------------- | ------------- | -------- | ---------- | ------------------------ |
| `location_id-index` | `location_id` | —        | ALL        | A location's equipment; deleted items are filtered out on read, and the API sorts and searches them in memory |

**Non-obvious attributes (not in table definition):**

- `equipment_type` (S) — `vehicle`, `trailer`, `vessel`, `radio`, `device`, `bag`, `box` or `tool`; more will be added
- `name` (S) — one line of plain text
- `description` (S, optional) — plain text; absent when empty
- `status` (S) — `in_service`, `has_issue` (usable, but something needs fixing), `out_of_service`, `missing` or `borrowed`. Changed only together with a new `equipment_log` row, in one transaction conditional on the status it changes from
- `status_changed_at` (N) — when the status last changed, or the equipment was added
- `last_checked_at` (N, optional) — when someone last logged a check
- `v` (N) — version, bumped by every edit (not by log entries); edits are conditional on it
- `created_by` / `updated_by` (S) — person IDs of the members who made and last changed it
- `deleted` (N) — Unix seconds when soft-deleted; absent otherwise. The log is kept
- `created_at` / `updated_at` (N)

---

### `{prefix}equipment_log`

Each piece of equipment's log of status changes and checks. Append-only.

| Attribute      | Type | Role                                  |
| -------------- | ---- | ------------------------------------- |
| `id`           | S    | Hash key (PK)                         |
| `equipment_id` | S    | GSI hash key (`equipment_id-index`)   |

**GSIs:**

| GSI                  | Hash key       | Sort key | Projection | Purpose                  |
| -------------------- | -------------- | -------- | ---------- | ------------------------ |
| `equipment_id-index` | `equipment_id` | —        | ALL        | A piece of equipment's log; the API sorts it newest first |

**Non-obvious attributes (not in table definition):**

- `location_id` (S) — the equipment's location
- `kind` (S) — `status_change` or `check`
- `from_status` (S, optional) — the status before a change; absent for checks and for the row written when the equipment was added
- `status` (S) — the status after a change, or the status it was checked in
- `comment` (S, optional) — why, as the member wrote it; required for a status change after the first, absent when empty
- `created_by` (S) — person ID of the member who made the entry
- `created_at` (N) — Unix seconds; `created_us` (N) — Unix microseconds, which orders rows written in the same second

---

### `{prefix}equipment_reservation`

Bookings of a location's equipment for a time range ("Reservations"). Overlaps are allowed; the API reports them.

| Attribute     | Type | Role                                                |
| ------------- | ---- | --------------------------------------------------- |
| `id`          | S    | Hash key (PK)                                       |
| `location_id` | S    | GSI hash key (`location_id-end_time-index`)         |
| `end_time`    | N    | GSI sort key (`location_id-end_time-index`)         |

**GSIs:**

| GSI                          | Hash key      | Sort key   | Projection | Purpose                  |
| ---------------------------- | ------------- | ---------- | ---------- | ------------------------ |
| `location_id-end_time-index` | `location_id` | `end_time` | ALL        | A location's reservations ending after a given time, so past ones are never read; deleted ones are filtered out on read, and the API sorts them and finds overlaps in memory |

**Non-obvious attributes (not in table definition):**

- `equipment_ids` (SS) — the equipment reserved, at least one; may name equipment deleted since
- `start_time` (N) — Unix seconds; `end_time` is after it, by at most 366 days
- `reserved_for_person_ids` (SS, optional) — person IDs of the members it is for; absent when `reserved_for_text` is set
- `reserved_for_person_id` (S, optional) — legacy: the one member a reservation made before `reserved_for_person_ids` is for, read when that set is absent and removed by the next edit
- `reserved_for_text` (S, optional) — one line naming who or what it is for instead of a member
- `description` (S, optional) — plain text; absent when empty
- `v` (N) — version, bumped by every write; edits are conditional on it
- `created_by` / `updated_by` (S) — person IDs of the members who made and last changed it
- `deleted` (N) — Unix seconds when soft-deleted (cancelled); absent otherwise
- `created_at` / `updated_at` (N)

---

### `{prefix}calendar`

Optional named, coloured calendars a location's events can be put in ("Calendar").

| Attribute     | Type | Role                                  |
| ------------- | ---- | ------------------------------------- |
| `id`          | S    | Hash key (PK)                         |
| `location_id` | S    | GSI hash key (`location_id-index`)    |

**GSIs:**

| GSI                 | Hash key      | Sort key | Projection | Purpose                  |
| ------------------- | ------------- | -------- | ---------- | ------------------------ |
| `location_id-index` | `location_id` | —        | ALL        | A location's calendars; deleted ones are filtered out on read |

**Non-obvious attributes (not in table definition):** `name` (S), `color` (S — `red`, `orange`, `yellow`, `green`, `teal`, `blue`, `indigo`, `purple`, `pink` or `grey`), `created_by` (S), `deleted` (N, soft delete — the calendar's events are left alone), `created_at` / `updated_at` (N).

---

### `{prefix}calendar_event`

The events in a location's shared calendar ("Calendar"). An event is timed (instants) or all day (dates), and may repeat daily or weekly.

| Attribute     | Type | Role                                                |
| ------------- | ---- | --------------------------------------------------- |
| `id`          | S    | Hash key (PK)                                       |
| `location_id` | S    | GSI hash key (`location_id-last_end-index`)         |
| `last_end`    | N    | GSI sort key (`location_id-last_end-index`)         |

**GSIs:**

| GSI                          | Hash key      | Sort key   | Projection | Purpose                  |
| ---------------------------- | ------------- | ---------- | ---------- | ------------------------ |
| `location_id-last_end-index` | `location_id` | `last_end` | ALL        | A location's events still running after a given time, so finished ones are never read; deleted ones are filtered out on read, and the API expands repeats and sorts in memory |

**Non-obvious attributes (not in table definition):**

- `last_end` (N) — Unix seconds, an upper bound on when the last occurrence ends (`calendar::last_end`): the end for a one-off event, the end of the `repeat_until` day plus the event's length for a repeating one, and 253402300799 (year 9999) for one repeating forever. Rewritten by every edit
- `start_time` / `end_time` (N) — a timed event's first occurrence, Unix seconds; absent when all day
- `start_date` / `end_date` (S, `YYYY-MM-DD`) — an all-day event's first and last date, inclusive, with no time zone; absent when timed
- `repeat` (S, optional) — `daily` or `weekly`; absent for a one-off event
- `repeat_until` (S, optional, `YYYY-MM-DD`) — the last date an occurrence may start on (Sydney time for a timed event); absent when it repeats forever
- `calendar_id` (S, optional) — the `calendar` it is in; it may name a deleted calendar
- `title` (S) — one line of plain text
- `description` (S, optional) — plain text; absent when empty
- `v` (N) — version, bumped by every write; edits are conditional on it
- `created_by` / `updated_by` (S) — person IDs of the members who made and last changed it
- `deleted` (N) — Unix seconds when soft-deleted; absent otherwise
- `created_at` / `updated_at` (N)

---

### `{prefix}location_share`

Members sharing their phone's position with a location's members ("Locate"), for a fixed time. One row per member: starting another share replaces it, and stopping deletes it. Only the latest position is kept; there is no history.

| Attribute     | Type | Role                                  |
| ------------- | ---- | ------------------------------------- |
| `id`          | S    | Hash key (PK) — the sharing member's person ID |
| `location_id` | S    | GSI hash key (`location_id-index`)    |

**GSIs:**

| GSI                 | Hash key      | Sort key | Projection | Purpose                  |
| ------------------- | ------------- | -------- | ---------- | ------------------------ |
| `location_id-index` | `location_id` | —        | ALL        | Who is sharing with a location; lapsed rows the TTL hasn't removed yet are filtered out on read |

**TTL:** `expires_at`.

**Non-obvious attributes (not in table definition):**

- `latitude` / `longitude` (N) — WGS 84 degrees, the latest position
- `accuracy`, `altitude`, `altitude_accuracy`, `heading`, `speed` (N, optional) — metres, degrees from north and metres per second, as the phone reported them; absent when it didn't
- `recorded_at` (N) — Unix seconds when the phone took the latest fix, never later than `updated_at`
- `updated_at` (N) — Unix seconds when the latest position arrived
- `interval_secs` (N) — how often the phone said it would report in, 60 to 300
- `started_at` / `ends_at` (N) — Unix seconds; positions after `ends_at` are refused
- `expires_at` (N) — `updated_at` + 7 minutes: the share stops showing then, ended or not

---

### `{prefix}notification`

Each member's in-app notification inbox (Tally), fanned out on write: an event notifying several members is stored as one full copy per recipient, so the inbox is a single query with no join. Titles and bodies are small and written once, so a copy costs the same single write unit as a reference would. Rows are removed by the TTL 90 days after they were created, read or not.

| Attribute        | Type | Role                                                                    |
| ---------------- | ---- | ----------------------------------------------------------------------- |
| `person_id`        | S    | Hash key (PK) — the recipient member's person ID                      |
| `sk`             | S    | Range key — `<created_at in ms, zero-padded to 13 digits>#<notification_id>`, so newest first is a descending query; also the API's notification ID and paging cursor |
| `unread_person_id` | S    | GSI hash key (`unread-index`) — equal to `person_id`, present **only while unread** |

**GSIs:**

| GSI            | Hash key         | Sort key | Projection | Purpose                  |
| -------------- | ---------------- | -------- | ---------- | ------------------------ |
| `unread-index` | `unread_person_id` | `sk`     | KEYS_ONLY  | Sparse: counts and lists a member's unread notifications; the rows are then fetched by key. Marking read removes the attribute, so the row leaves the index |

**TTL:** `expires_at`.

**Non-obvious attributes (not in table definition):**

- `notification_id` (S) — shared by every recipient's copy of one event
- `source` (S) — the producing app, e.g. `reservations`
- `kind` (S) — what happened, e.g. `reservation.for_you`
- `location_id` (S, optional) — the location it is about; the API hides the notification from a reader who no longer has access to it
- `actor_person_id` (S, optional) — the member whose action caused it, when a member did
- `title` (S); `body` (S, optional)
- `target_type` / `target_id` (S, optional) — what tapping it should open
- `created_at` (N) — Unix seconds
- `read_at` (N, optional) — Unix seconds; absent while unread
- `expires_at` (N) — `created_at` + 90 days

---

### `{prefix}audit_log`

An append-only record of business database writes, shown in the admin UI. Entries are kept forever (no TTL). One item is written **per affected location** for each audited event, so a write touching two locations produces two items sharing an `event_id`.

| Attribute     | Type | Role                                                                                      |
| ------------- | ---- | ----------------------------------------------------------------------------------------- |
| `id`          | S    | Hash key (PK) — 12-char nanoid, unique per item                                                    |
| `location_id` | S    | GSI hash key (`location_id-sk-index`) — absent for global entities                        |
| `scope`       | S    | GSI hash key (`scope-sk-index`) — `"all"` on exactly one item per event, otherwise absent |
| `sk`          | S    | GSI sort key — `"{ts:010}#{id}"`                                                          |

**GSIs:**

| GSI                    | Hash key      | Sort key | Projection | Purpose                                                                    |
| ---------------------- | ------------- | -------- | ---------- | -------------------------------------------------------------------------- |
| `location_id-sk-index` | `location_id` | `sk`     | ALL        | Per-location view, newest first (`ScanIndexForward = false`)               |
| `scope-sk-index`       | `scope`       | `sk`     | ALL        | All-locations view for super users, newest first, with no duplicate events |

`sk` is `ts` (Unix seconds) zero-padded to 10 digits, then `#`, then the item `id`, so lexical order equals time order and two events in the same second never collide.

Both indexes are **sparse** by design. Items for global entities (no location) carry no `location_id` and so are absent from the per-location index. `scope = "all"` is set on only one of the items written for an event, so the super-user view lists each event once even when the event was written to several locations.

Access patterns: per-location newest-first (`Query` on `location_id-sk-index`) and all-locations newest-first for super users (`Query` on `scope-sk-index` with `scope = "all"`), both paginated on `sk`. The base table is only read by `id`.

**Non-obvious attributes (not in table definition):**

- `ts` (N) — Unix seconds when the event happened
- `event_id` (S) — shared by every item written for the same event
- `action` (S) — `create`, `update`, `delete` or `restore`
- `entity_type` (S) — `user`, `person`, `period`, `session`, `api_token`, `location`, `category`, `nitc_group`, `nitc_tag`, `user_token`, `oauth_grant`, `webauthn_credential`, `task`, `task_list`, `note`, `note_folder`, `map_pin`, `map_layer`, `on_call_role`, `equipment`, `equipment_log_entry`, `equipment_reservation`, `calendar`, `calendar_event` or `location_share`
- `entity_id` (S) — ID of the entity changed
- `entity_label` (S) — human-readable name of the entity at the time; omitted when unknown
- `actor_kind` (S) — `user`, `member` (a member signed in to Tally), `session` (kiosk), `api_token`, `period_link`, `system` (a job or the CLI), `unauthenticated` or `unknown`
- `actor_id` (S) — the user/session/token ID, the person ID for `member`, the period ID for a period link, or the job name for `system`; omitted for `unauthenticated`/`unknown`
- `actor_via` (S) — `oauth_grant:<id>` (a connected app, over MCP or GraphQL), `user_token:<id>` or `oauth_client:<id>`; omitted for an ordinary web login and for actors other than users and members
- `ip` (S) — caller's IP address; omitted when unknown
- `changes` (S) — JSON array of `{"field", "before", "after"}`; a side that is absent (a create has no `before`, a cleared field has no `after`) is omitted. Secrets are never stored: a secret field that changed appears as `"[redacted]"`

---

## Known issues and risks

This section covers correctness bugs, race conditions, performance hazards, and consistency gaps in the current DynamoDB implementation (`api/src/dynamodb.rs`).

---

### Race conditions

#### `get_or_create_nitc_event_for_day` — duplicate event creation

The function reads the GSI to check for an existing event, then issues a bare `PutItem` if none is found. Two concurrent Phase 1 workers processing periods for the same location/topic/date will both see "not found" in the GSI read, both generate a different UUID, and both `PutItem` will succeed — because DynamoDB's table PK uniqueness only checks `id`, not the `(location_id, topic_date)` combination. The result is two separate `nitc_event` items for the same logical event. `get_nitc_event_for_day` will then return `Err(Integrity("Multiple nitc_events..."))` on any subsequent lookup.

The fix is to make the event ID deterministic from `(location_id, nitc_group_id, event_date)` — e.g. a UUID v5 namespaced hash — and add `ConditionExpression("attribute_not_exists(id)")` to the `PutItem`. On condition failure, re-read the item by its deterministic ID instead of re-querying the GSI.

#### `end_period` — concurrent check-out

`end_period` issues an unconditional `UpdateItem SET end_time = :now`. The caller reads the period first (to confirm it is active) and then calls `end_period`. Two concurrent check-out requests for the same member can both pass the "still active" read check and both write `end_time`. The result is idempotent (both set `end_time` to approximately the same timestamp), but the first check-out's `end_time` is silently overwritten. Adding `ConditionExpression("attribute_not_exists(end_time)")` would make the second call fail explicitly, which is easier to reason about and can be returned to the caller as a meaningful "already checked out" response.

#### `assign_period` — period re-assignment and version bump are not atomic

Phase 1 calls `set_period_nitc_event` followed by `bump_nitc_event_version` as two separate operations. If the process is killed between them, the period is assigned to the event but no SQS message is enqueued. The period has `nitc_exported_version = period.version` written at the end of `assign_period`, so `list_unsynced_nitc_period_ids_for_location` will not surface it again. The event will only sync if something else bumps its version.

The same window exists in the old-event path: `bump_nitc_event_version(old_event_id)` → `set_period_nitc_event(period_id, desired_event.id)`. If killed after the first bump but before the reassignment, the old event gets a Phase 2 run that correctly excludes the period (it hasn't been reassigned yet), but no Phase 2 runs for the new event unless something triggers it.

This is inherent to the multi-step nature of Phase 1 and is not unique to DynamoDB, but there is no compensating idempotency check in Phase 2 for the "period assigned but event version not bumped" state.

---

### Correctness bugs

All items in this section have been fixed. They are retained for historical context.

#### ~~`list_sessions(SuperOnly)` — missing key condition~~ ✓ Fixed

Was: `filter_expression("attribute_exists(super)")` with no `key_condition_expression`. Now: `key_condition_expression("super = :one")` with sentinel `"1"`.

#### ~~`create_session_super` — wrong `super` attribute type~~ ✓ Fixed

Was: `.item("super", S("y"))` but hydration read `bool_field("super")`. Now: `.item("super", S("1"))` and `TryInto<Session>` uses `string_field("super").map(|v| v == "1")`. The string type is required because DynamoDB GSI keys cannot be `Bool`.

#### ~~`create_session` — attribute name mismatch for `last_contact`~~ ✓ Fixed

Was: attribute written as `last_access`, read as `last_contact`. Now both use `last_contact`. The returned `Session` struct from `create_session`/`create_session_super` now populates `last_contact: Some(unix_time)`.

#### ~~`get_user_id_by_email` — fallback to deleted `username-index`~~ ✓ Already fixed

The function was `get_user_id_by_email` and had no `username-index` fallback in the implementation. The bug was pre-emptively resolved; no code change was needed.

#### ~~`update_period(Delete)` — hard delete~~ ✓ Already fixed

`PeriodUpdateShape::Delete` uses `UpdateItem SET deleted = :timestamp` (soft delete), matching the MySQL behaviour. No code change was needed.

#### ~~`set_period_nitc_event` — does not clear `nitc_participant_id`~~ ✓ Fixed

Was: `SET nitc_event_id = :event_id`. Now: `SET nitc_event_id = :event_id REMOVE nitc_participant_id`. Prevents stale participant IDs from surviving a period reassignment.

#### ~~`clear_period_nitc_participant` — does not clear `nitc_event_id`~~ ✓ Fixed

Was: `REMOVE nitc_participant_id SET nitc_exported_version = :v`. Now: `REMOVE nitc_participant_id, nitc_event_id SET nitc_exported_version = :v`.

#### ~~`create_category` — writes `active`, hydration reads `enabled`~~ ✓ Fixed

`create_category` and `update_category` now write `enabled`/`:enabled` consistently. `TryInto<Category>` reads `bool_field("enabled")`.

#### ~~`bump_period_version` — missing initial `version` attribute~~ ✓ Fixed

`start_period_for_person_location` and `create_period` now write `.item("version", N("1"))` explicitly. `ADD version :one` on a pre-set attribute increments correctly from 1 to 2 on the first bump, eliminating the ambiguous "unset/1" state.

---

### Performance and scale

#### `list_unsynced_nitc_period_ids_for_location` — full table Scan

The NITC export loop calls this once per location, and it issues a full `Scan` on the `period` table, applying `location_id = :loc ...` as a `FilterExpression`. DynamoDB's filter expression executes _after_ reading every page of items — the consumed capacity and latency scale with the total number of periods in the table, not with the number of periods for that location. A table with 500k periods will read all 500k, discard most, and return the relevant handful.

The fix is to use the `location_live-start_time-index` GSI with `KeyConditionExpression("location_live = :loc AND start_time >= :cutoff")`, then apply the remaining filters (`nitc_exported_version`, `end_time`) as a `FilterExpression` on the GSI query. This reduces the read to only the location's recent non-deleted periods (deleted periods are absent from the sparse index).

#### `get_person_ids_by_registration_number` and `get_person_ids_by_ses_api_person_id` — serial single-item queries

Both methods loop over the input slice and issue one `Query` per item, sequentially. For a member sync diff of 200 members, this is 200 serial round-trips. The DynamoDB equivalent is to run the queries concurrently (e.g. `futures::future::join_all`) since each Query is independent.

#### `list_users` and `list_categories` — Scans with no key filter

Both issue `Scan` with only a `FilterExpression`. These tables are small (~10s of users, ~900 categories) so the immediate cost is low, but the pattern is incorrect for any table that may grow. `list_categories` is called on every GraphQL request that resolves a category field, which amplifies the cost. Both are marked with `// TODO: do not use scan in production code`.

#### `list_locations` — Scan with filter

Same pattern as above. Locations are ~45 items. The `filter_expression("enabled = :enabled")` filters disabled locations after reading everything. Given the table size this is not a practical concern, but it scans the whole table on every `list_locations` call, including the NITC export loop which calls it once at startup.

#### ~~`get_records` — `BatchGetItem` does not retry `UnprocessedKeys`~~ ✓ Fixed

Was: the response's `UnprocessedKeys` field was never read, so any item DynamoDB deferred — under throttling, or when a response would exceed 16MB — was silently discarded and the caller saw a row that exists as if it were missing. Now: each chunk is re-requested until nothing is left unprocessed, with 50ms backoff doubling to a 1s ceiling over at most 5 attempts (~750ms total). Keys still unprocessed after that raise `Error::Infrastructure` rather than reporting the rows as absent, because `None` would be indistinguishable from "does not exist".

The 100-item-per-request limit noted here was already handled: `get_records` chunks IDs with `.chunks(100)`.

#### `list_periods_for_location` — `limit` applies before `FilterExpression`

When `only_active = true` is passed, the filter `attribute_not_exists(end_time) OR end_time = :null` is a `FilterExpression`, not a key condition. DynamoDB reads up to `limit` items from the GSI and then discards the ended ones. If the `limit` is, say, 50, and the 50 most recent periods are all ended, the response returns 0 items — even if there are active periods further in the index. Pagination would eventually find them, but the first page would be misleading. To reliably return `limit` active periods, the query must either over-fetch and trim in application code, or avoid using `FilterExpression` for `only_active` and instead rely on a sparse index (e.g. only write `end_time` when the period ends, and treat `attribute_not_exists(end_time)` as the key condition for an "active" GSI). The current behaviour is acceptable for the kiosk active-periods view because there is usually at most one active period per member and the page is small, but it is technically incorrect as a paginated interface.

---

### GSI eventual consistency

All GSI-based lookups use DynamoDB's default eventually-consistent reads. Strong consistency is not available on GSIs. This affects:

- **`get_user_id_by_name`** (`email-index`): A newly created user attempting to log in immediately after account creation may not be found. Low risk in practice since account creation and first login are rarely within milliseconds of each other.

- **`get_person_ids_by_registration_number`** (`registration_number-index`): A person record created moments before a scan-in attempt could be missed. Member creation and immediate check-in is uncommon in normal operation.

- **NITC event duplicate detection** (`location_id-topic_date-index` in `get_nitc_event_for_day`): Already called out under race conditions, but eventual consistency makes the window wider — the GSI entry for a newly created nitc_event may not be visible to a second worker for up to ~1 second after creation.

There is no mitigation for GSI eventual consistency in the current implementation. The primary table reads (`GetItem`, `BatchGetItem`) use strongly consistent reads by default when accessing the base table, so entity fetches by UUID are reliable.
