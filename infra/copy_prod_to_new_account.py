# /// script
# requires-python = ">=3.11"
# dependencies = ["boto3"]
# ///
"""Copy production DynamoDB data from the OLD AWS account to the NEW one.

  source: profile `seslogin`      (303170530482)  tables seslogin_prod_*
  dest:   profile `seslogin-new`  (641079927221)  tables seslogin_prod_*

Seeds the new account's prod tables with a snapshot of production so we can test
the migrated stack against real data. The `period` table is capped at the most
recent PERIOD_LIMIT rows (by start_time) to limit write/storage cost.

Usage:
  uv run infra/copy_prod_to_new_account.py --dry-run   # scan + counts only, no writes
  uv run infra/copy_prod_to_new_account.py             # perform the copy
  uv run infra/copy_prod_to_new_account.py --only person,location   # subset
"""

import argparse
import heapq
import sys

import boto3

REGION = "ap-southeast-2"
SRC_PROFILE = "seslogin"
DST_PROFILE = "seslogin-new"
PREFIX = "seslogin_prod"

# Copied in full. Ephemeral TTL tables (login_code, webauthn_state) are skipped —
# they hold short-lived codes/challenges not worth snapshotting.
FULL_TABLES = [
    "user",
    "category",
    "location",
    "person",
    "session",
    "api_token",
    "user_token",
    "webauthn_credential",
    "nitc_group",
    "nitc_tag",
    "nitc_event",
]

PERIOD_TABLE = "period"
PERIOD_LIMIT = 1000  # most recent by start_time


def scan_items(table):
    """Yield every item in a table, paging through the scan."""
    kwargs = {}
    while True:
        resp = table.scan(**kwargs)
        yield from resp["Items"]
        lek = resp.get("LastEvaluatedKey")
        if not lek:
            return
        kwargs["ExclusiveStartKey"] = lek


def copy_full(src_db, dst_db, name, dry_run):
    src = src_db.Table(f"{PREFIX}_{name}")
    count = 0
    if dry_run:
        for _ in scan_items(src):
            count += 1
        print(f"  {name}: {count} items")
        return
    dst = dst_db.Table(f"{PREFIX}_{name}")
    with dst.batch_writer() as batch:
        for item in scan_items(src):
            batch.put_item(Item=item)
            count += 1
    print(f"  {name}: copied {count} items")


def copy_period(src_db, dst_db, dry_run):
    src = src_db.Table(f"{PREFIX}_{PERIOD_TABLE}")
    # Bounded heap keeps only the PERIOD_LIMIT most-recent items in memory while
    # scanning, regardless of total table size.
    recent = heapq.nlargest(PERIOD_LIMIT, scan_items(src), key=lambda i: i["start_time"])
    print(f"  {PERIOD_TABLE}: selected {len(recent)} most-recent (cap {PERIOD_LIMIT})")
    if dry_run:
        return
    dst = dst_db.Table(f"{PREFIX}_{PERIOD_TABLE}")
    with dst.batch_writer() as batch:
        for item in recent:
            batch.put_item(Item=item)
    print(f"  {PERIOD_TABLE}: copied {len(recent)} items")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true", help="scan + count only, no writes")
    ap.add_argument("--only", help="comma-separated subset of table base names")
    args = ap.parse_args()

    src_sess = boto3.Session(profile_name=SRC_PROFILE, region_name=REGION)
    dst_sess = boto3.Session(profile_name=DST_PROFILE, region_name=REGION)

    # Safety: never run source==dest.
    src_acct = src_sess.client("sts").get_caller_identity()["Account"]
    dst_acct = dst_sess.client("sts").get_caller_identity()["Account"]
    print(f"source {SRC_PROFILE} ({src_acct})  ->  dest {DST_PROFILE} ({dst_acct})")
    if src_acct == dst_acct:
        sys.exit("ERROR: source and destination resolve to the same account — aborting")

    src_db = src_sess.resource("dynamodb")
    dst_db = dst_sess.resource("dynamodb")

    only = set(args.only.split(",")) if args.only else None
    mode = "DRY-RUN (no writes)" if args.dry_run else "COPYING"
    print(f"=== {mode} ===")

    for name in FULL_TABLES:
        if only and name not in only:
            continue
        copy_full(src_db, dst_db, name, args.dry_run)

    if not only or PERIOD_TABLE in only:
        copy_period(src_db, dst_db, args.dry_run)

    print("done")


if __name__ == "__main__":
    main()
