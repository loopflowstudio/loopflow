#!/usr/bin/env python3
"""Rebuild libintl and relink Loopflow from its packaged source materials."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import tarfile
import zipfile
from pathlib import Path

from app_bundle import install_app_executable


def _run(args: list[str], cwd: Path) -> None:
    subprocess.run(args, cwd=cwd, check=True)


def prepare(materials: Path, work: Path) -> None:
    work.mkdir(parents=True, exist_ok=True)
    for name in ["loopflow", "ghostty", "gettext"]:
        with tarfile.open(materials / f"{name}.tar.gz") as archive:
            archive.extractall(work, filter="data")
    with zipfile.ZipFile(materials / "GhosttyKit.zip") as archive:
        archive.extractall(work / "loopflow/swift")
    print(f"Edit {work}/gettext-0.24, then run this script with build {work}")


def build_libintl(source: Path, config: Path, architecture: str, output: Path) -> None:
    members = re.findall(r'^\s+"([^\"]+\.c)",', (config / "build.zig").read_text(), re.M)
    sdk = subprocess.check_output(["xcrun", "--show-sdk-path"], text=True).strip()
    intl = source / "gettext-runtime/intl"
    output.mkdir(parents=True, exist_ok=True)
    objects = []
    for name in members:
        obj = output / (Path(name).stem + ".o")
        _run(
            [
                "xcrun",
                "clang",
                "-arch",
                architecture,
                "-isysroot",
                sdk,
                "-mmacosx-version-min=15.0",
                "-O2",
                "-DHAVE_CONFIG_H",
                '-DLOCALEDIR=""',
                "-I",
                str(config),
                "-I",
                str(intl),
                "-I",
                str(intl / "gnulib-lib"),
                "-c",
                str(intl / name),
                "-o",
                str(obj),
            ],
            output,
        )
        objects.append(str(obj))
    library = output / "libintl.a"
    library.unlink(missing_ok=True)
    _run(["ar", "qc", str(library), *objects], output)
    _run(["ranlib", str(library)], output)


def build(work: Path) -> None:
    app_source = work / "loopflow"
    manifest = json.loads((app_source / "third_party/manifest.json").read_text())
    ghostty = work / ("ghostty-" + manifest["ghostty"]["revision"])
    framework = app_source / "swift/GhosttyKit.xcframework"
    library = next(framework.glob("*/libghostty.a"))
    slices = []
    for arch in ["arm64", "x86_64"]:
        output = work / arch
        build_libintl(work / "gettext-0.24", ghostty / "pkg/libintl", arch, output)
        thin = output / "libghostty.a"
        _run(["lipo", str(library), "-thin", arch, "-output", str(thin)], work)
        _run(["ar", "d", str(thin), *manifest["ghostty"]["libintl_members"]], work)
        _run(["ar", "q", str(thin), *(str(p) for p in sorted(output.glob("*.o")))], work)
        _run(["ranlib", str(thin)], work)
        slices.append(str(thin))
    _run(["lipo", "-create", *slices, "-output", str(library)], work)
    swift = app_source / "swift"
    package = swift / "Package.swift"
    text = package.read_text()
    text = re.sub(
        r'\.binaryTarget\(\s*name: "GhosttyKit",.*?\)',
        '.binaryTarget(name: "GhosttyKit", path: "GhosttyKit.xcframework")',
        text,
        count=1,
        flags=re.S,
    )
    package.write_text(text)
    _run(["swift", "build", "-c", "release", "--product", "LoopflowMac"], swift)
    _run(["cargo", "build", "--locked", "--release", "--bin", "lf"], app_source)
    contents = work / "Loopflow Rebuilt.app/Contents"
    (contents / "Resources").mkdir(parents=True, exist_ok=True)
    info = swift / "LoopflowMac/Info.plist"
    shutil.copyfile(info, contents / "Info.plist")
    install_app_executable(swift / ".build/release", info, contents)
    shutil.copy2(app_source / "target/release/lf", contents / "MacOS/lf")
    for bundle in (swift / ".build/release").glob("*.bundle"):
        shutil.copytree(bundle, contents / "Resources" / bundle.name, dirs_exist_ok=True)
    _run(["codesign", "--force", "--deep", "--sign", "-", str(contents.parent)], work)
    print(f"Relinked application: {contents.parent}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["prepare", "build"])
    parser.add_argument("work", type=Path)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(Path(__file__).resolve().parent, args.work.resolve())
    else:
        build(args.work.resolve())


if __name__ == "__main__":
    main()
