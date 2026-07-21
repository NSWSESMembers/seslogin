data "aws_iam_policy_document" "lambda_assume_role" {
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }
  }
}

data "aws_iam_policy_document" "scheduler_assume_role" {
  statement {
    actions = ["sts:AssumeRole"]
    principals {
      type        = "Service"
      identifiers = ["scheduler.amazonaws.com"]
    }
  }
}

# ── API lambda role ────────────────────────────────────────────────────────────

resource "aws_iam_role" "api_lambda" {
  name               = "seslogin-api-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "api_lambda_logs" {
  role       = aws_iam_role.api_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "api_lambda_sqs_send" {
  name = "sqs-send"
  role = aws_iam_role.api_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = ["sqs:SendMessage"]
      Resource = [
        aws_sqs_queue.member_sync.arn,
        aws_sqs_queue.nitc_export.arn,
        aws_sqs_queue.healthcheck.arn,
      ]
    }]
  })
}

resource "aws_iam_role_policy" "api_lambda_ses" {
  name = "ses-send"
  role = aws_iam_role.api_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["ses:SendEmail", "ses:SendRawEmail"]
      Resource = "*"
    }]
  })
}

resource "aws_iam_role_policy" "test_api_lambda_ses" {
  name = "ses-send"
  role = aws_iam_role.test_api_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["ses:SendEmail", "ses:SendRawEmail"]
      Resource = "*"
    }]
  })
}

# ── Sync lambda role ───────────────────────────────────────────────────────────

resource "aws_iam_role" "sync_lambda" {
  name               = "seslogin-sync-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "sync_lambda_logs" {
  role       = aws_iam_role.sync_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "sync_lambda_sqs_consume" {
  name = "sqs-consume"
  role = aws_iam_role.sync_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = [
        "sqs:ReceiveMessage",
        "sqs:DeleteMessage",
        "sqs:GetQueueAttributes",
      ]
      Resource = aws_sqs_queue.member_sync.arn
    }]
  })
}

# ── Dispatcher lambda role ─────────────────────────────────────────────────────

resource "aws_iam_role" "dispatcher_lambda" {
  name               = "seslogin-dispatcher-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "dispatcher_lambda_logs" {
  role       = aws_iam_role.dispatcher_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "dispatcher_lambda_sqs_send" {
  name = "sqs-send"
  role = aws_iam_role.dispatcher_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["sqs:SendMessage"]
      Resource = aws_sqs_queue.member_sync.arn
    }]
  })
}

# ── Checker lambda role ────────────────────────────────────────────────────────

resource "aws_iam_role" "checker_lambda" {
  name               = "seslogin-checker-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "checker_lambda_logs" {
  role       = aws_iam_role.checker_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "checker_lambda_sns_publish" {
  name = "sns-publish"
  role = aws_iam_role.checker_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["sns:Publish"]
      Resource = aws_sns_topic.member_sync_alerts.arn
    }]
  })
}

# ── NITC export lambda role ───────────────────────────────────────────────────

resource "aws_iam_role" "nitc_export_lambda" {
  name               = "seslogin-nitc-export-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "nitc_export_lambda_logs" {
  role       = aws_iam_role.nitc_export_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "nitc_export_lambda_sqs" {
  name = "sqs-access"
  role = aws_iam_role.nitc_export_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = [
        "sqs:ReceiveMessage",
        "sqs:DeleteMessage",
        "sqs:GetQueueAttributes",
        "sqs:SendMessage",
      ]
      Resource = aws_sqs_queue.nitc_export.arn
    }]
  })
}

# ── Healthcheck lambda role ────────────────────────────────────────────────────

resource "aws_iam_role" "healthcheck_lambda" {
  name               = "seslogin-healthcheck-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "healthcheck_lambda_logs" {
  role       = aws_iam_role.healthcheck_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "healthcheck_lambda_sqs_consume" {
  name = "sqs-consume"
  role = aws_iam_role.healthcheck_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = [
        "sqs:ReceiveMessage",
        "sqs:DeleteMessage",
        "sqs:GetQueueAttributes",
      ]
      Resource = aws_sqs_queue.healthcheck.arn
    }]
  })
}

# ── EventBridge Scheduler role ─────────────────────────────────────────────────

resource "aws_iam_role" "scheduler" {
  name               = "seslogin-scheduler-role"
  assume_role_policy = data.aws_iam_policy_document.scheduler_assume_role.json
}

resource "aws_iam_role_policy" "scheduler_invoke_dispatcher" {
  name = "invoke-dispatcher"
  role = aws_iam_role.scheduler.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["lambda:InvokeFunction"]
      Resource = aws_lambda_function.dispatcher.arn
    }]
  })
}

resource "aws_iam_role_policy" "scheduler_invoke_checker" {
  name = "invoke-checker"
  role = aws_iam_role.scheduler.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["lambda:InvokeFunction"]
      Resource = aws_lambda_function.checker.arn
    }]
  })
}

# ── Activity summary lambda role ───────────────────────────────────────────────

resource "aws_iam_role" "activity_summary_lambda" {
  name               = "seslogin-activity-summary-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "activity_summary_lambda_logs" {
  role       = aws_iam_role.activity_summary_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "activity_summary_lambda_ses" {
  name = "ses-send"
  role = aws_iam_role.activity_summary_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["ses:SendEmail", "ses:SendRawEmail"]
      Resource = "*"
    }]
  })
}

resource "aws_iam_role_policy" "scheduler_invoke_activity_summary" {
  name = "invoke-activity-summary"
  role = aws_iam_role.scheduler.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["lambda:InvokeFunction"]
      Resource = aws_lambda_function.activity_summary.arn
    }]
  })
}

# ── Badge digest lambda role ─────────────────────────────────────────────────

resource "aws_iam_role" "badge_digest_lambda" {
  name               = "seslogin-badge-digest-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "badge_digest_lambda_logs" {
  role       = aws_iam_role.badge_digest_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "badge_digest_lambda_ses" {
  name = "ses-send"
  role = aws_iam_role.badge_digest_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["ses:SendEmail", "ses:SendRawEmail"]
      Resource = "*"
    }]
  })
}

resource "aws_iam_role_policy" "scheduler_invoke_badge_digest" {
  name = "invoke-badge-digest"
  role = aws_iam_role.scheduler.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["lambda:InvokeFunction"]
      Resource = aws_lambda_function.badge_digest.arn
    }]
  })
}

# ── Badge nightly lambda role ────────────────────────────────────────────────

resource "aws_iam_role" "badge_nightly_lambda" {
  name               = "seslogin-badge-nightly-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "badge_nightly_lambda_logs" {
  role       = aws_iam_role.badge_nightly_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "scheduler_invoke_badge_nightly" {
  name = "invoke-badge-nightly"
  role = aws_iam_role.scheduler.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["lambda:InvokeFunction"]
      Resource = aws_lambda_function.badge_nightly.arn
    }]
  })
}

# ── Sync locations lambda role ─────────────────────────────────────────────────

resource "aws_iam_role" "sync_locations_lambda" {
  name               = "seslogin-sync-locations-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "sync_locations_lambda_logs" {
  role       = aws_iam_role.sync_locations_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "scheduler_invoke_sync_locations" {
  name = "invoke-sync-locations"
  role = aws_iam_role.scheduler.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["lambda:InvokeFunction"]
      Resource = aws_lambda_function.sync_locations.arn
    }]
  })
}
