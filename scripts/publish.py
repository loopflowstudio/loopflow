#!/usr/bin/env python3
"""Publish loopflow to PyPI and/or DMG to R2.

Idempotent release flow:
  1. First run: creates release PR, enables auto-merge
  2. Second run (after merge): tags, publishes to PyPI, builds DMG

Just run `python scripts/publish.py` and it figures out what to do.
"""

import argparse
import json
import subprocess
import sys
import urllib.request
from pathlib import Path

from loopflow.lf.messages import generate_release_notes
from loopflow.publish import (
    R2_PUBLIC_URL,
    build_dmg,
    build_package,
    bump_version,
    check_ci_passed,
    check_publish_ready,
    get_dmg_path,
    get_version,
    install_locally,
    publish_package,
    restart_daemon,
    run_tests,
    upload_dmg,
    write_version,
)

ROOT = Path(__file__).parent.parent
VERSION_FILE = ROOT / "VERSION"


# --- Version management ---


def _read_version() -> str:
    """Read version from VERSION file."""
    return VERSION_FILE.read_text().strip()


def _write_version_file(version: str) -> None:
    """Write version to VERSION file."""
    VERSION_FILE.write_text(version + "\n")


def _sync_versions(version: str) -> None:
    """Sync version to Python and Rust."""
    import re

    # Python: src/loopflow/__init__.py
    write_version(version)

    # Rust: Cargo.toml workspace version
    cargo_toml = ROOT / "Cargo.toml"
    content = cargo_toml.read_text()
    new_content = re.sub(r'version = "[^"]+"', f'version = "{version}"', content)
    cargo_toml.write_text(new_content)

    # VERSION file
    _write_version_file(version)


# --- Screenshot generation ---


def _generate_screenshots() -> tuple[bool, str]:
    """Run scripts/generate_screenshots.py, return (success, output)."""
    script = ROOT / "scripts" / "generate_screenshots.py"
    result = subprocess.run(
        [sys.executable, str(script)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    output = result.stdout + result.stderr
    return result.returncode == 0, output


# --- State detection ---


def _find_pending_release() -> str | None:
    """Find a release commit on main that hasn't been tagged/published yet.

    Searches the last 50 commits for release commits, robust to other commits
    landing on main after the release PR merges.
    """
    subprocess.run(["git", "fetch", "origin", "main"], cwd=ROOT, capture_output=True)
    result = subprocess.run(
        ["git", "log", "origin/main", "-50", "--format=%s"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    for line in result.stdout.strip().split("\n"):
        # Match "release: v0.7.1" or "release: v0.7.1 [skip ci]"
        if line.startswith("release: v"):
            version = line.replace("release: v", "").split()[0]
            if not _tag_exists(version) or not _is_on_pypi(version):
                return version
    return None


def _tag_exists(version: str) -> bool:
    """Check if a tag exists locally or on remote."""
    result = subprocess.run(
        ["git", "tag", "-l", f"v{version}"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return bool(result.stdout.strip())


def _get_open_release_pr() -> tuple[str | None, str | None]:
    """Check for an open release PR. Returns (version, pr_url) or (None, None)."""
    result = subprocess.run(
        [
            "gh",
            "pr",
            "list",
            "--head",
            "release/",
            "--json",
            "headRefName,url,state",
            "--limit",
            "10",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None, None
    try:
        prs = json.loads(result.stdout)
        for pr in prs:
            if pr.get("state") == "OPEN" and pr.get("headRefName", "").startswith("release/v"):
                version = pr["headRefName"].replace("release/v", "")
                return version, pr.get("url")
    except json.JSONDecodeError:
        pass
    return None, None


def _is_on_pypi(version: str) -> bool:
    """Check if version is already on PyPI."""
    try:
        with urllib.request.urlopen(
            "https://pypi.org/pypi/loopflow/json", timeout=10
        ) as response:
            data = json.loads(response.read())
            return version in data.get("releases", {})
    except Exception:
        return False


# --- Release phases ---


def _finalize_release(version: str, skip_dmg: bool) -> int:
    """Complete release: tag, publish to PyPI, build DMG."""
    print(f"Finalizing release v{version}...")

    # Update local main to match origin
    subprocess.run(["git", "checkout", "main"], cwd=ROOT, capture_output=True)
    subprocess.run(["git", "pull", "--ff-only"], cwd=ROOT, check=True)

    # Tag the release
    if _tag_exists(version):
        print(f"Tag v{version} already exists")
    else:
        print(f"Tagging v{version}...")
        result = subprocess.run(
            ["git", "tag", f"v{version}"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(f"Failed to create tag: {result.stderr}", file=sys.stderr)
            return 1
        result = subprocess.run(["git", "push", "origin", f"v{version}"], cwd=ROOT)
        if result.returncode != 0:
            print("Failed to push tags", file=sys.stderr)
            return 1
        print("Tag pushed.")

    # Check if already on PyPI
    if _is_on_pypi(version):
        print(f"v{version} already on PyPI")
    else:
        # Build package
        print("Building package...")
        success, output = build_package(ROOT)
        if not success:
            print("Build failed:", file=sys.stderr)
            print(output, file=sys.stderr)
            return 1
        print("Build succeeded.")

        # Publish to PyPI
        print("Publishing to PyPI...")
        success, output = publish_package(ROOT)
        if not success:
            print("Publish failed:", file=sys.stderr)
            print(output, file=sys.stderr)
            return 1
        print("Published to PyPI.")

    # Build and upload DMG
    if not skip_dmg:
        print("Building Concerto DMG...")
        success, output = build_dmg(ROOT)
        if not success:
            print("DMG build failed (continuing):", file=sys.stderr)
            print(output, file=sys.stderr)
        else:
            print("DMG built.")
            print("Uploading DMG...")
            dmg_path = get_dmg_path(ROOT)
            success, output = upload_dmg(dmg_path, version)
            if not success:
                print("DMG upload failed (continuing):", file=sys.stderr)
                print(output, file=sys.stderr)
            else:
                print(output)

    # Install locally
    print("Installing locally...")
    success, output = install_locally(ROOT)
    if not success:
        print("Local install failed:", file=sys.stderr)
        print(output, file=sys.stderr)
        return 1
    print("Installed locally.")

    # Clean up release branch if it exists
    release_branch = f"release/v{version}"
    subprocess.run(
        ["git", "push", "origin", "--delete", release_branch],
        cwd=ROOT,
        capture_output=True,
    )

    print(f"\nReleased v{version}")
    print("https://pypi.org/project/loopflow/")
    if not skip_dmg:
        print(f"{R2_PUBLIC_URL}/LoopflowConcerto-{version}.dmg")
    return 0


def _create_release_pr(
    bump: str,
    skip_tests: bool,
    skip_ci: bool,
    skip_screenshots: bool,
) -> int:
    """Create release PR with version bump."""
    # Step 1: Preflight checks
    print("Checking publish readiness...")
    state = check_publish_ready(ROOT)

    if not state.ready:
        print(f"Error: {state.message}", file=sys.stderr)
        return 1

    old_version = state.version
    new_version = bump_version(old_version, bump)

    # Step 2: Run tests
    if not skip_tests:
        print("Running tests...")
        success, output = run_tests(ROOT)
        if not success:
            print("Tests failed:", file=sys.stderr)
            print(output, file=sys.stderr)
            return 1
        print("Tests passed.")

    # Step 3: Verify CI passed
    if not skip_ci:
        print("Verifying CI passed...")
        success, message = check_ci_passed(ROOT)
        if not success:
            print(f"CI check failed: {message}", file=sys.stderr)
            return 1
        print(message)

    # Step 4: Generate screenshots
    if not skip_screenshots:
        print("Generating screenshots...")
        success, output = _generate_screenshots()
        if not success:
            print(f"Screenshot generation failed: {output}", file=sys.stderr)
            return 1
        print("Screenshots generated.")

    # Step 5: Generate release notes
    print("Generating release notes...")
    try:
        notes = generate_release_notes(ROOT, old_version, new_version)
    except Exception as e:
        print(f"Error generating release notes: {e}", file=sys.stderr)
        return 1
    print("Release notes generated.")

    # Step 6: Bump version and build package (validate before committing)
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

    # Step 7: Write release notes, create release branch, commit, push, create PR
    changes_md = "\n".join(f"- {change}" for change in notes.changes)
    release_notes_content = f"# v{new_version}\n\n{notes.summary}\n\n## Changes\n\n{changes_md}\n"
    (ROOT / "RELEASE_NOTES.md").write_text(release_notes_content)

    release_branch = f"release/v{new_version}"
    print(f"Creating release branch {release_branch}...")
    result = subprocess.run(
        ["git", "checkout", "-b", release_branch],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Failed to create release branch: {result.stderr}", file=sys.stderr)
        return 1

    print("Committing release...")
    subprocess.run(
        ["git", "add", "src/loopflow/__init__.py", "RELEASE_NOTES.md", "docs/*.png"],
        cwd=ROOT,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", f"release: v{new_version}\n\n[skip ci]"],
        cwd=ROOT,
        check=True,
    )

    print("Pushing release branch...")
    result = subprocess.run(
        ["git", "push", "-u", "origin", release_branch],
        cwd=ROOT,
    )
    if result.returncode != 0:
        print("Failed to push release branch", file=sys.stderr)
        return 1

    # Create PR
    print("Creating PR...")
    pr_body = f"Release v{new_version}\n\n{release_notes_content}"
    result = subprocess.run(
        [
            "gh",
            "pr",
            "create",
            "--base",
            "main",
            "--head",
            release_branch,
            "--title",
            f"release: v{new_version}",
            "--body",
            pr_body,
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Failed to create PR: {result.stderr}", file=sys.stderr)
        return 1
    pr_url = result.stdout.strip()
    print(f"Created: {pr_url}")

    # Enable admin auto-merge (bypasses branch protection since CI is skipped)
    print("Enabling admin auto-merge...")
    result = subprocess.run(
        [
            "gh", "pr", "merge", "--squash", "--auto", "--admin",
            "--subject", f"release: v{new_version}",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        error_msg = result.stderr.strip() or result.stdout.strip()
        auto_merge_disabled = (
            "auto merge is not allowed" in error_msg.lower()
            or "enablepullrequestautomerge" in error_msg.lower()
        )
        if auto_merge_disabled:
            print(
                "Auto-merge not enabled for this repo. Merge manually.",
                file=sys.stderr,
            )
        elif "must be a repository admin" in error_msg.lower():
            print(
                "Admin auto-merge requires admin rights. Merge manually.",
                file=sys.stderr,
            )
        else:
            print(f"Warning: Could not enable auto-merge: {error_msg}")
    else:
        print("Admin auto-merge enabled. PR will merge immediately.")

    # Open PR in browser
    subprocess.run(["open", pr_url])

    # Return to main branch
    subprocess.run(["git", "checkout", "main"], cwd=ROOT, capture_output=True)

    print(f"\nRelease PR created for v{new_version}")
    print("Run this command again after the PR merges to complete the release.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Publish loopflow to PyPI and DMG to R2. "
            "Idempotent: run twice (once to create PR, once after merge)."
        )
    )
    parser.add_argument("bump", nargs="?", choices=["patch", "minor", "major"], default="patch")
    parser.add_argument("-n", "--dry-run", action="store_true", help="Show what would be done")
    parser.add_argument(
        "--local", action="store_true", help="Build and install locally only (no publish)"
    )
    parser.add_argument("--skip-tests", action="store_true", help="Skip test run")
    parser.add_argument("--skip-ci", action="store_true", help="Skip CI check")
    parser.add_argument(
        "--dmg-only", action="store_true", help="Only build and upload DMG (no PyPI)"
    )
    parser.add_argument("--skip-dmg", action="store_true", help="Skip DMG build/upload")
    parser.add_argument(
        "--skip-screenshots", action="store_true", help="Skip screenshot generation"
    )
    parser.add_argument(
        "--version",
        type=str,
        help="Explicitly specify version to finalize (bypasses state detection)",
    )
    args = parser.parse_args()

    # Validate args
    if args.dmg_only and args.skip_dmg:
        print("Error: --dmg-only and --skip-dmg are mutually exclusive", file=sys.stderr)
        return 1

    # Handle DMG-only mode
    if args.dmg_only:
        version = get_version()
        print(f"Current version: {version}")

        if args.dry_run:
            print("Would build Concerto DMG")
            print(f"Would upload DMG to {R2_PUBLIC_URL}/LoopflowConcerto-{version}.dmg")
            print(f"Would upload DMG to {R2_PUBLIC_URL}/LoopflowConcerto-latest.dmg")
            return 0

        print("Building Concerto DMG...")
        success, output = build_dmg(ROOT)
        if not success:
            print("DMG build failed:", file=sys.stderr)
            print(output, file=sys.stderr)
            return 1
        print("DMG built.")

        print("Uploading DMG...")
        dmg_path = get_dmg_path(ROOT)
        success, output = upload_dmg(dmg_path, version)
        if not success:
            print("DMG upload failed:", file=sys.stderr)
            print(output, file=sys.stderr)
            return 1
        print(output)

        print(f"\nDMG published: {R2_PUBLIC_URL}/LoopflowConcerto-{version}.dmg")
        return 0

    # Handle --local mode: build, install, and restart daemon
    if args.local:
        if args.dry_run:
            print("Would build package, install locally, and restart daemon")
            return 0

        print("Building package...")
        success, output = build_package(ROOT)
        if not success:
            print("Build failed:", file=sys.stderr)
            print(output, file=sys.stderr)
            return 1
        print("Build succeeded.")

        print("Installing locally...")
        success, output = install_locally(ROOT)
        if not success:
            print("Local install failed:", file=sys.stderr)
            print(output, file=sys.stderr)
            return 1
        print("Installed locally.")

        print("Restarting daemon...")
        success, message = restart_daemon()
        if success:
            print(message)
        else:
            print(f"Warning: {message}", file=sys.stderr)

        return 0

    # --- Idempotent release flow: detect state and act accordingly ---

    # Handle explicit --version: bypass state detection and finalize directly
    if args.version:
        version = args.version.lstrip("v")
        print(f"Finalizing specified version v{version}...")
        if args.dry_run:
            print(f"Would finalize release v{version}:")
            print("  - Tag and push (if not exists)")
            print("  - Publish to PyPI (if not published)")
            if not args.skip_dmg:
                print("  - Build and upload DMG")
            print("  - Install locally")
            return 0
        return _finalize_release(version, args.skip_dmg)

    print("Checking release state...")

    # State 1: Release commit on main, not yet tagged/published → finalize
    release_version = _find_pending_release()
    if release_version:
        if _tag_exists(release_version) and _is_on_pypi(release_version):
            print(f"v{release_version} already released (tagged and on PyPI)")
            return 0

        if args.dry_run:
            print(f"Would finalize release v{release_version}:")
            print("  - Tag and push")
            print("  - Publish to PyPI")
            if not args.skip_dmg:
                print("  - Build and upload DMG")
            print("  - Install locally")
            return 0

        return _finalize_release(release_version, args.skip_dmg)

    # State 2: Open release PR exists → tell user to wait
    pr_version, pr_url = _get_open_release_pr()
    if pr_version:
        print(f"Release PR for v{pr_version} is open: {pr_url}")
        print("Waiting for CI to pass and PR to merge.")
        print("Run this command again after the PR merges.")
        return 0

    # State 3: No release in progress → start new release
    if args.dry_run:
        state = check_publish_ready(ROOT)
        if not state.ready:
            print(f"Error: {state.message}", file=sys.stderr)
            return 1
        new_version = bump_version(state.version, args.bump)
        print(f"Would create release PR for v{new_version}:")
        print(f"  - Bump version: {state.version} → {new_version}")
        print("  - Run tests" if not args.skip_tests else "  - Skip tests")
        print("  - Verify CI passed" if not args.skip_ci else "  - Skip CI check")
        print("  - Generate screenshots" if not args.skip_screenshots else "  - Skip screenshots")
        print("  - Generate release notes")
        print("  - Build package")
        print(f"  - Create branch release/v{new_version}")
        print("  - Create PR with auto-merge")
        print("\nAfter PR merges, run again to finalize (tag, publish, DMG).")
        return 0

    return _create_release_pr(args.bump, args.skip_tests, args.skip_ci, args.skip_screenshots)


if __name__ == "__main__":
    sys.exit(main())
