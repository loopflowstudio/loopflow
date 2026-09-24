import io
import json
import subprocess
import tarfile
from pathlib import Path

import pytest

from scripts import publish_release


def _native_artifacts(directory: Path) -> None:
    binaries = []
    for name in ("lf", "lfd"):
        binary = directory / name
        binary.write_bytes(f"loopflow release {name}".encode())
        binaries.append(binary)
    for target in publish_release.TARGETS:
        package_dir = directory / target
        package_dir.mkdir()
        with tarfile.open(package_dir / f"lf-{target}.tar.gz", "w:gz") as package:
            for binary in binaries:
                package.add(binary, arcname=binary.name)


def test_publisher_requires_the_complete_native_matrix(tmp_path: Path):
    (tmp_path / "lf-aarch64-apple-darwin.tar.gz").touch()

    with pytest.raises(RuntimeError, match="x86_64-apple-darwin"):
        publish_release._find_native_archives(tmp_path)


def test_publisher_rejects_unexpected_archive_contents(tmp_path: Path):
    archive = tmp_path / "lf-aarch64-apple-darwin.tar.gz"
    with tarfile.open(archive, "w:gz") as package:
        member = tarfile.TarInfo("../lf")
        member.size = 4
        package.addfile(member, io.BytesIO(b"nope"))

    with pytest.raises(RuntimeError, match="unexpected archive contents"):
        publish_release._extract_arm_binaries((archive,), tmp_path)


def test_publisher_extracts_the_arm_control_plane_pair(tmp_path: Path):
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    _native_artifacts(artifacts)
    archives = publish_release._find_native_archives(artifacts)
    output = tmp_path / "extracted"
    output.mkdir()

    cli, daemon = publish_release._extract_arm_binaries(archives, output)

    assert cli.read_bytes() == b"loopflow release lf"
    assert daemon.read_bytes() == b"loopflow release lfd"
    assert cli.stat().st_mode & 0o111
    assert daemon.stat().st_mode & 0o111


def test_publisher_rejects_validation_only_control_plane(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    binary = tmp_path / "lf"
    binary.touch()
    monkeypatch.setattr(
        publish_release,
        "_run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(
            [], 0, '{"candidate":{"authority":"validation_only"}}', ""
        ),
    )

    with pytest.raises(RuntimeError, match="validation-only"):
        publish_release._validate_release_candidate(binary, tmp_path)


def test_publisher_accepts_published_identity_when_home_preflight_refuses(tmp_path: Path):
    binary = tmp_path / "lf"
    binary.write_text(
        "#!/bin/sh\n"
        'echo \'{"candidate":{"authority":"published"},'
        '"verdict":{"kind":"reject"}}\'\n'
        "echo 'Error: promotion preflight refused' >&2\n"
        "exit 1\n"
    )
    binary.chmod(0o755)

    publish_release._validate_release_candidate(binary, tmp_path)


def test_publisher_prepares_exact_artifacts_before_marking_release_published(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    artifact_dir = tmp_path / "artifacts"
    artifact_dir.mkdir()
    _native_artifacts(artifact_dir)

    (tmp_path / "release").mkdir()
    (tmp_path / "release/install.sh").write_text("#!/bin/sh\n")
    (tmp_path / "RELEASE_NOTES.md").write_text("# v1.2.3\n")
    (tmp_path / "swift/dist").mkdir(parents=True)
    commands: list[list[str]] = []

    def fake_run(
        command: list[str],
        *,
        cwd: Path = tmp_path,
        capture: bool = False,
        env: dict[str, str] | None = None,
        check: bool = True,
    ) -> subprocess.CompletedProcess[str]:
        commands.append(command)
        if command[:3] == ["git", "tag", "--points-at"]:
            return subprocess.CompletedProcess(command, 0, "v1.2.3\n", "")
        if command[:3] == ["git", "rev-parse", "HEAD"]:
            return subprocess.CompletedProcess(command, 0, "abc123\n", "")
        if command[1:] == ["install", "preflight", "--json"]:
            return subprocess.CompletedProcess(
                command,
                0,
                '{"candidate":{"authority":"published"}}\n',
                "",
            )
        if command[-1:] == ["scripts/release-loopflow.py"]:
            (tmp_path / "swift/dist/Loopflow.dmg").write_bytes(b"notarized dmg")
        return subprocess.CompletedProcess(command, 0, "", "")

    monkeypatch.setattr(publish_release, "ROOT", tmp_path)
    monkeypatch.setattr(publish_release, "check_release_host", lambda: None)
    monkeypatch.setattr(publish_release, "_run", fake_run)
    monkeypatch.setattr(publish_release, "_publish_crate", lambda: None)
    monkeypatch.setattr(publish_release, "_upload_dmg", lambda *args: None)
    monkeypatch.setenv("LF_RELEASE_MAIN_REPO", str(tmp_path))
    monkeypatch.setenv("LF_RELEASE_WORKFLOW_RUN_ID", "42")

    prepared_dir = tmp_path / "prepared"
    candidate = publish_release.prepare_release("v1.2.3", artifact_dir, prepared_dir)
    (prepared_dir / "Loopflow.dmg").write_bytes(b"corrupt")
    candidate = publish_release.prepare_release("v1.2.3", artifact_dir, prepared_dir)
    prepare_commands = len(commands)
    receipt = publish_release.publish_release("v1.2.3", prepared_dir)

    assert candidate.source_commit == "abc123"
    assert candidate.completed_stages == (
        "artifacts_verified",
        "installer_verified",
        "dmg_notarized",
        "website_candidate_verified",
        "ui_host_verified",
    )
    assert receipt.workflow_run_id == "42"
    assert receipt.source_commit == "abc123"
    assert receipt.completed_stages == (
        "artifacts_verified",
        "installer_verified",
        "dmg_notarized",
        "website_candidate_verified",
        "ui_host_verified",
        "github_draft_staged",
        "crate_published",
        "versioned_dmg_uploaded",
        "website_deployed",
        "latest_dmg_uploaded",
        "github_release_published",
    )
    assert set(receipt.artifact_sha256) == {
        *(f"lf-{target}.tar.gz" for target in publish_release.TARGETS),
        "Loopflow.dmg",
        "install.sh",
        "SHA256SUMS",
    }
    checksum_lines = (prepared_dir / "SHA256SUMS").read_text().splitlines()
    assert {line.split(maxsplit=1)[1] for line in checksum_lines} == {
        *(f"lf-{target}.tar.gz" for target in publish_release.TARGETS),
        "Loopflow.dmg",
        "install.sh",
    }
    retained = json.loads((tmp_path / ".lf/logs/release.v1.2.3.json").read_text())
    assert retained["artifact_sha256"] == receipt.artifact_sha256
    prepared = json.loads((tmp_path / ".lf/logs/release.v1.2.3.candidate.json").read_text())
    assert prepared["artifact_sha256"] == receipt.artifact_sha256
    assert not any(
        command[:3] == ["lf", "release", "publish"] for command in commands[:prepare_commands]
    )
    deploy = next(command for command in commands if "deploy_website.py" in command[1])
    assert deploy[1] == str(publish_release.CONTROL_ROOT / "scripts/deploy_website.py")
    assert deploy[-2:] == ["--repo", str(tmp_path)]


def test_public_artifact_hashes_reject_modified_and_missing_assets(tmp_path: Path):
    names = {
        *(f"lf-{target}.tar.gz" for target in publish_release.TARGETS),
        "Loopflow.dmg",
        "install.sh",
        "SHA256SUMS",
    }
    for name in names:
        (tmp_path / name).write_bytes(name.encode())
    hashes = {name: publish_release._sha256(tmp_path / name) for name in names}
    publish_release._check_public_hashes(tmp_path, hashes)
    (tmp_path / "Loopflow.dmg").write_bytes(b"substituted")
    with pytest.raises(RuntimeError, match="Loopflow.dmg"):
        publish_release._check_public_hashes(tmp_path, hashes)
    (tmp_path / "Loopflow.dmg").unlink()
    with pytest.raises(RuntimeError, match="Loopflow.dmg"):
        publish_release._check_public_hashes(tmp_path, hashes)


def test_public_proof_recovers_after_publisher_dies_before_final_receipt(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    for target in publish_release.TARGETS:
        with tarfile.open(artifacts / f"lf-{target}.tar.gz", "w:gz") as package:
            for name in ("lf", "lfd"):
                body = f"#!/bin/sh\necho '{name} 1.2.3'\n".encode()
                info = tarfile.TarInfo(name)
                info.mode = 0o755
                info.size = len(body)
                package.addfile(info, io.BytesIO(body))
    (artifacts / "Loopflow.dmg").write_bytes(b"notarized artifact")
    (artifacts / "install.sh").write_text(
        '#!/bin/sh\nmkdir -p "$LF_INSTALL_DIR"\ncp native/lf native/lfd "$LF_INSTALL_DIR/"\n'
    )
    publish_release._write_checksums(tuple(artifacts.iterdir()), artifacts / "SHA256SUMS")
    hashes = {p.name: publish_release._sha256(p) for p in artifacts.iterdir()}
    monkeypatch.setenv("LF_RELEASE_MAIN_REPO", str(tmp_path))
    monkeypatch.setenv("LF_RELEASE_WORKFLOW_RUN_ID", "42")
    monkeypatch.delenv("LF_RELEASE_LOCK_FD", raising=False)
    monkeypatch.setattr(publish_release.platform, "system", lambda: "Darwin")
    monkeypatch.setattr(publish_release.platform, "machine", lambda: "arm64")
    candidate = publish_release.CandidateReceipt(
        "v1.2.3", "exact-commit", "42", hashes, publish_release.CANDIDATE_STAGES
    )
    publish_release._write_receipt(candidate, ".candidate")
    run = publish_release._run

    def external_command(command, **kwargs):
        if command[:3] == ["git", "rev-parse", "HEAD"]:
            return subprocess.CompletedProcess(command, 0, "exact-commit\n", "")
        if command[:3] == ["gh", "release", "view"]:
            return subprocess.CompletedProcess(
                command,
                0,
                json.dumps(
                    {
                        "tagName": "v1.2.3",
                        "isDraft": False,
                        "assets": [
                            {"name": name, "url": f"https://public/{name}"} for name in hashes
                        ],
                    }
                ),
                "",
            )
        return run(command, **kwargs)

    def download(url: str, destination: Path) -> None:
        if url.endswith("/healthz"):
            destination.write_text('{"status":"ok","release":"v1.2.3"}')
        elif "crates.io" in url:
            destination.write_text('{"version":{"num":"1.2.3"}}')
        elif url.endswith("Loopflow-1.2.3.dmg"):
            destination.write_bytes((artifacts / "Loopflow.dmg").read_bytes())
        else:
            destination.write_bytes((artifacts / url.rsplit("/", 1)[1]).read_bytes())

    monkeypatch.setattr(publish_release, "_run", external_command)
    monkeypatch.setattr(publish_release, "_download", download)
    result = publish_release.verify_release("v1.2.3")
    assert result.artifact_sha256 == hashes
    assert "exact_tag_smoke_passed" in result.completed_stages
    assert "public_artifacts_verified" in result.completed_stages
    assert not (tmp_path / ".lf/logs/release.v1.2.3.json").exists()
    assert (tmp_path / ".lf/logs/release.v1.2.3.verified.json").exists()

    # Historic preparation without the required host gate cannot gain a pass
    # simply because assets are already public.
    candidate = publish_release.CandidateReceipt(
        "v1.2.3",
        "exact-commit",
        "42",
        hashes,
        tuple(s for s in publish_release.CANDIDATE_STAGES if s != "ui_host_verified"),
    )
    publish_release._write_receipt(candidate, ".candidate")

    def missing_ui(*_args):
        raise RuntimeError("required verification unavailable: ui-host")

    monkeypatch.setattr(publish_release, "_verify_ui_host", missing_ui)
    with pytest.raises(RuntimeError, match="required verification"):
        publish_release.verify_release("v1.2.3")


def test_publisher_child_retains_release_lock_through_uv(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    lock_path = tmp_path / "release.lock"
    with lock_path.open("w") as lock:
        monkeypatch.setenv("LF_RELEASE_LOCK_FD", str(lock.fileno()))
        # uv is the configured publisher launcher. This exercises descriptor
        # propagation through it and a real Python subprocess, without auth.
        result = publish_release._run(
            [
                "uv",
                "run",
                "--no-project",
                "python",
                "-c",
                "import os; print(os.fstat(int(os.environ['LF_RELEASE_LOCK_FD'])).st_ino)",
            ],
            cwd=tmp_path,
            capture=True,
        )
        assert int(result.stdout.strip()) == lock_path.stat().st_ino


def test_direct_publisher_stage_cannot_bypass_the_owning_release_operation(
    monkeypatch: pytest.MonkeyPatch,
):
    monkeypatch.delenv("LF_RELEASE_LOCK_FD", raising=False)
    monkeypatch.setattr(
        publish_release.sys, "argv", ["publish_release", "verify", "--tag", "v1.2.3"]
    )
    with pytest.raises(RuntimeError, match="invoke lf release run"):
        publish_release.main()
