resource "aws_lambda_function" "change_digest" {
  function_name = "seslogin-change-digest"
  role          = aws_iam_role.change_digest_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 120
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      DB_BACKEND = "dynamodb"
      DB_PREFIX  = var.db_prefix
      # Links in the digest email are built from this (base_url::web_base_url).
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
