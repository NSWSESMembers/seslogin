resource "aws_s3_bucket" "preprod_web" {
  bucket = "seslogin-preprod-web-641079927221"
}

resource "aws_s3_bucket_public_access_block" "preprod_web" {
  bucket                  = aws_s3_bucket.preprod_web.id
  block_public_acls       = true
  ignore_public_acls      = true
  block_public_policy     = true
  restrict_public_buckets = true
}

resource "aws_cloudfront_origin_access_control" "preprod_web" {
  name                              = "seslogin-preprod-web"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

resource "aws_s3_bucket_policy" "preprod_web" {
  bucket = aws_s3_bucket.preprod_web.id
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Sid       = "AllowCloudFrontOAC"
      Effect    = "Allow"
      Principal = { Service = "cloudfront.amazonaws.com" }
      Action    = "s3:GetObject"
      Resource  = "arn:aws:s3:::preprod.seslogin.com/*"
      Condition = { StringEquals = {
        "AWS:SourceArn" = aws_cloudfront_distribution.preprod.arn
      } }
    }]
  })
}

resource "aws_cloudfront_distribution" "preprod" {
  aliases             = ["preprod.seslogin.com"]
  enabled             = true
  http_version        = "http2"
  is_ipv6_enabled     = true
  price_class         = "PriceClass_All"
  default_root_object = "index.html"

  origin {
    origin_id                = "preprod-web-s3"
    domain_name              = aws_s3_bucket.preprod_web.bucket_regional_domain_name
    origin_access_control_id = aws_cloudfront_origin_access_control.preprod_web.id
  }

  default_cache_behavior {
    target_origin_id       = "preprod-web-s3"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["GET", "HEAD"]
    cached_methods         = ["GET", "HEAD"]
    compress               = true
    min_ttl                = 0
    default_ttl            = 300
    max_ttl                = 300
    forwarded_values {
      query_string = false
      cookies { forward = "none" }
    }
  }

  ordered_cache_behavior {
    path_pattern           = "/assets/*"
    target_origin_id       = "preprod-web-s3"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["GET", "HEAD"]
    cached_methods         = ["GET", "HEAD"]
    compress               = true
    cache_policy_id        = "658327ea-f89d-4fab-a63d-7e88639e58f6"
  }

  # OAC returns 403 (not 404) for missing S3 keys — catch both for SPA routing
  custom_error_response {
    error_code            = 403
    response_code         = 200
    response_page_path    = "/index.html"
    error_caching_min_ttl = 10
  }
  custom_error_response {
    error_code            = 404
    response_code         = 200
    response_page_path    = "/index.html"
    error_caching_min_ttl = 10
  }

  restrictions {
    geo_restriction { restriction_type = "none" }
  }

  viewer_certificate {
    acm_certificate_arn      = aws_acm_certificate_validation.preprod.certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }
}

resource "aws_route53_record" "preprod_a" {
  zone_id = data.aws_route53_zone.seslogin.zone_id
  name    = "preprod.seslogin.com"
  type    = "A"
  alias {
    name                   = aws_cloudfront_distribution.preprod.domain_name
    zone_id                = aws_cloudfront_distribution.preprod.hosted_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "preprod_aaaa" {
  zone_id = data.aws_route53_zone.seslogin.zone_id
  name    = "preprod.seslogin.com"
  type    = "AAAA"
  alias {
    name                   = aws_cloudfront_distribution.preprod.domain_name
    zone_id                = aws_cloudfront_distribution.preprod.hosted_zone_id
    evaluate_target_health = false
  }
}
