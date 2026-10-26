resource "aws_iam_role" "test_api_lambda" {
  name               = "seslogin-test-api-lambda-role"
  assume_role_policy = data.aws_iam_policy_document.lambda_assume_role.json
}

resource "aws_iam_role_policy_attachment" "test_api_lambda_logs" {
  role       = aws_iam_role.test_api_lambda.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

# No SQS access: the test API runs on the test tables and has no queues
# configured (see lambda_api_test.tf), so it can't hand prod's workers an ID
# from the wrong database.
