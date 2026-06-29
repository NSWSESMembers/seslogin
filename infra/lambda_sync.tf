resource "aws_lambda_function" "sync_members" {
  function_name = "seslogin-sync-members"
  role          = aws_iam_role.sync_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 300
  memory_size   = 256
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      SES_API_KEY            = var.ses_api_key
      SES_API_BASE_URL       = var.ses_api_base_url
      SES_SYNC_DRY_RUN       = "false"
      SES_SYNC_ADOPT         = "false"
      SES_PAGE_LIMIT         = "100"
      SES_SYNC_MAX_RETRIES   = "3"
      SES_SYNC_MAX_MUTATIONS = "100"
      DB_BACKEND             = "dynamodb"
      DB_PREFIX              = var.db_prefix
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}

resource "aws_lambda_event_source_mapping" "sync_members_sqs" {
  event_source_arn = aws_sqs_queue.member_sync.arn
  function_name    = aws_lambda_function.sync_members.arn
  batch_size       = 1
  enabled          = var.background_jobs_enabled
}
