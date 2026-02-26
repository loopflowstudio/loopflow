#!/usr/bin/env bash
(
  set -euo pipefail

  # `lf` shell integration exports this so top-level commands can request
  # parent-shell actions (like auto-cd). E2E tests spawn nested `lf` commands
  # and should never mutate an outer shell session.
  unset LOOPFLOW_DIRECTIVE_FILE

  ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

  origin_dir=$(mktemp -d)
  repo_dir=$(mktemp -d)

  cleanup() {
    rm -rf "$origin_dir" "$repo_dir"
  }
  trap cleanup EXIT

  git init --bare "$origin_dir" >/dev/null
  git clone "$origin_dir" "$repo_dir" >/dev/null

  cd "$repo_dir"
  git checkout -B main >/dev/null
  git config user.email "loopflow@example.com"
  git config user.name "Loopflow"
  git commit --allow-empty -m "init" >/dev/null
  git push -u origin main >/dev/null

  mkdir -p .lf/steps
  echo "# Test" > .lf/steps/debug.md

  cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p loopflow --bin lf-prompt -- \
    --repo "$repo_dir" \
    --step debug \
    --surface headless \
    --lfdocs false \
    --diff-files false \
    --diff false \
    | grep -q "Test"

  cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p loopflow --bin lf -- ops wt create smoke-test >/dev/null

  repo_name=$(basename "$repo_dir")
  wt_path="$(dirname "$repo_dir")/${repo_name}.smoke-test"

  if [ ! -d "$wt_path" ]; then
    echo "expected worktree at $wt_path" >&2
    exit 1
  fi

  cd "$wt_path"
  echo "change" > file.txt
  git add file.txt

  cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p loopflow --bin lf -- \
    ops commit -m "smoke test" --no-lint >/dev/null

  git log -1 --pretty=%B | grep -q "smoke test"

  echo "PASS"
)
