resource "aws_lambda_function" "activity_summary" {
  function_name = "seslogin-activity-summary"
  role          = aws_iam_role.activity_summary_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 120
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      DB_BACKEND   = "dynamodb"
      DB_PREFIX    = var.db_prefix
      SES_ROLE_ARN = var.ses_role_arn
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}
