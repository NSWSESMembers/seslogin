#!/usr/bin/env bash
#
# Start/stop a local DynamoDB for `make dev-local`.
#
# Two ways to run it, because different machines have different things installed:
#
#   java    Amazon's DynamoDB Local, run directly. One process, no VM. Any of
#           the `dynamodb-local` command (brew cask), a JAR fetched by `fetch`
#           below, or a Maven-resolved distribution in dynamodb-local/lib (also
#           `fetch`, when Amazon's download host is unreachable). Needs a JRE 17+.
#   docker  The `amazon/dynamodb-local` image via local/docker-compose.yml.
#
# `start` picks whichever is available, preferring java. Set LOCAL_DDB=java or
# LOCAL_DDB=docker to force one.
#
# Other knobs:
#   LOCAL_DDB_PORT     port to listen on (default 8100; must match local/local.env)
#   LOCAL_DDB_MEMORY   1 to run in memory — nothing survives a restart. Sensible
#                      in a throwaway sandbox; on a laptop you want the default.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
PORT="${LOCAL_DDB_PORT:-8100}"
DATA_DIR="$HERE/.dynamodb-data"
PID_FILE="$HERE/.dynamodb.pid"
LOG_FILE="$HERE/.dynamodb.log"
DIST_DIR="$HERE/dynamodb-local"
JAR="$DIST_DIR/DynamoDBLocal.jar"
# The Maven fallback resolves the same program as a plain classpath: one directory
# of jars plus the sqlite4java natives, with no launcher of its own.
LIB_DIR="$DIST_DIR/lib"
MAIN_CLASS="com.amazonaws.services.dynamodbv2.local.main.ServerRunner"
# Published to Maven Central, unlike the tarball. Pinned so a fetch is repeatable.
MAVEN_COORDS="com.amazonaws:DynamoDBLocal:2.6.1"
COMPOSE=(docker compose -f "$HERE/docker-compose.yml")

# Amazon's current distribution. The older s3.<region>.amazonaws.com/dynamodb-local
# URLs are retired; this is the one the AWS docs point at.
TARBALL_URL="https://d1ni2b6xgvw0s0.cloudfront.net/v2.x/dynamodb_local_latest.tar.gz"

die() { echo "==> $*" >&2; exit 1; }

have_docker() { command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; }
have_java()   { command -v java >/dev/null 2>&1 && java -version >/dev/null 2>&1; }
have_wrapper(){ command -v dynamodb-local >/dev/null 2>&1; }
have_mvn()    { command -v mvn >/dev/null 2>&1; }
# A Maven-resolved distribution: the DynamoDBLocal jar plus everything it needs.
have_lib()    { compgen -G "$LIB_DIR/DynamoDBLocal-*.jar" >/dev/null 2>&1; }
# Any of the three java routes, given a JRE for the two that need one.
have_java_dist(){ have_wrapper || { have_java && { [ -f "$JAR" ] || have_lib; }; }; }

# Which runtime to use: an explicit LOCAL_DDB, else whatever is installed.
pick_runtime() {
  case "${LOCAL_DDB:-}" in
    java)
      have_java_dist \
        || die "LOCAL_DDB=java but no usable DynamoDB Local was found (no dynamodb-local command, no $JAR, no $LIB_DIR). $(install_hint)"
      echo java ;;
    docker)
      have_docker || die "LOCAL_DDB=docker but the Docker daemon is not reachable."
      echo docker ;;
    "")
      if have_java_dist; then echo java
      elif have_docker; then echo docker
      else die "No way to run DynamoDB Local. $(install_hint)"
      fi ;;
    *) die "LOCAL_DDB must be 'java' or 'docker', got '${LOCAL_DDB}'." ;;
  esac
}

install_hint() {
  cat <<'HINT'

Install one of:
  Java (lightest — one process, no VM; needs a JRE 17+):
      brew install --cask temurin dynamodb-local
    or, to fetch a distribution into this repo instead of the brew cask:
      make local-fetch
    `make local-fetch` needs only a JRE, and falls back to Maven Central when
    Amazon's download host is unreachable — so on a Linux box, a CI runner or a
    sandbox, a JRE 17+ and Maven are enough. No Homebrew required.
  Docker:
      brew install colima docker docker-compose && colima start
HINT
}

running_pid() {
  [ -f "$PID_FILE" ] || return 1
  local pid
  pid="$(cat "$PID_FILE")"
  kill -0 "$pid" 2>/dev/null || return 1
  echo "$pid"
}

wait_for_port() {
  printf '==> Waiting for DynamoDB on :%s' "$PORT"
  for _ in $(seq 1 60); do
    if curl -s -o /dev/null "http://localhost:$PORT/"; then echo " ready"; return 0; fi
    printf '.'
    sleep 0.5
  done
  echo
  return 1
}

# Amazon's tarball. Returns non-zero rather than dying, so fetch can fall back.
fetch_tarball() {
  # Deliberately not `local`: the EXIT trap runs after the function returns, and
  # under `set -u` a local would be unbound by then.
  tmp="$(mktemp -d)"
  trap 'rm -rf "${tmp:-}"' EXIT
  echo "==> Downloading $TARBALL_URL"
  curl -fSL --progress-bar -o "$tmp/ddb.tar.gz" "$TARBALL_URL" || return 1
  echo "==> Verifying checksum"
  local want got
  want="$(curl -fsSL "$TARBALL_URL.sha256" | awk '{print $1}')" || return 1
  got="$(shasum -a 256 "$tmp/ddb.tar.gz" | awk '{print $1}')"
  [ -n "$want" ] || die "Could not fetch the published checksum."
  # A mismatch is not a reachability problem, so it is fatal rather than a
  # reason to try somewhere else.
  [ "$want" = "$got" ] || die "Checksum mismatch: expected $want, got $got. Refusing to install."
  tar -xzf "$tmp/ddb.tar.gz" -C "$DIST_DIR" || return 1
  [ -f "$JAR" ] || die "Tarball did not contain DynamoDBLocal.jar."
  echo "==> Installed $JAR"
}

# Maven Central publishes the same program, so a blocked or retired download host
# is not the end of it. Maven verifies its own checksums on download.
fetch_maven() {
  have_mvn || return 1
  echo "==> Resolving $MAVEN_COORDS from Maven Central"
  mkdir -p "$LIB_DIR"
  # A throwaway POM, because dependency:copy-dependencies resolves a project's
  # dependencies -- it has no mode that takes coordinates directly.
  cat >"$DIST_DIR/pom.xml" <<POM
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>local</groupId><artifactId>ddb-fetch</artifactId><version>1</version>
  <dependencies><dependency>
    <groupId>${MAVEN_COORDS%%:*}</groupId>
    <artifactId>$(echo "$MAVEN_COORDS" | cut -d: -f2)</artifactId>
    <version>${MAVEN_COORDS##*:}</version>
  </dependency></dependencies>
</project>
POM
  ( cd "$DIST_DIR" && mvn -q -B dependency:copy-dependencies -DoutputDirectory=lib ) || return 1
  have_lib || return 1
  echo "==> Installed $LIB_DIR"
}

cmd_fetch() {
  have_java || die "A JRE 17+ is required to run DynamoDB Local. Install one: brew install --cask temurin"
  mkdir -p "$DIST_DIR"
  fetch_tarball && return 0
  echo "==> Could not install from Amazon's download host; trying Maven Central" >&2
  fetch_maven && return 0
  die "Could not install DynamoDB Local. The download host was unreachable and $(have_mvn \
    && echo "the Maven Central fallback also failed" \
    || echo "the Maven Central fallback needs mvn on PATH"). $(install_hint)"
}

cmd_start() {
  if pid="$(running_pid)"; then
    echo "==> DynamoDB Local already running (pid $pid) on :$PORT"
    return 0
  fi
  local runtime
  runtime="$(pick_runtime)"

  local storage=(-dbPath "$DATA_DIR")
  if [ "${LOCAL_DDB_MEMORY:-}" = "1" ]; then
    storage=(-inMemory)
    echo "==> LOCAL_DDB_MEMORY=1: nothing will survive a restart"
  else
    mkdir -p "$DATA_DIR"
  fi

  if [ "$runtime" = docker ]; then
    echo "==> Starting DynamoDB Local (docker)"
    "${COMPOSE[@]}" up -d
    wait_for_port || die "Timed out waiting for DynamoDB Local. Try: ${COMPOSE[*]} logs"
    return 0
  fi

  # -sharedDb is not optional: without it DynamoDB Local partitions data by access
  # key id + region, so rows written under one credential are invisible to another.
  local -a argv
  if have_wrapper; then
    argv=(dynamodb-local -sharedDb -port "$PORT" "${storage[@]}")
  elif [ -f "$JAR" ]; then
    argv=(java "-Djava.library.path=$DIST_DIR/DynamoDBLocal_lib"
          -jar "$JAR" -sharedDb -port "$PORT" "${storage[@]}")
  else
    # Maven layout: no launcher jar to run with -jar (its manifest carries no
    # Class-Path), so name the main class and hand it the whole directory.
    argv=(java "-Djava.library.path=$LIB_DIR" -cp "$LIB_DIR/*"
          "$MAIN_CLASS" -sharedDb -port "$PORT" "${storage[@]}")
  fi
  echo "==> Starting DynamoDB Local (java): ${argv[*]}"
  ( cd "$DIST_DIR" 2>/dev/null || cd "$HERE"; exec "${argv[@]}" ) >"$LOG_FILE" 2>&1 &
  echo $! >"$PID_FILE"
  if ! wait_for_port; then
    rm -f "$PID_FILE"
    echo "==> DynamoDB Local did not come up. Its output:" >&2
    sed 's/^/    /' "$LOG_FILE" >&2
    if have_wrapper; then
      echo "==> If the dynamodb-local wrapper does not accept these flags, use the JAR instead: make local-fetch" >&2
    fi
    exit 1
  fi
}

cmd_stop() {
  if pid="$(running_pid)"; then
    kill "$pid" 2>/dev/null || true
    rm -f "$PID_FILE"
    echo "==> Stopped DynamoDB Local (pid $pid)"
  fi
  # The compose stack is separate from the PID file, so stop it too if it's there.
  if have_docker && "${COMPOSE[@]}" ps -q 2>/dev/null | grep -q .; then
    "${COMPOSE[@]}" down
  fi
}

cmd_reset() {
  cmd_stop
  if have_docker && [ -n "$(docker volume ls -q -f name=dynamodb-data 2>/dev/null)" ]; then
    "${COMPOSE[@]}" down -v 2>/dev/null || true
  fi
  rm -rf "$DATA_DIR"
  echo "==> Deleted every local table and row"
}

cmd_status() {
  if pid="$(running_pid)"; then
    echo "running (java, pid $pid) on :$PORT"
  elif have_docker && "${COMPOSE[@]}" ps -q 2>/dev/null | grep -q .; then
    echo "running (docker) on :$PORT"
  else
    echo "not running"
  fi
}

case "${1:-}" in
  start)  cmd_start ;;
  stop)   cmd_stop ;;
  reset)  cmd_reset ;;
  fetch)  cmd_fetch ;;
  status) cmd_status ;;
  *) die "usage: $0 {start|stop|reset|fetch|status}" ;;
esac
