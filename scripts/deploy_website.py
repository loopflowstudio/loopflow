#!/usr/bin/env python3

from __future__ import annotations

import argparse
import fcntl
import json
import os
import shlex
import subprocess
import time
import urllib.error
import urllib.request
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Literal, NoReturn

APP = "loopflow-website"
PRODUCTION_URL = "https://loopflow.studio"
HEALTH_USER_AGENT = "loopflow-release-health/1"


@dataclass(frozen=True)
class DeployReceipt:
    tag: str
    source_commit: str
    previous_image: str | None
    deployed_image: str | None
    outcome: Literal["deployed", "unchanged", "rolled_back"]


def _run(
    cmd: list[str],
    cwd: Path,
    *,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    print(f"$ {shlex.join(cmd)}", flush=True)
    return subprocess.run(
        cmd,
        cwd=cwd,
        check=True,
        capture_output=capture,
        text=True,
    )


def _fetch_json(url: str) -> dict[str, object] | None:
    try:
        request = urllib.request.Request(url, headers={"User-Agent": HEALTH_USER_AGENT})
        with urllib.request.urlopen(request, timeout=10) as response:
            if response.status != 200:
                return None
            value = json.loads(response.read())
            return value if isinstance(value, dict) else None
    except (OSError, json.JSONDecodeError, urllib.error.HTTPError):
        return None


def _root_is_healthy() -> bool:
    try:
        request = urllib.request.Request(
            f"{PRODUCTION_URL}/", headers={"User-Agent": HEALTH_USER_AGENT}
        )
        with urllib.request.urlopen(request, timeout=10) as response:
            return response.status == 200
    except (OSError, urllib.error.HTTPError):
        return False


def _release_is_healthy(tag: str) -> bool:
    health = _fetch_json(f"{PRODUCTION_URL}/healthz")
    return health == {"status": "ok", "release": tag} and _root_is_healthy()


def _wait_for_release(tag: str, attempts: int = 10) -> bool:
    for attempt in range(1, attempts + 1):
        if _release_is_healthy(tag):
            return True
        print(f"Production proof {attempt}/{attempts} did not report {tag}", flush=True)
        if attempt < attempts:
            time.sleep(5)
    return False


def _wait_for_root(attempts: int = 10) -> bool:
    for attempt in range(1, attempts + 1):
        if _root_is_healthy():
            return True
        print(f"Rollback proof {attempt}/{attempts} is not healthy", flush=True)
        if attempt < attempts:
            time.sleep(5)
    return False


def _current_image(repo: Path) -> str | None:
    result = _run(
        ["flyctl", "releases", "--json", "--image", "-a", APP],
        repo,
        capture=True,
    )
    releases = json.loads(result.stdout)
    if not isinstance(releases, list) or not releases:
        return None
    image = releases[0].get("ImageRef") if isinstance(releases[0], dict) else None
    return image if isinstance(image, str) and image else None


def _verify_tag(repo: Path, tag: str) -> str:
    result = _run(["git", "tag", "--points-at", "HEAD"], repo, capture=True)
    if tag not in result.stdout.splitlines():
        raise RuntimeError(f"publisher checkout is not tagged {tag}")
    return _run(["git", "rev-parse", "HEAD"], repo, capture=True).stdout.strip()


def _write_receipt(repo: Path, receipt: DeployReceipt) -> None:
    main_repo = Path(os.environ.get("LF_RELEASE_MAIN_REPO", repo))
    log_dir = main_repo / ".lf" / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    path = log_dir / f"website.{receipt.tag.replace('/', '-')}.json"
    pending = path.with_suffix(".tmp")
    pending.write_text(json.dumps(asdict(receipt), indent=2) + "\n")
    pending.replace(path)


def _rollback(
    repo: Path,
    tag: str,
    source_commit: str,
    previous_image: str | None,
    deployed_image: str | None,
    reason: str,
) -> NoReturn:
    if previous_image is None:
        raise RuntimeError(f"{reason}; no previous image to roll back")

    try:
        _run(
            [
                "flyctl",
                "deploy",
                "--image",
                previous_image,
                "-a",
                APP,
                "--wait-timeout",
                "120",
            ],
            repo / "website",
        )
    except subprocess.CalledProcessError as error:
        raise RuntimeError(f"{reason}; rollback command failed") from error

    if not _wait_for_root():
        raise RuntimeError(f"{reason}; rollback did not restore health")

    receipt = DeployReceipt(tag, source_commit, previous_image, deployed_image, "rolled_back")
    _write_receipt(repo, receipt)
    raise RuntimeError(f"{reason}; restored {previous_image}")


def deploy_website(tag: str, repo: Path) -> DeployReceipt:
    if not os.environ.get("FLY_API_TOKEN"):
        raise RuntimeError("FLY_API_TOKEN is missing")

    source_commit = _verify_tag(repo, tag)
    if _release_is_healthy(tag):
        receipt = DeployReceipt(tag, source_commit, None, None, "unchanged")
        _write_receipt(repo, receipt)
        return receipt

    _run(
        ["uv", "run", "python", "website/dev.py", "sync-docs", "--source", "docs"],
        repo,
    )
    _run(["uv", "run", "python", "scripts/check_website_screens.py"], repo)
    previous_image = _current_image(repo)

    deploy_failed = False
    try:
        _run(
            [
                "flyctl",
                "deploy",
                "--config",
                "fly.toml",
                "--remote-only",
                "--wait-timeout",
                "120",
                "--build-arg",
                f"LOOPFLOW_RELEASE_TAG={tag}",
            ],
            repo / "website",
        )
    except subprocess.CalledProcessError:
        deploy_failed = True

    deployed_image = _current_image(repo)
    if _wait_for_release(tag):
        receipt = DeployReceipt(tag, source_commit, previous_image, deployed_image, "deployed")
        _write_receipt(repo, receipt)
        return receipt

    reason = (
        f"Fly deployment failed for {tag}" if deploy_failed else f"production did not report {tag}"
    )
    _rollback(
        repo,
        tag,
        source_commit,
        previous_image,
        deployed_image,
        reason,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Deploy one tagged Loopflow website release")
    parser.add_argument("--tag", required=True)
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
    )
    args = parser.parse_args()
    repo = args.repo.resolve()
    main_repo = Path(os.environ.get("LF_RELEASE_MAIN_REPO", repo))
    lock_dir = main_repo / ".lf" / "locks"
    lock_dir.mkdir(parents=True, exist_ok=True)
    with (lock_dir / "website-deploy.lock").open("w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError("another website deployment is already running") from error
        receipt = deploy_website(args.tag, repo)
    print(json.dumps(asdict(receipt), sort_keys=True))


if __name__ == "__main__":
    main()
