resource "aws_acm_certificate" "qa" {
  provider          = aws.us_east_1
  domain_name       = "qa.seslogin.com"
  validation_method = "DNS"
  lifecycle { create_before_destroy = true }
}

resource "aws_route53_record" "qa_cert_validation" {
  for_each = {
    for dvo in aws_acm_certificate.qa.domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      type   = dvo.resource_record_type
      record = dvo.resource_record_value
    }
  }
  zone_id = aws_route53_zone.seslogin.zone_id
  name    = each.value.name
  type    = each.value.type
  ttl     = 300
  records = [each.value.record]
}

resource "aws_acm_certificate_validation" "qa" {
  provider                = aws.us_east_1
  certificate_arn         = aws_acm_certificate.qa.arn
  validation_record_fqdns = [for r in aws_route53_record.qa_cert_validation : r.fqdn]
}
