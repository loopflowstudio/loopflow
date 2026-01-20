#!/usr/bin/env python3
"""Generate Maestro screenshots for documentation.

Usage:
    python scripts/generate_screenshots.py
    python scripts/generate_screenshots.py --output docs/
    python scripts/generate_screenshots.py --manifest scripts/screenshots.yaml
"""

import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import yaml


@dataclass
class Screenshot:
    name: str
    window_size: tuple[int, int] | None = None
    select_branch: str | None = None
    mock_loops: bool = False


@dataclass
class Manifest:
    repo_url: str
    local_path: Path
    screenshots: list[Screenshot]


def load_manifest(path: Path) -> Manifest:
    with open(path) as f:
        data = yaml.safe_load(f)

    screenshots = []
    for shot in data.get("screenshots", []):
        size = shot.get("window_size")
        screenshots.append(
            Screenshot(
                name=shot["name"],
                window_size=tuple(size) if size else None,
                select_branch=shot.get("select_branch"),
                mock_loops=shot.get("mock_loops", False),
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
        # Convert HTTPS URL to SSH for cloning
        ssh_url = manifest.repo_url.replace(
            "https://github.com/", "git@github.com:"
        )
        if not ssh_url.endswith(".git"):
            ssh_url += ".git"
        print(f"Cloning {ssh_url} to {repo_path}...")
        subprocess.run(
            ["git", "clone", ssh_url, str(repo_path)],
            check=True,
        )

    # Create worktrees if needed
    existing_worktrees = _list_worktrees(repo_path)
    if len(existing_worktrees) <= 1:  # Only main
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
        # Make a commit to show ahead-of-main
        dummy_file = worktree_path / f"{branch}.txt"
        dummy_file.write_text(f"Demo file for {branch}\n")
        subprocess.run(["git", "add", "."], cwd=worktree_path, check=True)
        subprocess.run(
            ["git", "commit", "-m", f"Add {branch} feature"],
            cwd=worktree_path,
            check=True,
        )


def find_maestro_executable() -> Path:
    """Find the Maestro executable."""
    # Check DerivedData for xcodebuild output
    derived_data = Path.home() / "Library/Developer/Xcode/DerivedData"
    for d in derived_data.iterdir():
        if d.name.startswith("Maestro-"):
            release = d / "Build/Products/Release/Maestro"
            if release.exists():
                return release

    # Try building
    repo_root = Path(__file__).parent.parent
    maestro_dir = repo_root / "Maestro"
    if maestro_dir.exists():
        print("Building Maestro...")
        subprocess.run(
            [
                "xcodebuild",
                "-scheme",
                "Maestro",
                "-configuration",
                "Release",
                "-destination",
                "platform=macOS",
                "build",
            ],
            cwd=maestro_dir,
            check=True,
            capture_output=True,
        )
        # Check again
        for d in derived_data.iterdir():
            if d.name.startswith("Maestro-"):
                release = d / "Build/Products/Release/Maestro"
                if release.exists():
                    return release

    raise FileNotFoundError("Could not find Maestro. Build it first.")


def capture_screenshot(
    shot: Screenshot, repo_path: Path, output_dir: Path, executable: Path
) -> Path:
    """Launch Maestro with --capture, wait for output."""
    output_path = output_dir / f"{shot.name}.png"

    # Build args for Maestro
    maestro_args = [
        "--capture",
        str(output_path),
        "--repo",
        str(repo_path),
    ]

    if shot.window_size:
        maestro_args.extend(["--size", f"{shot.window_size[0]}x{shot.window_size[1]}"])

    if shot.select_branch:
        maestro_args.extend(["--select", shot.select_branch])

    if shot.mock_loops:
        maestro_args.append("--mock-loops")

    # Run directly since it's a SwiftUI app executable
    # The executable handles its own NSApplication lifecycle
    args = [str(executable)] + maestro_args

    print(f"Capturing {shot.name}...")
    result = subprocess.run(args, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"  stderr: {result.stderr}")

    if not output_path.exists():
        raise RuntimeError(f"Screenshot not created: {output_path}")

    print(f"  → {output_path}")
    return output_path


def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description="Generate Maestro screenshots")
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
    args = parser.parse_args()

    # Find repo root
    script_dir = Path(__file__).parent
    repo_root = script_dir.parent
    manifest_path = repo_root / args.manifest
    output_dir = repo_root / args.output

    if not manifest_path.exists():
        print(f"Manifest not found: {manifest_path}")
        sys.exit(1)

    output_dir.mkdir(parents=True, exist_ok=True)

    manifest = load_manifest(manifest_path)
    executable = find_maestro_executable()
    repo_path = setup_demo_repo(manifest)

    print(f"Using Maestro: {executable}")
    print(f"Using repo: {repo_path}")
    print(f"Output: {output_dir}")
    print()

    for shot in manifest.screenshots:
        capture_screenshot(shot, repo_path, output_dir, executable)

    print()
    print(f"Done! {len(manifest.screenshots)} screenshots generated.")


if __name__ == "__main__":
    main()
