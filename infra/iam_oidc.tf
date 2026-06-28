# ── GitHub Actions OIDC deploy role ───────────────────────────────────────────
# CI assumes this role via GitHub OIDC (no long-lived access keys). Scoped to
# exactly what the deploy workflows need: update Lambda code, sync the web S3
# buckets, and invalidate CloudFront.

resource "aws_iam_openid_connect_provider" "github" {
  url             = "https://token.actions.githubusercontent.com"
  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = ["6938fd4d98bab03faadb97b34396831e3780aea1"]
}

resource "aws_iam_role" "github_deploy" {
  name = "seslogin-github-deploy"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Federated = aws_iam_openid_connect_provider.github.arn }
      Action    = "sts:AssumeRoleWithWebIdentity"
      Condition = {
        StringEquals = { "token.actions.githubusercontent.com:aud" = "sts.amazonaws.com" }
        StringLike   = { "token.actions.githubusercontent.com:sub" = "repo:NSWSESMembers/seslogin:*" }
      }
    }]
  })
}

resource "aws_iam_role_policy" "github_lambda_deploy" {
  name = "seslogin-github-lambda-deploy"
  role = aws_iam_role.github_deploy.id
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

resource "aws_iam_role_policy" "github_s3_deploy" {
  name = "seslogin-github-s3-deploy"
  role = aws_iam_role.github_deploy.id
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["s3:ListBucket"]
        Resource = [aws_s3_bucket.prod_web.arn, aws_s3_bucket.preprod_web.arn, aws_s3_bucket.test_web.arn]
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

resource "aws_iam_role_policy" "github_cloudfront" {
  name = "seslogin-github-cloudfront"
  role = aws_iam_role.github_deploy.id
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = ["cloudfront:CreateInvalidation"]
      Resource = [aws_cloudfront_distribution.prod.arn, aws_cloudfront_distribution.preprod.arn, aws_cloudfront_distribution.test.arn]
    }]
  })
}

output "github_deploy_role_arn" {
  description = "Set as the AWS_DEPLOY_ROLE_ARN GitHub repo variable for the OIDC workflows."
  value       = aws_iam_role.github_deploy.arn
}
