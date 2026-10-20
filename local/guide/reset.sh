#!/usr/bin/env bash
#
# Put the local database into the exact state the user-guide screenshots expect:
# wipe it, load the standard fixtures, add the invented demo members and history,
# and leave two members signed in. Run before every render so each take is identical.
#
# Local only: every step goes through local/local.env (DB_PREFIX=seslogin_local).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
API_URL="${API_URL:-http://localhost:8000/}"

set -a
# shellcheck disable=SC1091
. "$ROOT/local/local.env"
set +a

"$ROOT/local/dynamodb.sh" reset
"$ROOT/local/dynamodb.sh" start
( cd "$ROOT/api" && cargo run --quiet --bin local-tables && cargo run --quiet --bin local-seed -- apply ) >/dev/null

API_URL="$API_URL" node "$HERE/seed-demo.mjs" | tail -1

# Two members still signed in, for "Incomplete Periods": one from tonight, and one
# who forgot to sign out after last Tuesday's training.
signin() {
  ( cd "$ROOT/api" && cargo run --quiet --bin cli -- period create-signin \
      --person "$1" --location TestAUnit001 --session TestAKiosk02 --hours-ago "$2" --dry-run false ) >/dev/null
}
signin TestAMember1 1
signin TestAMember2 26

# The fixture kiosks have not been in touch for months and show as red "Problem"
# rows; remove them so the Kiosks page shows only what a capture sets up.
for id in TestAKiosk01 TestAKiosk02; do
  curl -s "$API_URL" -H 'content-type: application/json' \
    -H 'authorization: Bearer slu_TokenSuperUsr.localdev-super' \
    -d "{\"query\":\"mutation { deleteSession(id: \\\"$id\\\") }\"}" >/dev/null
done

rm -rf "$ROOT/local/mail"
echo "==> Local database reset for the user-guide screenshots"
