# Human/admin access is via IAM Identity Center (SSO) — see the SesloginAdmin
# permission set (PowerUserAccess + iam:*). No IAM users/groups or custom
# seslogin-terraform policy are managed here anymore (single-tenant account).
#
# CI/CD authenticates via GitHub OIDC — the OIDC provider, deploy role, and its
# scoped policies (Lambda code update, S3 web sync, CloudFront invalidation) are
# defined in iam_oidc.tf (added in Phase 5).
