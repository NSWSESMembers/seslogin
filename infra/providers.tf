provider "aws" {
  alias   = "us_east_1"
  region  = "us-east-1"
  profile = var.aws_profile
}

# Melbourne — cross-region copy target for DynamoDB backups (see backup.tf).
provider "aws" {
  alias   = "melbourne"
  region  = "ap-southeast-4"
  profile = var.aws_profile
}
