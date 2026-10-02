import io
import subprocess
import tarfile
import urllib.error
from pathlib import Path

import pytest
from botocore.exceptions import ClientError

from scripts import publish_release


def _native_artifacts(directory: Path) -> None:
    binaries = []
    for name in ("lf",):
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
        publish_release._extract_arm_binary((archive,), tmp_path)


def test_publisher_extracts_the_arm_cli(tmp_path: Path):
    artifacts = tmp_path / "artifacts"
    artifacts.mkdir()
    _native_artifacts(artifacts)
    archives = publish_release._find_native_archives(artifacts)
    output = tmp_path / "extracted"
    output.mkdir()

    cli = publish_release._extract_arm_binary(archives, output)

    assert cli.read_bytes() == b"loopflow release lf"
    assert cli.stat().st_mode & 0o111


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


def test_publisher_rejects_published_candidate_that_cannot_install(tmp_path: Path):
    binary = tmp_path / "lf"
    binary.write_text(
        "#!/bin/sh\n"
        'echo \'{"candidate":{"authority":"published"},'
        '"verdict":{"kind":"reject"}}\'\n'
        "echo 'Error: promotion preflight refused' >&2\n"
        "exit 1\n"
    )
    binary.chmod(0o755)

    with pytest.raises(RuntimeError, match="cannot install into a fresh Home"):
        publish_release._validate_release_candidate(binary, tmp_path)


@pytest.mark.parametrize("rejected_on_retry", [False, True])
def test_publisher_prepares_exact_artifacts_before_marking_release_published(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, rejected_on_retry: bool
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
    reject_candidate = False

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
            if reject_candidate:
                return subprocess.CompletedProcess(
                    command,
                    1,
                    '{"candidate":{"authority":"published"},"verdict":{"kind":"reject"}}',
                    "pending migration draft",
                )
            return subprocess.CompletedProcess(
                command,
                0,
                '{"candidate":{"authority":"published"},"verdict":{"kind":"promote"}}\n',
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
    if rejected_on_retry:
        reject_candidate = True
        with pytest.raises(RuntimeError, match="pending migration draft"):
            publish_release.prepare_release("v1.2.3", artifact_dir, prepared_dir)
        with pytest.raises(RuntimeError, match="pending migration draft"):
            publish_release.publish_release("v1.2.3", prepared_dir)
        assert receipts == []
        return
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
        command[:3] == ["lf", "release", "publish"] for command in commands[:prepare_commands]
    )
    deploy = next(command for command in commands if "deploy_website.py" in command[1])
    assert deploy[1] == str(publish_release.CONTROL_ROOT / "scripts/deploy_website.py")
    assert deploy[-2:] == ["--repo", str(tmp_path)]


@pytest.mark.parametrize("publication", ["absent", "github", "crate", "dmg", "unknown"])
def test_source_replacement_requires_known_publication_state(
    monkeypatch: pytest.MonkeyPatch, publication: str
):
    def run(command, **kwargs):
        if command[0] == "git":
            return subprocess.CompletedProcess(command, 0, "drafts/remove_ask.sql\n", "")
        if publication == "github":
            return subprocess.CompletedProcess(command, 0, '{"isDraft":true}', "")
        error = "authentication failed" if publication == "unknown" else "release not found"
        return subprocess.CompletedProcess(command, 1, "", error)

    def urlopen(*args, **kwargs):
        if publication == "crate":
            return io.BytesIO(b"{}")
        raise urllib.error.HTTPError("https://crates.io", 404, "missing", {}, None)

    class Downloads:
        def head_object(self, **kwargs):
            if publication != "dmg":
                raise ClientError({"Error": {"Code": "404"}}, "HeadObject")
            return {}

    monkeypatch.setattr(publish_release, "_run", run)
    monkeypatch.setattr(publish_release.urllib.request, "urlopen", urlopen)
    monkeypatch.setattr(publish_release, "_r2_client", Downloads)
    if publication == "unknown":
        with pytest.raises(RuntimeError, match="authentication failed"):
            publish_release.inspect_source("exact-commit", "v0.12.30", check_publication=True)
    else:
        result = publish_release.inspect_source("exact-commit", "v0.12.30", check_publication=True)
        assert result["preparation_required"] == ["drafts/remove_ask.sql"]
        assert bool(result["publications"]) == (publication != "absent")


def test_source_inspection_uses_the_commit_not_the_working_tree(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    def run(command, **kwargs):
        return subprocess.run(command, cwd=tmp_path, text=True, capture_output=True, check=True)

    monkeypatch.setattr(publish_release, "_run", run)
    run(["git", "init"])
    run(["git", "config", "user.name", "Release test"])
    run(["git", "config", "user.email", "release@example.test"])
    drafts = tmp_path / "rust/loopflow/src/store/migrations/drafts"
    drafts.mkdir(parents=True)
    (drafts / "README.md").write_text("Drafts\n")
    run(["git", "add", "."])
    run(["git", "commit", "-m", "Prepared source"])
    prepared = run(["git", "rev-parse", "HEAD"]).stdout.strip()
    (drafts / "incoming.sql").write_text("SELECT 1;\n")
    run(["git", "add", "."])
    run(["git", "commit", "-m", "Concurrent migration"])
    integrated = run(["git", "rev-parse", "HEAD"]).stdout.strip()
    (drafts / "incoming.sql").unlink()

    assert publish_release.inspect_source(prepared, "v1.2.3", check_publication=False) == {
        "preparation_required": [],
        "publications": None,
    }
    observed = publish_release.inspect_source(integrated, "v1.2.3", check_publication=False)
    assert observed["preparation_required"] == [
        "rust/loopflow/src/store/migrations/drafts/incoming.sql"
    ]
    assert observed["publications"] is None
