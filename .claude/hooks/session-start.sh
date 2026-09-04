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

# `make check` runs actionlint and terraform before it gets anywhere near the web
# or api checks, and gha-lint exits 1 when actionlint is missing — so without
# these a web-only change gets no signal at all from the documented gate, and its
# install hint (`brew install actionlint`) is useless on this Linux container.
# Neither is fatal: a session with no network, or an upstream that has moved,
# should still start and run the tests.
TOOLS_DIR="$REPO_DIR/.claude/tools"
mkdir -p "$TOOLS_DIR"

if [ -x "$TOOLS_DIR/actionlint" ]; then
  echo "==> actionlint already installed"
else
  echo "==> Installing actionlint..."
  # Official installer; resolves the latest release and unpacks a single binary.
  ACTIONLINT_URL="https://raw.githubusercontent.com/rhysd/actionlint/main/scripts/download-actionlint.bash"
  curl -sSfL "$ACTIONLINT_URL" | bash -s -- "" "$TOOLS_DIR" >/dev/null 2>&1 ||
    echo "!!> actionlint install failed; \`make check\` will stop at gha-lint." >&2
fi

if [ -x "$TOOLS_DIR/terraform" ]; then
  echo "==> terraform already installed"
else
  # infra/main.tf asks for >= 1.5; CI uses hashicorp/setup-terraform (latest).
  # Pinned rather than "latest" so a session's toolchain doesn't move under it.
  TERRAFORM_VERSION="1.9.8"
  echo "==> Installing terraform $TERRAFORM_VERSION..."
  TERRAFORM_URL="https://releases.hashicorp.com/terraform/${TERRAFORM_VERSION}"
  TERRAFORM_URL="${TERRAFORM_URL}/terraform_${TERRAFORM_VERSION}_linux_amd64.zip"
  if curl -sSfL -o /tmp/terraform.zip "$TERRAFORM_URL" &&
    unzip -oq /tmp/terraform.zip -d "$TOOLS_DIR"; then
    rm -f /tmp/terraform.zip
  else
    echo "!!> terraform install failed; \`make check\` will stop at the infra step." >&2
  fi
fi

# Only useful if it is on PATH for every later command in the session.
if ! grep -qs "seslogin/.claude/tools" "$HOME/.bashrc"; then
  echo "export PATH=\"$TOOLS_DIR:\$PATH\"" >> "$HOME/.bashrc"
fi

echo "==> Session start complete."
