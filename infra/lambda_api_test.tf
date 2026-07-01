resource "aws_lambda_function" "test_api" {
  function_name = "seslogin-test-api"
  role          = aws_iam_role.test_api_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 30
  memory_size   = 128
  filename      = "placeholder.zip"
  logging_config {
    log_format = "JSON"
  }

  lifecycle { ignore_changes = [filename, source_code_hash] }

  environment {
    variables = {
      JWT_SECRET            = var.jwt_secret_test
      SES_API_KEY           = var.ses_api_key_test
      SES_API_BASE_URL      = var.ses_api_base_url
      MEMBER_SYNC_QUEUE_URL = aws_sqs_queue.member_sync.url
      NITC_EXPORT_QUEUE_URL = aws_sqs_queue.nitc_export.url
      HEALTHCHECK_QUEUE_URL = aws_sqs_queue.healthcheck.url
      DB_BACKEND            = "dynamodb"
      DB_PREFIX             = var.db_prefix
      READ_ONLY             = "false"
      TURNSTILE_SECRET_KEY  = var.turnstile_secret_key_test
      WEBAUTHN_RP_ID        = "seslogin.com"
      WEBAUTHN_RP_ORIGIN    = "https://test.seslogin.com"
      SES_ROLE_ARN          = var.ses_role_arn
    }
  }
}

resource "aws_lambda_function_url" "test_api" {
  function_name      = aws_lambda_function.test_api.function_name
  authorization_type = "NONE"
  cors {
    allow_origins = ["https://test.seslogin.com"]
    allow_methods = ["GET", "POST"]
    allow_headers = ["authorization", "content-type", "x-client-version"]
    max_age       = 600
  }
}
