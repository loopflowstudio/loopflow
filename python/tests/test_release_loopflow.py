from __future__ import annotations

import importlib.util
import plistlib
import sys
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"
SCRIPT_PATH = SCRIPTS_DIR / "release-loopflow.py"
MODULE_NAME = "_release_loopflow_script_module"
spec = importlib.util.spec_from_file_location(MODULE_NAME, SCRIPT_PATH)
if spec is None or spec.loader is None:
    raise RuntimeError(f"failed to load module spec from {SCRIPT_PATH}")
release_loopflow = importlib.util.module_from_spec(spec)
sys.path.insert(0, str(SCRIPTS_DIR))
try:
    sys.modules[MODULE_NAME] = release_loopflow
    spec.loader.exec_module(release_loopflow)
finally:
    sys.path.pop(0)


def test_release_bundle_renames_swift_product_to_bundle_executable(tmp_path: Path) -> None:
    build_dir = tmp_path / "build"
    build_dir.mkdir()
    (build_dir / "LoopflowMac").write_bytes(b"swift-product")

    info_plist = tmp_path / "Info.plist"
    with info_plist.open("wb") as file:
        plistlib.dump({"CFBundleExecutable": "Loopflow"}, file)

    app_macos_dir = tmp_path / "Loopflow.app" / "Contents" / "MacOS"
    app_macos_dir.mkdir(parents=True)
    release_loopflow._copy_app_executable(build_dir, info_plist, app_macos_dir)

    assert (app_macos_dir / "Loopflow").read_bytes() == b"swift-product"
    assert not (app_macos_dir / "LoopflowMac").exists()
