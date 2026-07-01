resource "aws_lambda_function" "preprod_api" {
  function_name = "seslogin-preprod-api"
  role          = aws_iam_role.preprod_api_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 30
  memory_size   = 128
  filename      = "${path.module}/placeholder.zip"

  # Preprod is a clone of prod: it intentionally shares the prod database, queues,
  # and secrets. The only differences from the prod lambda are the function name,
  # IAM role, and the WebAuthn/CORS origin (preprod.seslogin.com).
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
      WEBAUTHN_RP_ORIGIN    = "https://preprod.seslogin.com"
      SES_ROLE_ARN          = var.ses_role_arn
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}

resource "aws_lambda_function_url" "preprod_api" {
  function_name      = aws_lambda_function.preprod_api.function_name
  authorization_type = "NONE"

  cors {
    allow_credentials = true
    allow_headers     = ["authorization", "content-type", "x-client-version"]
    allow_methods     = ["GET", "POST"]
    allow_origins     = ["https://preprod.seslogin.com"]
    max_age           = 600
  }
}
