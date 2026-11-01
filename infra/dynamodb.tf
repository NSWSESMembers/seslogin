# DynamoDB tables for seslogin, including tables and GSIs required for NITC support.
#
# Prod tables use var.db_prefix; test tables use var.db_prefix_test.
# All tables use PAY_PER_REQUEST billing (on-demand capacity).

# ── Prod tables ────────────────────────────────────────────────────────────────

resource "aws_dynamodb_table" "prod_user" {
  name                        = "${var.db_prefix}_user"
  billing_mode                = "PAY_PER_REQUEST"
  hash_key                    = "id"
  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "email"
    type = "S"
  }

  # username-index dropped — username login is no longer supported
  global_secondary_index {
    name = "email-index"
    key_schema {
      attribute_name = "email"
      key_type       = "HASH"
    }
    projection_type = "KEYS_ONLY"
  }
}

resource "aws_dynamodb_table" "prod_category" {
  name                        = "${var.db_prefix}_category"
  billing_mode                = "PAY_PER_REQUEST"
  hash_key                    = "id"
  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "nitc_group_id"
    type = "S"
  }

  # Required for get_nitc_group: find categories by NITC group ID
  global_secondary_index {
    name = "nitc_group_id-index"
    key_schema {
      attribute_name = "nitc_group_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

resource "aws_dynamodb_table" "prod_location" {
  name                        = "${var.db_prefix}_location"
  billing_mode                = "PAY_PER_REQUEST"
  hash_key                    = "id"
  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
}

resource "aws_dynamodb_table" "prod_period" {
  name                        = "${var.db_prefix}_period"
  billing_mode                = "PAY_PER_REQUEST"
  hash_key                    = "id"
  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "person_id"
    type = "S"
  }
  attribute {
    name = "start_time"
    type = "N"
  }
  attribute {
    name = "nitc_event_id"
    type = "S"
  }
  attribute {
    name = "location_open"
    type = "S"
  }
  attribute {
    name = "location_live"
    type = "S"
  }

  global_secondary_index {
    name = "person_id-start_time-index"
    key_schema {
      attribute_name = "person_id"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "start_time"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
  # Required for list_periods_for_nitc_event resolver
  global_secondary_index {
    name = "nitc_event_id-index"
    key_schema {
      attribute_name = "nitc_event_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
  # Sparse index: only open (no end_time), non-deleted periods. Used by onlyActive=true queries.
  global_secondary_index {
    name = "location_open-start_time-index"
    key_schema {
      attribute_name = "location_open"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "start_time"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
  # Sparse index: only non-deleted periods (open or closed). Used by onlyActive=false queries.
  global_secondary_index {
    name = "location_live-start_time-index"
    key_schema {
      attribute_name = "location_live"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "start_time"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
}

resource "aws_dynamodb_table" "prod_person" {
  name                        = "${var.db_prefix}_person"
  billing_mode                = "PAY_PER_REQUEST"
  hash_key                    = "id"
  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }
  attribute {
    name = "registration_number"
    type = "S"
  }
  attribute {
    name = "ses_api_person_id"
    type = "S"
  }
  attribute {
    name = "contact_feed"
    type = "S"
  }
  attribute {
    name = "contact_key"
    type = "S"
  }
  attribute {
    name = "email_key"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
  global_secondary_index {
    name = "registration_number-index"
    key_schema {
      attribute_name = "registration_number"
      key_type       = "HASH"
    }
    projection_type = "KEYS_ONLY"
  }
  # Used by get_person_ids_by_ses_api_person_id
  global_secondary_index {
    name = "ses_api_person_id-index"
    key_schema {
      attribute_name = "ses_api_person_id"
      key_type       = "HASH"
    }
    projection_type = "KEYS_ONLY"
  }
  # The contacts feed: every person in one partition, ordered by when a field the
  # contacts API returns last changed. See SCHEMA.md.
  global_secondary_index {
    name = "contact_feed-contact_key-index"
    key_schema {
      attribute_name = "contact_feed"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "contact_key"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
  # Tally sign-in: email -> member. Sparse: only people with an email. See SCHEMA.md.
  global_secondary_index {
    name = "email_key-index"
    key_schema {
      attribute_name = "email_key"
      key_type       = "HASH"
    }
    projection_type = "KEYS_ONLY"
  }
}

resource "aws_dynamodb_table" "prod_session" {
  name                        = "${var.db_prefix}_session"
  billing_mode                = "PAY_PER_REQUEST"
  hash_key                    = "id"
  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }
  attribute {
    name = "active"
    type = "N"
  }
  attribute {
    name = "key_fingerprint"
    type = "S"
  }

  global_secondary_index {
    name = "active-location_id-index"
    key_schema {
      attribute_name = "active"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "location_id"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
  global_secondary_index {
    name = "key_fingerprint-index"
    key_schema {
      attribute_name = "key_fingerprint"
      key_type       = "HASH"
    }
    projection_type = "KEYS_ONLY"
  }
}

resource "aws_dynamodb_table" "prod_api_token" {
  name                        = "${var.db_prefix}_api_token"
  billing_mode                = "PAY_PER_REQUEST"
  hash_key                    = "id"
  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "active"
    type = "N"
  }

  # Sparse index for listing live (non-revoked) tokens in the admin UI.
  # `active` is set to "1" on creation and REMOVEd on revoke, so the GSI stays sparse.
  global_secondary_index {
    name = "active-index"
    key_schema {
      attribute_name = "active"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

resource "aws_dynamodb_table" "prod_nitc_group" {
  name                        = "${var.db_prefix}_nitc_group"
  billing_mode                = "PAY_PER_REQUEST"
  hash_key                    = "id"
  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
}

resource "aws_dynamodb_table" "prod_nitc_tag" {
  name                        = "${var.db_prefix}_nitc_tag"
  billing_mode                = "PAY_PER_REQUEST"
  hash_key                    = "id"
  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
}

resource "aws_dynamodb_table" "prod_nitc_event" {
  name                        = "${var.db_prefix}_nitc_event"
  billing_mode                = "PAY_PER_REQUEST"
  hash_key                    = "id"
  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }
  # Composite sort key: "{nitc_topic_group_id}#{event_date}" e.g. "42#2026-05-01"
  # Enables exact-match and begins_with queries for a given location+topic_group
  attribute {
    name = "topic_date"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-topic_date-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "topic_date"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
}

resource "aws_dynamodb_table" "prod_login_code" {
  name         = "${var.db_prefix}_login_code"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "email"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "email"
    type = "S"
  }

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

resource "aws_dynamodb_table" "prod_user_token" {
  name         = "${var.db_prefix}_user_token"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }

  # `expires_at` slides forward on each use (`TouchLastUsed`), so only tokens
  # left unused past their lifetime are removed. Deletion is lazy (often up to
  # a couple of days late); auth rejects expired tokens itself regardless.
  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

resource "aws_dynamodb_table" "prod_webauthn_credential" {
  name         = "${var.db_prefix}_webauthn_credential"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "user_id"
    type = "S"
  }
  attribute {
    name = "person_id"
    type = "S"
  }

  # Used to list / count a user's passkeys in the admin UI.
  global_secondary_index {
    name = "user_id-index"
    key_schema {
      attribute_name = "user_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }

  # Used to list / count a member's passkeys (Tally). A row has exactly one of
  # user_id and person_id, so each index only holds its own kind of passkey.
  global_secondary_index {
    name = "person_id-index"
    key_schema {
      attribute_name = "person_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Generic ephemeral key/value store with native TTL. Items are namespaced by a
# `kind` discriminator and carry an opaque JSON `payload`; `expires_at` drives
# DynamoDB TTL auto-deletion. Backs kiosk public-key enrollment (kind
# "kiosk_enroll") and WebAuthn challenge state (kind "reg"/"auth"), the latter
# migrated off the former prod_webauthn_state table.
resource "aws_dynamodb_table" "prod_ephemeral_state" {
  name         = "${var.db_prefix}_ephemeral_state"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

resource "aws_dynamodb_table" "prod_oauth_grant" {
  name         = "${var.db_prefix}_oauth_grant"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "user_id"
    type = "S"
  }

  # Backs the "connected apps" list (a user's own grants).
  global_secondary_index {
    name = "user_id-index"
    key_schema {
      attribute_name = "user_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

# Audit log of business DB writes (one item per affected location per event).
# Entries are kept forever (no TTL). `sk` is "{ts:010}#{id}" so lexical order is
# time order.
resource "aws_dynamodb_table" "prod_audit_log" {
  name         = "${var.db_prefix}_audit_log"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }
  attribute {
    name = "scope"
    type = "S"
  }
  attribute {
    name = "sk"
    type = "S"
  }

  # Per-location view. Items for global entities have no location_id, so they
  # are absent from this index (sparse).
  global_secondary_index {
    name = "location_id-sk-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "sk"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
  # Sparse index: `scope = "all"` is set on exactly one item per audited event,
  # giving super users an all-locations view without duplicates.
  global_secondary_index {
    name = "scope-sk-index"
    key_schema {
      attribute_name = "scope"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "sk"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
}

# One row per (member, SES directory entity) the member is assigned to, written
# by directory sync. See SCHEMA.md.
resource "aws_dynamodb_table" "prod_person_unit" {
  name         = "${var.db_prefix}_person_unit"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "person_id"
  range_key    = "entity_id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "person_id"
    type = "S"
  }
  attribute {
    name = "entity_id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }

  # Sparse: only assignments to an entity that is one of our locations carry a
  # location_id. Nothing reads it yet; it is for per-unit membership views.
  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Tasks: a location's shared to-do list. `open_location_id` is a sparse copy of
# `location_id` held only while the task is still to be done, so the index lists
# exactly a location's open tasks, by the (Sydney-local, YYYY-MM-DD) day each
# starts showing.
resource "aws_dynamodb_table" "prod_task" {
  name         = "${var.db_prefix}_task"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "open_location_id"
    type = "S"
  }
  attribute {
    name = "show_date"
    type = "S"
  }

  global_secondary_index {
    name = "open_location_id-show_date-index"
    key_schema {
      attribute_name = "open_location_id"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "show_date"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
}

# Optional named lists a location's tasks can be grouped on.
resource "aws_dynamodb_table" "prod_task_list" {
  name         = "${var.db_prefix}_task_list"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Notes: a location's shared notebook. Notes and folders are listed per location;
# deleted ones are filtered out on read.
resource "aws_dynamodb_table" "prod_note" {
  name         = "${var.db_prefix}_note"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Optional named folders a location's notes can be filed in.
resource "aws_dynamodb_table" "prod_note_folder" {
  name         = "${var.db_prefix}_note_folder"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Map: a location's shared map of pins. Pins and layers are listed per location;
# deleted ones are filtered out on read.
resource "aws_dynamodb_table" "prod_map_pin" {
  name         = "${var.db_prefix}_map_pin"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Optional named layers a location's map pins can be put on.
resource "aws_dynamodb_table" "prod_map_layer" {
  name         = "${var.db_prefix}_map_layer"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# On Call: a location's on-call roles and who holds each now. Listed per location;
# deleted ones are filtered out on read.
resource "aws_dynamodb_table" "prod_on_call_role" {
  name         = "${var.db_prefix}_on_call_role"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Shared contacts: phone numbers and emails added by hand, either a location's (its Duty
# Officer phone, …) or statewide (SOC, EAP, peer support). `scope` is the location ID,
# or "state" for statewide ones, so each list is one query; deleted ones are filtered
# out on read.
resource "aws_dynamodb_table" "prod_shared_contact" {
  name         = "${var.db_prefix}_shared_contact"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "scope"
    type = "S"
  }

  global_secondary_index {
    name = "scope-index"
    key_schema {
      attribute_name = "scope"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Equipment: a location's equipment register, listed per location; deleted ones are
# filtered out on read.
resource "aws_dynamodb_table" "prod_equipment" {
  name         = "${var.db_prefix}_equipment"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Each piece of equipment's log of status changes and checks, listed per equipment.
resource "aws_dynamodb_table" "prod_equipment_log" {
  name         = "${var.db_prefix}_equipment_log"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "equipment_id"
    type = "S"
  }

  global_secondary_index {
    name = "equipment_id-index"
    key_schema {
      attribute_name = "equipment_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Equipment reservations: bookings of a location's equipment for a time range. Listed
# per location by end time, so upcoming ones are read without touching past ones;
# deleted ones are filtered out on read.
resource "aws_dynamodb_table" "prod_equipment_reservation" {
  name         = "${var.db_prefix}_equipment_reservation"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }
  attribute {
    name = "end_time"
    type = "N"
  }

  global_secondary_index {
    name = "location_id-end_time-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "end_time"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
}

# Calendar: optional named, coloured calendars a location's events can be put in.
resource "aws_dynamodb_table" "prod_calendar" {
  name         = "${var.db_prefix}_calendar"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# A location's calendar events. last_end bounds when an event's last occurrence
# ends (far in the future for one repeating forever), so reading a window skips
# events that finished before it.
resource "aws_dynamodb_table" "prod_calendar_event" {
  name         = "${var.db_prefix}_calendar_event"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }
  attribute {
    name = "last_end"
    type = "N"
  }

  global_secondary_index {
    name = "location_id-last_end-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "last_end"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
}

# Locate: one row per member sharing their position with a location's members,
# keyed by person ID. Listed per location; the TTL removes a share 7 minutes after its
# latest position, and reads skip any the TTL hasn't reached yet.
resource "aws_dynamodb_table" "prod_location_share" {
  name         = "${var.db_prefix}_location_share"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }

  global_secondary_index {
    name = "location_id-index"
    key_schema {
      attribute_name = "location_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

# Notifications: each member's in-app inbox, one full copy of a notification per
# recipient (person_id, newest first by sk). unread-index is sparse: rows carry
# unread_person_id only while unread, so counting and listing unread is a small
# query. Rows are removed by the TTL 90 days after they were created.
resource "aws_dynamodb_table" "prod_notification" {
  name         = "${var.db_prefix}_notification"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "person_id"
  range_key    = "sk"

  deletion_protection_enabled = true

  point_in_time_recovery {
    enabled                 = true
    recovery_period_in_days = 35
  }

  attribute {
    name = "person_id"
    type = "S"
  }
  attribute {
    name = "sk"
    type = "S"
  }
  attribute {
    name = "unread_person_id"
    type = "S"
  }

  global_secondary_index {
    name = "unread-index"
    key_schema {
      attribute_name = "unread_person_id"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "sk"
      key_type       = "RANGE"
    }
    projection_type = "KEYS_ONLY"
  }

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

# ── Integration test tables ───────────────────────────────────────────────────
# Disposable fixture for the pagination end-to-end integration test.
# No deletion protection / PITR — data is regenerated by infra/seed_test_pagination.py.

resource "aws_dynamodb_table" "test_pagination" {
  name         = "${var.db_prefix_test}_test_pagination"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "group_id"
    type = "N"
  }
  attribute {
    name = "number"
    type = "N"
  }

  global_secondary_index {
    name = "group_id-number-index"
    key_schema {
      attribute_name = "group_id"
      key_type       = "HASH"
    }
    key_schema {
      attribute_name = "number"
      key_type       = "RANGE"
    }
    projection_type = "ALL"
  }
}

# ── IAM: DynamoDB access policies ─────────────────────────────────────────────
# Added to each Lambda role so they can access DynamoDB (DB_BACKEND=dynamodb).

locals {
  dynamodb_actions = [
    "dynamodb:GetItem",
    "dynamodb:PutItem",
    "dynamodb:UpdateItem",
    "dynamodb:DeleteItem",
    "dynamodb:BatchGetItem",
    "dynamodb:BatchWriteItem",
    "dynamodb:Query",
    "dynamodb:Scan",
  ]
}

resource "aws_iam_role_policy" "api_lambda_dynamodb" {
  name = "dynamodb-access"
  role = aws_iam_role.api_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "test_api_lambda_dynamodb" {
  name = "dynamodb-access"
  role = aws_iam_role.test_api_lambda.id

  # The test tables only: test.seslogin.com must not be able to reach prod data.
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix_test}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix_test}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "preprod_api_lambda_dynamodb" {
  name = "dynamodb-access"
  role = aws_iam_role.preprod_api_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "qa_api_lambda_dynamodb" {
  name = "dynamodb-access"
  role = aws_iam_role.qa_api_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "dispatcher_lambda_dynamodb" {
  name = "dynamodb-access"
  role = aws_iam_role.dispatcher_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "sync_lambda_dynamodb" {
  name = "dynamodb-access"
  role = aws_iam_role.sync_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "checker_lambda_dynamodb" {
  name = "dynamodb-access"
  role = aws_iam_role.checker_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "nitc_export_lambda_dynamodb" {
  name = "dynamodb-access"
  role = aws_iam_role.nitc_export_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "activity_summary_lambda_dynamodb" {
  name = "dynamodb"
  role = aws_iam_role.activity_summary_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "badge_nightly_lambda_dynamodb" {
  name = "dynamodb"
  role = aws_iam_role.badge_nightly_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "badge_digest_lambda_dynamodb" {
  name = "dynamodb"
  role = aws_iam_role.badge_digest_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "sync_locations_lambda_dynamodb" {
  name = "dynamodb"
  role = aws_iam_role.sync_locations_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}

resource "aws_iam_role_policy" "sync_directory_lambda_dynamodb" {
  name = "dynamodb"
  role = aws_iam_role.sync_directory_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = local.dynamodb_actions
      Resource = [
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*",
        "arn:aws:dynamodb:${local.region}:${local.account_id}:table/${var.db_prefix}*/index/*",
      ]
    }]
  })
}
