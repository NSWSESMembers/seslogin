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

  # Used to list / count a user's passkeys in the admin UI.
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
