resource "aws_lambda_function" "healthcheck" {
  function_name = "seslogin-healthcheck"
  role          = aws_iam_role.healthcheck_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 30
  memory_size   = 128
  filename      = "${path.module}/placeholder.zip"

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}

resource "aws_lambda_event_source_mapping" "healthcheck_sqs" {
  event_source_arn = aws_sqs_queue.healthcheck.arn
  function_name    = aws_lambda_function.healthcheck.arn
  batch_size       = 10
  enabled          = var.background_jobs_enabled
}
