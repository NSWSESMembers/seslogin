# SES delivery events, one log line per event, so a member's "I never got the
# email" can be answered: delivered (with the receiving server's SMTP response),
# bounced, rejected, delayed or complained about. Searchable by recipient in the
# `/aws/events/seslogin-ses-events` log group, and by the `message_id` the API
# logs for each send ("SES accepted email").
#
# SES → default EventBridge bus → CloudWatch Logs. Deliberately not the
# CloudWatch *metrics* event destination, which bills a custom metric per
# dimension combination, and not reputation metrics either. EventBridge doesn't
# charge for AWS service events, so the only cost is log ingestion and storage.
#
# Every environment sends from the one seslogin.com identity in this account, so
# every environment's mail lands here; the `ses:caller-identity` tag in
# `detail.mail.tags` (the sending Lambda's role) tells them apart.
#
# What this can't show: anything after the receiving server accepts the message.
# Junk filtering and quarantine are visible only in the recipient's own message
# trace, which the `Message-ID` header (`<message_id@ap-southeast-2.amazonses.com>`)
# lets their admins search.

resource "aws_sesv2_configuration_set" "default" {
  configuration_set_name = "seslogin-default"
}

resource "aws_sesv2_configuration_set_event_destination" "eventbridge" {
  configuration_set_name = aws_sesv2_configuration_set.default.configuration_set_name
  event_destination_name = "eventbridge"

  event_destination {
    enabled = true
    # Never OPEN or CLICK: enabling those makes SES add a tracking pixel and
    # rewrite every link in every message. SEND is left out too: the API already
    # logs each accepted send with its message ID.
    matching_event_types = [
      "REJECT",
      "BOUNCE",
      "COMPLAINT",
      "DELIVERY",
      "DELIVERY_DELAY",
      "RENDERING_FAILURE",
    ]

    event_bridge_destination {
      event_bus_arn = "arn:aws:events:${local.region}:${local.account_id}:event-bus/default"
    }
  }
}

# The sending identity, created by hand before this file existed, adopted here
# only to make the configuration set its default, so every message from
# seslogin.com publishes events without the code naming the set on each send.
# DKIM (Easy DKIM, 2048-bit) and the mail.seslogin.com MAIL FROM domain stay
# managed outside Terraform. Destroying this resource would delete the identity
# and stop all mail, hence prevent_destroy.
import {
  to = aws_sesv2_email_identity.seslogin
  id = "seslogin.com"
}

resource "aws_sesv2_email_identity" "seslogin" {
  email_identity         = "seslogin.com"
  configuration_set_name = aws_sesv2_configuration_set.default.configuration_set_name

  lifecycle {
    prevent_destroy = true
  }
}

resource "aws_cloudwatch_log_group" "ses_events" {
  name = "/aws/events/seslogin-ses-events"
  # Long enough to look into a complaint about last month's email; the events
  # hold recipient addresses, so not forever.
  retention_in_days = 90
}

resource "aws_cloudwatch_event_rule" "ses_events" {
  name        = "seslogin-ses-events"
  description = "SES delivery events from the seslogin-default configuration set, to CloudWatch Logs"
  event_pattern = jsonencode({
    source = ["aws.ses"]
    detail = {
      mail = {
        tags = {
          "ses:configuration-set" = [aws_sesv2_configuration_set.default.configuration_set_name]
        }
      }
    }
  })
}

resource "aws_cloudwatch_event_target" "ses_events" {
  rule = aws_cloudwatch_event_rule.ses_events.name
  arn  = aws_cloudwatch_log_group.ses_events.arn
}

# EventBridge writes to a log group through the log group's resource policy, not
# an IAM role.
resource "aws_cloudwatch_log_resource_policy" "ses_events" {
  policy_name = "seslogin-ses-events"
  policy_document = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect    = "Allow"
      Principal = { Service = ["events.amazonaws.com", "delivery.logs.amazonaws.com"] }
      Action    = ["logs:CreateLogStream", "logs:PutLogEvents"]
      Resource  = "${aws_cloudwatch_log_group.ses_events.arn}:*"
      Condition = {
        ArnEquals = { "aws:SourceArn" = aws_cloudwatch_event_rule.ses_events.arn }
      }
    }]
  })
}
