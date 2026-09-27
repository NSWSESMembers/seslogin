resource "aws_s3_bucket" "test_web" {
  bucket = "seslogin-test-web-641079927221"
}

resource "aws_s3_bucket_public_access_block" "test_web" {
  bucket                  = aws_s3_bucket.test_web.id
  block_public_acls       = true
  ignore_public_acls      = true
  block_public_policy     = true
  restrict_public_buckets = true
}

resource "aws_cloudfront_origin_access_control" "test_web" {
  name                              = "seslogin-test-web"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

resource "aws_s3_bucket_policy" "test_web" {
  bucket = aws_s3_bucket.test_web.id
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Sid       = "AllowCloudFrontOAC"
      Effect    = "Allow"
      Principal = { Service = "cloudfront.amazonaws.com" }
      Action    = "s3:GetObject"
      Resource  = "${aws_s3_bucket.test_web.arn}/*"
      Condition = { StringEquals = {
        "AWS:SourceArn" = aws_cloudfront_distribution.test.arn
      } }
    }]
  })
}

# Forwards exactly the headers the API reads. Host must NOT be forwarded: a
# Function URL rejects any request whose Host isn't its own domain.
# CloudFront-Viewer-Address carries the real client IP (the Function URL's own
# source IP is a CloudFront edge server); User-Agent would otherwise arrive as
# "Amazon CloudFront". Authorization can't be listed in an origin request
# policy, and needn't be: CloudFront passes it through unmodified on POST, and
# every API call is a POST (GET only serves GraphiQL, which is unauthenticated).
resource "aws_cloudfront_origin_request_policy" "api" {
  name = "seslogin-api"

  headers_config {
    header_behavior = "whitelist"
    headers {
      items = [
        "Content-Type",
        "User-Agent",
        "X-Client-Version",
        "X-Client-Info",
        "CloudFront-Viewer-Address",
      ]
    }
  }
  cookies_config {
    cookie_behavior = "none"
  }
  query_strings_config {
    query_string_behavior = "none"
  }
}

resource "aws_cloudfront_distribution" "test" {
  aliases             = ["test.seslogin.com"]
  enabled             = true
  http_version        = "http2"
  is_ipv6_enabled     = true
  price_class         = "PriceClass_All"
  default_root_object = "index.html"

  origin {
    origin_id                = "test-web-s3"
    domain_name              = aws_s3_bucket.test_web.bucket_regional_domain_name
    origin_access_control_id = aws_cloudfront_origin_access_control.test_web.id
  }

  # The API, served same-origin at /graphql so the browser needs no CORS
  # preflight. The Function URL is still public (AuthType=NONE) and keeps its
  # own CORS config, so builds still calling it directly keep working.
  origin {
    origin_id   = "test-api-lambda"
    domain_name = trimsuffix(trimprefix(aws_lambda_function_url.test_api.function_url, "https://"), "/")
    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
    }
  }

  default_cache_behavior {
    target_origin_id       = "test-web-s3"
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
    target_origin_id       = "test-web-s3"
    viewer_protocol_policy = "redirect-to-https"
    allowed_methods        = ["GET", "HEAD"]
    cached_methods         = ["GET", "HEAD"]
    compress               = true
    cache_policy_id        = "658327ea-f89d-4fab-a63d-7e88639e58f6"
  }

  # Never cached (managed CachingDisabled policy). The custom_error_response
  # blocks below are distribution-wide, so an API 403/404 would be swapped for
  # index.html: the handler never returns either (errors are 400/401/500/503),
  # and a Function URL 403 means the origin is misconfigured anyway.
  ordered_cache_behavior {
    path_pattern             = "/graphql"
    target_origin_id         = "test-api-lambda"
    viewer_protocol_policy   = "https-only"
    allowed_methods          = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods           = ["GET", "HEAD"]
    compress                 = true
    cache_policy_id          = "4135ea2d-6df8-44a3-9df3-4b5a84be39ad"
    origin_request_policy_id = aws_cloudfront_origin_request_policy.api.id
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
    acm_certificate_arn      = aws_acm_certificate_validation.test.certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }
}

# App DNS alias record (test.seslogin.com) is centralized in route53.tf.
