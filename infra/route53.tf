# ── seslogin.com hosted zone (new account) ────────────────────────────────────
# Created fresh in the new account. Domain registration stays with the external
# registrar; the NS delegation is repointed manually to this zone's nameservers
# (see output zone_nameservers). The records below replicate the old zone so the
# NS flip is zero-downtime. App aliases point at the OLD account's CloudFront
# (var.app_cf_domain_*) until the Phase 7 cutover, when those vars are flipped to
# the new distributions.
resource "aws_route53_zone" "seslogin" {
  name = "seslogin.com"
}

locals {
  # CloudFront's fixed hosted-zone id for Route53 alias targets (global, all distributions).
  cf_alias_zone_id = "Z2FDTNDATAQYW2"

  # Per-env alias target: the NEW distribution once that env cuts over, otherwise
  # the OLD account's CloudFront (var.app_cf_domain_*) so the live site is unchanged.
  prod_alias_target    = var.cutover_prod ? aws_cloudfront_distribution.prod.domain_name : var.app_cf_domain_prod
  preprod_alias_target = var.cutover_preprod ? aws_cloudfront_distribution.preprod.domain_name : var.app_cf_domain_preprod
  test_alias_target    = var.cutover_test ? aws_cloudfront_distribution.test.domain_name : var.app_cf_domain_test
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

# test.seslogin.com had an A alias only (no AAAA) in the old zone.
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

# ── Email: forwardemail.net inbound forwarding + DMARC ────────────────────────
resource "aws_route53_record" "apex_mx" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "seslogin.com"
  type    = "MX"
  ttl     = 300
  records = ["10 mx1.forwardemail.net", "20 mx2.forwardemail.net"]
}

resource "aws_route53_record" "apex_txt" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "seslogin.com"
  type    = "TXT"
  ttl     = 300
  records = ["forward-email=alerts:seslogin@sdunster.com"]
}

resource "aws_route53_record" "dmarc" {
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = "_dmarc.seslogin.com"
  type    = "TXT"
  ttl     = 300
  records = ["v=DMARC1; p=none;"]
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
