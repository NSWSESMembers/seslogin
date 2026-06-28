terraform {
  backend "s3" {
    bucket  = "seslogin-terraform-state-641079927221"
    key     = "seslogin/terraform.tfstate"
    region  = "ap-southeast-2"
    encrypt = true
    profile = "seslogin-new"
  }
}
