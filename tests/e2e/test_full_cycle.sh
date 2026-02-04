#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

origin_dir=$(mktemp -d)
work_dir=$(mktemp -d)

cleanup() {
  rm -rf "$origin_dir" "$work_dir"
}
trap cleanup EXIT

git init --bare "$origin_dir" >/dev/null

git clone "$origin_dir" "$work_dir" >/dev/null

cd "$work_dir"
git checkout -b main >/dev/null

git config user.email "loopflow@example.com"
git config user.name "Loopflow"

echo "base" > file.txt
git add file.txt
git commit -m "init" >/dev/null
git push -u origin main >/dev/null

git checkout -b feature/full-cycle >/dev/null

echo "change" >> file.txt

cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p lf -- \
  ops commit -m "lf test: full cycle" --no-lint

cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p lf -- \
  ops land --local --strict --no-lint

current_branch=$(git rev-parse --abbrev-ref HEAD)
if [ "$current_branch" != "main" ]; then
  echo "expected main branch after land, got $current_branch" >&2
  exit 1
fi

if git show-ref --verify --quiet refs/heads/feature/full-cycle; then
  echo "feature branch should be deleted after land" >&2
  exit 1
fi

cd "$ROOT_DIR" >/dev/null
