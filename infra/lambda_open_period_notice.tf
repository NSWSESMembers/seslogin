resource "aws_lambda_function" "open_period_notice" {
  function_name = "seslogin-open-period-notice"
  role          = aws_iam_role.open_period_notice_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  # Longer than the other scheduled jobs: this one sweeps every allow-listed
  # location and then sends email. Expected runtime is a few seconds; the
  # headroom is so a slow run is a metric to look at rather than a timeout.
  timeout  = 300
  filename = "${path.module}/placeholder.zip"

  environment {
    variables = {
      DB_BACKEND    = "dynamodb"
      DB_PREFIX     = var.db_prefix
      SNS_TOPIC_ARN = aws_sns_topic.member_sync_alerts.arn
      # OPEN_PERIOD_NOTICE_PERSON_IDS / OPEN_PERIOD_NOTICE_LOCATION_IDS are
      # deliberately absent: with no allow list the job does nothing, so the
      # feature ships off by omission. Add them here to enable a scope.
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}
