# ── Developer group ────────────────────────────────────────────────────────────
# Human developer accounts are created manually in the console and added to this group.

resource "aws_iam_group" "developers" {
  name = "seslogin-developers"
}

resource "aws_iam_group_policy_attachment" "developers_dev" {
  group      = aws_iam_group.developers.name
  policy_arn = var.seslogin_terraform_policy_arn
}

resource "aws_iam_group_policy" "developers_self_service" {
  name  = "seslogin-developers-self-service"
  group = aws_iam_group.developers.name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Sid    = "ManageOwnAccessKeys"
      Effect = "Allow"
      Action = [
        "iam:CreateAccessKey",
        "iam:DeleteAccessKey",
        "iam:ListAccessKeys",
        "iam:UpdateAccessKey",
        "iam:GetUser",
      ]
      Resource = "arn:aws:iam::${var.aws_account_id}:user/$${aws:username}"
    }]
  })
}

# ── GitHub Actions (service account) ──────────────────────────────────────────

resource "aws_iam_user" "github" {
  name = "seslogin-github"

  tags = {}

  lifecycle {
    ignore_changes = [tags]
  }
}

resource "aws_iam_policy" "github_lambda_deploy" {
  name        = "seslogin-github-lambda-deploy"
  description = "Allows GitHub Actions to deploy the seslogin Lambda functions"

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = [
        "lambda:GetFunction",
        "lambda:GetFunctionConfiguration",
        "lambda:UpdateFunctionCode",
        "lambda:UpdateFunctionConfiguration",
      ]
      Resource = [
        aws_lambda_function.api.arn,
        aws_lambda_function.preprod_api.arn,
        aws_lambda_function.sync_members.arn,
        aws_lambda_function.dispatcher.arn,
        aws_lambda_function.checker.arn,
        aws_lambda_function.test_api.arn,
        aws_lambda_function.nitc_export.arn,
        aws_lambda_function.healthcheck.arn,
        aws_lambda_function.activity_summary.arn,
        aws_lambda_function.sync_locations.arn,
      ]
    }]
  })
}

resource "aws_iam_user_policy_attachment" "github_lambda_deploy" {
  user       = aws_iam_user.github.name
  policy_arn = aws_iam_policy.github_lambda_deploy.arn
}

resource "aws_iam_user_policy" "github_s3_deploy" {
  name = "seslogin-github-s3-deploy"
  user = aws_iam_user.github.name
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = ["s3:ListBucket"]
        Resource = [
          aws_s3_bucket.prod_web.arn,
          aws_s3_bucket.preprod_web.arn,
          aws_s3_bucket.test_web.arn,
        ]
      },
      {
        Effect = "Allow"
        Action = ["s3:PutObject", "s3:DeleteObject", "s3:GetObject"]
        Resource = [
          "${aws_s3_bucket.prod_web.arn}/*",
          "${aws_s3_bucket.preprod_web.arn}/*",
          "${aws_s3_bucket.test_web.arn}/*",
        ]
      },
    ]
  })
}

resource "aws_iam_user_policy" "github_cloudfront" {
  name = "seslogin-github-cloudfront"
  user = aws_iam_user.github.name
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = ["cloudfront:CreateInvalidation"]
      Resource = [
        aws_cloudfront_distribution.prod.arn,
        aws_cloudfront_distribution.preprod.arn,
        aws_cloudfront_distribution.test.arn,
      ]
    }]
  })
}
