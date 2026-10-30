# Test copies of the prod DynamoDB tables (var.db_prefix_test = seslogin_test_*).
#
# Deliberately a *separate* duplicate of the definitions in dynamodb.tf (not a
# shared module/loop) so we can iterate on the test schema freely with zero risk
# of changes leaking to the prod tables. When a change is proven, mirror it into
# dynamodb.tf for prod.
#
# Differences from prod, on purpose, so these are a disposable sandbox:
#   - deletion_protection disabled (lets us destroy/recreate when changing keys)
#   - no point-in-time recovery (not needed for a sandbox; avoids the cost)
#   - not included in the AWS Backup plan (see backup.tf — prod tables only)
#
# These are the test environment's database: seslogin-test-api runs with
# DB_PREFIX = var.db_prefix_test, and so does local dev (`.env`). See the note at
# the bottom of this file for how test is kept off the prod tables.

resource "aws_dynamodb_table" "test_user" {
  name         = "${var.db_prefix_test}_user"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "email"
    type = "S"
  }

  global_secondary_index {
    name = "email-index"
    key_schema {
      attribute_name = "email"
      key_type       = "HASH"
    }
    projection_type = "KEYS_ONLY"
  }
}

resource "aws_dynamodb_table" "test_category" {
  name         = "${var.db_prefix_test}_category"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "nitc_group_id"
    type = "S"
  }

  global_secondary_index {
    name = "nitc_group_id-index"
    key_schema {
      attribute_name = "nitc_group_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

resource "aws_dynamodb_table" "test_location" {
  name         = "${var.db_prefix_test}_location"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
}

resource "aws_dynamodb_table" "test_period" {
  name         = "${var.db_prefix_test}_period"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
  global_secondary_index {
    name = "nitc_event_id-index"
    key_schema {
      attribute_name = "nitc_event_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
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

resource "aws_dynamodb_table" "test_person" {
  name         = "${var.db_prefix_test}_person"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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

resource "aws_dynamodb_table" "test_session" {
  name         = "${var.db_prefix_test}_session"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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

resource "aws_dynamodb_table" "test_api_token" {
  name         = "${var.db_prefix_test}_api_token"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "active"
    type = "N"
  }

  global_secondary_index {
    name = "active-index"
    key_schema {
      attribute_name = "active"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

resource "aws_dynamodb_table" "test_nitc_group" {
  name         = "${var.db_prefix_test}_nitc_group"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
}

resource "aws_dynamodb_table" "test_nitc_tag" {
  name         = "${var.db_prefix_test}_nitc_tag"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
}

resource "aws_dynamodb_table" "test_nitc_event" {
  name         = "${var.db_prefix_test}_nitc_event"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "location_id"
    type = "S"
  }
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

resource "aws_dynamodb_table" "test_login_code" {
  name         = "${var.db_prefix_test}_login_code"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "email"

  attribute {
    name = "email"
    type = "S"
  }

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

resource "aws_dynamodb_table" "test_user_token" {
  name         = "${var.db_prefix_test}_user_token"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

resource "aws_dynamodb_table" "test_webauthn_credential" {
  name         = "${var.db_prefix_test}_webauthn_credential"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "user_id"
    type = "S"
  }

  global_secondary_index {
    name = "user_id-index"
    key_schema {
      attribute_name = "user_id"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }
}

# Generic ephemeral key/value store with native TTL. Items are namespaced by a
# `kind` discriminator and carry an opaque JSON `payload`; `expires_at` drives
# DynamoDB TTL auto-deletion. Backs kiosk public-key enrollment (kind
# "kiosk_enroll") and WebAuthn challenge state (kind "reg"/"auth").
resource "aws_dynamodb_table" "test_ephemeral_state" {
  name         = "${var.db_prefix_test}_ephemeral_state"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

resource "aws_dynamodb_table" "test_oauth_grant" {
  name         = "${var.db_prefix_test}_oauth_grant"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }
  attribute {
    name = "user_id"
    type = "S"
  }

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

# Audit log of business DB writes (mirrors prod_audit_log in dynamodb.tf).
resource "aws_dynamodb_table" "test_audit_log" {
  name         = "${var.db_prefix_test}_audit_log"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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

resource "aws_dynamodb_table" "test_person_unit" {
  name         = "${var.db_prefix_test}_person_unit"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "person_id"
  range_key    = "entity_id"

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
resource "aws_dynamodb_table" "test_task" {
  name         = "${var.db_prefix_test}_task"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
resource "aws_dynamodb_table" "test_task_list" {
  name         = "${var.db_prefix_test}_task_list"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
resource "aws_dynamodb_table" "test_note" {
  name         = "${var.db_prefix_test}_note"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
resource "aws_dynamodb_table" "test_note_folder" {
  name         = "${var.db_prefix_test}_note_folder"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
resource "aws_dynamodb_table" "test_map_pin" {
  name         = "${var.db_prefix_test}_map_pin"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
resource "aws_dynamodb_table" "test_map_layer" {
  name         = "${var.db_prefix_test}_map_layer"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
resource "aws_dynamodb_table" "test_on_call_role" {
  name         = "${var.db_prefix_test}_on_call_role"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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

# Equipment: a location's equipment register, listed per location; deleted ones are
# filtered out on read.
resource "aws_dynamodb_table" "test_equipment" {
  name         = "${var.db_prefix_test}_equipment"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
resource "aws_dynamodb_table" "test_equipment_log" {
  name         = "${var.db_prefix_test}_equipment_log"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
resource "aws_dynamodb_table" "test_equipment_reservation" {
  name         = "${var.db_prefix_test}_equipment_reservation"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
resource "aws_dynamodb_table" "test_calendar" {
  name         = "${var.db_prefix_test}_calendar"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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
resource "aws_dynamodb_table" "test_calendar_event" {
  name         = "${var.db_prefix_test}_calendar_event"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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

# Locate: one row per user sharing their position with a location's members, keyed
# by user ID. Listed per location; the TTL removes a share 7 minutes after its
# latest position, and reads skip any the TTL hasn't reached yet.
resource "aws_dynamodb_table" "test_location_share" {
  name         = "${var.db_prefix_test}_location_share"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

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

# Notifications: each user's in-app inbox, one full copy of a notification per
# recipient (user_id, newest first by sk). unread-index is sparse: rows carry
# unread_user_id only while unread, so counting and listing unread is a small
# query. Rows are removed by the TTL 90 days after they were created.
resource "aws_dynamodb_table" "test_notification" {
  name         = "${var.db_prefix_test}_notification"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "person_id"
  range_key    = "sk"

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

# ── Test API access to the test tables ────────────────────────────────────────
# seslogin-test-api's DB_PREFIX is var.db_prefix_test, and its dynamodb-access
# policy in dynamodb.tf is scoped to ${var.db_prefix_test}* only, so the test
# environment cannot reach the prod tables. Nor can it enqueue onto prod's SQS
# queues, whose consumers (the workers Lambdas) read the prod tables: it has no
# queue URLs or SQS permissions, so member sync and NITC export are off there.
