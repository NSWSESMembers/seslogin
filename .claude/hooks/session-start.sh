#!/bin/bash
set -euo pipefail

# Only run in Claude Code remote (cloud) environments
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

REPO_DIR="${CLAUDE_PROJECT_DIR:-$(git -C "$(dirname "$0")" rev-parse --show-toplevel)}"

echo "==> Installing web dependencies..."
cd "$REPO_DIR/web"
# `npm ci` rather than `npm install`: it installs strictly from the lockfile and
# never writes to it, which is what CI does (`_check-web.yml`) and what keeps the
# working tree clean here. `npm install` rewrites package-lock.json whenever the
# local npm differs from the one that generated it — this container ships npm 10
# (node 22) while CI runs node 24 / npm 11, and npm 10 silently strips the `libc`
# fields npm 11 writes, so every session started with a 63-line phantom diff.
# Fall back to `npm install` if the lockfile is out of sync with package.json,
# so a mid-dependency-change checkout can still start a session.
npm ci || {
  echo "!!> npm ci failed (lockfile likely out of sync with package.json)." >&2
  echo "!!> Falling back to npm install; expect package-lock.json to be rewritten." >&2
  npm install
}

echo "==> Fetching Rust dependencies..."
cd "$REPO_DIR/api"
cargo fetch

echo "==> Session start complete."
