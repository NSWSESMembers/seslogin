resource "aws_lambda_function" "badge_digest" {
  function_name = "seslogin-badge-digest"
  role          = aws_iam_role.badge_digest_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 120
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      DB_BACKEND = "dynamodb"
      DB_PREFIX  = var.db_prefix
      # Badge icon URLs in the digest email are built from this (see
      # api/src/base_url.rs's web_base_url, which api/src/badge_digest.rs
      # reuses) rather than a hard-coded hostname, so they come from the site
      # whose tables the digest reads.
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
