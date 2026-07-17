#!/usr/bin/env python3
"""Refresh public product captures from the promoted app and live Wave state."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from generate_screenshots import load_manifest, select_screenshots
from website_screens import (
    CaptureProvenance,
    CaptureUnavailable,
    changed_meaningfully,
    image_dimensions,
    read_app_build,
    require_live_wave,
    validate_capture,
    write_json,
)


@dataclass(frozen=True)
class PreparedCapture:
    candidate: Path
    sidecar: Path
    status: Path
    target: Path


def _run_stdout(args: list[str], cwd: Path) -> str:
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def _git_is_clean(repo_root: Path) -> bool:
    return not _run_stdout(["git", "status", "--porcelain"], repo_root)


def _current_branch(repo_root: Path) -> str:
    return _run_stdout(["git", "branch", "--show-current"], repo_root)


def _default_branch(repo_root: Path) -> str:
    result = subprocess.run(
        ["git", "symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip().removeprefix("origin/")
    return "main"


def _worktree_paths(repo_root: Path) -> set[str]:
    output = _run_stdout(
        ["git", "status", "--porcelain", "--untracked-files=all"],
        repo_root,
    )
    return {line[3:].split(" -> ")[-1] for line in output.splitlines() if len(line) > 3}


def _live_status(lf_binary: Path, repo_root: Path, wave: str) -> dict:
    result = subprocess.run(
        [str(lf_binary), "status", wave, "--json"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise CaptureUnavailable(result.stderr.strip() or "lf status failed")
    payload = json.loads(result.stdout)
    require_live_wave(payload, wave)
    return payload


def _publish(repo_root: Path, lf_binary: Path, captures: list[PreparedCapture]) -> None:
    expected = {
        path.relative_to(repo_root).as_posix()
        for capture in captures
        for path in (
            capture.target,
            capture.target.with_suffix(".json"),
            capture.target.with_suffix(".status.json"),
        )
    }
    actual = _worktree_paths(repo_root)
    if actual != expected:
        unexpected = ", ".join(sorted(actual - expected)) or "none"
        missing = ", ".join(sorted(expected - actual)) or "none"
        raise CaptureUnavailable(
            f"refusing to publish mixed worktree changes; unexpected: {unexpected}; "
            f"missing: {missing}"
        )
    changed = len(captures)
    subprocess.run(
        [
            str(lf_binary),
            "commit",
            "-m",
            f"website: refresh {changed} live product capture{'s' if changed != 1 else ''}",
            "-p",
        ],
        cwd=repo_root,
        check=True,
    )


def refresh(args: argparse.Namespace) -> int:
    repo_root = Path(__file__).resolve().parent.parent
    if args.publish:
        if not _git_is_clean(repo_root):
            raise CaptureUnavailable("publish requires a clean worktree")
        branch = _current_branch(repo_root)
        default_branch = _default_branch(repo_root)
        if branch != default_branch:
            current = branch or "detached HEAD"
            raise CaptureUnavailable(
                f"publish runs only on {default_branch}; current branch is {current}"
            )

    manifest = load_manifest(repo_root / "scripts/screenshots.yaml")
    shots = select_screenshots(manifest, "website")
    waves = {shot.wave for shot in shots}
    if len(waves) != 1 or None in waves:
        raise RuntimeError("website screenshot set must use one explicit live Wave")
    wave = next(iter(waves))
    assert wave is not None
    status = _live_status(args.lf_binary, repo_root, wave)
    app_build = read_app_build(args.executable)
    if app_build.source_dirty:
        raise CaptureUnavailable("promoted app was built from a dirty source tree")

    staging = repo_root / ".lf/tmp/website-screens"
    shutil.rmtree(staging, ignore_errors=True)
    captures = staging / "captures"
    command = [
        sys.executable,
        str(repo_root / "scripts/generate_screenshots.py"),
        "--set",
        "website",
        "--output",
        str(captures),
        "--repo-path",
        str(repo_root),
        "--executable",
        str(args.executable),
        "--lf-binary",
        str(args.lf_binary),
        "--log-dir",
        ".lf/logs/website-screens",
    ]
    subprocess.run(command, cwd=repo_root, check=True)

    captured_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    prepared = []
    for shot in shots:
        assert shot.output is not None and shot.view is not None and shot.width is not None
        candidate = captures / f"{shot.name}.png"
        target = repo_root / shot.output
        meaningful = changed_meaningfully(target, candidate)
        if (
            not meaningful
            and target.with_suffix(".json").is_file()
            and target.with_suffix(".status.json").is_file()
        ):
            print(f"website-screens: {shot.name}.png unchanged")
            continue

        staged_sidecar = candidate.with_suffix(".json")
        staged_status = candidate.with_suffix(".status.json")
        write_json(staged_status, status)
        pixel_width, pixel_height = image_dimensions(candidate)
        provenance = CaptureProvenance(
            captured_at=captured_at,
            wave=wave,
            view=shot.view,
            app_version=app_build.version,
            app_commit=app_build.commit,
            source_dirty=app_build.source_dirty,
            pixel_width=pixel_width,
            pixel_height=pixel_height,
            status_snapshot=f"/static/{target.with_suffix('.status.json').name}",
        )
        write_json(staged_sidecar, provenance.as_dict())
        errors = validate_capture(
            candidate,
            expected_wave=wave,
            expected_view=shot.view,
            logical_width=shot.width,
        )
        if errors:
            raise CaptureUnavailable("; ".join(errors))
        prepared.append(
            PreparedCapture(
                candidate=candidate,
                sidecar=staged_sidecar,
                status=staged_status,
                target=target,
            )
        )

    if args.publish and not _git_is_clean(repo_root):
        raise CaptureUnavailable("worktree changed while captures were running")

    for capture in prepared:
        capture.target.parent.mkdir(parents=True, exist_ok=True)
        for source, target in (
            (capture.candidate, capture.target),
            (capture.sidecar, capture.target.with_suffix(".json")),
            (capture.status, capture.target.with_suffix(".status.json")),
        ):
            temporary = target.with_suffix(f"{target.suffix}.tmp")
            shutil.copy2(source, temporary)
            temporary.replace(target)
        print(f"website-screens: captured {capture.target.name} (changed)")

    shutil.rmtree(staging, ignore_errors=True)
    if not prepared:
        print("website-screens: no perceptual changes")
        return 0
    if args.publish:
        _publish(repo_root, args.lf_binary, prepared)
    else:
        count = len(prepared)
        print(f"website-screens: updated {count} capture{'s' if count != 1 else ''}")
    return 0


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--executable",
        type=Path,
        default=Path("/Applications/Loopflow.app/Contents/MacOS/Loopflow"),
    )
    parser.add_argument("--lf-binary", type=Path, default=Path("lf"))
    parser.add_argument("--publish", action="store_true")
    parser.add_argument(
        "--skip-unavailable",
        action="store_true",
        help="Exit successfully when live state or a clean default branch is unavailable",
    )
    args = parser.parse_args()
    try:
        raise SystemExit(refresh(args))
    except (CaptureUnavailable, subprocess.CalledProcessError, FileNotFoundError) as exc:
        if args.skip_unavailable:
            print(f"website-screens: skipped ({exc})")
            raise SystemExit(0) from exc
        raise


if __name__ == "__main__":
    main()
