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
      Resource  = "${aws_s3_bucket.preprod_web.arn}/*"
      Condition = { StringEquals = {
        "AWS:SourceArn" = aws_cloudfront_distribution.preprod.arn
      } }
    }]
  })
}

# CORS for the MCP/OAuth endpoints, which browser-based MCP clients call from
# their own origin (the web app and consent page are same-origin and need none).
# Any origin is allowed: these endpoints are bearer-token/PKCE only and never
# read cookies, so there's no ambient credential for a foreign page to ride on.
# The headers come from here rather than the Function URL's CORS config because
# the viewer's Origin isn't forwarded, so that config never fires through
# CloudFront; origin_override makes this the one source either way. A policy
# only adds headers, so the handler itself answers OPTIONS with 204.
resource "aws_cloudfront_response_headers_policy" "preprod_oauth_mcp_cors" {
  name = "seslogin-preprod-oauth-mcp-cors"

  cors_config {
    access_control_allow_credentials = false
    access_control_allow_origins {
      items = ["*"]
    }
    access_control_allow_methods {
      items = ["GET", "POST", "DELETE", "OPTIONS"]
    }
    access_control_allow_headers {
      items = ["Authorization", "Content-Type", "Mcp-Protocol-Version", "Mcp-Session-Id", "Last-Event-Id"]
    }
    access_control_expose_headers {
      items = ["WWW-Authenticate", "Mcp-Session-Id", "Mcp-Protocol-Version"]
    }
    access_control_max_age_sec = 600
    origin_override            = true
  }
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

  # The API, served same-origin at /graphql so the browser needs no CORS
  # preflight, plus the MCP endpoint and its OAuth authorization server. The
  # Function URL is still public (AuthType=NONE) and keeps its own CORS config,
  # so builds still calling it directly keep working.
  origin {
    origin_id   = "preprod-api-lambda"
    domain_name = trimsuffix(trimprefix(aws_lambda_function_url.preprod_api.function_url, "https://"), "/")
    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
    }
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

  # Never cached (managed CachingDisabled policy). The origin request policy
  # forwards the API's headers but not Host, which a Function URL rejects
  # (aws_cloudfront_origin_request_policy.api, defined in web_test.tf). The
  # custom_error_response blocks below are distribution-wide, so an API 403/404
  # would be swapped for index.html: the handler never returns either (errors
  # are 400/401/500/503), and a Function URL 403 means the origin is
  # misconfigured anyway.
  ordered_cache_behavior {
    path_pattern             = "/graphql"
    target_origin_id         = "preprod-api-lambda"
    viewer_protocol_policy   = "https-only"
    allowed_methods          = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods           = ["GET", "HEAD"]
    compress                 = true
    cache_policy_id          = "4135ea2d-6df8-44a3-9df3-4b5a84be39ad"
    origin_request_policy_id = aws_cloudfront_origin_request_policy.api.id
  }

  # The MCP endpoint and its OAuth authorization server, on the same Lambda and
  # with the same settings as /graphql plus cross-origin CORS (see
  # preprod_oauth_mcp_cors). The 403/404 rewrite below is safe here too: these
  # handlers return only 200/201/202/204/400/401/405/500/503. API_BASE_URL on
  # the Lambda must name this host, since CloudFront doesn't forward Host.
  dynamic "ordered_cache_behavior" {
    for_each = ["/mcp", "/oauth/*", "/.well-known/oauth-*"]
    content {
      path_pattern               = ordered_cache_behavior.value
      target_origin_id           = "preprod-api-lambda"
      viewer_protocol_policy     = "https-only"
      allowed_methods            = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
      cached_methods             = ["GET", "HEAD"]
      compress                   = true
      cache_policy_id            = "4135ea2d-6df8-44a3-9df3-4b5a84be39ad"
      origin_request_policy_id   = aws_cloudfront_origin_request_policy.api.id
      response_headers_policy_id = aws_cloudfront_response_headers_policy.preprod_oauth_mcp_cors.id
    }
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

# App DNS alias records (preprod.seslogin.com) are centralized in route53.tf.
