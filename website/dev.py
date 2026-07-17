#!/usr/bin/env python3
"""
Website development CLI.

Usage:
    python dev.py serve                    # Start dev server
    python dev.py serve -k                 # Kill existing and start
    python dev.py test                     # Run all tests
    python dev.py test -a                  # Run accessibility tests only
    python dev.py sync-docs                # Sync docs from loopflow repo
    python dev.py figma-list <url>         # List exportable Figma nodes
    python dev.py figma-export <url>       # Export assets from Figma

Figma commands require FIGMA_TOKEN env var (Settings > Personal access tokens)
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent
REPO_ROOT = ROOT.parent
DEFAULT_DOCS_SOURCE = REPO_ROOT / "docs"


def run(cmd: list[str], cwd: Path | None = None) -> None:
    """Run a command."""
    print(f"$ {' '.join(cmd)}")
    subprocess.run(cmd, cwd=cwd, check=True)


def kill_port(port: int) -> bool:
    """Kill process listening on port. Returns True if a process was killed."""
    result = subprocess.run(
        ["lsof", "-ti", f":{port}"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return False

    pids = result.stdout.strip().split("\n")
    for pid in pids:
        print(f"Killing process {pid} on port {port}")
        subprocess.run(["kill", "-9", pid])
    return True


def serve(args: argparse.Namespace) -> None:
    """Start the development server."""
    if args.kill:
        kill_port(5001)
    sync_docs(args)
    run(["uv", "run", "python", "main.py"], cwd=ROOT)


def test(args: argparse.Namespace) -> None:
    """Run tests."""
    sync_docs(args)

    # Install test dependencies if needed
    run(["uv", "sync", "--extra", "test"], cwd=ROOT)

    # Install playwright browsers if needed
    run(["uv", "run", "playwright", "install", "chromium"], cwd=ROOT)

    cmd = ["uv", "run", "pytest", "tests/", "-v"]

    if args.a11y:
        cmd.extend(["-k", "accessibility or Accessibility or aria or focus or contrast"])

    if args.k:
        cmd.extend(["-k", args.k])

    if args.headed:
        cmd.append("--headed")

    run(cmd, cwd=ROOT)


INSTALL_REWRITES = [
    # Shorten GitHub release URL to our hosted redirect
    (
        "https://github.com/loopflowstudio/loopflow/releases/latest/download/install.sh",
        "loopflow.studio/install.sh",
    ),
]


def sync_docs(args: argparse.Namespace) -> None:
    """Sync docs from the canonical repo docs directory."""
    docs_source = Path(getattr(args, "source", None) or DEFAULT_DOCS_SOURCE)
    if not docs_source.exists():
        print(f"Error: loopflow docs not found at {docs_source}")
        sys.exit(1)

    docs_dest = ROOT / "docs"
    static_dest = ROOT / "static"
    docs_dest.mkdir(exist_ok=True)
    static_dest.mkdir(exist_ok=True)

    # Remove stale .md files not in source
    source_names = {p.name for p in docs_source.glob("*.md")}
    for existing in docs_dest.glob("*.md"):
        if existing.name not in source_names:
            existing.unlink()
            print(f"  ✗ docs/{existing.name} (removed)")

    synced = []

    # Copy .md files with URL rewrites
    for src in docs_source.glob("*.md"):
        text = src.read_text()
        for old, new in INSTALL_REWRITES:
            text = text.replace(old, new)
        dest = docs_dest / src.name
        dest.write_text(text)
        synced.append(f"docs/{src.name}")

    # Copy images/gifs to static/
    for pattern in ["*.png", "*.gif"]:
        for src in docs_source.glob(pattern):
            shutil.copy2(src, static_dest / src.name)
            synced.append(f"static/{src.name}")

    print(f"Synced {len(synced)} files from {docs_source}")
    for name in sorted(synced):
        print(f"  ✓ {name}")


def figma_list(args: argparse.Namespace) -> None:
    """List exportable nodes in a Figma file."""
    run(["uv", "run", "python", "figma.py", "list", args.url], cwd=ROOT)


def figma_export(args: argparse.Namespace) -> None:
    """Export assets from Figma."""
    cmd = ["uv", "run", "python", "figma.py", "export", args.url]
    if args.node:
        for node in args.node:
            cmd.extend(["--node", node])
    if args.format:
        cmd.extend(["--format", args.format])
    if args.scale:
        cmd.extend(["--scale", str(args.scale)])
    if args.output:
        cmd.extend(["--output", args.output])
    run(cmd, cwd=ROOT)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Website development CLI",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    # serve
    serve_parser = subparsers.add_parser("serve", help="Start dev server")
    serve_parser.add_argument(
        "-k", "--kill", action="store_true", help="Kill existing process on port 5001"
    )
    serve_parser.set_defaults(func=serve)

    # test
    test_parser = subparsers.add_parser("test", help="Run tests")
    test_parser.add_argument(
        "-a", "--a11y", action="store_true", help="Run accessibility tests only"
    )
    test_parser.add_argument("-k", help="Filter tests by keyword")
    test_parser.add_argument("--headed", action="store_true", help="Run in headed mode")
    test_parser.set_defaults(func=test)

    # sync-docs
    sync_parser = subparsers.add_parser("sync-docs", help="Sync docs from loopflow repo")
    sync_parser.add_argument("--source", help="Docs source directory")
    sync_parser.set_defaults(func=sync_docs)

    # figma-list
    figma_list_parser = subparsers.add_parser("figma-list", help="List Figma nodes")
    figma_list_parser.add_argument("url", help="Figma file URL")
    figma_list_parser.set_defaults(func=figma_list)

    # figma-export
    figma_export_parser = subparsers.add_parser("figma-export", help="Export from Figma")
    figma_export_parser.add_argument("url", help="Figma file URL (can include node-id)")
    figma_export_parser.add_argument(
        "-n", "--node", action="append", help="Node ID to export (can repeat)"
    )
    figma_export_parser.add_argument(
        "-f", "--format", choices=["png", "svg", "jpg", "pdf"], default="png"
    )
    figma_export_parser.add_argument(
        "-s", "--scale", type=float, default=2.0, help="Scale factor (default: 2)"
    )
    figma_export_parser.add_argument("-o", "--output", default="static", help="Output directory")
    figma_export_parser.set_defaults(func=figma_export)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
