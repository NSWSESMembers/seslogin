# seslogin AWS account migration — status & cutover runbook

Migrating all AWS resources from the **old** account `303170530482` to the **new**
account `641079927221` (region `ap-southeast-2`). Work lives on the
`new_aws_account` branch; `main` still targets the old account.

## Accounts / profiles
- Old: profile `seslogin` (IAM user), account `303170530482`, state bucket `seslogin-terraform-state`.
- New: profile `seslogin-new` (IAM Identity Center SSO, `SesloginAdmin` = PowerUserAccess + `iam:*`), account `641079927221`, state bucket `seslogin-terraform-state-641079927221`.
- New Route53 zone: `Z07519672PRSQ6Z9FE5E7` (NS already delegated at the registrar). Old zone `Z2X4360EUGI76W` kept until decommission.

## Terraform toggles (in `terraform.tfvars` / `-var`)
- `enable_certs` — create ACM certs (implied by any `cutover_*`).
- `cutover_test|preprod|prod` — per-env: attach real alias + ACM cert to the new CF and point the zone record at it; else default cert / no alias and record → old CF (`app_cf_domain_*`).
- `background_jobs_enabled` — worker schedules + SQS ESMs (member sync, dispatcher, checker, nitc-export, healthcheck, activity-summary, sync-locations). `false` during testing.
- `ses_role_arn` — temporary cross-account SES bridge (see below).

## Done
- New account fully built: DynamoDB, Lambdas (real code via GitHub OIDC `deploy-newaccount.yml`), SQS/SNS, CloudFront (per-env cutover), AWS Backup, EventBridge, IAM.
- New DNS zone applied + verified; NS delegated; 8 transition records (old DKIM + old ACM validation) added via CLI (`scratchpad/transition-records.json`) — **prune at decommission**.
- SES: new-account `seslogin.com` identity verified + Easy DKIM + custom MAIL FROM `mail.seslogin.com`; **production access still pending**.
- **test.seslogin.com fully cut over** to the new account (`cutover_test=true`), user-validated.
- Prod snapshot copied into new `seslogin_prod_*` (period capped 1000). Separate `seslogin_test_*` sandbox tables created (`dynamodb_test.tf`) + seeded (`copy_prod_to_new_account.py --dest-prefix seslogin_test`).
- Workers disabled on new account (`background_jobs_enabled=false`).
- Lambda Function URLs need BOTH `lambda:InvokeFunctionUrl` (added by cargo-lambda) and `lambda:InvokeFunction` (Terraform `lambda_url_permissions.tf`) for public `AuthType=NONE`.

## SES cross-account bridge (while new-account SES production access is pending)
New account is still sandboxed, and cross-account *sending authorization* does NOT bypass the delegate's sandbox — so email must be **sent by the old (production) account**.
- Old account: role `seslogin-ses-sender` (trusts the 4 new-account email-sending Lambda roles; allows `ses:SendEmail`/`SendRawEmail`).
- New account: `SES_ROLE_ARN` env on api/preprod-api/test-api/activity-summary + `sts:AssumeRole` policy (`ses_cross_account.tf`); `mail.rs` assumes the role for the SES client when `SES_ROLE_ARN` is set.
- Verified working from test. Prod + preprod are wired identically.
- **Teardown when new account gets production SES**: clear `ses_role_arn` in tfvars → apply → redeploy; delete old-account `seslogin-ses-sender` role.

---

# Preprod + Prod cutover runbook

**One window for both:** preprod and prod share the `seslogin_prod` tables, and
`preprod.seslogin.com` locations write live prod data. Freeze both, copy once.

**Write freeze is kiosk-safe:** `READ_ONLY=true` makes writes return
`MutationDisabled` while reads work. Sessions are stateless JWTs; validation is a
read and `refresh_token` (`issue_token_for_session_id`) is a pure HMAC re-sign
with no DB access. With the reused `JWT_SECRET` + copied `session` table, kiosks
carry their session across to the new account — no re-pairing.

**Old CF distribution IDs:** prod `E22454VVK9EB64` (`seslogin.com` + `new.seslogin.com`), preprod `E3MXCYP17FQ7F` (`preprod.seslogin.com`).

## A. Before the window (no impact)
1. Confirm new account ready: certs `ISSUED`, SES bridge OK, latest code+web deployed.
2. Copy script real-copy flags: **`--all-periods`** (drop the 1000 cap) and **`--clear-dest`** (delete existing items per table key schema first, so test cruft in `seslogin_prod` doesn't ship to prod). *(Added — `copy_prod_to_new_account.py`.)*
3. Cutover tfvars staged in **`infra/cutover.tfvars`** (`cutover_prod=true`, `cutover_preprod=true`, `background_jobs_enabled=true`). It is a plain `.tfvars`, so `terraform apply` ignores it until passed with `-var-file` in step B.4 — nothing is applied now. *(Done.)*
4. Announce the window.

## B. The window
1. **Freeze writes on OLD account** (both APIs), keep reads:
   ```bash
   for f in seslogin-api seslogin-preprod-api; do
     env=$(aws lambda get-function-configuration --function-name $f --query Environment.Variables --output json --profile seslogin)
     newenv=$(echo "$env" | python3 -c "import sys,json;e=json.load(sys.stdin);e['READ_ONLY']='true';print(json.dumps({'Variables':e}))")
     aws lambda update-function-configuration --function-name $f --environment "$newenv" --profile seslogin
   done
   ```
   Also stop the old-account job **producers** so no new work is enqueued: disable the EventBridge schedules (`seslogin-member-sync-hourly`, `seslogin-checker-daily`, `seslogin-activity-summary-nightly`, `seslogin-location-sync-nightly`). Leave the SQS ESMs (consumers) running for now so the queues can still drain in step 2. Verify a mutation errors `MutationDisabled`, reads work, kiosk still authed.
2. **Drain the OLD-account queues** — with producers stopped but consumers still running, wait for every queue to reach 0 visible **and** 0 in-flight, so nothing is mid-write against the old prod tables:
   ```bash
   for q in seslogin-member-sync seslogin-member-sync-dlq \
            seslogin-nitc-export seslogin-nitc-export-dlq seslogin-healthcheck; do
     url=$(aws sqs get-queue-url --queue-name $q --profile seslogin --query QueueUrl --output text)
     aws sqs get-queue-attributes --queue-url "$url" --profile seslogin \
       --attribute-names ApproximateNumberOfMessages ApproximateNumberOfMessagesNotVisible \
       --query "join(': ', ['$q', join('/', values(Attributes))])" --output text
   done
   ```
   Every line must read `0/0`. (Pending member-sync/location-sync messages are safe to let drain — those syncs are idempotent and re-run on the new account once workers re-enable — but the point is to end with nothing **in flight** so it can't write post-snapshot. Inspect/redrive the DLQs if they're non-empty.)
3. **Disable the old-account background jobs (consumers)** — once the queues read `0/0`, disable the SQS event source mappings (`sync-members`, `nitc-export`, `healthcheck`) so no worker starts processing after the snapshot. Do this *before* the copy. (Together with the frozen APIs, the old prod tables are now fully static.)
4. **Release CNAMEs from old CFs** (start early, ~5–15 min each; custom domains go down here — writes already frozen). Remove aliases from `E22454VVK9EB64` and `E3MXCYP17FQ7F`, set them to default cert, then `aws cloudfront wait distribution-deployed`.
5. **Fresh copy** (after step 3, i.e. queues `0/0` and consumers disabled; parallel with step 4): `uv run infra/copy_prod_to_new_account.py --clear-dest --all-periods`; validate counts.
6. **Cut over** (after old CFs release CNAMEs AND copy done): `terraform apply -var-file=cutover.tfvars`. New prod+preprod CFs claim aliases+certs, zone records flip to them, workers re-enable. Wait for new CFs `Deployed`.

## C. Validate
- `seslogin.com`, `new.seslogin.com`, `preprod.seslogin.com` load over HTTPS.
- Login email sends (bridge). Kiosk logged in before window is still authed and can check in/out (writes live on new account).
- Spot-check data; watch CloudWatch + DLQ alarms.

## D. Rollback (mid-window)
Old account is only read from, so: old `READ_ONLY=false`, re-enable the old-account schedules + SQS ESMs disabled in B.1/B.3, re-add CNAMEs to old CFs, zone records back to old CF (`cutover_*=false`). ~15 min (CF redeploys), fully recoverable.

## E. After
- Clear `ses_role_arn` once new-account SES production access lands; delete `seslogin-ses-sender`.
- Prune the 8 transition DNS records; decommission the old account.

## Known caveat
CloudFront won't allow the same CNAME on two distributions, so the old CF must
release each alias before the new one claims it → **~15–30 min of custom-domain
read downtime** during the window (writes frozen throughout; kiosks keep sessions).
