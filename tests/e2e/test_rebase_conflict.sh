#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

origin_dir=$(mktemp -d)
work_dir=$(mktemp -d)
other_dir=$(mktemp -d)

cleanup() {
  rm -rf "$origin_dir" "$work_dir" "$other_dir"
}
trap cleanup EXIT

git init --bare "$origin_dir" >/dev/null

git clone "$origin_dir" "$work_dir" >/dev/null

git -C "$work_dir" checkout -B main >/dev/null

git -C "$work_dir" config user.email "loopflow@example.com"
git -C "$work_dir" config user.name "Loopflow"

echo "line" > "$work_dir/file.txt"
git -C "$work_dir" add file.txt
git -C "$work_dir" commit -m "init" >/dev/null
git -C "$work_dir" push -u origin main >/dev/null

git clone "$origin_dir" "$other_dir" >/dev/null
git -C "$other_dir" checkout -B main >/dev/null

git -C "$other_dir" config user.email "loopflow@example.com"
git -C "$other_dir" config user.name "Loopflow"

echo "conflict from main" > "$other_dir/file.txt"
git -C "$other_dir" add file.txt
git -C "$other_dir" commit -m "main change" >/dev/null
git -C "$other_dir" push origin main >/dev/null

git -C "$work_dir" checkout -b feature/conflict >/dev/null

echo "conflict from feature" > "$work_dir/file.txt"
git -C "$work_dir" add file.txt
git -C "$work_dir" commit -m "feature change" >/dev/null

git -C "$work_dir" push -u origin feature/conflict >/dev/null

set +e
cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p lf -- \
  ops rebase origin/main
status=$?
set -e

if [ $status -eq 0 ]; then
  echo "expected rebase to fail due to conflict" >&2
  exit 1
fi

cd "$ROOT_DIR" >/dev/null
