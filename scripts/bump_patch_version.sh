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
        commits_tmp=$(mktemp)
        git log "$last_tag..HEAD" --pretty=format:'%s' > "$commits_tmp"
        python3 - "$last_tag" "$commit_count" "$commits_tmp" <<'PY'
import collections
import re
import sys

last_tag = sys.argv[1]
commit_count = sys.argv[2]
subjects_path = sys.argv[3]
with open(subjects_path, encoding="utf-8") as handle:
    subjects = [line.strip() for line in handle if line.strip()]

sections = [
    (
        "Release and self-hosting infrastructure",
        lambda subject: any(
            marker in subject.lower()
            for marker in ("release", "deploy", "cron", "host", "budget", "local binary")
        ),
    ),
    (
        "Authentication and remote execution",
        lambda subject: any(
            marker in subject.lower()
            for marker in ("auth", "token", "credential", "remote", "lfd")
        ),
    ),
    (
        "Concerto and user surfaces",
        lambda subject: any(
            marker in subject.lower()
            for marker in ("concerto", "desktop", "mobile", "portfolio", "website")
        ),
    ),
    (
        "Agent workflows and developer tooling",
        lambda subject: any(
            marker in subject.lower()
            for marker in ("lf:", "engine", "installer", "skill", "workflow", "wave")
        ),
    ),
    (
        "Dependency updates",
        lambda subject: subject.startswith("build(deps"),
    ),
]

grouped: dict[str, list[str]] = collections.defaultdict(list)
for subject in subjects:
    if subject.startswith("build(deps"):
        grouped["Dependency updates"].append(subject)
        continue
    for name, predicate in sections:
        if predicate(subject):
            grouped[name].append(subject)
            break
    else:
        grouped["Other changes"].append(subject)

print(f"Weekly auto-release with {commit_count} commits since `{last_tag}`.")
print()
print(
    "Commits are grouped by theme instead of truncated. "
    "This is a deterministic token-compression pass for CI: preserve every unique commit subject, merge repetition into structure, and avoid first-N summaries."
)
print()

for name, _ in sections:
    items = grouped.get(name, [])
    if not items:
        continue
    print(f"## {name}")
    print()
    if name == "Dependency updates":
        packages = []
        pattern = re.compile(r"bump ([^ ]+) from ([^ ]+) to ([^ ]+)")
        for item in items:
            match = pattern.search(item)
            if match:
                package, old, new = match.groups()
                packages.append(f"{package} {old} → {new}")
            else:
                packages.append(item)
        print(f"- {len(items)} dependency update(s): {', '.join(packages)}")
    else:
        for item in items:
            print(f"- {item}")
    print()

other = grouped.get("Other changes", [])
if other:
    print("## Other changes")
    print()
    for item in other:
        print(f"- {item}")
    print()
PY
        rm -f "$commits_tmp"
    else
        printf 'Weekly auto-release.\n\n'
    fi
    cat RELEASE_NOTES.md
} > "$tmp"
mv "$tmp" RELEASE_NOTES.md

# Last line — captured by the workflow.
echo "next=${next}"
