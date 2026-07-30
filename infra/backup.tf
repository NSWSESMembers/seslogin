# AWS Backup for the prod DynamoDB tables.
#
# The prod tables already have PITR (35-day continuous backups) and deletion
# protection (see dynamodb.tf). PITR is same-account/same-region only and caps at
# 35 days, so this adds independent, longer-retention snapshots stored in a
# separate vault and copied cross-region to Melbourne for region-failure
# resilience.
#
# AWS Backup always writes the primary recovery point to a vault in the same
# region as the source table (Sydney, ap-southeast-2). The Sydney vault is
# therefore required but kept at a short 5-day retention as a staging point; the
# Melbourne copy holds the real 90-day archive.

# ── Tables to back up ─────────────────────────────────────────────────────────
# Durable business + auth data only. The two short-lived TTL'd tables
# (prod_login_code, prod_ephemeral_state) are intentionally excluded — they hold
# only ephemeral challenge/code state, so backing them up adds cost with no
# value. They keep PITR regardless.

locals {
  backup_tables = [
    aws_dynamodb_table.prod_user,
    aws_dynamodb_table.prod_category,
    aws_dynamodb_table.prod_location,
    aws_dynamodb_table.prod_period,
    aws_dynamodb_table.prod_person,
    aws_dynamodb_table.prod_session,
    aws_dynamodb_table.prod_api_token,
    aws_dynamodb_table.prod_nitc_group,
    aws_dynamodb_table.prod_nitc_tag,
    aws_dynamodb_table.prod_nitc_event,
    aws_dynamodb_table.prod_user_token,
    aws_dynamodb_table.prod_webauthn_credential,
  ]
}

# ── IAM role for AWS Backup ───────────────────────────────────────────────────

data "aws_iam_policy_document" "backup_assume_role" {
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["backup.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "backup" {
  name               = "seslogin-backup-role"
  assume_role_policy = data.aws_iam_policy_document.backup_assume_role.json
}

resource "aws_iam_role_policy_attachment" "backup" {
  role       = aws_iam_role.backup.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSBackupServiceRolePolicyForBackup"
}

resource "aws_iam_role_policy_attachment" "backup_restore" {
  role       = aws_iam_role.backup.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSBackupServiceRolePolicyForRestores"
}

# ── Vaults ────────────────────────────────────────────────────────────────────

# Sydney (in-region) — short-lived staging vault for the primary recovery point.
resource "aws_backup_vault" "seslogin_prod" {
  name = "seslogin-prod"
}

# Melbourne — cross-region archive, holds the long-retention copy.
resource "aws_backup_vault" "seslogin_prod_melbourne" {
  provider = aws.melbourne
  name     = "seslogin-prod-melbourne"
}

# ── Backup plan ───────────────────────────────────────────────────────────────

resource "aws_backup_plan" "seslogin_prod" {
  name = "seslogin-prod"

  rule {
    rule_name         = "daily"
    target_vault_name = aws_backup_vault.seslogin_prod.name
    # 14:00 UTC ≈ midnight Sydney (quiet hour).
    schedule = "cron(0 14 * * ? *)"

    # Sydney primary kept only as a staging point for the cross-region copy.
    lifecycle {
      delete_after = 5
    }

    # The real archive: copy each recovery point to Melbourne, retained 90 days.
    # The copy persists independently after the Sydney primary expires.
    copy_action {
      destination_vault_arn = aws_backup_vault.seslogin_prod_melbourne.arn
      lifecycle {
        delete_after = 90
      }
    }
  }
}

# ── Selection ─────────────────────────────────────────────────────────────────

resource "aws_backup_selection" "seslogin_prod" {
  name         = "seslogin-prod-dynamodb"
  iam_role_arn = aws_iam_role.backup.arn
  plan_id      = aws_backup_plan.seslogin_prod.id
  resources    = local.backup_tables[*].arn
}
