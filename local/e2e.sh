#!/usr/bin/env bash
#
# Bring the AWS-free local stack up *detached*, for scripts rather than people.
#
# `make dev-local` is the interactive form: it holds the terminal, streams three
# processes into it, and dies with Ctrl-C. That is the wrong shape for a browser
# test or a CI job, which needs the stack running in the background and a command
# that returns once it is actually answering.
#
#   up      start DynamoDB Local, create tables, seed, then run the API and the
#           web dev server in the background; return once both answer.
#   down    stop the API and the web server. The database keeps running, and its
#           data with it — `make local-down` stops that separately.
#   status  report what is up.
#
# The API is the ordinary `poem-local`, with no dev-auth override: the seeded
# user token in local/seed/synthetic.json is how a script authenticates, which is
# the same code path a real login produces. See DEVELOPMENT.md §9.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
RUN_DIR="$HERE/.e2e"
API_PID="$RUN_DIR/api.pid"
WEB_PID="$RUN_DIR/web.pid"
API_LOG="$RUN_DIR/api.log"
WEB_LOG="$RUN_DIR/web.log"
API_URL="http://localhost:8000/"
WEB_URL="http://localhost:5173/"

die() { echo "==> $*" >&2; exit 1; }

pid_alive() {
  [ -f "$1" ] || return 1
  local pid; pid="$(cat "$1")"
  kill -0 "$pid" 2>/dev/null || return 1
  echo "$pid"
}

# Wait for a URL to answer at all. Any HTTP response means the server is up —
# DynamoDB Local answers a bare GET with 400, and the API's own readiness is the
# same question, so status codes are deliberately not checked.
wait_for() {
  local url="$1" what="$2" pidfile="$3" logfile="$4"
  printf '==> Waiting for %s' "$what"
  for _ in $(seq 1 240); do
    if curl -s -o /dev/null "$url"; then echo " ready"; return 0; fi
    if ! pid_alive "$pidfile" >/dev/null; then
      echo; echo "==> $what exited before it became ready. Its output:" >&2
      sed 's/^/    /' "$logfile" >&2
      return 1
    fi
    printf '.'; sleep 0.5
  done
  echo
  die "Timed out after 120s waiting for $what. See $logfile"
}

cmd_up() {
  mkdir -p "$RUN_DIR"

  if pid_alive "$API_PID" >/dev/null && pid_alive "$WEB_PID" >/dev/null; then
    echo "==> Already up (api $(cat "$API_PID"), web $(cat "$WEB_PID"))"
    cmd_status
    return 0
  fi

  "$HERE/dynamodb.sh" start

  # Exported variables beat .env — dotenvy never overrides one already set — so
  # this file, not .env, decides where everything below points.
  set -a
  # shellcheck disable=SC1091
  . "$HERE/local.env"
  set +a

  ( cd "$ROOT/api" && cargo run --quiet --bin local-tables )
  ( cd "$ROOT/api" && cargo run --quiet --bin local-seed -- apply )

  # Build before backgrounding, so a compile error is a visible failure here
  # rather than a server that never comes up.
  echo "==> Building API (first build may take a few minutes)..."
  ( cd "$ROOT/api" && cargo build --bin poem-local )

  echo "==> Starting API"
  ( cd "$ROOT/api" && RUST_LOG="${RUST_LOG:-info}" \
      exec cargo run --quiet --bin poem-local -- --enable-mutations ) >"$API_LOG" 2>&1 &
  echo $! >"$API_PID"
  wait_for "$API_URL" "API on :8000" "$API_PID" "$API_LOG" || die "API did not start."

  # Relay artifacts must exist before vite serves, and this is a one-shot compile
  # rather than the watcher `make dev-local` runs: a script is not editing queries.
  echo "==> Compiling Relay artifacts"
  ( cd "$ROOT/web" && npm run --silent relay )

  echo "==> Starting web"
  ( cd "$ROOT/web" && exec npm run --silent dev ) >"$WEB_LOG" 2>&1 &
  echo $! >"$WEB_PID"
  wait_for "$WEB_URL" "web on :5173" "$WEB_PID" "$WEB_LOG" || die "Web server did not start."

  cmd_status
}

cmd_down() {
  local stopped=
  for pidfile in "$API_PID" "$WEB_PID"; do
    if pid="$(pid_alive "$pidfile")"; then
      # The recorded pid is a subshell; the server is its child, so signal the
      # whole process group or vite and cargo outlive it.
      kill -- "-$(ps -o pgid= "$pid" | tr -d ' ')" 2>/dev/null || kill "$pid" 2>/dev/null || true
      stopped=1
    fi
    rm -f "$pidfile"
  done
  [ -n "$stopped" ] && echo "==> Stopped the API and web server" \
    || echo "==> Nothing to stop"
  echo "==> DynamoDB Local is still running; stop it with: make local-down"
}

cmd_status() {
  local api web
  api="$(pid_alive "$API_PID" || echo '')"
  web="$(pid_alive "$WEB_PID" || echo '')"
  echo "    API  ${API_URL}      $([ -n "$api" ] && echo "running (pid $api)" || echo 'not running')   log: $API_LOG"
  echo "    web  ${WEB_URL}      $([ -n "$web" ] && echo "running (pid $web)" || echo 'not running')   log: $WEB_LOG"
  echo "    database: $("$HERE/dynamodb.sh" status)"
}

case "${1:-}" in
  up)     cmd_up ;;
  down)   cmd_down ;;
  status) cmd_status ;;
  *) die "usage: $0 {up|down|status}" ;;
esac
