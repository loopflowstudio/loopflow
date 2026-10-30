"""Tests for scripts/app_bundle.py against real Mach-O binaries."""

from __future__ import annotations

import platform
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
try:
    import app_bundle
finally:
    sys.path.pop(0)

pytestmark = pytest.mark.skipif(
    platform.system() != "Darwin" or shutil.which("cc") is None,
    reason="links Mach-O binaries with the macOS toolchain",
)


def _swiftpm_build(build_dir: Path) -> Path:
    """Lay out a build like SwiftPM's: Sparkle.framework beside an `@loader_path` executable."""
    versions = build_dir / "Sparkle.framework" / "Versions"
    (versions / "B").mkdir(parents=True)
    source = build_dir / "main.c"
    source.write_text("int sparkle(void);\nint main(void) { return sparkle(); }\n")
    (build_dir / "sparkle.c").write_text("int sparkle(void) { return 0; }\n")
    subprocess.run(
        [
            "cc",
            "-dynamiclib",
            "-install_name",
            "@rpath/Sparkle.framework/Versions/B/Sparkle",
            "-o",
            str(versions / "B" / "Sparkle"),
            str(build_dir / "sparkle.c"),
        ],
        check=True,
    )
    (versions / "Current").symlink_to("B")
    (build_dir / "Sparkle.framework" / "Sparkle").symlink_to("Versions/Current/Sparkle")
    executable = build_dir / "LoopflowMac"
    subprocess.run(
        [
            "cc",
            "-o",
            str(executable),
            str(source),
            str(versions / "B" / "Sparkle"),
            "-Wl,-rpath,@loader_path",
            "-Wl,-headerpad_max_install_names",
        ],
        check=True,
    )
    return executable


def _bundle_executable(build_dir: Path, app: Path) -> Path:
    macos = app / "Contents" / "MacOS"
    macos.mkdir(parents=True)
    executable = macos / "Loopflow"
    shutil.copy(build_dir / "LoopflowMac", executable)
    return executable


def test_bare_swiftpm_binary_in_a_bundle_cannot_load_sparkle(tmp_path: Path) -> None:
    build_dir = tmp_path / "build"
    _swiftpm_build(build_dir)
    executable = _bundle_executable(build_dir, tmp_path / "Loopflow.app")

    with pytest.raises(RuntimeError, match="cannot load Sparkle.framework/Versions/B/Sparkle"):
        app_bundle.verify_rpath_libraries(executable)
    launch = subprocess.run([str(executable)], capture_output=True, text=True)
    assert "Library not loaded: @rpath/Sparkle.framework" in launch.stderr


def test_embedded_sparkle_loads_from_contents_frameworks(tmp_path: Path) -> None:
    build_dir = tmp_path / "build"
    _swiftpm_build(build_dir)
    app = tmp_path / "Loopflow.app"
    executable = _bundle_executable(build_dir, app)

    app_bundle.embed_frameworks(build_dir, executable)
    app_bundle.embed_frameworks(build_dir, executable)  # rebuilding in place replaces it

    app_bundle.verify_rpath_libraries(executable)
    shutil.rmtree(build_dir)
    assert subprocess.run([str(executable)]).returncode == 0
    assert (app / "Contents" / "Frameworks" / "Sparkle.framework" / "Sparkle").is_symlink()


def test_embedding_requires_the_built_framework(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="Missing built Sparkle.framework"):
        app_bundle.embed_frameworks(tmp_path, tmp_path / "Loopflow.app/Contents/MacOS/Loopflow")
