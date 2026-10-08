"""SwiftPM must recognize and link the packaged static library."""

import importlib.util
import plistlib
from pathlib import Path

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
