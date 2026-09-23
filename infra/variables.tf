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

variable "ses_intranet_search_api_key" {
  description = "Subscription key for the SES intranet contact-directory search API (used to sync member emails)"
  type        = string
  sensitive   = true
}

variable "ses_intranet_search_api_base_url" {
  description = "Base URL for the SES intranet contact-directory search API"
  type        = string
  default     = "https://api.ses.nsw.gov.au/intranet/search"
}

variable "aws_account_id" {
  description = "AWS account ID for constructing ARNs (must be set explicitly)"
  type        = string
}

variable "aws_profile" {
  description = "AWS CLI/SSO profile Terraform uses for all providers"
  type        = string
  default     = "seslogin"
}

# Background workers (member sync, dispatcher, checker, nitc-export, healthcheck,
# activity-summary, location-sync) via their EventBridge schedules + SQS event
# source mappings. Set false to pause all background processing; the 3 API
# servers are unaffected.
variable "background_jobs_enabled" {
  description = "Enable the worker lambdas' schedules + SQS triggers."
  type        = bool
  default     = true
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

variable "ably_api_key" {
  description = "Ably API key (`keyName:keySecret`) for publishing kiosk realtime updates and signing kiosk subscribe tokens. Shared by prod, preprod and the test API (they all share the prod database and so the prod channel space, `kiosk:seslogin_prod:*`). The key's own capability must include publish and subscribe on `kiosk:*` — a token can only narrow a key's capability, never widen it. Optionally scope a non-prod key to its own prefix, e.g. `kiosk:seslogin_test:*`. Empty disables realtime (kiosks fall back to polling) rather than failing startup, so this is safe to leave unset before the key exists."
  type        = string
  sensitive   = true
  default     = ""
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

