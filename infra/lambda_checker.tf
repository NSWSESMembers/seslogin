resource "aws_lambda_function" "checker" {
  function_name = "seslogin-checker"
  role          = aws_iam_role.checker_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 60
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      SNS_TOPIC_ARN = aws_sns_topic.member_sync_alerts.arn
      DB_BACKEND    = "dynamodb"
      DB_PREFIX     = var.db_prefix
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}
