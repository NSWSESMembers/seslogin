output "nitc_queue_url" {
  description = "URL of the NITC export SQS queue"
  value       = aws_sqs_queue.nitc_export.url
}

output "member_sync_queue_url" {
  description = "URL of the member sync SQS queue"
  value       = aws_sqs_queue.member_sync.url
}

output "member_sync_dlq_url" {
  description = "URL of the member sync dead-letter queue"
  value       = aws_sqs_queue.member_sync_dlq.url
}

output "api_function_url" {
  description = "HTTPS endpoint for the seslogin-api Lambda function"
  value       = aws_lambda_function_url.api.function_url
}

output "api_lambda_arn" {
  description = "ARN of the seslogin-api Lambda function"
  value       = aws_lambda_function.api.arn
}

output "sync_lambda_arn" {
  description = "ARN of the seslogin-sync-members Lambda function"
  value       = aws_lambda_function.sync_members.arn
}

output "dispatcher_lambda_arn" {
  description = "ARN of the seslogin-dispatcher Lambda function"
  value       = aws_lambda_function.dispatcher.arn
}

output "alerts_sns_topic_arn" {
  description = "ARN of the SNS topic for member sync failure alerts"
  value       = aws_sns_topic.member_sync_alerts.arn
}

output "test_api_lambda_function_url" {
  description = "HTTPS endpoint for the seslogin-test-api Lambda function"
  value       = aws_lambda_function_url.test_api.function_url
}

output "preprod_api_lambda_function_url" {
  description = "HTTPS endpoint for the seslogin-preprod-api Lambda function"
  value       = aws_lambda_function_url.preprod_api.function_url
}

output "preprod_cloudfront_distribution_id" {
  description = "CloudFront distribution ID for preprod.seslogin.com (hard-coded in deploy-preprod.yml)"
  value       = aws_cloudfront_distribution.preprod.id
}

output "qa_api_lambda_function_url" {
  description = "HTTPS endpoint for the seslogin-qa-api Lambda function"
  value       = aws_lambda_function_url.qa_api.function_url
}

output "qa_cloudfront_distribution_id" {
  description = "CloudFront distribution ID for qa.seslogin.com (hard-coded in deploy-qa.yml)"
  value       = aws_cloudfront_distribution.qa.id
}

output "test_cloudfront_distribution_id" {
  description = "CloudFront distribution ID for test.seslogin.com"
  value       = aws_cloudfront_distribution.test.id
}

output "prod_cloudfront_distribution_id" {
  description = "CloudFront distribution ID for seslogin.com / new.seslogin.com"
  value       = aws_cloudfront_distribution.prod.id
}
