resource "aws_sns_topic" "member_sync_alerts" {
  name = "seslogin-member-sync-alerts"
}

resource "aws_sns_topic_subscription" "member_sync_email" {
  topic_arn = aws_sns_topic.member_sync_alerts.arn
  protocol  = "email"
  endpoint  = "alerts@seslogin.com"
}

resource "aws_cloudwatch_dashboard" "api" {
  dashboard_name = "seslogin-api"
  dashboard_body = jsonencode({
    widgets = [
      {
        type   = "metric"
        x      = 0
        y      = 0
        width  = 12
        height = 6
        properties = {
          title  = "Request Outcomes (per minute)"
          region = "ap-southeast-2"
          metrics = [
            ["Seslogin/API", "RequestCount", "OperationType", "query", { label = "requests (query)", stat = "Sum" }],
            ["Seslogin/API", "RequestCount", "OperationType", "mutation", { label = "requests (mutation)", stat = "Sum" }],
            ["Seslogin/API", "SuccessCount", "OperationType", "query", { label = "success (query)", stat = "Sum" }],
            ["Seslogin/API", "SuccessCount", "OperationType", "mutation", { label = "success (mutation)", stat = "Sum" }],
          ]
          period = 60
          view   = "timeSeries"
        }
      },
      {
        type   = "metric"
        x      = 12
        y      = 0
        width  = 12
        height = 6
        properties = {
          title  = "Errors (per minute)"
          region = "ap-southeast-2"
          metrics = [
            ["Seslogin/API", "AuthErrorCount", "OperationType", "unknown", { label = "auth errors (401)", stat = "Sum" }],
            ["Seslogin/API", "ServerErrorCount", "OperationType", "unknown", { label = "server errors (503)", stat = "Sum" }],
            ["Seslogin/API", "GraphQLErrorCount", "OperationType", "query", { label = "graphql errors (query)", stat = "Sum" }],
            ["Seslogin/API", "GraphQLErrorCount", "OperationType", "mutation", { label = "graphql errors (mutation)", stat = "Sum" }],
          ]
          period = 60
          view   = "timeSeries"
        }
      },
      {
        type   = "metric"
        x      = 0
        y      = 6
        width  = 12
        height = 6
        properties = {
          title  = "Latency (ms)"
          region = "ap-southeast-2"
          metrics = [
            ["Seslogin/API", "LatencyMs", "OperationType", "query", { label = "p50 query", stat = "p50" }],
            ["Seslogin/API", "LatencyMs", "OperationType", "query", { label = "p95 query", stat = "p95" }],
            ["Seslogin/API", "LatencyMs", "OperationType", "mutation", { label = "p50 mutation", stat = "p50" }],
            ["Seslogin/API", "LatencyMs", "OperationType", "mutation", { label = "p95 mutation", stat = "p95" }],
          ]
          period = 60
          view   = "timeSeries"
        }
      },
      {
        type   = "metric"
        x      = 12
        y      = 6
        width  = 12
        height = 6
        properties = {
          title  = "DynamoDB Capacity Units (per minute)"
          region = "ap-southeast-2"
          metrics = [
            ["Seslogin/API", "DynamoDBReadUnits", "OperationType", "query", { label = "read units (query)", stat = "Sum" }],
            ["Seslogin/API", "DynamoDBReadUnits", "OperationType", "mutation", { label = "read units (mutation)", stat = "Sum" }],
            ["Seslogin/API", "DynamoDBWriteUnits", "OperationType", "mutation", { label = "write units (mutation)", stat = "Sum" }],
          ]
          period = 60
          view   = "timeSeries"
        }
      },
    ]
  })
}

resource "aws_cloudwatch_metric_alarm" "dlq_not_empty" {
  alarm_name          = "seslogin-member-sync-dlq-not-empty"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 1
  metric_name         = "ApproximateNumberOfMessagesVisible"
  namespace           = "AWS/SQS"
  period              = 300
  statistic           = "Sum"
  threshold           = 0
  alarm_description   = "Member sync DLQ has messages — syncs are failing"
  treat_missing_data  = "notBreaching"

  alarm_actions = [aws_sns_topic.member_sync_alerts.arn]
  ok_actions    = [aws_sns_topic.member_sync_alerts.arn]

  dimensions = {
    QueueName = aws_sqs_queue.member_sync_dlq.name
  }
}
