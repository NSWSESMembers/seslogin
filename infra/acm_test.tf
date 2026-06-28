resource "aws_acm_certificate" "test" {
  count             = local.create_certs ? 1 : 0
  provider          = aws.us_east_1
  domain_name       = "test.seslogin.com"
  validation_method = "DNS"
  lifecycle { create_before_destroy = true }
}

resource "aws_route53_record" "test_cert_validation" {
  for_each = local.create_certs ? {
    for dvo in aws_acm_certificate.test[0].domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      type   = dvo.resource_record_type
      record = dvo.resource_record_value
    }
  } : {}
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = each.value.name
  type    = each.value.type
  ttl     = 300
  records = [each.value.record]
}

resource "aws_acm_certificate_validation" "test" {
  count                   = local.create_certs ? 1 : 0
  provider                = aws.us_east_1
  certificate_arn         = aws_acm_certificate.test[0].arn
  validation_record_fqdns = [for r in aws_route53_record.test_cert_validation : r.fqdn]
}
