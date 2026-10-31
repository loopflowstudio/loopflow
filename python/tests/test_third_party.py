from __future__ import annotations

import gzip
import hashlib
import io
import json
import shutil
import subprocess
import tarfile
import zipfile
from pathlib import Path

import pytest

from scripts import third_party


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value))


def _source_archive(path: Path, files: dict[str, bytes]) -> None:
    with (
        path.open("wb") as raw,
        gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as compressed,
        tarfile.open(fileobj=compressed, mode="w") as archive,
    ):
        for name, content in files.items():
            member = tarfile.TarInfo(name)
            member.size = len(content)
            archive.addfile(member, io.BytesIO(content))


def test_packaged_source_retains_source_and_notices_without_prebuilt_fixtures(
    tmp_path: Path,
) -> None:
    files = {"breakpad/crash.c": b"int main(void) { return 1; }", "breakpad/LICENSE": b"notice"}
    upstream, expected, packaged = (
        tmp_path / name for name in ["upstream.gz", "expected.gz", "packaged.gz"]
    )
    _source_archive(upstream, files | {"breakpad/crash": b"prebuilt executable"})
    _source_archive(expected, files)
    specification = {
        "packaged": {
            "omit_binary_fixtures": ["breakpad/crash"],
            "sha256": third_party._sha(expected),
        }
    }
    third_party._package_source(upstream, packaged, specification)
    assert packaged.read_bytes() == expected.read_bytes()
    specification["packaged"]["omit_binary_fixtures"] = ["breakpad/missing"]
    with pytest.raises(ValueError, match="audited fixture omissions"):
        third_party._package_source(upstream, packaged, specification)


def _packaged_materials(root: Path) -> Path:
    notice = root / "ThirdPartyNotices"
    sources = root / "ThirdPartySources"
    sources.mkdir(parents=True)
    notice.mkdir()
    (notice / "NOTICES.txt").write_text("Example license")
    source = b"example corresponding source"
    component = {"id": "example", "source": {"sha256": hashlib.sha256(source).hexdigest()}}
    _write_json(notice / "inventory.json", {"components": [component]})
    (sources / "example.tar.gz").write_bytes(source)
    with zipfile.ZipFile(sources / "GhosttyKit.zip", "w"):
        pass
    for name in [
        "loopflow.tar.gz",
        "REBUILD.md",
        "rebuild.py",
        "app_bundle.py",
        "libintl-arm64.a",
        "libintl-x86_64.a",
    ]:
        (sources / name).write_text(name)
    _write_json(
        notice / "contents.json",
        {
            str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob("*")
            if p.is_file()
        },
    )
    return root


@pytest.mark.parametrize(
    "missing",
    [
        "ThirdPartyNotices/NOTICES.txt",
        "ThirdPartySources/example.tar.gz",
        "ThirdPartySources/libintl-arm64.a",
        "ThirdPartySources/rebuild.py",
        "ThirdPartySources/app_bundle.py",
    ],
)
def test_final_bundle_rejects_missing_required_material(tmp_path: Path, missing: str) -> None:
    bundle = _packaged_materials(tmp_path / "Loopflow.app/Contents/Resources")
    third_party.verify(bundle)
    (bundle / missing).unlink()
    with pytest.raises(ValueError, match="Missing or corrupted"):
        third_party.verify(bundle)


def test_archive_is_verified_after_compression(tmp_path: Path) -> None:
    framework = _packaged_materials(tmp_path / "GhosttyKit.xcframework")
    archive = tmp_path / "GhosttyKit.zip"
    with zipfile.ZipFile(archive, "w") as package:
        for path in framework.rglob("*"):
            if path.is_file():
                package.write(path, str(path.relative_to(tmp_path)))
    third_party.verify_archive(archive)
    with (
        pytest.warns(UserWarning, match="Duplicate name"),
        zipfile.ZipFile(archive, "a") as package,
    ):
        # A changed receipt cannot excuse source that differs from the inventory.
        package.writestr("GhosttyKit.xcframework/ThirdPartySources/example.tar.gz", "wrong source")
    with pytest.raises(ValueError, match="Missing or corrupted"):
        third_party.verify_archive(archive)


def test_packaging_receipt_cannot_omit_required_sources(tmp_path: Path) -> None:
    bundle = _packaged_materials(tmp_path)
    receipt = bundle / "ThirdPartyNotices/contents.json"
    data = json.loads(receipt.read_text())
    del data["ThirdPartySources/example.tar.gz"]
    _write_json(receipt, data)
    with pytest.raises(ValueError, match="Incomplete"):
        third_party.verify(bundle)


def test_final_bundle_checks_the_pinned_packaged_source_hash(tmp_path: Path) -> None:
    bundle = _packaged_materials(tmp_path)
    source = bundle / "ThirdPartySources/example.tar.gz"
    inventory_path = bundle / "ThirdPartyNotices/inventory.json"
    inventory = json.loads(inventory_path.read_text())
    inventory["components"][0]["source"]["packaged"] = {
        "sha256": third_party._sha(source),
        "omit_binary_fixtures": ["prebuilt-fixture"],
    }
    inventory["components"][0]["source"]["sha256"] = hashlib.sha256(b"upstream archive").hexdigest()
    _write_json(inventory_path, inventory)
    receipt_path = bundle / "ThirdPartyNotices/contents.json"
    receipt = json.loads(receipt_path.read_text())
    receipt["ThirdPartyNotices/inventory.json"] = third_party._sha(inventory_path)
    _write_json(receipt_path, receipt)
    third_party.verify(bundle)
    source.write_bytes(b"upstream archive")
    receipt["ThirdPartySources/example.tar.gz"] = third_party._sha(source)
    _write_json(receipt_path, receipt)
    with pytest.raises(ValueError, match="Source does not match inventory"):
        third_party.verify(bundle)


def test_shipped_inputs_have_current_notices() -> None:
    inventory = third_party.check()
    names = {c["id"] for c in inventory["components"]}
    assert {"ghostty", "gettext", "z2d", "nerd-glyphs", "app-fonts"} <= names
    assert any(name.startswith("option-ext@") for name in names)


def test_resource_update_invalidates_inventory(tmp_path: Path) -> None:
    root = third_party.ROOT
    paths = third_party._inputs(root)
    for name in paths:
        target = tmp_path / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(root / name, target)
    shutil.copytree(root / third_party.NOTICES, tmp_path / third_party.NOTICES)
    font = tmp_path / "swift/Loopflow/Fonts/JetBrainsMono-Regular.ttf"
    font.write_bytes(font.read_bytes() + b"changed")
    with pytest.raises(ValueError, match="inputs changed"):
        third_party.check(tmp_path)


def test_uncombined_library_can_link_without_the_rest(tmp_path: Path) -> None:
    if not shutil.which("cc") or not shutil.which("ar"):
        pytest.skip("C toolchain unavailable")
    for name, code in [
        ("intl", "int message(void) { return 42; }"),
        ("other", "int unrelated(void) { return 1; }"),
    ]:
        (tmp_path / f"{name}.c").write_text(code)
        subprocess.run(["cc", "-c", f"{name}.c", "-o", f"{name}.o"], cwd=tmp_path, check=True)
    combined = tmp_path / "combined.a"
    subprocess.run(["ar", "qc", str(combined), "intl.o", "other.o"], cwd=tmp_path, check=True)
    separate = tmp_path / "libintl.a"
    third_party._split_libintl(combined, ["intl.o"], separate)
    (tmp_path / "main.c").write_text("int message(void); int main(void) {return message() != 42;}")
    subprocess.run(["cc", "main.c", str(separate), "-o", "proof"], cwd=tmp_path, check=True)
    assert subprocess.run([str(tmp_path / "proof")], check=False).returncode == 0
    members = subprocess.check_output(["ar", "t", str(separate)], text=True).splitlines()
    assert "intl.o" in members and "other.o" not in members


def test_source_snapshot_includes_dirty_work_and_excludes_scratch(tmp_path: Path) -> None:
    subprocess.run(["git", "init", "-q", str(tmp_path)], check=True)
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scratch").mkdir()
    (tmp_path / "scripts/rebuild.py").write_text("current source")
    (tmp_path / "scratch/private.md").write_text("working notes")
    subprocess.run(["git", "add", "."], cwd=tmp_path, check=True)
    (tmp_path / "scripts/rebuild.py").write_text("modified source")
    archive = tmp_path / "source.tar.gz"
    third_party._source_snapshot(tmp_path, archive)
    with tarfile.open(archive) as source:
        assert source.getnames() == ["loopflow/scripts/rebuild.py"]
        assert source.extractfile("loopflow/scripts/rebuild.py").read() == b"modified source"


def test_archive_rejects_a_library_changed_after_source_packaging(tmp_path: Path) -> None:
    framework = _packaged_materials(tmp_path / "GhosttyKit.xcframework")
    library = framework / "macos/libghostty.a"
    library.parent.mkdir()
    library.write_bytes(b"replaced library")
    relink = framework / "ThirdPartySources/GhosttyKit.zip"
    with zipfile.ZipFile(relink, "w") as package:
        package.writestr("GhosttyKit.xcframework/macos/libghostty.a", b"original library")
    receipt = framework / "ThirdPartyNotices/contents.json"
    data = json.loads(receipt.read_text())
    data["ThirdPartySources/GhosttyKit.zip"] = hashlib.sha256(relink.read_bytes()).hexdigest()
    _write_json(receipt, data)
    archive = tmp_path / "GhosttyKit.zip"
    with zipfile.ZipFile(archive, "w") as package:
        for path in framework.rglob("*"):
            if path.is_file():
                package.write(path, str(path.relative_to(tmp_path)))
    with pytest.raises(ValueError, match="Relink framework differs"):
        third_party.verify_archive(archive)
