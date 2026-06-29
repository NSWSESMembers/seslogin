resource "aws_scheduler_schedule" "member_sync_hourly" {
  name       = "seslogin-member-sync-hourly"
  group_name = "default"
  state      = var.background_jobs_enabled ? "ENABLED" : "DISABLED"

  flexible_time_window {
    mode = "OFF"
  }

  # Run once per hour so the dispatcher can splay locations across 24 buckets.
  schedule_expression          = "cron(0 * * * ? *)"
  schedule_expression_timezone = "UTC"

  target {
    arn      = aws_lambda_function.dispatcher.arn
    role_arn = aws_iam_role.scheduler.arn
    input    = "{}"
  }
}

resource "aws_scheduler_schedule" "checker_daily" {
  name       = "seslogin-checker-daily"
  group_name = "default"
  state      = var.background_jobs_enabled ? "ENABLED" : "DISABLED"

  flexible_time_window {
    mode = "OFF"
  }

  # 6 AM Sydney time (handles AEST/AEDT automatically).
  # Runs a few hours after all locations should have completed their nightly sync.
  schedule_expression          = "cron(0 6 * * ? *)"
  schedule_expression_timezone = "Australia/Sydney"

  target {
    arn      = aws_lambda_function.checker.arn
    role_arn = aws_iam_role.scheduler.arn
    input    = "{}"
  }
}

resource "aws_scheduler_schedule" "activity_summary_nightly" {
  name       = "seslogin-activity-summary-nightly"
  group_name = "default"
  state      = var.background_jobs_enabled ? "ENABLED" : "DISABLED"

  flexible_time_window {
    mode = "OFF"
  }

  # 00:05 Sydney time — sends the previous day's activity summary emails.
  schedule_expression          = "cron(5 0 * * ? *)"
  schedule_expression_timezone = "Australia/Sydney"

  target {
    arn      = aws_lambda_function.activity_summary.arn
    role_arn = aws_iam_role.scheduler.arn
    input    = "{}"
  }
}

resource "aws_scheduler_schedule" "location_sync_nightly" {
  name       = "seslogin-location-sync-nightly"
  group_name = "default"
  state      = var.background_jobs_enabled ? "ENABLED" : "DISABLED"

  flexible_time_window {
    mode = "OFF"
  }

  # 2 AM UTC daily — off-peak, no DST complications.
  schedule_expression          = "cron(0 2 * * ? *)"
  schedule_expression_timezone = "UTC"

  target {
    arn      = aws_lambda_function.sync_locations.arn
    role_arn = aws_iam_role.scheduler.arn
    input    = "{}"
  }
}
