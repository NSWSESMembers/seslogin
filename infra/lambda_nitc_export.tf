resource "aws_lambda_function" "nitc_export" {
  function_name = "seslogin-nitc-export"
  role          = aws_iam_role.nitc_export_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 300
  memory_size   = 256
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      SES_API_KEY           = var.ses_api_key
      SES_API_BASE_URL      = var.ses_api_base_url
      NITC_EXPORT_QUEUE_URL = aws_sqs_queue.nitc_export.url
      DB_BACKEND            = "dynamodb"
      DB_PREFIX             = var.db_prefix
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}

resource "aws_lambda_event_source_mapping" "nitc_export_sqs" {
  event_source_arn = aws_sqs_queue.nitc_export.arn
  function_name    = aws_lambda_function.nitc_export.arn
  batch_size       = 1
  enabled          = var.background_jobs_enabled

  scaling_config {
    maximum_concurrency = 2
  }
}
