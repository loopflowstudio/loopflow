#!/usr/bin/env python3
"""Publish loopflow to PyPI and/or DMG to R2."""

import argparse
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).parent.parent
WEBSITE_ROOT = ROOT.parent / "loopflowstudio" / "website"
MAESTRO_RELEASE_PATH = WEBSITE_ROOT / "static" / "maestro-release.json"


def update_website_release(version: str, public_url: str, dry_run: bool) -> bool:
    release = {
        "version": version,
        "latest_url": f"{public_url}/LoopflowMaestro-latest.dmg",
        "versioned_url": f"{public_url}/LoopflowMaestro-{version}.dmg",
        "published_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    }

    if not WEBSITE_ROOT.exists():
        print(f"Website repo not found at {WEBSITE_ROOT}. Skipping website update.")
        return False

    if dry_run:
        print(f"Would update website release metadata at {MAESTRO_RELEASE_PATH}")
        print(json.dumps(release, indent=2))
        print("Would deploy website via: python dev.py deploy --prod")
        return True

    MAESTRO_RELEASE_PATH.parent.mkdir(parents=True, exist_ok=True)
    MAESTRO_RELEASE_PATH.write_text(json.dumps(release, indent=2) + "\n")
    return deploy_website()


def deploy_website() -> bool:
    try:
        subprocess.run(
            ["python", "dev.py", "deploy", "--prod"],
            cwd=WEBSITE_ROOT,
            check=True,
        )
    except subprocess.CalledProcessError:
        return False
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description="Publish loopflow to PyPI and DMG to R2")
    parser.add_argument("bump", nargs="?", choices=["patch", "minor", "major"])
    parser.add_argument("-n", "--dry-run", action="store_true", help="Show what would be done")
    parser.add_argument("--skip-tests", action="store_true", help="Skip test run")
    parser.add_argument("-f", "--force", action="store_true", help="Skip main branch check")
    parser.add_argument("--dmg-only", action="store_true", help="Only build and upload DMG (no PyPI)")
    parser.add_argument("--skip-dmg", action="store_true", help="Skip DMG build/upload")
    parser.add_argument("--skip-website", action="store_true", help="Skip website update/deploy")
    args = parser.parse_args()

    # Validate args
    if args.dmg_only and args.bump:
        print("Error: --dmg-only cannot be used with version bump", file=sys.stderr)
        return 1
    if args.dmg_only and args.skip_dmg:
        print("Error: --dmg-only and --skip-dmg are mutually exclusive", file=sys.stderr)
        return 1

    # Import here so --help works without dependencies
    from loopflow.llm_http import generate_release_notes
    from loopflow.publish import (
        build_dmg,
        bump_version,
        build_package,
        check_publish_ready,
        get_dmg_path,
        get_version,
        install_locally,
        publish_package,
        run_tests,
        upload_dmg,
        write_version,
        R2_PUBLIC_URL,
    )

    # Handle DMG-only mode
    if args.dmg_only:
        version = get_version()
        print(f"Current version: {version}")

        if args.dry_run:
            print("Would build Maestro DMG")
            print(f"Would upload DMG to {R2_PUBLIC_URL}/LoopflowMaestro-{version}.dmg")
            print(f"Would upload DMG to {R2_PUBLIC_URL}/LoopflowMaestro-latest.dmg")
            if not args.skip_website:
                print("Would update website release metadata and deploy")
            return 0

        print("Building Maestro DMG...")
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

        print(f"\nDMG published: {R2_PUBLIC_URL}/LoopflowMaestro-{version}.dmg")
        if not args.skip_website:
            print("Updating website...")
            if not update_website_release(version, R2_PUBLIC_URL, dry_run=False):
                print("Website update failed.", file=sys.stderr)
                return 1
        return 0

    # Full release flow
    if not args.bump:
        args.bump = "patch"

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
        if not args.skip_dmg:
            print("Would build Maestro DMG")
            print(f"Would upload DMG to {R2_PUBLIC_URL}/LoopflowMaestro-{new_version}.dmg")
            if not args.skip_website:
                print("Would update website release metadata and deploy")
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

    # Step 8: Build and upload DMG
    if not args.skip_dmg:
        print("Building Maestro DMG...")
        success, output = build_dmg(ROOT)
        if not success:
            print("DMG build failed (continuing):", file=sys.stderr)
            print(output, file=sys.stderr)
        else:
            print("DMG built.")
            print("Uploading DMG...")
            dmg_path = get_dmg_path(ROOT)
            success, output = upload_dmg(dmg_path, new_version)
            if not success:
                print("DMG upload failed (continuing):", file=sys.stderr)
                print(output, file=sys.stderr)
            else:
                print(output)
                if not args.skip_website:
                    print("Updating website...")
                    if not update_website_release(new_version, R2_PUBLIC_URL, dry_run=False):
                        print("Website update failed.", file=sys.stderr)
                        return 1

    # Step 9: Install locally from the built wheel
    print("Installing locally...")
    success, output = install_locally(ROOT)
    if not success:
        print("Local install failed:", file=sys.stderr)
        print(output, file=sys.stderr)
        return 1
    print("Installed locally.")

    print(f"\nReleased v{new_version}")
    print("https://pypi.org/project/loopflow/")
    if not args.skip_dmg:
        print(f"{R2_PUBLIC_URL}/LoopflowMaestro-{new_version}.dmg")
    return 0


if __name__ == "__main__":
    sys.exit(main())
