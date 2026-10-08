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
# Wiring these into the test environment (test API DB_PREFIX + IAM) is a separate
# step — see the note at the bottom of this file.

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
    name = "code"
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
    name = "code-index"
    key_schema {
      attribute_name = "code"
      key_type       = "HASH"
    }
    projection_type = "KEYS_ONLY"
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

# ── Test API access to the test tables ────────────────────────────────────────
# The test API role's dynamodb-access policy in dynamodb.tf is scoped to
# ${var.db_prefix}* (prod). To run the test environment against these tables,
# point seslogin-test-api's DB_PREFIX at var.db_prefix_test and broaden that
# policy to also cover ${var.db_prefix_test}*. Left as a deliberate separate step
# so creating these tables doesn't change what test.seslogin.com reads today.
