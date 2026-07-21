resource "aws_lambda_function" "badge_nightly" {
  function_name = "seslogin-badge-nightly"
  role          = aws_iam_role.badge_nightly_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 120
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      DB_BACKEND = "dynamodb"
      DB_PREFIX  = var.db_prefix
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}