variable "jwt_secret" {
  description = "JWT signing secret"
  type        = string
  sensitive   = true
}

variable "ses_api_key" {
  description = "SES API key for the external headquarters system"
  type        = string
  sensitive   = true
}

variable "ses_api_base_url" {
  description = "Base URL for the SES API"
  type        = string
}

variable "aws_account_id" {
  description = "AWS account ID for constructing ARNs (must be set explicitly)"
  type        = string
}

variable "aws_profile" {
  description = "AWS CLI/SSO profile Terraform uses for all providers"
  type        = string
  default     = "seslogin-new"
}

variable "jwt_secret_test" {
  description = "JWT signing secret for the test environment"
  type        = string
  sensitive   = true
}

variable "ses_api_key_test" {
  description = "SES API key for the test environment"
  type        = string
  sensitive   = true
}

variable "turnstile_secret_key" {
  description = "Cloudflare Turnstile secret key for the production environment"
  type        = string
  sensitive   = true
}

variable "turnstile_secret_key_test" {
  description = "Cloudflare Turnstile secret key for the test environment"
  type        = string
  sensitive   = true
}

variable "db_prefix" {
  description = "DynamoDB table name prefix for the production environment (e.g. seslogin_prod_)"
  type        = string
  default     = "seslogin_prod"
}

variable "db_prefix_test" {
  description = "DynamoDB table name prefix for the test environment (e.g. seslogin_test_)"
  type        = string
  default     = "seslogin_test"
}

