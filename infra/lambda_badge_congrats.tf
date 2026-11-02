# Runs as the weekly digest's role: the same tables and the same SES send.
resource "aws_lambda_function" "badge_congrats" {
  function_name = "seslogin-badge-congrats"
  role          = aws_iam_role.badge_digest_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 300
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      DB_BACKEND = "dynamodb"
      DB_PREFIX  = var.db_prefix
      # Badge icon and logo URLs in the email are built from this (see
      # api/src/base_url.rs's web_base_url) rather than a hard-coded hostname.
      WEB_BASE_URL = "https://seslogin.com"
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}
