#!/usr/bin/env python3
"""Finalize baseline schema version after consolidating migrations.

Usage:
    # After manually updating schema SQL:

    # Set placeholder version before commit
    ./scripts/collapse_migrations.py --clean

    # Commit the changes
    git add -A && git commit -m "consolidate migrations"

    # Stamp with commit timestamp + sha
    ./scripts/collapse_migrations.py --finalize
    git commit --amend --no-edit
"""

import argparse
import re
import subprocess
from pathlib import Path

SCHEMA_PATH = Path(__file__).parent.parent / "rust/loopflow/src/lfd/store/schema.rs"


def get_git_sha() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "--short", "HEAD"],
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


def get_git_commit_timestamp() -> str:
    result = subprocess.run(
        ["git", "show", "-s", "--format=%cI", "HEAD"],
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


def update_version_in_file(version: str) -> None:
    content = SCHEMA_PATH.read_text()
    new_content = re.sub(
        r'pub const SCHEMA_VERSION: &str = "[^"]*";',
        f'pub const SCHEMA_VERSION: &str = "{version}";',
        content,
    )
    SCHEMA_PATH.write_text(new_content)
    print(f"Updated SCHEMA_VERSION to: {version}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Manage baseline schema version")
    parser.add_argument(
        "--clean", action="store_true", help="Set placeholder version before commit"
    )
    parser.add_argument(
        "--finalize", action="store_true", help="Update version to current commit SHA"
    )
    args = parser.parse_args()

    if not args.clean and not args.finalize:
        parser.print_help()
        return

    if args.clean:
        update_version_in_file("baseline_PENDING")
        print("Next steps:")
        print("  1. git add -A && git commit -m 'consolidate migrations'")
        print("  2. ./scripts/collapse_migrations.py --finalize")
        print("  3. git commit --amend --no-edit")

    if args.finalize:
        timestamp = get_git_commit_timestamp()
        sha = get_git_sha()
        version = f"{timestamp}_{sha}"
        update_version_in_file(version)
        print("Run: git commit --amend --no-edit")


if __name__ == "__main__":
    main()
