# Uploaded assets (photos now, other files later). Private: nothing is served from
# these buckets directly. Clients upload with a presigned POST to pending/<id>, the
# API copies the object to originals/<id>, and derived (resized) images are written
# under derived/<id>/ by the media code. See SCHEMA.md ("asset").
#
# One bucket for prod, preprod, qa and the workers (they share the prod tables), and
# one for test, which must not be able to reach prod data. No versioning.

resource "aws_s3_bucket" "assets_prod" {
  bucket = "seslogin-assets-prod-641079927221"
}

resource "aws_s3_bucket" "assets_test" {
  bucket = "seslogin-assets-test-641079927221"
}

resource "aws_s3_bucket_public_access_block" "assets_prod" {
  bucket                  = aws_s3_bucket.assets_prod.id
  block_public_acls       = true
  ignore_public_acls      = true
  block_public_policy     = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_public_access_block" "assets_test" {
  bucket                  = aws_s3_bucket.assets_test.id
  block_public_acls       = true
  ignore_public_acls      = true
  block_public_policy     = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_server_side_encryption_configuration" "assets_prod" {
  bucket = aws_s3_bucket.assets_prod.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "assets_test" {
  bucket = aws_s3_bucket.assets_test.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

resource "aws_s3_bucket_ownership_controls" "assets_prod" {
  bucket = aws_s3_bucket.assets_prod.id
  rule {
    object_ownership = "BucketOwnerEnforced"
  }
}

resource "aws_s3_bucket_ownership_controls" "assets_test" {
  bucket = aws_s3_bucket.assets_test.id
  rule {
    object_ownership = "BucketOwnerEnforced"
  }
}

# Uploads that were never completed are removed after a day (the asset row's TTL
# does the same for its record), as are abandoned multipart uploads.
resource "aws_s3_bucket_lifecycle_configuration" "assets_prod" {
  bucket = aws_s3_bucket.assets_prod.id

  rule {
    id     = "expire-pending"
    status = "Enabled"
    filter {
      prefix = "pending/"
    }
    expiration {
      days = 1
    }
  }

  rule {
    id     = "abort-incomplete-multipart"
    status = "Enabled"
    filter {}
    abort_incomplete_multipart_upload {
      days_after_initiation = 1
    }
  }
}

resource "aws_s3_bucket_lifecycle_configuration" "assets_test" {
  bucket = aws_s3_bucket.assets_test.id

  rule {
    id     = "expire-pending"
    status = "Enabled"
    filter {
      prefix = "pending/"
    }
    expiration {
      days = 1
    }
  }

  rule {
    id     = "abort-incomplete-multipart"
    status = "Enabled"
    filter {}
    abort_incomplete_multipart_upload {
      days_after_initiation = 1
    }
  }
}

# ── API role access ───────────────────────────────────────────────────────────
# The API presigns uploads into pending/, then on completion reads the object back,
# copies it to originals/ (CopyObject needs GetObject on the source and PutObject on
# the destination) and deletes the pending one. Derived images are the media
# Lambda's business, not the API's.

resource "aws_iam_role_policy" "api_lambda_assets" {
  name = "assets-access"
  role = aws_iam_role.api_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["s3:PutObject", "s3:GetObject", "s3:DeleteObject"]
        Resource = "${aws_s3_bucket.assets_prod.arn}/pending/*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:PutObject", "s3:GetObject"]
        Resource = "${aws_s3_bucket.assets_prod.arn}/originals/*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:ListBucket"]
        Resource = aws_s3_bucket.assets_prod.arn
      },
    ]
  })
}

resource "aws_iam_role_policy" "preprod_api_lambda_assets" {
  name = "assets-access"
  role = aws_iam_role.preprod_api_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["s3:PutObject", "s3:GetObject", "s3:DeleteObject"]
        Resource = "${aws_s3_bucket.assets_prod.arn}/pending/*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:PutObject", "s3:GetObject"]
        Resource = "${aws_s3_bucket.assets_prod.arn}/originals/*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:ListBucket"]
        Resource = aws_s3_bucket.assets_prod.arn
      },
    ]
  })
}

resource "aws_iam_role_policy" "qa_api_lambda_assets" {
  name = "assets-access"
  role = aws_iam_role.qa_api_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["s3:PutObject", "s3:GetObject", "s3:DeleteObject"]
        Resource = "${aws_s3_bucket.assets_prod.arn}/pending/*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:PutObject", "s3:GetObject"]
        Resource = "${aws_s3_bucket.assets_prod.arn}/originals/*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:ListBucket"]
        Resource = aws_s3_bucket.assets_prod.arn
      },
    ]
  })
}

resource "aws_iam_role_policy" "test_api_lambda_assets" {
  name = "assets-access"
  role = aws_iam_role.test_api_lambda.id

  # The test bucket only: test.seslogin.com must not be able to reach prod uploads.
  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect   = "Allow"
        Action   = ["s3:PutObject", "s3:GetObject", "s3:DeleteObject"]
        Resource = "${aws_s3_bucket.assets_test.arn}/pending/*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:PutObject", "s3:GetObject"]
        Resource = "${aws_s3_bucket.assets_test.arn}/originals/*"
      },
      {
        Effect   = "Allow"
        Action   = ["s3:ListBucket"]
        Resource = aws_s3_bucket.assets_test.arn
      },
    ]
  })
}
