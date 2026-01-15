#!/usr/bin/env python3
"""Publish loopflow to PyPI."""

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent


def main() -> int:
    parser = argparse.ArgumentParser(description="Publish loopflow to PyPI")
    parser.add_argument("bump", nargs="?", default="patch", choices=["patch", "minor", "major"])
    parser.add_argument("-n", "--dry-run", action="store_true", help="Show what would be done")
    parser.add_argument("--skip-tests", action="store_true", help="Skip test run")
    parser.add_argument("-f", "--force", action="store_true", help="Skip main branch check")
    args = parser.parse_args()

    # Import here so --help works without dependencies
    from loopflow.llm_http import generate_release_notes
    from loopflow.publish import (
        bump_version,
        build_package,
        check_publish_ready,
        install_locally,
        publish_package,
        run_tests,
        write_version,
    )

    # Step 1: Preflight checks
    print("Checking publish readiness...")
    state = check_publish_ready(ROOT)

    if not args.force:
        if not state.ready:
            print(f"Error: {state.message}", file=sys.stderr)
            return 1
    elif not state.on_main:
        print(f"Warning: {state.message} (continuing due to --force)")

    old_version = state.version
    new_version = bump_version(old_version, args.bump)

    if args.dry_run:
        print(f"Would bump version: {old_version} → {new_version} ({args.bump})")
        print("Would run tests" if not args.skip_tests else "Would skip tests")
        print("Would generate release notes")
        print(f"Would commit: release: v{new_version}")
        print(f"Would tag: v{new_version}")
        print("Would build package")
        print("Would publish to PyPI")
        print("Would install locally")
        return 0

    # Step 2: Run tests
    if not args.skip_tests:
        print("Running tests...")
        success, output = run_tests(ROOT)
        if not success:
            print("Tests failed:", file=sys.stderr)
            print(output, file=sys.stderr)
            return 1
        print("Tests passed.")

    # Step 3: Generate release notes (before any git changes)
    print("Generating release notes...")
    try:
        notes = generate_release_notes(ROOT, old_version, new_version)
    except Exception as e:
        print(f"Error generating release notes: {e}", file=sys.stderr)
        return 1
    print("Release notes generated.")

    # Step 4: Bump version and build package (validate before committing)
    print(f"Bumping version: {old_version} → {new_version}")
    write_version(new_version)

    print("Building package...")
    success, output = build_package(ROOT)
    if not success:
        print("Build failed:", file=sys.stderr)
        print(output, file=sys.stderr)
        write_version(old_version)
        return 1
    print("Build succeeded.")

    # Step 5: Write release notes and commit (now safe - build validated)
    changes_md = "\n".join(f"- {change}" for change in notes.changes)
    release_notes_content = f"# v{new_version}\n\n{notes.summary}\n\n## Changes\n\n{changes_md}\n"
    (ROOT / "RELEASE_NOTES.md").write_text(release_notes_content)

    print("Committing release...")
    result = subprocess.run(
        ["git", "add", "src/loopflow/__init__.py", "RELEASE_NOTES.md"],
        cwd=ROOT,
    )
    if result.returncode != 0:
        print("Failed to stage files", file=sys.stderr)
        return 1

    result = subprocess.run(
        ["git", "commit", "-m", f"release: v{new_version}"],
        cwd=ROOT,
    )
    if result.returncode != 0:
        print("Failed to commit", file=sys.stderr)
        return 1

    result = subprocess.run(["git", "push"], cwd=ROOT)
    if result.returncode != 0:
        print("Failed to push", file=sys.stderr)
        return 1
    print("Committed and pushed.")

    # Step 6: Tag
    print(f"Tagging v{new_version}...")
    result = subprocess.run(["git", "tag", f"v{new_version}"], cwd=ROOT)
    if result.returncode != 0:
        print("Failed to create tag", file=sys.stderr)
        return 1

    result = subprocess.run(["git", "push", "--tags"], cwd=ROOT)
    if result.returncode != 0:
        print("Failed to push tags", file=sys.stderr)
        return 1
    print("Tag pushed.")

    # Step 7: Publish (package already built)
    print("Publishing to PyPI...")
    success, output = publish_package(ROOT)
    if not success:
        print("Publish failed:", file=sys.stderr)
        print(output, file=sys.stderr)
        return 1
    print("Published to PyPI.")

    # Step 8: Install locally
    print("Installing locally...")
    success, output = install_locally()
    if not success:
        print("Local install failed:", file=sys.stderr)
        print(output, file=sys.stderr)
        return 1
    print("Installed locally.")

    print(f"\nReleased v{new_version}")
    print("https://pypi.org/project/loopflow/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
