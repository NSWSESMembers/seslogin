# Database Schema Reference

seslogin uses DynamoDB as its database backend. All tables are defined in `infra/dynamodb.tf`. Tables come in pairs — a production set with prefix `var.db_prefix` and a test set with prefix `var.db_prefix_test`. All tables use `PAY_PER_REQUEST` billing (on-demand capacity); there is no provisioned throughput to tune.

DynamoDB only enforces uniqueness on the primary key. All other uniqueness requirements (e.g. one registration_number per person) are enforced at the application layer.

In DynamoDB, only attributes that are part of a key or GSI key must be declared in the table definition. All other fields are schema-free per item.

All IDs are exposed to the API layer as opaque UUID strings. Conversion happens inside `dynamodb.rs`, never in callers.

---

## Schema

### `{prefix}user`

| Attribute | Type | Role |
|-----------|------|------|
| `id` | S | Hash key (PK) — UUID |
| `email` | S | GSI hash key |

**GSIs:**

| GSI | Hash key | Sort key | Projection | Purpose |
|-----|----------|----------|------------|---------|
| `email-index` | `email` | — | KEYS_ONLY | Email-code login: resolve email → user ID without reading the full item |

`username-index` is intentionally absent — username login is not supported in v2.

`KEYS_ONLY` projection is deliberately minimal: the login path only needs the user ID; fetching the full user record happens in a separate `GetItem` call, so paying to replicate all attributes into the index is wasteful.

**Non-obvious attributes (not in table definition):**
- `is_super` (Bool) — superuser flag
- `location_grants` (SS) — string set of location UUIDs this user can access
- `deleted` (Bool)
- `access_time` (N) — Unix timestamp

---

### `{prefix}category`

| Attribute | Type | Role |
|-----------|------|------|
| `id` | S | Hash key (PK) — UUID |
| `nitc_group_id` | S | GSI hash key |

**GSIs:**

| GSI | Hash key | Sort key | Projection | Purpose |
|-----|----------|----------|------------|---------|
| `nitc_group_id-index` | `nitc_group_id` | — | ALL | `get_nitc_group`: find all categories sharing a topic group without scanning the table |

**Non-obvious attributes:**
- `parent_name` (S) — the parent category's display name, denormalized into each item during data import. No join is needed to build the full display name; the item already contains `"{parent_name} - {name}"` or equivalent.
- `nitc_enabled` (Bool)
- `nitc_type` (S) — `'Training'`, `'Other'`, or `'Community Engagement'`
- `nitc_participant_type` (S) — `'Attendee'`, `'Trainer'`, or `'Assessor'`
- `nitc_tag_ids` (SS) — native DynamoDB string set of tag ID strings (e.g. `{"316", "317", "318"}`)
- `enabled` (Bool)

---

### `{prefix}location`

| Attribute | Type | Role |
|-----------|------|------|
| `id` | S | Hash key (PK) — UUID |

No GSIs. Locations are always fetched by ID or via a full-table Scan (the table has ~45 items; a Scan is acceptable at this scale and avoids the cost of maintaining a GSI).

**Non-obvious attributes:**
- `ses_api_headquarters_id` (S) — the HQ system ID; absent means not linked
- `last_successful_member_sync` (N) — Unix timestamp
- `nitc_enabled` (Bool)
- `enabled` (Bool)

---

### `{prefix}period`

| Attribute | Type | Role |
|-----------|------|------|
| `id` | S | Hash key (PK) — UUID |
| `location_id` | S | Item data only (not a GSI key) |
| `person_id` | S | GSI hash key |
| `start_time` | N | GSI sort key |
| `nitc_event_id` | S | GSI hash key |
| `location_open` | S | Sparse GSI hash key — present only on open (no `end_time`), non-deleted periods; absent otherwise |
| `location_live` | S | Sparse GSI hash key — present only on non-deleted periods; absent on deleted periods |

**GSIs:**

| GSI | Hash key | Sort key | Projection | Purpose |
|-----|----------|----------|------------|---------|
| `location_open-start_time-index` | `location_open` | `start_time` | ALL | List open non-deleted periods for a location (`onlyActive=true`). Sparse — only open periods are indexed |
| `location_live-start_time-index` | `location_live` | `start_time` | ALL | List all non-deleted periods for a location (`onlyActive=false`). Sparse — deleted periods are excluded |
| `person_id-start_time-index` | `person_id` | `start_time` | ALL | List periods for a person, ordered by time |
| `nitc_event_id-index` | `nitc_event_id` | — | ALL | List all periods assigned to a given NITC event |

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

| Attribute | Type | Role |
|-----------|------|------|
| `id` | S | Hash key (PK) — UUID |
| `location_id` | S | GSI hash key |
| `registration_number` | S | GSI hash key |
| `ses_api_person_id` | S | GSI hash key |

**GSIs:**

| GSI | Hash key | Sort key | Projection | Purpose |
|-----|----------|----------|------------|---------|
| `location_id-index` | `location_id` | — | ALL | List all members at a location for admin views and member sync |
| `registration_number-index` | `registration_number` | — | KEYS_ONLY | Batch lookup during kiosk scan-in: registration number → person ID |
| `ses_api_person_id-index` | `ses_api_person_id` | — | KEYS_ONLY | Member sync reconciliation: HQ person ID → local person ID |

`KEYS_ONLY` on the scan-in and sync indexes avoids storing a full copy of each person item per index, since these lookups only need the primary key to drive a subsequent `BatchGetItem` or direct update.

`ses_api_person_id-index` is defined in Terraform. DynamoDB supports adding a GSI to an existing table online via `UpdateTable`; it backfills from existing items automatically while the table stays live, so `terraform apply` will add it in-place with no table recreation required — queries against the GSI will fail only during the brief `CREATING` backfill window.

**Non-obvious attributes:**
- `registration_number` (S) — the SES member registration number (zero-padded)
- `ses_api_person_id` (S) — absent until member sync links the record
- `deleted` (Bool)
- `missing_since` (N) — unix seconds at which member sync first saw this person absent from
  their location's SES payload. Absent on a healthy record; cleared by any sync that sees
  them again, and removed on soft-delete so a marker only ever exists on a live row. Once
  it ages past the grace window (default 7 days) the person is soft-deleted. Note the
  absence pass reads the roster through `location_id-index`, which is `ALL`-projected — a
  `KEYS_ONLY` projection there would hydrate every row with `missing_since: None` and
  silently prevent all deletions.

---

### `{prefix}session`

| Attribute | Type | Role |
|-----------|------|------|
| `id` | S | Hash key (PK) — UUID |
| `code` | S | GSI hash key |
| `location_id` | S | GSI sort key |
| `super` | S | GSI sort key |
| `active` | N | GSI hash key — present (`1`) on live sessions, absent on deleted ones |
| `legacy_id` | S | GSI hash key |

**GSIs:**

| GSI | Hash key | Sort key | Projection | Purpose |
|-----|----------|----------|------------|---------|
| `code-index` | `code` | — | KEYS_ONLY | Kiosk login: look up session by 6-digit code |
| `active-location_id-index` | `active` | `location_id` | ALL | Admin UI: list all live sessions at a location |
| `active-super-index` | `active` | `super` | ALL | Admin UI: list all live super-sessions |
| `legacy_id-index` | `legacy_id` | — | KEYS_ONLY | Migration: look up session by v1 legacy ID |

The `active` attribute is set to `1` on creation and **removed** on soft-delete. Because DynamoDB GSIs only project items that have the GSI hash key attribute, deleted sessions automatically disappear from `active-location_id-index` and `active-super-index` without any filter expression.

The `super` attribute is a sentinel string `"1"` on super-sessions and is **absent** on regular sessions. It is the sort key of `active-super-index`; querying `active = 1 AND super = "1"` returns only live super-sessions.

`code-index` uses `KEYS_ONLY` because the login flow only needs the session ID; the code is then wiped and a full `GetItem` fetches the session. Deleted sessions may still appear in `code-index` and `legacy_id-index` (the `active` attribute is not their hash key), so lookups against those indexes check for the presence of `active` on the returned item before returning a result.

**Non-obvious attributes:**
- `active` (N) — absent on deleted sessions; `1` on live ones
- `code` (S) — absent after first use (wiped by `wipe_session_code`)
- `super` (S) — present only on super-sessions, value `"1"`
- `healthcheck_url` (S) — absent if not configured
- `last_contact` (N) — Unix timestamp
- `config` (M) — JSON-like map of UI config key/value pairs

---

### `{prefix}nitc_event`

| Attribute | Type | Role |
|-----------|------|------|
| `id` | S | Hash key (PK) — UUID |
| `location_id` | S | GSI hash key |
| `topic_date` | S | GSI sort key |

**GSIs:**

| GSI | Hash key | Sort key | Projection | Purpose |
|-----|----------|----------|------------|---------|
| `location_id-topic_date-index` | `location_id` | `topic_date` | ALL | Find or create the event for a given location, topic group, and date |

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

| Attribute | Type | Role |
|-----------|------|------|
| `id` | S | Hash key (PK) |
| `nitc_type` | S | The NITC role this group represents (e.g. attendee, trainer, assessor) |
| `nitc_tag_ids` | SS | String set of integer tag ID strings — the SES API tags applied when exporting a participant of this type |

No GSIs. The only access pattern is `GetItem` by `id` (called from `get_nitc_group`). This is a reference/lookup table; the application never writes to it. Data is imported once at table creation time.

---

## Known issues and risks

This section covers correctness bugs, race conditions, performance hazards, and consistency gaps in the current DynamoDB implementation (`api/src/dynamodb.rs`).

---

### Race conditions

#### `get_or_create_nitc_event_for_day` — duplicate event creation

The function reads the GSI to check for an existing event, then issues a bare `PutItem` if none is found. Two concurrent Phase 1 workers processing periods for the same location/topic/date will both see "not found" in the GSI read, both generate a different UUID, and both `PutItem` will succeed — because DynamoDB's table PK uniqueness only checks `id`, not the `(location_id, topic_date)` combination. The result is two separate `nitc_event` items for the same logical event. `get_nitc_event_for_day` will then return `Err(Integrity("Multiple nitc_events..."))` on any subsequent lookup.

The fix is to make the event ID deterministic from `(location_id, nitc_group_id, event_date)` — e.g. a UUID v5 namespaced hash — and add `ConditionExpression("attribute_not_exists(id)")` to the `PutItem`. On condition failure, re-read the item by its deterministic ID instead of re-querying the GSI.

#### `wipe_session_code` — double-use of a session code

`get_session_by_code` reads the session, the caller issues a JWT, then `wipe_session_code` removes the `code` attribute. A second concurrent request calling `get_session_by_code` with the same code between those two steps will also find the session. Both callers receive a valid JWT. The second JWT is for the session that should have been single-use.

This is a short window (two calls in the same request lifecycle), but a malicious actor who intercepts or guesses a code could exploit it. The fix is to use a conditional `UpdateItem` that removes the code and returns `ALL_OLD` in a single atomic operation, treating a missing-code response as "already used".

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

The NITC export loop calls this once per location, and it issues a full `Scan` on the `period` table, applying `location_id = :loc ...` as a `FilterExpression`. DynamoDB's filter expression executes *after* reading every page of items — the consumed capacity and latency scale with the total number of periods in the table, not with the number of periods for that location. A table with 500k periods will read all 500k, discard most, and return the relevant handful.

The fix is to use the `location_live-start_time-index` GSI with `KeyConditionExpression("location_live = :loc AND start_time >= :cutoff")`, then apply the remaining filters (`nitc_exported_version`, `end_time`) as a `FilterExpression` on the GSI query. This reduces the read to only the location's recent non-deleted periods (deleted periods are absent from the sparse index).

#### `get_person_ids_by_registration_number` and `get_person_ids_by_ses_api_person_id` — serial single-item queries

Both methods loop over the input slice and issue one `Query` per item, sequentially. For a member sync diff of 200 members, this is 200 serial round-trips. The DynamoDB equivalent is to run the queries concurrently (e.g. `futures::future::join_all`) since each Query is independent.

#### `list_users` and `list_categories` — Scans with no key filter

Both issue `Scan` with only a `FilterExpression`. These tables are small (~10s of users, ~900 categories) so the immediate cost is low, but the pattern is incorrect for any table that may grow. `list_categories` is called on every GraphQL request that resolves a category field, which amplifies the cost. Both are marked with `// TODO: do not use scan in production code`.

#### `list_locations` — Scan with filter

Same pattern as above. Locations are ~45 items. The `filter_expression("enabled = :enabled")` filters disabled locations after reading everything. Given the table size this is not a practical concern, but it scans the whole table on every `list_locations` call, including the NITC export loop which calls it once at startup.

#### `get_records` — `BatchGetItem` does not retry `UnprocessedKeys`

DynamoDB's `BatchGetItem` can return `UnprocessedKeys` when throughput is exceeded or when the request is throttled. The implementation does not check this field and silently discards any unprocessed items, causing `NotFound` errors for IDs that were not returned. For `PAY_PER_REQUEST` tables this is rare but not impossible under burst load.

Additionally, `BatchGetItem` has a hard limit of 100 items per request. Callers passing more than 100 IDs will receive a `ValidationException`. There is no chunking logic.

#### `list_periods_for_location` — `limit` applies before `FilterExpression`

When `only_active = true` is passed, the filter `attribute_not_exists(end_time) OR end_time = :null` is a `FilterExpression`, not a key condition. DynamoDB reads up to `limit` items from the GSI and then discards the ended ones. If the `limit` is, say, 50, and the 50 most recent periods are all ended, the response returns 0 items — even if there are active periods further in the index. Pagination would eventually find them, but the first page would be misleading. To reliably return `limit` active periods, the query must either over-fetch and trim in application code, or avoid using `FilterExpression` for `only_active` and instead rely on a sparse index (e.g. only write `end_time` when the period ends, and treat `attribute_not_exists(end_time)` as the key condition for an "active" GSI). The current behaviour is acceptable for the kiosk active-periods view because there is usually at most one active period per member and the page is small, but it is technically incorrect as a paginated interface.

---

### GSI eventual consistency

All GSI-based lookups use DynamoDB's default eventually-consistent reads. Strong consistency is not available on GSIs. This affects:

- **`get_session_by_code`** (`code-index`): A kiosk that creates a new session and immediately scans the code could hit a replica that has not yet indexed the new item. The result is "code not found" on first attempt. This is the most user-visible case — a member scans a QR code and gets an error, then retries a second later and succeeds.

- **`get_user_id_by_name`** (`email-index`): A newly created user attempting to log in immediately after account creation may not be found. Low risk in practice since account creation and first login are rarely within milliseconds of each other.

- **`get_person_ids_by_registration_number`** (`registration_number-index`): A person record created moments before a scan-in attempt could be missed. Member creation and immediate check-in is uncommon in normal operation.

- **NITC event duplicate detection** (`location_id-topic_date-index` in `get_nitc_event_for_day`): Already called out under race conditions, but eventual consistency makes the window wider — the GSI entry for a newly created nitc_event may not be visible to a second worker for up to ~1 second after creation.

There is no mitigation for GSI eventual consistency in the current implementation. The primary table reads (`GetItem`, `BatchGetItem`) use strongly consistent reads by default when accessing the base table, so entity fetches by UUID are reliable.

