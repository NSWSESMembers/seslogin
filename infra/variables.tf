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

# ── Migration toggles ─────────────────────────────────────────────────────────
# Lets us stand up everything except ACM certs before the registrar NS flip
# (cert DNS-validation needs the new zone authoritative), then flip the app
# aliases/cert onto the new distributions at the Phase 7 cutover.
variable "enable_certs" {
  description = "Create the ACM certs + DNS validation (set true once NS delegation has propagated to the new zone)."
  type        = bool
  default     = false
}

# Background workers (member sync, dispatcher, checker, nitc-export, healthcheck,
# activity-summary, location-sync) via their EventBridge schedules + SQS event
# source mappings. Disabled during new-account testing so we don't sync members,
# export to NITC, send summary emails, or run healthchecks. Flip to true at the
# prod cutover. The 3 API servers are unaffected.
variable "background_jobs_enabled" {
  description = "Enable the worker lambdas' schedules + SQS triggers. false during testing; true at prod cutover."
  type        = bool
  default     = true
}

# Per-environment cutover. When an env's flag is true, that distribution gets its
# real alias(es) + ACM cert and the zone's app record points at the new CF; when
# false it serves the default cert with no alias and the record points at the old
# account's CF (var.app_cf_domain_*). Lets us cut over test -> preprod -> prod
# independently. Any of these implies enable_certs.
variable "cutover_prod" {
  description = "Cut seslogin.com / new.seslogin.com over to the new prod CloudFront."
  type        = bool
  default     = false
}

variable "cutover_preprod" {
  description = "Cut preprod.seslogin.com over to the new preprod CloudFront."
  type        = bool
  default     = false
}

variable "cutover_test" {
  description = "Cut test.seslogin.com over to the new test CloudFront."
  type        = bool
  default     = false
}

# ── App DNS cutover targets ───────────────────────────────────────────────────
# The prod/preprod/test app aliases point at these CloudFront domains. Defaults
# are the OLD account's distributions so the live site is unchanged while we
# delegate DNS early (zero downtime). At Phase 7 cutover, override these in
# terraform.tfvars with the NEW distributions' domain names, then apply.
variable "app_cf_domain_prod" {
  description = "CloudFront domain for seslogin.com / new.seslogin.com (old account until cutover)"
  type        = string
  default     = "d2pn13262vttbm.cloudfront.net"
}

variable "app_cf_domain_preprod" {
  description = "CloudFront domain for preprod.seslogin.com (old account until cutover)"
  type        = string
  default     = "d3jwxg5r3n1e4d.cloudfront.net"
}

variable "app_cf_domain_test" {
  description = "CloudFront domain for test.seslogin.com (old account until cutover)"
  type        = string
  default     = "di0xztg8lcjxs.cloudfront.net"
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

