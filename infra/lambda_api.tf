resource "aws_lambda_function" "api" {
  function_name = "seslogin-api"
  role          = aws_iam_role.api_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 30
  memory_size   = 128
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      JWT_SECRET            = var.jwt_secret
      SES_API_KEY           = var.ses_api_key
      SES_API_BASE_URL      = var.ses_api_base_url
      MEMBER_SYNC_QUEUE_URL = aws_sqs_queue.member_sync.url
      NITC_EXPORT_QUEUE_URL = aws_sqs_queue.nitc_export.url
      HEALTHCHECK_QUEUE_URL = aws_sqs_queue.healthcheck.url
      DB_BACKEND            = "dynamodb"
      DB_PREFIX             = var.db_prefix
      READ_ONLY             = "false"
      TURNSTILE_SECRET_KEY  = var.turnstile_secret_key
      WEBAUTHN_RP_ID        = "seslogin.com"
      WEBAUTHN_RP_ORIGIN    = "https://new.seslogin.com,https://seslogin.com"
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}

resource "aws_lambda_function_url" "api" {
  function_name      = aws_lambda_function.api.function_name
  authorization_type = "NONE"

  cors {
    allow_credentials = true
    allow_headers     = ["authorization", "content-type", "x-client-version"]
    allow_methods     = ["GET", "POST"]
    allow_origins     = ["https://seslogin.com", "https://new.seslogin.com"]
    max_age           = 600
  }
}
