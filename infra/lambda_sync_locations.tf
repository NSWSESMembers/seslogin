resource "aws_lambda_function" "sync_locations" {
  function_name = "seslogin-sync-locations"
  role          = aws_iam_role.sync_locations_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 300
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      SES_API_KEY                      = var.ses_api_key
      SES_API_BASE_URL                 = var.ses_api_base_url
      SES_INTRANET_SEARCH_API_KEY      = var.ses_intranet_search_api_key
      SES_INTRANET_SEARCH_API_BASE_URL = var.ses_intranet_search_api_base_url
      SES_SYNC_MAX_RETRIES             = "3"
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
