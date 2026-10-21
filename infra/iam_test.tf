resource "aws_iam_role" "test_api_lambda" {
  name               = "seslogin-test-api-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "test_api_lambda_logs" {
  role       = aws_iam_role.test_api_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy" "test_api_lambda_sqs_send" {
  name = "sqs-send"
  role = aws_iam_role.test_api_lambda.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Action = ["sqs:SendMessage"]
      Resource = [
        aws_sqs_queue.member_sync.arn,
        aws_sqs_queue.nitc_export.arn,
      ]
    }]
  })
}
