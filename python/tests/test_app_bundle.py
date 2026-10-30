"""Tests for scripts/app_bundle.py and the bundle builders, against real Mach-O binaries."""

from __future__ import annotations

import json
import platform
import plistlib
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts"
sys.path[:0] = [str(SCRIPTS), str(SCRIPTS / "benchmarks/desktop-performance")]

import app_bundle  # noqa: E402
import launch  # noqa: E402
import rebuild_third_party as rebuild  # noqa: E402

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
    # codesign accepts a framework only with its Info.plist.
    (versions / "B" / "Resources").mkdir()
    (versions / "B" / "Resources" / "Info.plist").write_bytes(
        plistlib.dumps({"CFBundleIdentifier": "org.sparkle-project.Sparkle"})
    )
    (versions / "Current").symlink_to("B")
    (build_dir / "Sparkle.framework" / "Sparkle").symlink_to("Versions/Current/Sparkle")
    (build_dir / "Sparkle.framework" / "Resources").symlink_to("Versions/Current/Resources")
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


def _info_plist(path: Path, executable: str = "Loopflow") -> Path:
    path.write_bytes(plistlib.dumps({"CFBundleExecutable": executable}))
    return path


def test_installed_app_executable_is_named_by_the_plist_and_loads_sparkle(tmp_path: Path) -> None:
    build_dir = tmp_path / "build"
    _swiftpm_build(build_dir)
    contents = tmp_path / "Loopflow Dev.app" / "Contents"
    info = _info_plist(tmp_path / "Info.plist", "Loopflow Dev")

    app_bundle.install_app_executable(build_dir, info, contents)
    executable = app_bundle.install_app_executable(build_dir, info, contents)  # rebuild in place

    shutil.rmtree(build_dir)
    assert executable == contents / "MacOS" / "Loopflow Dev"
    assert subprocess.run([str(executable)]).returncode == 0
    assert (contents / "Frameworks" / "Sparkle.framework" / "Sparkle").is_symlink()


def test_installing_requires_the_built_app_and_framework(tmp_path: Path) -> None:
    info = _info_plist(tmp_path / "Info.plist")
    contents = tmp_path / "Loopflow.app" / "Contents"

    with pytest.raises(RuntimeError, match="Missing built app executable: .*LoopflowMac"):
        app_bundle.install_app_executable(tmp_path, info, contents)
    (tmp_path / "LoopflowMac").write_bytes(b"")
    with pytest.raises(RuntimeError, match="Missing built Sparkle.framework"):
        app_bundle.install_app_executable(tmp_path, info, contents)


def _swift_package(swift: Path) -> None:
    """A SwiftPM release build plus the app sources the builders copy beside it."""
    _swiftpm_build(swift / ".build/release")
    (swift / "LoopflowMac").mkdir()
    (swift / "LoopflowMac/Info.plist").write_bytes(
        plistlib.dumps({"CFBundleExecutable": "Loopflow", "CFBundleIdentifier": "test"})
    )
    (swift / "LoopflowMac/AppIcon.icns").write_bytes(b"icon")


def _assert_launches_with_embedded_sparkle(app: Path) -> None:
    assert (app / "Contents/Frameworks/Sparkle.framework/Versions/B/Sparkle").is_file()
    assert subprocess.run([str(app / "Contents/MacOS/Loopflow")]).returncode == 0


def test_launch_benchmark_bundle_embeds_sparkle(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    swift = tmp_path / "swift"
    _swift_package(swift)
    monkeypatch.setattr(launch, "SWIFT", swift)
    real_sh = launch._sh
    monkeypatch.setattr(
        launch, "_sh", lambda cmd, **kw: None if cmd[0] == "swift" else real_sh(cmd, **kw)
    )

    launch.bundle(tmp_path / "Loopflow Bench.app", strip=False)

    shutil.rmtree(swift / ".build")
    _assert_launches_with_embedded_sparkle(tmp_path / "Loopflow Bench.app")


def test_third_party_rebuild_embeds_sparkle(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    source = tmp_path / "loopflow"
    swift = source / "swift"
    _swift_package(swift)
    (swift / "Package.swift").write_text("// package")
    (swift / "GhosttyKit.xcframework/macos").mkdir(parents=True)
    (swift / "GhosttyKit.xcframework/macos/libghostty.a").write_bytes(b"")
    (source / "third_party").mkdir()
    (source / "third_party/manifest.json").write_text(
        json.dumps({"ghostty": {"revision": "r", "libintl_members": []}})
    )
    (source / "target/release").mkdir(parents=True)
    (source / "target/release/lf").write_bytes(b"")
    # Compiling libintl, relinking GhosttyKit and the swift/cargo builds are out of scope here.
    monkeypatch.setattr(rebuild, "build_libintl", lambda *args: None)
    monkeypatch.setattr(
        rebuild,
        "_run",
        lambda args, cwd: (
            subprocess.run(args, cwd=cwd, check=True) if args[0] == "codesign" else None
        ),
    )

    rebuild.build(tmp_path)

    shutil.rmtree(swift / ".build")
    _assert_launches_with_embedded_sparkle(tmp_path / "Loopflow Rebuilt.app")
