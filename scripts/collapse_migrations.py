#!/usr/bin/env python3
"""Finalize baseline schema version after consolidating migrations.

Usage:
    # After manually updating baseline.py with schema changes:

    # Clean up incremental migrations and set placeholder version
    ./scripts/collapse_migrations.py --clean

    # Commit the changes
    git add -A && git commit -m "consolidate migrations into baseline"

    # Update version to commit SHA
    ./scripts/collapse_migrations.py --finalize
    git commit --amend --no-edit

The workflow:
    1. Manually update baseline.py with the consolidated schema
    2. Run --clean to remove m_*.py files and set placeholder version
    3. Commit
    4. Run --finalize to stamp with commit SHA
    5. Amend the commit
"""

import argparse
import re
import subprocess
from pathlib import Path

MIGRATIONS_DIR = Path(__file__).parent.parent / "src/loopflow/lfd/migrations"
BASELINE_PATH = MIGRATIONS_DIR / "baseline.py"


def get_git_sha() -> str:
    """Get short SHA of HEAD."""
    result = subprocess.run(
        ["git", "rev-parse", "--short", "HEAD"],
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


def get_git_commit_timestamp() -> str:
    """Get committer timestamp of HEAD in ISO format (reproducible)."""
    result = subprocess.run(
        ["git", "show", "-s", "--format=%cI", "HEAD"],
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


def find_incremental_migrations() -> list[Path]:
    """Find any m_*.py migration files."""
    return sorted(MIGRATIONS_DIR.glob("m_*.py"))


def update_version_in_file(version: str) -> None:
    """Update SCHEMA_VERSION in baseline.py."""
    content = BASELINE_PATH.read_text()
    new_content = re.sub(
        r'SCHEMA_VERSION = "[^"]*"',
        f'SCHEMA_VERSION = "{version}"',
        content,
    )
    BASELINE_PATH.write_text(new_content)
    print(f"Updated SCHEMA_VERSION to: {version}")


def main():
    parser = argparse.ArgumentParser(description="Manage baseline schema version")
    parser.add_argument(
        "--clean", action="store_true", help="Remove m_*.py and set placeholder version"
    )
    parser.add_argument(
        "--finalize", action="store_true", help="Update version to current commit SHA"
    )
    args = parser.parse_args()

    if not args.clean and not args.finalize:
        parser.print_help()
        print()
        incremental = find_incremental_migrations()
        if incremental:
            print("Incremental migrations found:")
            for path in incremental:
                print(f"  - {path.name}")
        else:
            print("No incremental migrations found.")
        return

    if args.clean:
        incremental = find_incremental_migrations()
        for path in incremental:
            path.unlink()
            print(f"Removed {path.name}")

        update_version_in_file("baseline_PENDING")
        print()
        print("Next steps:")
        print("  1. git add -A && git commit -m 'consolidate migrations'")
        print("  2. ./scripts/collapse_migrations.py --finalize")
        print("  3. git commit --amend --no-edit")

    if args.finalize:
        timestamp = get_git_commit_timestamp()
        sha = get_git_sha()
        # Use commit timestamp (reproducible, ordered) + SHA (traceable)
        version = f"{timestamp}_{sha}"
        update_version_in_file(version)
        print()
        print("Run: git commit --amend --no-edit")


if __name__ == "__main__":
    main()
