terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
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
