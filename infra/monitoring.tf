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
            ["Seslogin/API", "RequestSuccess", { label = "success (2xx-4xx)", stat = "Sum", color = "#2ca02c" }],
            ["Seslogin/API", "RequestFailure", { label = "failure (5xx)", stat = "Sum", color = "#d62728" }],
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
          title  = "GraphQL Field Failures (per minute)"
          region = "ap-southeast-2"
          metrics = [
            ["Seslogin/API", "QueryFailure", { label = "query failures", stat = "Sum" }],
            ["Seslogin/API", "MutationFailure", { label = "mutation failures", stat = "Sum" }],
          ]
          period = 60
          view   = "timeSeries"
        }
      },
      {
        # Latency and DynamoDB usage are no longer custom metrics — they live as fields on the
        # structured `api_request` log lines. This widget derives them via Logs Insights.
        type   = "log"
        x      = 0
        y      = 6
        width  = 24
        height = 6
        properties = {
          title  = "Latency & DynamoDB usage (from api_request logs, prod + preprod)"
          region = "ap-southeast-2"
          query  = "SOURCE '/aws/lambda/seslogin-api' SOURCE '/aws/lambda/seslogin-preprod-api' | filter log_type = 'api_request' | stats avg(latency_ms) as avg_ms, pct(latency_ms, 95) as p95_ms, sum(rru) as read_units, sum(wru) as write_units, sum(ddb_calls) as ddb_calls by bin(1m)"
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
