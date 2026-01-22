#!/usr/bin/env python3
"""Generate Loopflow screenshots for documentation.

Supports both Swift (Concerto) and web client captures.

Usage:
    python scripts/generate_screenshots.py
    python scripts/generate_screenshots.py --output docs/
    python scripts/generate_screenshots.py --manifest scripts/screenshots.yaml
    python scripts/generate_screenshots.py --web-only
    python scripts/generate_screenshots.py --swift-only
"""

import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Literal

import yaml


@dataclass
class Screenshot:
    name: str
    type: Literal["swift", "web"] = "swift"
    # Swift options
    window_size: tuple[int, int] | None = None
    select_branch: str | None = None
    mock_loops: bool = False
    # Web options
    url: str | None = None
    viewport: tuple[int, int] | None = None
    wait_for: str | None = None  # CSS selector to wait for


@dataclass
class Manifest:
    repo_url: str
    local_path: Path
    screenshots: list[Screenshot] = field(default_factory=list)


def load_manifest(path: Path) -> Manifest:
    with open(path) as f:
        data = yaml.safe_load(f)

    screenshots = []
    for shot in data.get("screenshots", []):
        size = shot.get("window_size")
        viewport = shot.get("viewport")
        screenshots.append(
            Screenshot(
                name=shot["name"],
                type=shot.get("type", "swift"),
                window_size=tuple(size) if size else None,
                select_branch=shot.get("select_branch"),
                mock_loops=shot.get("mock_loops", False),
                url=shot.get("url"),
                viewport=tuple(viewport) if viewport else None,
                wait_for=shot.get("wait_for"),
            )
        )

    local = data.get("local", "~/src/loopflow-demos")
    return Manifest(
        repo_url=data.get("repo", ""),
        local_path=Path(local).expanduser(),
        screenshots=screenshots,
    )


def setup_demo_repo(manifest: Manifest) -> Path:
    """Clone or locate demo repo, ensure it has worktrees."""
    repo_path = manifest.local_path

    if not repo_path.exists():
        ssh_url = manifest.repo_url.replace("https://github.com/", "git@github.com:")
        if not ssh_url.endswith(".git"):
            ssh_url += ".git"
        print(f"Cloning {ssh_url} to {repo_path}...")
        subprocess.run(
            ["git", "clone", ssh_url, str(repo_path)],
            check=True,
        )

    existing_worktrees = _list_worktrees(repo_path)
    if len(existing_worktrees) <= 1:
        print("Creating demo worktrees...")
        _create_demo_worktrees(repo_path)

    return repo_path


def _list_worktrees(repo_path: Path) -> list[str]:
    result = subprocess.run(
        ["git", "worktree", "list", "--porcelain"],
        cwd=repo_path,
        capture_output=True,
        text=True,
    )
    worktrees = []
    for line in result.stdout.split("\n"):
        if line.startswith("worktree "):
            worktrees.append(line.split(" ", 1)[1])
    return worktrees


def _create_demo_worktrees(repo_path: Path) -> None:
    branches = ["add-auth", "fix-cache"]
    for branch in branches:
        worktree_path = repo_path.parent / f"{repo_path.name}.{branch}"
        if worktree_path.exists():
            continue
        subprocess.run(
            ["git", "worktree", "add", "-b", branch, str(worktree_path)],
            cwd=repo_path,
            check=True,
        )
        dummy_file = worktree_path / f"{branch}.txt"
        dummy_file.write_text(f"Demo file for {branch}\n")
        subprocess.run(["git", "add", "."], cwd=worktree_path, check=True)
        subprocess.run(
            ["git", "commit", "-m", f"Add {branch} feature"],
            cwd=worktree_path,
            check=True,
        )


def find_concerto_executable() -> Path:
    """Find the Concerto executable."""
    derived_data = Path.home() / "Library/Developer/Xcode/DerivedData"

    # Check for Concerto or swift- (package name) builds
    for pattern in ["Concerto-", "swift-"]:
        for d in derived_data.iterdir():
            if d.name.startswith(pattern):
                for app_name in ["Concerto", "Concerto.app/Contents/MacOS/Concerto"]:
                    release = d / "Build/Products/Release" / app_name
                    debug = d / "Build/Products/Debug" / app_name
                    if release.exists():
                        return release
                    if debug.exists():
                        return debug

    # Try building
    repo_root = Path(__file__).parent.parent
    swift_dir = repo_root / "swift"
    if swift_dir.exists():
        print("Building Concerto...")
        subprocess.run(
            [
                "xcodebuild",
                "-scheme",
                "Concerto",
                "-configuration",
                "Release",
                "-destination",
                "platform=macOS",
                "build",
            ],
            cwd=swift_dir,
            check=True,
            capture_output=True,
        )
        # Check again
        for d in derived_data.iterdir():
            if d.name.startswith("swift-"):
                release = d / "Build/Products/Release/Concerto"
                if release.exists():
                    return release

    raise FileNotFoundError("Could not find Concerto. Build it first with: xcodebuild -scheme Concerto -configuration Release -destination 'platform=macOS' build")


def capture_swift_screenshot(
    shot: Screenshot, repo_path: Path, output_dir: Path, executable: Path
) -> Path:
    """Launch Concerto with --capture, wait for output."""
    output_path = output_dir / f"{shot.name}.png"

    args = [
        str(executable),
        "--capture",
        str(output_path),
        "--repo",
        str(repo_path),
    ]

    if shot.window_size:
        args.extend(["--size", f"{shot.window_size[0]}x{shot.window_size[1]}"])

    if shot.select_branch:
        args.extend(["--select", shot.select_branch])

    if shot.mock_loops:
        args.append("--mock-loops")

    print(f"Capturing {shot.name} (Swift)...")
    result = subprocess.run(args, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"  stderr: {result.stderr}")

    if not output_path.exists():
        raise RuntimeError(f"Screenshot not created: {output_path}")

    print(f"  -> {output_path}")
    return output_path


def capture_web_screenshot(shot: Screenshot, output_dir: Path) -> Path:
    """Capture web screenshot using Playwright."""
    output_path = output_dir / f"{shot.name}.png"

    if not shot.url:
        raise ValueError(f"Web screenshot {shot.name} requires a 'url' field")

    # Check if playwright is available
    try:
        from playwright.sync_api import sync_playwright
    except ImportError:
        print("  Playwright not installed. Install with: pip install playwright && playwright install chromium")
        raise

    print(f"Capturing {shot.name} (Web)...")

    with sync_playwright() as p:
        browser = p.chromium.launch()
        context = browser.new_context(
            viewport={
                "width": shot.viewport[0] if shot.viewport else 1200,
                "height": shot.viewport[1] if shot.viewport else 800,
            }
        )
        page = context.new_page()
        page.goto(shot.url)

        # Wait for content to load
        if shot.wait_for:
            page.wait_for_selector(shot.wait_for, timeout=10000)
        else:
            page.wait_for_load_state("networkidle")

        page.screenshot(path=str(output_path))
        browser.close()

    print(f"  -> {output_path}")
    return output_path


def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description="Generate Loopflow screenshots")
    parser.add_argument(
        "--output",
        "-o",
        type=Path,
        default=Path("docs"),
        help="Output directory for screenshots",
    )
    parser.add_argument(
        "--manifest",
        "-m",
        type=Path,
        default=Path("scripts/screenshots.yaml"),
        help="Path to manifest file",
    )
    parser.add_argument(
        "--swift-only",
        action="store_true",
        help="Only capture Swift/Concerto screenshots",
    )
    parser.add_argument(
        "--web-only",
        action="store_true",
        help="Only capture web screenshots",
    )
    args = parser.parse_args()

    script_dir = Path(__file__).parent
    repo_root = script_dir.parent
    manifest_path = repo_root / args.manifest
    output_dir = repo_root / args.output

    if not manifest_path.exists():
        print(f"Manifest not found: {manifest_path}")
        sys.exit(1)

    output_dir.mkdir(parents=True, exist_ok=True)

    manifest = load_manifest(manifest_path)

    # Filter screenshots by type
    screenshots = manifest.screenshots
    if args.swift_only:
        screenshots = [s for s in screenshots if s.type == "swift"]
    elif args.web_only:
        screenshots = [s for s in screenshots if s.type == "web"]

    # Setup for Swift captures
    swift_screenshots = [s for s in screenshots if s.type == "swift"]
    web_screenshots = [s for s in screenshots if s.type == "web"]

    executable = None
    repo_path = None

    if swift_screenshots:
        executable = find_concerto_executable()
        repo_path = setup_demo_repo(manifest)
        print(f"Using Concerto: {executable}")
        print(f"Using repo: {repo_path}")

    print(f"Output: {output_dir}")
    print()

    captured = 0
    for shot in screenshots:
        try:
            if shot.type == "swift" and executable and repo_path:
                capture_swift_screenshot(shot, repo_path, output_dir, executable)
                captured += 1
            elif shot.type == "web":
                capture_web_screenshot(shot, output_dir)
                captured += 1
        except Exception as e:
            print(f"  Error: {e}")

    print()
    print(f"Done! {captured}/{len(screenshots)} screenshots generated.")


if __name__ == "__main__":
    main()
