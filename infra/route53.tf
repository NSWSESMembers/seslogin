# ── seslogin.com hosted zone (new account) ────────────────────────────────────
# Domain registration stays with the external registrar; the NS delegation points
# at this zone's nameservers (see output zone_nameservers). The app aliases point
# at this account's CloudFront distributions.
resource "aws_route53_zone" "seslogin" {
  name = "seslogin.com"
}

locals {
  # CloudFront's fixed hosted-zone id for Route53 alias targets (global, all distributions).
  cf_alias_zone_id = "Z2FDTNDATAQYW2"

  prod_alias_target    = aws_cloudfront_distribution.prod.domain_name
  preprod_alias_target = aws_cloudfront_distribution.preprod.domain_name
  test_alias_target    = aws_cloudfront_distribution.test.domain_name
}

# ── App aliases ───────────────────────────────────────────────────────────────
resource "aws_route53_record" "apex_a" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "seslogin.com"
  type    = "A"
  alias {
    name                   = local.prod_alias_target
    zone_id                = local.cf_alias_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "apex_aaaa" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "seslogin.com"
  type    = "AAAA"
  alias {
    name                   = local.prod_alias_target
    zone_id                = local.cf_alias_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "new_a" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "new.seslogin.com"
  type    = "A"
  alias {
    name                   = local.prod_alias_target
    zone_id                = local.cf_alias_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "new_aaaa" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "new.seslogin.com"
  type    = "AAAA"
  alias {
    name                   = local.prod_alias_target
    zone_id                = local.cf_alias_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "preprod_a" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "preprod.seslogin.com"
  type    = "A"
  alias {
    name                   = local.preprod_alias_target
    zone_id                = local.cf_alias_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "preprod_aaaa" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "preprod.seslogin.com"
  type    = "AAAA"
  alias {
    name                   = local.preprod_alias_target
    zone_id                = local.cf_alias_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "test_a" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "test.seslogin.com"
  type    = "A"
  alias {
    name                   = local.test_alias_target
    zone_id                = local.cf_alias_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "test_aaaa" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "test.seslogin.com"
  type    = "AAAA"
  alias {
    name                   = local.test_alias_target
    zone_id                = local.cf_alias_zone_id
    evaluate_target_health = false
  }
}

# ── Email: apex MX (SES inbound) and SPF ──────────────────────────────────────
# These two records predate Terraform management and were adopted by the import
# blocks below; the import blocks can be deleted once applied everywhere.
resource "aws_route53_record" "apex_mx" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "seslogin.com"
  type    = "MX"
  ttl     = 300
  records = ["10 inbound-smtp.ap-southeast-2.amazonaws.com"]
}

import {
  to = aws_route53_record.apex_mx
  id = "${aws_route53_zone.seslogin.zone_id}_seslogin.com_MX"
}

resource "aws_route53_record" "apex_txt" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "seslogin.com"
  type    = "TXT"
  ttl     = 300
  records = ["v=spf1 include:amazonses.com ~all"]
}

import {
  to = aws_route53_record.apex_txt
  id = "${aws_route53_zone.seslogin.zone_id}_seslogin.com_TXT"
}

# ── Email: DMARC ────────────────────────────────────────────────────────────
resource "aws_route53_record" "dmarc" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "_dmarc.seslogin.com"
  type    = "TXT"
  ttl     = 300
  records = ["v=DMARC1; p=none; rua=mailto:dmarc@sdunster.com;"]
}

# ── SES custom MAIL FROM (mail.seslogin.com) ──────────────────────────────────
resource "aws_route53_record" "mail_mx" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "mail.seslogin.com"
  type    = "MX"
  ttl     = 300
  records = ["10 feedback-smtp.ap-southeast-2.amazonses.com"]
}

resource "aws_route53_record" "mail_txt" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "mail.seslogin.com"
  type    = "TXT"
  ttl     = 300
  records = ["v=spf1 include:amazonses.com ~all"]
}

# ── SES Easy DKIM (new account identity) ──────────────────────────────────────
# Tokens from the seslogin.com SES domain identity created in the new account.
resource "aws_route53_record" "ses_dkim" {
  for_each = toset([
    "zwzw7o6lnbbnoxathizzp7hvjuhz7dpj",
    "5ckhe5zfbi3knep2axo7j2euzzquravq",
    "yijf7xq3qdbqcxdrfnzfgtotwreb35ap",
  ])
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "${each.value}._domainkey.seslogin.com"
  type    = "CNAME"
  ttl     = 1800
  records = ["${each.value}.dkim.amazonses.com"]
}

output "zone_nameservers" {
  description = "Set these as the NS records for seslogin.com at the external registrar (Phase 3 cutover)."
  value       = aws_route53_zone.seslogin.name_servers
}
