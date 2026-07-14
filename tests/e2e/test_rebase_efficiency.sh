#!/usr/bin/env bash
set -euo pipefail

unset LOOPFLOW_DIRECTIVE_FILE

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)

origin_dir=$(mktemp -d)
repo_dir=$(mktemp -d)
other_dir=$(mktemp -d)

cleanup() {
  rm -rf "$origin_dir" "$repo_dir" "$other_dir"
}
trap cleanup EXIT

run_lf() {
  cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -p loopflow --bin lf -- "$@"
}

git init --bare "$origin_dir" >/dev/null
git clone "$origin_dir" "$repo_dir" >/dev/null
git clone "$origin_dir" "$other_dir" >/dev/null

git -C "$repo_dir" checkout -B main >/dev/null
git -C "$repo_dir" config user.email "loopflow@example.com"
git -C "$repo_dir" config user.name "Loopflow"
echo "base" > "$repo_dir/file.txt"
git -C "$repo_dir" add file.txt
git -C "$repo_dir" commit -m "init" >/dev/null
git -C "$repo_dir" push -u origin main >/dev/null

git -C "$other_dir" fetch origin main >/dev/null
git -C "$other_dir" checkout -B main origin/main >/dev/null
git -C "$other_dir" config user.email "loopflow@example.com"
git -C "$other_dir" config user.name "Loopflow"

git -C "$repo_dir" checkout -b demo.parent >/dev/null
git -C "$repo_dir" push -u origin demo.parent >/dev/null

echo "main advance" >> "$other_dir/file.txt"
git -C "$other_dir" add file.txt
git -C "$other_dir" commit -m "main advance" >/dev/null
git -C "$other_dir" push origin main >/dev/null

mkdir -p "$repo_dir/scratch"
echo "notes" > "$repo_dir/scratch/design.md"

plan=$(cd "$repo_dir" && run_lf rebase --plan)
grep -q "class: scratch_only" <<<"$plan"
grep -q "strategy: reset_to_base" <<<"$plan"
grep -q "agent_launched: false" <<<"$plan"

(cd "$repo_dir" && run_lf rebase >/dev/null)

if [ ! -f "$repo_dir/scratch/design.md" ]; then
  echo "expected scratch/design.md to survive reset" >&2
  exit 1
fi

if ! git -C "$repo_dir" diff --quiet HEAD origin/main -- file.txt; then
  echo "expected branch to reset to origin/main" >&2
  exit 1
fi

echo "PASS"
