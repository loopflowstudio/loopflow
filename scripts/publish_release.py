#!/usr/bin/env python3

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import platform
import shlex
import shutil
import subprocess
import tarfile
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path

import boto3

ROOT = Path(__file__).resolve().parent.parent
TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
)
REQUIRED_SECRETS = (
    "CARGO_REGISTRY_TOKEN",
    "FLY_API_TOKEN",
    "NOTARY_ISSUER",
    "NOTARY_KEY",
    "NOTARY_KEY_ID",
    "R2_ACCESS_KEY_ID",
    "R2_ACCOUNT_ID",
    "R2_SECRET_ACCESS_KEY",
)


@dataclass(frozen=True)
class ReleaseArtifacts:
    tag: str
    native_archives: tuple[Path, ...]
    dmg: Path
    installer: Path


@dataclass(frozen=True)
class PublishReceipt:
    tag: str
    source_commit: str
    workflow_run_id: str | None
    artifact_sha256: dict[str, str]
    completed_stages: tuple[str, ...]


def _run(
    cmd: list[str],
    *,
    cwd: Path = ROOT,
    capture: bool = False,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    print(f"$ {shlex.join(cmd)}", flush=True)
    return subprocess.run(
        cmd,
        cwd=cwd,
        check=True,
        capture_output=capture,
        text=True,
        env=env,
    )


def _r2_client():
    return boto3.client(
        "s3",
        endpoint_url=(
            f"https://{os.environ['R2_ACCOUNT_ID'].strip()}.r2.cloudflarestorage.com"
        ),
        aws_access_key_id=os.environ["R2_ACCESS_KEY_ID"].strip(),
        aws_secret_access_key=os.environ["R2_SECRET_ACCESS_KEY"].strip(),
        region_name="auto",
    )


def check_release_host() -> None:
    required_commands = ("cargo", "flyctl", "gh", "lf", "security", "swift", "uv", "xcrun")
    missing_commands = [command for command in required_commands if shutil.which(command) is None]
    missing_secrets = [name for name in REQUIRED_SECRETS if not os.environ.get(name)]
    if missing_commands or missing_secrets:
        details = []
        if missing_commands:
            details.append(f"commands: {', '.join(missing_commands)}")
        if missing_secrets:
            details.append(f"Doppler secrets: {', '.join(missing_secrets)}")
        raise RuntimeError("release host is missing " + "; ".join(details))
    if platform.system() != "Darwin" or platform.machine() not in {"arm64", "aarch64"}:
        raise RuntimeError("release publisher requires an Apple Silicon macOS host")

    identity = _run(
        ["security", "find-identity", "-v", "-p", "codesigning"],
        capture=True,
    )
    if "Developer ID Application" not in identity.stdout:
        raise RuntimeError("Developer ID Application signing identity is unavailable")
    _run(["gh", "auth", "status"], capture=True)
    _run(["flyctl", "status", "-a", "loopflow-website"], capture=True)
    _r2_client().head_bucket(Bucket="downloads")
    print("Release host preflight passed", flush=True)


def _find_native_archives(artifact_dir: Path) -> tuple[Path, ...]:
    archives = []
    for target in TARGETS:
        matches = list(artifact_dir.rglob(f"lf-{target}.tar.gz"))
        if len(matches) != 1:
            raise RuntimeError(
                f"expected one lf-{target}.tar.gz artifact, found {len(matches)}"
            )
        archives.append(matches[0])
    return tuple(archives)


def _extract_arm_binaries(archives: tuple[Path, ...], output_dir: Path) -> tuple[Path, Path]:
    arm_archive = next(path for path in archives if "aarch64-apple-darwin" in path.name)
    with tarfile.open(arm_archive, "r:gz") as package:
        members = package.getmembers()
        if sorted(member.name for member in members) != ["lf", "lfd"] or not all(
            member.isfile() for member in members
        ):
            raise RuntimeError(f"unexpected archive contents in {arm_archive.name}")
        binaries = []
        for name in ("lf", "lfd"):
            member = next(member for member in members if member.name == name)
            source = package.extractfile(member)
            if source is None:
                raise RuntimeError(f"could not read {name} from {arm_archive.name}")
            binary = output_dir / name
            with binary.open("wb") as destination:
                shutil.copyfileobj(source, destination)
            binary.chmod(0o755)
            binaries.append(binary)
    return binaries[0], binaries[1]


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _stage_github_release(artifacts: ReleaseArtifacts) -> None:
    command = [
        "lf",
        "release",
        "publish",
        artifacts.tag,
        "--notes",
        str(ROOT / "RELEASE_NOTES.md"),
    ]
    for asset in (*artifacts.native_archives, artifacts.dmg, artifacts.installer):
        command.extend(["--asset", str(asset)])
    _run(command)


def _publish_crate() -> None:
    result = subprocess.run(
        ["cargo", "publish", "-p", "loopflow"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    output = f"{result.stdout}\n{result.stderr}".strip()
    if result.returncode == 0:
        print(output, flush=True)
        return
    if "already exists on crates.io index" in output or "already uploaded" in output:
        print("Version already exists on crates.io; continuing", flush=True)
        return
    raise RuntimeError(f"cargo publish failed\n{output}")


def _upload_dmg(dmg: Path, key: str, cache_control: str) -> None:
    print(f"Uploading {key} to R2", flush=True)
    _r2_client().upload_file(
        str(dmg),
        "downloads",
        key,
        ExtraArgs={
            "ContentType": "application/x-apple-diskimage",
            "CacheControl": cache_control,
        },
    )


def _write_receipt(receipt: PublishReceipt) -> None:
    main_repo = Path(os.environ.get("LF_RELEASE_MAIN_REPO", ROOT))
    log_dir = main_repo / ".lf" / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    path = log_dir / f"release.{receipt.tag.replace('/', '-')}.json"
    pending = path.with_suffix(".tmp")
    pending.write_text(json.dumps(asdict(receipt), indent=2, sort_keys=True) + "\n")
    pending.replace(path)


def publish_release(tag: str, artifact_dir: Path) -> PublishReceipt:
    check_release_host()
    if tag not in _run(["git", "tag", "--points-at", "HEAD"], capture=True).stdout.splitlines():
        raise RuntimeError(f"publisher checkout is not tagged {tag}")
    source_commit = _run(["git", "rev-parse", "HEAD"], capture=True).stdout.strip()

    archives = _find_native_archives(artifact_dir)
    installer = ROOT / "release" / "install.sh"
    if not installer.is_file():
        raise RuntimeError(f"installer not found: {installer}")

    stages: list[str] = ["artifacts_verified"]
    with tempfile.TemporaryDirectory() as temp:
        arm_binary, _arm_daemon = _extract_arm_binaries(archives, Path(temp))
        env = {
            **os.environ,
            "LF_RELEASE_BINARY": str(arm_binary),
            "LOOPFLOW_BUILD_PROVENANCE": "release",
            "LOOPFLOW_MIGRATION_AUTHORITY": "published",
            "RELEASE_TAG": tag,
        }
        _run(["python3", "-u", "scripts/release-loopflow.py"], env=env)

    dmg = ROOT / "swift" / "dist" / "Loopflow.dmg"
    if not dmg.is_file():
        raise RuntimeError(f"DMG builder did not produce {dmg}")
    stages.append("dmg_notarized")

    artifacts = ReleaseArtifacts(tag, archives, dmg, installer)
    _stage_github_release(artifacts)
    stages.append("github_draft_staged")

    _publish_crate()
    stages.append("crate_published")

    version = tag.removeprefix("v")
    _upload_dmg(dmg, f"Loopflow-{version}.dmg", "public, max-age=31536000, immutable")
    stages.append("versioned_dmg_uploaded")

    _run(["uv", "run", "python", "scripts/deploy_website.py", "--tag", tag])
    stages.append("website_deployed")

    _upload_dmg(dmg, "Loopflow-latest.dmg", "public, max-age=60")
    stages.append("latest_dmg_uploaded")

    _run(["lf", "release", "publish", tag, "--finalize"])
    stages.append("github_release_published")

    paths = (*archives, dmg, installer)
    receipt = PublishReceipt(
        tag=tag,
        source_commit=source_commit,
        workflow_run_id=os.environ.get("LF_RELEASE_WORKFLOW_RUN_ID"),
        artifact_sha256={path.name: _sha256(path) for path in paths},
        completed_stages=tuple(stages),
    )
    _write_receipt(receipt)
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description="Publish a Loopflow release from the cron host")
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("check")
    publish = subparsers.add_parser("publish")
    publish.add_argument("--tag", required=True)
    publish.add_argument("--artifacts", type=Path, required=True)
    args = parser.parse_args()

    if args.command == "check":
        check_release_host()
        return

    main_repo = Path(os.environ.get("LF_RELEASE_MAIN_REPO", ROOT))
    lock_dir = main_repo / ".lf" / "locks"
    lock_dir.mkdir(parents=True, exist_ok=True)
    with (lock_dir / "release-publish.lock").open("w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError("another release publisher is already running") from error
        receipt = publish_release(args.tag, args.artifacts)
    print(json.dumps(asdict(receipt), sort_keys=True))


if __name__ == "__main__":
    main()
