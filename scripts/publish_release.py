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
import sys
import tarfile
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import asdict, dataclass, replace
from pathlib import Path

import boto3
from botocore.exceptions import ClientError

CONTROL_ROOT = Path(__file__).resolve().parent.parent
ROOT = Path(os.environ.get("LF_RELEASE_SOURCE_REPO", CONTROL_ROOT))
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
CANDIDATE_STAGES = (
    "artifacts_verified",
    "installer_verified",
    "dmg_notarized",
    "website_candidate_verified",
)


@dataclass(frozen=True)
class ReleaseArtifacts:
    tag: str
    native_archives: tuple[Path, ...]
    dmg: Path
    installer: Path
    checksums: Path


@dataclass(frozen=True)
class ArtifactReceipt:
    tag: str
    source_commit: str
    workflow_run_id: str | None
    artifact_sha256: dict[str, str]
    completed_stages: tuple[str, ...]


@dataclass(frozen=True)
class PublicReleaseReceipt(ArtifactReceipt):
    verified_at: int
    asset_urls: dict[str, str]
    platform: str
    smoke_versions: dict[str, str]
    versioned_dmg_url: str
    latest_dmg_url: str


def _run(
    cmd: list[str],
    *,
    cwd: Path = ROOT,
    capture: bool = False,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    print(f"$ {shlex.join(cmd)}", file=sys.stderr, flush=True)
    return subprocess.run(
        cmd,
        cwd=cwd,
        check=check,
        capture_output=capture,
        text=True,
        env=env,
        pass_fds=_release_fds(),
    )


def _release_fds() -> tuple[int, ...]:
    descriptors = []
    for name in ("LF_RELEASE_LOCK_FD", "LF_WORKTREE_LEASE_FD"):
        value = os.environ.get(name)
        if value is None:
            continue
        fd = int(value)
        if fd < 3:
            raise RuntimeError(f"invalid inherited descriptor: {name}")
        try:
            os.fstat(fd)
        except OSError as error:
            raise RuntimeError(
                f"publisher launcher dropped inherited descriptor: {name}"
            ) from error
        descriptors.append(fd)
    return tuple(descriptors)


def _r2_client():
    return boto3.client(
        "s3",
        endpoint_url=(f"https://{os.environ['R2_ACCOUNT_ID'].strip()}.r2.cloudflarestorage.com"),
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
            raise RuntimeError(f"expected one lf-{target}.tar.gz artifact, found {len(matches)}")
        archives.append(matches[0])
    return tuple(archives)


def _extract_arm_binary(archives: tuple[Path, ...], output_dir: Path) -> Path:
    arm_archive = next(path for path in archives if "aarch64-apple-darwin" in path.name)
    with tarfile.open(arm_archive, "r:gz") as package:
        members = package.getmembers()
        if sorted(member.name for member in members) != ["lf"] or not all(
            member.isfile() for member in members
        ):
            raise RuntimeError(f"unexpected archive contents in {arm_archive.name}")
        binaries = []
        for name in ("lf",):
            member = next(member for member in members if member.name == name)
            source = package.extractfile(member)
            if source is None:
                raise RuntimeError(f"could not read {name} from {arm_archive.name}")
            binary = output_dir / name
            with binary.open("wb") as destination:
                shutil.copyfileobj(source, destination)
            binary.chmod(0o755)
            binaries.append(binary)
    return binaries[0]


def _validate_release_candidate(binary: Path, scratch: Path) -> None:
    home = scratch / "preflight-home"
    home.mkdir()
    result = _run(
        [str(binary), "install", "preflight", "--json"],
        capture=True,
        check=False,
        env={**os.environ, "LF_HOME": str(home)},
    )
    try:
        preview = json.loads(result.stdout)
        candidate = preview["candidate"]
    except (KeyError, TypeError, json.JSONDecodeError) as exc:
        raise RuntimeError("release candidate did not emit a promotion identity") from exc
    if candidate.get("authority") != "published":
        raise RuntimeError("release candidate has validation-only migration authority")
    verdict = preview.get("verdict", {})
    if result.returncode != 0 or verdict.get("kind") not in {"promote", "promote_and_migrate"}:
        reasons = "; ".join(verdict.get("reasons", [])) or result.stderr.strip()
        raise RuntimeError(f"release candidate cannot install into a fresh Home: {reasons}")


def inspect_source(commit: str, tag: str, *, check_publication: bool) -> dict[str, object]:
    """Inspect immutable source; external uncertainty never means unpublished."""
    files = _run(
        [
            "git",
            "ls-tree",
            "-r",
            "--name-only",
            commit,
            "--",
            "rust/loopflow/src/store/migrations/drafts",
        ],
        capture=True,
    ).stdout.splitlines()
    drafts = [name for name in files if name.endswith(".sql")]
    publications: list[str] | None = None
    if drafts and check_publication:
        publications = []
        release = _run(
            ["gh", "release", "view", tag, "--json", "isDraft"],
            capture=True,
            check=False,
        )
        if release.returncode == 0:
            publications.append("GitHub Release (draft or published)")
        elif release.stderr.strip() != "release not found":
            raise RuntimeError(
                f"cannot establish GitHub publication state: {release.stderr.strip()}"
            )
        version = tag.removeprefix("v")
        request = urllib.request.Request(
            f"https://crates.io/api/v1/crates/loopflow/{version}",
            headers={"User-Agent": "loopflow-release"},
        )
        try:
            with urllib.request.urlopen(request, timeout=30):
                publications.append("crates.io")
        except urllib.error.HTTPError as error:
            if error.code != 404:
                raise
        try:
            _r2_client().head_object(Bucket="downloads", Key=f"Loopflow-{version}.dmg")
            publications.append("versioned DMG")
        except ClientError as error:
            if error.response["Error"]["Code"] not in {"404", "NoSuchKey", "NotFound"}:
                raise
    return {"preparation_required": drafts, "publications": publications}


def _validate_archives(artifact_dir: Path) -> None:
    with tempfile.TemporaryDirectory() as temp:
        scratch = Path(temp)
        binary = _extract_arm_binary(_find_native_archives(artifact_dir), scratch)
        _validate_release_candidate(binary, scratch)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _write_checksums(paths: tuple[Path, ...], destination: Path) -> None:
    lines = [f"{_sha256(path)}  {path.name}" for path in paths]
    destination.write_text("\n".join(lines) + "\n")


def _stage_github_release(artifacts: ReleaseArtifacts) -> None:
    command = [
        "lf",
        "release",
        "publish",
        artifacts.tag,
        "--notes",
        str(ROOT / "RELEASE_NOTES.md"),
    ]
    for asset in (
        *artifacts.native_archives,
        artifacts.dmg,
        artifacts.installer,
        artifacts.checksums,
    ):
        command.extend(["--asset", str(asset)])
    _run(command)


def _publish_crate() -> None:
    result = subprocess.run(
        ["cargo", "publish", "-p", "loopflow"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        pass_fds=_release_fds(),
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


def _write_receipt(receipt: ArtifactReceipt, suffix: str = "") -> None:
    main_repo = Path(os.environ.get("LF_RELEASE_MAIN_REPO", ROOT))
    log_dir = main_repo / ".lf" / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    path = log_dir / f"release.{receipt.tag.replace('/', '-')}{suffix}.json"
    with tempfile.NamedTemporaryFile(mode="w", dir=log_dir, delete=False) as pending:
        json.dump(asdict(receipt), pending, indent=2, sort_keys=True)
        pending.write("\n")
        pending.flush()
        os.fsync(pending.fileno())
    Path(pending.name).replace(path)
    directory = os.open(log_dir, os.O_RDONLY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def _record_repaired_stage(receipt: ArtifactReceipt, stage: str) -> ArtifactReceipt:
    receipt = replace(
        receipt, completed_stages=tuple(dict.fromkeys((*receipt.completed_stages, stage)))
    )
    _write_receipt(receipt)
    return receipt


def _candidate_receipt_path(artifact_dir: Path) -> Path:
    return artifact_dir / "candidate.json"


def _read_candidate_receipt(artifact_dir: Path) -> ArtifactReceipt:
    try:
        value = json.loads(_candidate_receipt_path(artifact_dir).read_text())
        return ArtifactReceipt(
            tag=value["tag"],
            source_commit=value["source_commit"],
            workflow_run_id=value["workflow_run_id"],
            artifact_sha256=value["artifact_sha256"],
            completed_stages=tuple(value["completed_stages"]),
        )
    except (KeyError, TypeError, json.JSONDecodeError, OSError) as exc:
        raise RuntimeError("prepared release candidate receipt is invalid") from exc


def _verify_candidate_receipt(
    receipt: ArtifactReceipt,
    artifact_dir: Path,
    tag: str,
    source_commit: str,
) -> None:
    if receipt.tag != tag or receipt.source_commit != source_commit:
        raise RuntimeError(
            f"prepared release candidate identity does not match {tag} at {source_commit}"
        )
    workflow_run_id = os.environ.get("LF_RELEASE_WORKFLOW_RUN_ID")
    if workflow_run_id and receipt.workflow_run_id != workflow_run_id:
        raise RuntimeError("prepared release candidate came from a different workflow run")
    expected_artifacts = {
        *(f"lf-{target}.tar.gz" for target in TARGETS),
        "Loopflow.dmg",
        "install.sh",
        "SHA256SUMS",
    }
    if set(receipt.artifact_sha256) != expected_artifacts:
        raise RuntimeError("prepared release candidate artifact set is incomplete")
    if receipt.completed_stages != CANDIDATE_STAGES:
        raise RuntimeError("prepared release candidate proof is incomplete")
    for name, expected in receipt.artifact_sha256.items():
        path = artifact_dir / name
        if not path.is_file() or _sha256(path) != expected:
            raise RuntimeError(f"prepared release artifact changed: {name}")


def prepare_release(tag: str, artifact_dir: Path, output_dir: Path) -> ArtifactReceipt:
    check_release_host()
    source_commit = _run(["git", "rev-parse", "HEAD"], capture=True).stdout.strip()

    if _candidate_receipt_path(output_dir).is_file():
        try:
            receipt = _read_candidate_receipt(output_dir)
            _verify_candidate_receipt(receipt, output_dir, tag, source_commit)
        except RuntimeError as error:
            print(f"Rebuilding invalid prepared candidate: {error}", flush=True)
        else:
            _validate_archives(output_dir)
            _write_receipt(receipt, ".candidate")
            return receipt

    archives = _find_native_archives(artifact_dir)
    installer = ROOT / "release" / "install.sh"
    if not installer.is_file():
        raise RuntimeError(f"installer not found: {installer}")

    stages: list[str] = [CANDIDATE_STAGES[0]]
    with tempfile.TemporaryDirectory() as temp:
        scratch = Path(temp)
        arm_binary = _extract_arm_binary(archives, scratch)
        _validate_release_candidate(arm_binary, scratch)
        _run(["sh", "-n", str(installer)])
        stages.append(CANDIDATE_STAGES[1])
        env = {
            **os.environ,
            "LF_RELEASE_BINARY": str(arm_binary),
            "LOOPFLOW_BUILD_PROVENANCE": "release",
            "LOOPFLOW_MIGRATION_AUTHORITY": "published",
            "RELEASE_TAG": tag,
        }
        _run(["python3", "-u", "scripts/release-loopflow.py"], env=env)
        stages.append(CANDIDATE_STAGES[2])
        _run(["uv", "run", "python", "website/dev.py", "sync-docs", "--source", "docs"])
        _run(["uv", "run", "python", "scripts/check_website_screens.py"])
        stages.append(CANDIDATE_STAGES[3])

    dmg = ROOT / "swift" / "dist" / "Loopflow.dmg"
    if not dmg.is_file():
        raise RuntimeError(f"DMG builder did not produce {dmg}")

    output_dir.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output_dir.parent) as temp:
        prepared = Path(temp) / "candidate"
        prepared.mkdir()
        for archive in archives:
            shutil.copy2(archive, prepared / archive.name)
        shutil.copy2(dmg, prepared / dmg.name)
        shutil.copy2(installer, prepared / installer.name)
        paths = tuple(
            sorted(
                (path for path in prepared.iterdir() if path.is_file()),
                key=lambda path: path.name,
            )
        )
        checksums = prepared / "SHA256SUMS"
        _write_checksums(paths, checksums)
        paths = (*paths, checksums)
        receipt = ArtifactReceipt(
            tag=tag,
            source_commit=source_commit,
            workflow_run_id=os.environ.get("LF_RELEASE_WORKFLOW_RUN_ID"),
            artifact_sha256={path.name: _sha256(path) for path in paths},
            completed_stages=tuple(stages),
        )
        _candidate_receipt_path(prepared).write_text(
            json.dumps(asdict(receipt), indent=2, sort_keys=True) + "\n"
        )
        if output_dir.exists():
            shutil.rmtree(output_dir)
        prepared.replace(output_dir)
    _write_receipt(receipt, ".candidate")
    return receipt


def publish_release(tag: str, artifact_dir: Path) -> ArtifactReceipt:
    check_release_host()
    if tag not in _run(["git", "tag", "--points-at", "HEAD"], capture=True).stdout.splitlines():
        raise RuntimeError(f"publisher checkout is not tagged {tag}")
    source_commit = _run(["git", "rev-parse", "HEAD"], capture=True).stdout.strip()
    candidate = _read_candidate_receipt(artifact_dir)
    _verify_candidate_receipt(candidate, artifact_dir, tag, source_commit)
    _validate_archives(artifact_dir)
    archives = _find_native_archives(artifact_dir)
    dmg = artifact_dir / "Loopflow.dmg"
    installer = artifact_dir / "install.sh"
    checksums = artifact_dir / "SHA256SUMS"
    artifacts = ReleaseArtifacts(tag, archives, dmg, installer, checksums)
    stages = list(candidate.completed_stages)
    _stage_github_release(artifacts)
    stages.append("github_draft_staged")

    _publish_crate()
    stages.append("crate_published")

    version = tag.removeprefix("v")
    _upload_dmg(dmg, f"Loopflow-{version}.dmg", "public, max-age=31536000, immutable")
    stages.append("versioned_dmg_uploaded")

    _deploy_website(tag)
    stages.append("website_deployed")

    _upload_dmg(dmg, "Loopflow-latest.dmg", "public, max-age=60")
    stages.append("latest_dmg_uploaded")

    _run(["lf", "release", "publish", tag, "--finalize"])
    stages.append("github_release_published")

    paths = (*archives, dmg, installer, checksums)
    receipt = ArtifactReceipt(
        tag=tag,
        source_commit=source_commit,
        workflow_run_id=os.environ.get("LF_RELEASE_WORKFLOW_RUN_ID"),
        artifact_sha256={path.name: _sha256(path) for path in paths},
        completed_stages=tuple(stages),
    )
    _write_receipt(receipt)
    return receipt


def _deploy_website(tag: str) -> None:
    _run(
        [
            sys.executable,
            str(CONTROL_ROOT / "scripts/deploy_website.py"),
            "--tag",
            tag,
            "--repo",
            str(ROOT),
        ]
    )


def _download(url: str, destination: Path) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "loopflow-release-proof/1"})
    with urllib.request.urlopen(request, timeout=60) as response, destination.open("wb") as output:
        shutil.copyfileobj(response, output)


def _download_if_present(url: str, destination: Path) -> bool:
    try:
        _download(url, destination)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise
    return True


def _check_public_hashes(directory: Path, expected: dict[str, str]) -> None:
    required = {
        *(f"lf-{target}.tar.gz" for target in TARGETS),
        "Loopflow.dmg",
        "install.sh",
        "SHA256SUMS",
    }
    if set(expected) != required:
        raise RuntimeError("public release proof lacks the complete artifact manifest")
    for name, digest in expected.items():
        artifact = directory / name
        if not artifact.is_file() or _sha256(artifact) != digest:
            raise RuntimeError(f"public artifact hash mismatch or missing asset: {name}")


def verify_release(tag: str, *, repair: bool = False) -> PublicReleaseReceipt:
    if platform.system() != "Darwin" or platform.machine() not in {"arm64", "aarch64"}:
        raise RuntimeError("public installer smoke requires the Apple Silicon release host")
    source_commit = _run(["git", "rev-parse", "HEAD"], capture=True).stdout.strip()
    main_repo = Path(os.environ.get("LF_RELEASE_MAIN_REPO", ROOT))
    logs = main_repo / ".lf/logs"
    retained = logs / f"release.{tag.replace('/', '-')}.json"
    if not retained.exists():
        retained = logs / f"release.{tag.replace('/', '-')}.candidate.json"
    try:
        value = json.loads(retained.read_text())
        proof = ArtifactReceipt(**value)
    except (OSError, TypeError, json.JSONDecodeError) as error:
        raise RuntimeError(
            f"missing retained exact artifact proof for {tag}: {retained}"
        ) from error
    if (
        proof.tag != tag
        or proof.source_commit != source_commit
        or proof.workflow_run_id != os.environ.get("LF_RELEASE_WORKFLOW_RUN_ID")
    ):
        raise RuntimeError("public release identity differs from retained candidate proof")
    # A retained candidate proves preparation even when the publisher died before
    # its final receipt. Reconstruct publication only from public read-back.
    if not set(CANDIDATE_STAGES).issubset(proof.completed_stages):
        raise RuntimeError("retained candidate lacks required preparation verification")
    release = json.loads(
        _run(
            ["gh", "release", "view", tag, "--json", "tagName,isDraft,assets"], capture=True
        ).stdout
    )
    if release["tagName"] != tag or release["isDraft"]:
        raise RuntimeError("exact release is absent or remains a draft")
    names = {asset["name"] for asset in release["assets"]}
    if names != set(proof.artifact_sha256):
        raise RuntimeError("public release asset set differs from prepared candidate")
    version = tag.removeprefix("v")
    smoke_versions: dict[str, str] = {}
    with tempfile.TemporaryDirectory() as temp:
        scratch = Path(temp)
        for asset in release["assets"]:
            # Never send publisher credentials to downloaded code or asset URLs.
            _download(asset["url"], scratch / asset["name"])
        _check_public_hashes(scratch, proof.artifact_sha256)
        versioned_dmg = scratch / "versioned.dmg"
        versioned_url = f"https://downloads.loopflow.studio/Loopflow-{version}.dmg"
        versioned_present = _download_if_present(versioned_url, versioned_dmg)
        if versioned_present and _sha256(versioned_dmg) != proof.artifact_sha256["Loopflow.dmg"]:
            raise RuntimeError("versioned public DMG differs from prepared artifact")
        latest_dmg = scratch / "latest.dmg"
        latest_url = "https://downloads.loopflow.studio/Loopflow-latest.dmg"
        latest_matches = (
            _download_if_present(latest_url, latest_dmg)
            and _sha256(latest_dmg) == proof.artifact_sha256["Loopflow.dmg"]
        )
        health = scratch / "health.json"
        health_url = "https://loopflow.studio/healthz"
        website_matches = _download_if_present(health_url, health) and json.loads(
            health.read_text()
        ) == {"status": "ok", "release": tag}
        crate = scratch / "crate.json"
        crate_url = f"https://crates.io/api/v1/crates/loopflow/{version}"
        crate_present = _download_if_present(crate_url, crate)
        if crate_present and json.loads(crate.read_text())["version"]["num"] != version:
            raise RuntimeError("public crate version does not match release")
        missing = [
            name
            for name, present in (
                ("crate", crate_present),
                ("versioned DMG", versioned_present),
                ("website", website_matches),
                ("latest DMG", latest_matches),
            )
            if not present
        ]
        if missing:
            if not repair:
                raise RuntimeError(f"incomplete public release stages: {', '.join(missing)}")
            # Mutable endpoints must never roll back a newer release while an
            # older opportunity is being reconciled. Unknown authority is fatal.
            current = json.loads(
                _run(["gh", "release", "view", "--json", "tagName"], capture=True).stdout
            )
            if current["tagName"] != tag:
                raise RuntimeError(f"cannot repair {tag}: latest release is {current['tagName']}")
            dmg = scratch / "Loopflow.dmg"
            if not crate_present:
                _publish_crate()
                proof = _record_repaired_stage(proof, "crate_published")
            if not versioned_present:
                _upload_dmg(dmg, f"Loopflow-{version}.dmg", "public, max-age=31536000, immutable")
                proof = _record_repaired_stage(proof, "versioned_dmg_uploaded")
            if not website_matches:
                _deploy_website(tag)
                proof = _record_repaired_stage(proof, "website_deployed")
            if not latest_matches:
                _upload_dmg(dmg, "Loopflow-latest.dmg", "public, max-age=60")
                proof = _record_repaired_stage(proof, "latest_dmg_uploaded")
            # Successful mutation commands do not prove public availability.
            _download(versioned_url, versioned_dmg)
            _download(latest_url, latest_dmg)
            _download(health_url, health)
            _download(crate_url, crate)
            if (
                _sha256(versioned_dmg) != proof.artifact_sha256["Loopflow.dmg"]
                or _sha256(latest_dmg) != proof.artifact_sha256["Loopflow.dmg"]
                or json.loads(health.read_text()) != {"status": "ok", "release": tag}
                or json.loads(crate.read_text())["version"]["num"] != version
            ):
                raise RuntimeError("publication repair did not pass public read-back")
        native = scratch / "native"
        native.mkdir()
        expected_binaries = (_extract_arm_binary(_find_native_archives(scratch), native),)
        home = scratch / "home"
        home.mkdir()
        install_dir = home / "bin"
        smoke_env = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": str(home),
            "LF_HOME": str(home / ".lf"),
            "LF_INSTALL_DIR": str(install_dir),
        }
        _run(
            ["sh", str(scratch / "install.sh"), "--version", tag, "--cli-only"],
            cwd=scratch,
            env=smoke_env,
        )
        for expected_binary in expected_binaries:
            name = expected_binary.name
            binary = install_dir / name
            if _sha256(binary) != _sha256(expected_binary):
                raise RuntimeError(f"installed {name} differs from the exact public artifact")
            reported = _run(
                [str(binary), "--version"], cwd=scratch, env=smoke_env, capture=True
            ).stdout.strip()
            smoke_versions[name] = reported
            if reported != f"{name} {version}":
                raise RuntimeError(f"public {name} reported {reported!r}, expected {version}")
            _run([str(binary), "--help"], cwd=scratch, env=smoke_env, capture=True)
        _run(
            [str(install_dir / "lf"), "catalog", "--json"], cwd=scratch, env=smoke_env, capture=True
        )
    verified = PublicReleaseReceipt(
        verified_at=int(time.time()),
        asset_urls={asset["name"]: asset["url"] for asset in release["assets"]},
        platform=f"{platform.system()} {platform.machine()}",
        smoke_versions=smoke_versions,
        versioned_dmg_url=f"https://downloads.loopflow.studio/Loopflow-{version}.dmg",
        latest_dmg_url="https://downloads.loopflow.studio/Loopflow-latest.dmg",
        tag=tag,
        source_commit=source_commit,
        workflow_run_id=proof.workflow_run_id,
        artifact_sha256=proof.artifact_sha256,
        completed_stages=tuple(
            dict.fromkeys(
                (
                    *proof.completed_stages,
                    "public_artifacts_verified",
                    "versioned_dmg_verified",
                    "latest_dmg_verified",
                    "website_release_verified",
                    "crate_version_verified",
                    "exact_tag_smoke_passed",
                )
            )
        ),
    )
    _write_receipt(verified, ".verified")
    return verified


def main() -> None:
    parser = argparse.ArgumentParser(description="Publish a Loopflow release from the cron host")
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("check")
    inspect = subparsers.add_parser("inspect")
    inspect.add_argument("--commit", required=True)
    inspect.add_argument("--tag", required=True)
    inspect.add_argument("--check-publication", action="store_true")
    prepare = subparsers.add_parser("prepare")
    prepare.add_argument("--tag", required=True)
    prepare.add_argument("--artifacts", type=Path, required=True)
    prepare.add_argument("--output", type=Path, required=True)
    publish = subparsers.add_parser("publish")
    publish.add_argument("--tag", required=True)
    publish.add_argument("--artifacts", type=Path, required=True)
    verify = subparsers.add_parser("verify")
    verify.add_argument("--tag", required=True)
    reconcile = subparsers.add_parser("reconcile")
    reconcile.add_argument("--tag", required=True)
    args = parser.parse_args()

    if args.command == "inspect":
        print(
            json.dumps(
                inspect_source(args.commit, args.tag, check_publication=args.check_publication)
            )
        )
        return

    if args.command == "check":
        check_release_host()
        return
    if args.command == "inspect":
        print(
            json.dumps(
                inspect_source(
                    args.commit,
                    args.tag,
                    check_publication=args.check_publication,
                )
            )
        )
        return

    if "LF_RELEASE_LOCK_FD" not in os.environ or not _release_fds():
        raise RuntimeError(
            "publisher stages require the owning release lock; invoke lf release run"
        )
    main_repo = Path(os.environ.get("LF_RELEASE_MAIN_REPO", ROOT))
    lock_dir = main_repo / ".lf" / "locks"
    lock_dir.mkdir(parents=True, exist_ok=True)
    with (lock_dir / "release-publish.lock").open("w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError("another release publisher is already running") from error
        if args.command == "prepare":
            receipt = prepare_release(args.tag, args.artifacts, args.output)
        elif args.command == "publish":
            receipt = publish_release(args.tag, args.artifacts)
        else:
            receipt = verify_release(args.tag, repair=args.command == "reconcile")
    print(json.dumps(asdict(receipt), sort_keys=True))


if __name__ == "__main__":
    main()
