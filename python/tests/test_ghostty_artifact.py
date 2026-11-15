"""SwiftPM must recognize and link the packaged static library."""

import importlib.util
import plistlib
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "loopflow-dev.py"
SPEC = importlib.util.spec_from_file_location("loopflow_dev", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
loopflow_dev = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(loopflow_dev)


def test_static_library_packaging_preserves_bytes_and_matches_manifest(tmp_path: Path) -> None:
    library = tmp_path / "macos-arm64_x86_64"
    library.mkdir()
    archive = b"!<arch>\nfixture static archive"
    (library / "ghostty-internal.a").write_bytes(archive)
    manifest = tmp_path / "Info.plist"
    manifest.write_bytes(plistlib.dumps({"AvailableLibraries": [{
        "LibraryIdentifier": library.name,
        "LibraryPath": "ghostty-internal.a",
        "BinaryPath": "ghostty-internal.a",
        "HeadersPath": "Headers",
    }]}))

    loopflow_dev._normalize_ghostty_library(tmp_path)
    loopflow_dev._normalize_ghostty_library(tmp_path)

    item = plistlib.loads(manifest.read_bytes())["AvailableLibraries"][0]
    assert item["LibraryPath"] == item["BinaryPath"] == "libghostty.a"
    assert (library / item["LibraryPath"]).read_bytes() == archive
    assert not (library / "ghostty-internal.a").exists()
    assert item["HeadersPath"] == "Headers"


def test_packaged_framework_rejects_a_linked_crash_reporter(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    if sys.platform != "darwin":
        pytest.skip("GhosttyKit archives use the macOS toolchain")
    monkeypatch.setattr(loopflow_dev, "SWIFT_DIR", tmp_path)
    source_archive = tmp_path / ".build/ghostty-kit/source.tar.gz"
    source_archive.parent.mkdir(parents=True)
    source_archive.write_bytes(b"source")
    patches = tmp_path / "GhosttyKitPatches"
    patches.mkdir()
    (patches / "embedder.patch").write_bytes(b"patch")
    framework = tmp_path / "GhosttyKit.xcframework"
    library = framework / "macos-arm64"
    library.mkdir(parents=True)
    (framework / "Info.plist").write_bytes(plistlib.dumps({"AvailableLibraries": [{
        "LibraryIdentifier": library.name, "LibraryPath": "libghostty.a",
    }]}))
    source = tmp_path / "reporter.c"
    source.write_text("void sentry_init(void) {}\n")
    subprocess.run(["cc", "-c", str(source), "-o", str(tmp_path / "reporter.o")], check=True)
    subprocess.run([
        "ar", "rcs", str(library / "libghostty.a"), str(tmp_path / "reporter.o"),
    ], check=True)
    with pytest.raises(ValueError, match="crash reporter is still linked"):
        loopflow_dev._record_ghostty_build(framework, ["true", "build", "-Dsentry=false"])
    assert not (framework / "build-provenance.json").exists()
