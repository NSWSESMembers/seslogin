# Temporary cross-account SES bridge (while this account's SES production access
# is pending). Lets the email-sending Lambdas assume var.ses_role_arn in the
# production-SES (old) account so email is sent by that account, out of sandbox.
# Only created while var.ses_role_arn is set; remove the tfvars value to tear
# down once this account has production SES.

locals {
  ses_sender_roles = var.ses_role_arn == "" ? {} : {
    api              = aws_iam_role.api_lambda.id
    preprod_api      = aws_iam_role.preprod_api_lambda.id
    test_api         = aws_iam_role.test_api_lambda.id
    activity_summary = aws_iam_role.activity_summary_lambda.id
  }
}

resource "aws_iam_role_policy" "ses_assume_role" {
  for_each = local.ses_sender_roles
  name     = "ses-assume-role"
  role     = each.value

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect   = "Allow"
      Action   = "sts:AssumeRole"
      Resource = var.ses_role_arn
    }]
  })
}
