terraform {
  required_providers {
    aws = {
      source = "hashicorp/aws"
      # 6.37+ for GSI `key_schema` (6.29) without its perpetual-diff (6.32.1) and
      # GSI-removal (6.37.0) bugs.
      version = ">= 6.37, < 7.0"
    }
  }
  required_version = ">= 1.5"
}

provider "aws" {
  region  = "ap-southeast-2"
  profile = var.aws_profile
}

locals {
  account_id = var.aws_account_id
  region     = "ap-southeast-2"
}
