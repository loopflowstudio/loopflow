#!/usr/bin/env bash
#
# Bump the patch version in Cargo.toml and pyproject.toml, then prepend a
# stub entry to RELEASE_NOTES.md. Prints `next=<version>` on its last line so
# the weekly-release workflow can capture the bumped version.
#
# Usage: bump_patch_version.sh <last_tag_or_empty> <commit_count_or_zero>
#
# Designed to be safe to run locally for a smoke check; the workflow owns the
# commit + push.

set -euo pipefail

last_tag="${1:-}"
commit_count="${2:-0}"

current=$(python3 -c "
import re, pathlib
m = re.search(r'^version = \"([^\"]+)\"', pathlib.Path('Cargo.toml').read_text(), re.M)
print(m.group(1) if m else '')
")

if [[ -z "$current" ]] || ! [[ "$current" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "::error::Cargo.toml version is not plain semver: '$current'" >&2
    exit 1
fi

IFS=. read -r major minor patch <<< "$current"
next="${major}.${minor}.$((patch + 1))"

echo "bumping ${current} -> ${next}"

python3 - "$next" <<'PY'
import pathlib
import re
import sys

next_version = sys.argv[1]
pattern = re.compile(r'^version = "[^"]+"', re.MULTILINE)
replacement = f'version = "{next_version}"'

for path in (pathlib.Path("Cargo.toml"), pathlib.Path("pyproject.toml")):
    text = path.read_text()
    new_text, n = pattern.subn(replacement, text, count=1)
    if n == 0:
        raise SystemExit(f"no version line found in {path}")
    path.write_text(new_text)
PY

# Build the new RELEASE_NOTES.md header.
tmp=$(mktemp)
{
    printf '# v%s\n\n' "$next"
    if [ -n "$last_tag" ]; then
        printf 'Weekly auto-release with %s commits since `%s`.\n\n' "$commit_count" "$last_tag"
        printf '## Commits\n\n'
        git log "$last_tag..HEAD" --max-count=50 --pretty=format:'- %s'
        printf '\n\n'
    else
        printf 'Weekly auto-release.\n\n'
    fi
    cat RELEASE_NOTES.md
} > "$tmp"
mv "$tmp" RELEASE_NOTES.md

# Last line — captured by the workflow.
echo "next=${next}"
