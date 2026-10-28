resource "aws_lambda_function" "sync_directory" {
  function_name = "seslogin-sync-directory"
  role          = aws_iam_role.sync_directory_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  # The whole directory is ~10 MB over ~14 requests, and every member and
  # person_unit row is read once; writes are only the day's deltas.
  timeout     = 900
  memory_size = 512
  filename    = "${path.module}/placeholder.zip"

  environment {
    variables = {
      SES_INTRANET_SEARCH_API_KEY      = var.ses_intranet_search_api_key
      SES_INTRANET_SEARCH_API_BASE_URL = var.ses_intranet_search_api_base_url
      SES_SYNC_MAX_RETRIES             = "3"
      DIRECTORY_SYNC_MAX_MUTATIONS     = "500"
      DB_BACKEND                       = "dynamodb"
      DB_PREFIX                        = var.db_prefix
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}
