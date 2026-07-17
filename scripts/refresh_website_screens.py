#!/usr/bin/env python3
"""Refresh the public product captures from the promoted app and live Wave state.

    uv run python scripts/refresh_website_screens.py            # capture in place
    uv run python scripts/refresh_website_screens.py --publish  # commit real changes

Publishes only what perceptibly changed, and only from a clean default branch.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
from dataclasses import asdict
from datetime import datetime, timezone
from pathlib import Path

from website_screens import (
    REPO_ROOT,
    CaptureProvenance,
    CaptureUnavailable,
    LiveCapture,
    capture,
    captured_wave,
    changed_meaningfully,
    live_status,
    load_captures,
    read_app_build,
    sidecar_paths,
    validate_capture,
    write_json,
)

STAGING = REPO_ROOT / ".lf/tmp/website-screens"


def _git_stdout(args: list[str], *, check: bool = True) -> str:
    result = subprocess.run(
        ["git", *args], cwd=REPO_ROOT, capture_output=True, text=True, check=check
    )
    return result.stdout.strip() if result.returncode == 0 else ""


def _worktree_paths() -> set[str]:
    output = _git_stdout(["status", "--porcelain", "--untracked-files=all"])
    return {line[3:].split(" -> ")[-1] for line in output.splitlines() if len(line) > 3}


def _require_publishable_branch() -> None:
    if _worktree_paths():
        raise CaptureUnavailable("publish requires a clean worktree")
    # origin/HEAD is unset on a fresh clone; main is the fallback, not an error.
    default = _git_stdout(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"], check=False)
    default = default.removeprefix("origin/") or "main"
    branch = _git_stdout(["branch", "--show-current"]) or "detached HEAD"
    if branch != default:
        raise CaptureUnavailable(f"publish runs only on {default}; current branch is {branch}")


def _publish(lf_binary: Path, targets: list[Path]) -> None:
    allowed = {
        path.relative_to(REPO_ROOT).as_posix()
        for target in targets
        for path in (target, *sidecar_paths(target))
    }
    missing_files = sorted(
        path.relative_to(REPO_ROOT).as_posix()
        for target in targets
        for path in (target, *sidecar_paths(target))
        if not path.is_file()
    )
    actual = _worktree_paths()
    changed_images = {target.relative_to(REPO_ROOT).as_posix() for target in targets}
    unexpected = sorted(actual - allowed)
    unchanged_images = sorted(changed_images - actual)
    if missing_files or unexpected or unchanged_images:
        raise CaptureUnavailable(
            "refusing to publish an incomplete or mixed capture set; "
            f"missing files: {', '.join(missing_files) or 'none'}; "
            f"unexpected changes: {', '.join(unexpected) or 'none'}; "
            f"unchanged images: {', '.join(unchanged_images) or 'none'}"
        )
    count = len(targets)
    subprocess.run(
        [
            str(lf_binary),
            "commit",
            "-m",
            f"website: refresh {count} live product capture{'s' if count != 1 else ''}",
            "-p",
        ],
        cwd=REPO_ROOT,
        check=True,
    )


def _stage(candidate: Path, shot: LiveCapture, provenance: CaptureProvenance, status: dict) -> None:
    """Complete the capture triple in staging and prove it publishable before it lands."""
    sidecar, status_path = sidecar_paths(candidate)
    write_json(sidecar, asdict(provenance))
    write_json(status_path, status)
    # A capture staged moments ago must be beyond structural *and* freshness
    # complaint; either kind blocks the refresh.
    errors, warnings = validate_capture(candidate, shot)
    if errors or warnings:
        raise CaptureUnavailable("; ".join(errors + warnings))


def _install(candidate: Path, target: Path) -> None:
    target.parent.mkdir(parents=True, exist_ok=True)
    pairs = list(zip((candidate, *sidecar_paths(candidate)), (target, *sidecar_paths(target))))
    temporaries = [destination.with_suffix(f"{destination.suffix}.tmp") for _, destination in pairs]
    try:
        for (source, _), temporary in zip(pairs, temporaries):
            shutil.copy2(source, temporary)
        for (_, destination), temporary in zip(pairs, temporaries):
            temporary.replace(destination)
    finally:
        for temporary in temporaries:
            temporary.unlink(missing_ok=True)


def refresh(executable: Path, lf_binary: Path, publish: bool) -> int:
    if publish:
        _require_publishable_branch()

    shots = load_captures()
    wave = captured_wave(shots)
    status = live_status(lf_binary, REPO_ROOT, wave)
    app_build = read_app_build(executable)
    captured_at = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    provenance = CaptureProvenance(
        captured_at=captured_at,
        wave=wave,
        app_version=app_build.version,
        app_commit=app_build.commit,
    )

    shutil.rmtree(STAGING, ignore_errors=True)
    landed = []
    for shot in shots:
        target = REPO_ROOT / shot.output
        candidate = STAGING / f"{shot.name}.png"
        capture(shot, executable=executable, repo_path=REPO_ROOT, output=candidate)
        if not changed_meaningfully(target, candidate) and all(
            path.is_file() for path in sidecar_paths(target)
        ):
            print(f"website-screens: {shot.name}.png unchanged")
            continue
        _stage(candidate, shot, provenance, status)
        landed.append((candidate, target))

    if publish and _worktree_paths():
        raise CaptureUnavailable("worktree changed while captures were running")
    for candidate, target in landed:
        _install(candidate, target)
        print(f"website-screens: captured {target.name} (changed)")
    shutil.rmtree(STAGING, ignore_errors=True)

    if not landed:
        print("website-screens: no perceptual changes")
        return 0
    if publish:
        _publish(lf_binary, [target for _, target in landed])
    else:
        print(f"website-screens: updated {len(landed)} capture{'s' if len(landed) != 1 else ''}")
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
        raise SystemExit(refresh(args.executable, args.lf_binary, args.publish))
    except (CaptureUnavailable, subprocess.CalledProcessError, FileNotFoundError) as exc:
        if args.skip_unavailable:
            print(f"website-screens: skipped ({exc})")
            raise SystemExit(0) from exc
        raise


if __name__ == "__main__":
    main()
