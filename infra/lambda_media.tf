# Derived-image server (api/src/media.rs, bin/media-lambda.rs). Serves
# GET /media/{asset_id}/{variant} behind each CloudFront distribution's /media/*
# behaviour: prod, preprod and qa share seslogin-media (the prod asset bucket), test has
# seslogin-test-media (the test bucket). It needs only the bucket and the server secret
# that signs media URLs: no DynamoDB, so no tables to prefix.

resource "aws_iam_role" "media_lambda" {
  name               = "seslogin-media-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "media_lambda_logs" {
  role       = aws_iam_role.media_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

# Originals are read, never written; derived images are read and written. ListBucket
# turns a missing key into a 404 for the SDK instead of a 403.
resource "aws_iam_role_policy" "media_lambda_assets" {
  name = "assets-access"
  role = aws_iam_role.media_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["s3:GetObject"]
        Resource = ["${aws_s3_bucket.assets_prod.arn}/originals/*", "${aws_s3_bucket.assets_prod.arn}/derived/*"]
      },
      {
        Effect   = "Allow"
        Action   = ["s3:PutObject"]
        Resource = "${aws_s3_bucket.assets_prod.arn}/derived/*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:ListBucket"]
        Resource = aws_s3_bucket.assets_prod.arn
      },
    ]
  })
}

resource "aws_lambda_function" "media" {
  function_name = "seslogin-media"
  role          = aws_iam_role.media_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 30
  # Decoding a large photo needs room: the 100-megapixel limit allows ~768 MiB of buffers.
  memory_size = 2048
  filename    = "${path.module}/placeholder.zip"

  environment {
    variables = {
      JWT_SECRET   = var.jwt_secret
      ASSET_BUCKET = aws_s3_bucket.assets_prod.bucket
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}

resource "aws_lambda_function_url" "media" {
  function_name      = aws_lambda_function.media.function_name
  authorization_type = "NONE"
}

resource "aws_iam_role" "test_media_lambda" {
  name               = "seslogin-test-media-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "test_media_lambda_logs" {
  role       = aws_iam_role.test_media_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

# The test bucket only: test.seslogin.com must not be able to reach prod photos.
resource "aws_iam_role_policy" "test_media_lambda_assets" {
  name = "assets-access"
  role = aws_iam_role.test_media_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["s3:GetObject"]
        Resource = ["${aws_s3_bucket.assets_test.arn}/originals/*", "${aws_s3_bucket.assets_test.arn}/derived/*"]
      },
      {
        Effect   = "Allow"
        Action   = ["s3:PutObject"]
        Resource = "${aws_s3_bucket.assets_test.arn}/derived/*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:ListBucket"]
        Resource = aws_s3_bucket.assets_test.arn
      },
    ]
  })
}

resource "aws_lambda_function" "test_media" {
  function_name = "seslogin-test-media"
  role          = aws_iam_role.test_media_lambda.arn
  runtime       = "provided.al2023"
  handler       = "bootstrap"
  timeout       = 30
  memory_size   = 2048
  filename      = "${path.module}/placeholder.zip"

  environment {
    variables = {
      JWT_SECRET   = var.jwt_secret_test
      ASSET_BUCKET = aws_s3_bucket.assets_test.bucket
    }
  }

  logging_config {
    log_format = "JSON"
  }

  lifecycle {
    ignore_changes = [filename, source_code_hash]
  }
}

resource "aws_lambda_function_url" "test_media" {
  function_name      = aws_lambda_function.test_media.function_name
  authorization_type = "NONE"
}
