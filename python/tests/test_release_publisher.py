import io
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
        "echo '{\"candidate\":{\"authority\":\"published\"},"
        "\"verdict\":{\"kind\":\"reject\"}}'\n"
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
    receipts: list[publish_release.PublishReceipt] = []
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
    monkeypatch.setattr(publish_release, "_write_receipt", receipts.append)
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
    )
    assert receipt.workflow_run_id == "42"
    assert receipt.source_commit == "abc123"
    assert receipt.completed_stages == (
        "artifacts_verified",
        "installer_verified",
        "dmg_notarized",
        "website_candidate_verified",
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
    assert receipts == [receipt]
    assert not any(
        command[:3] == ["lf", "release", "publish"]
        for command in commands[:prepare_commands]
    )
    deploy = next(command for command in commands if "deploy_website.py" in command[1])
    assert deploy[1] == str(publish_release.CONTROL_ROOT / "scripts/deploy_website.py")
    assert deploy[-2:] == ["--repo", str(tmp_path)]
