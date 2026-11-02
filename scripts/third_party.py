#!/usr/bin/env python3
"""Generate offline notices and package matching source for Desktop/GhosttyKit."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import re
import shutil
import subprocess
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = Path("third_party/manifest.json")
NOTICES = Path("swift/LoopflowMac/ThirdPartyNotices")
TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
)
RESOURCE_DIRS = (
    "swift/Loopflow/Fonts",
    "swift/Loopflow/Themes",
    "swift/LoopflowMac/GhosttyResources",
    "swift/GhosttyKitPatches",
    "third_party",
)


def _sha(path: Path) -> str:
    with path.open("rb") as source:
        digest = hashlib.sha256()
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
        return digest.hexdigest()


def _run(args: list[str], cwd: Path = ROOT) -> str:
    return subprocess.check_output(args, cwd=cwd, text=True)


def _json(path: Path) -> dict:
    return json.loads(path.read_text())


def _write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def _inputs(root: Path) -> dict[str, str]:
    paths = [root / p for p in ("Cargo.lock", "swift/Package.swift", "swift/Package.resolved")]
    for directory in RESOURCE_DIRS:
        paths.extend(p for p in (root / directory).rglob("*") if p.is_file())
    return {str(p.relative_to(root)): _sha(p) for p in sorted(paths)}


def _verify_pin(root: Path, manifest: dict) -> None:
    package = (root / "swift/Package.swift").read_text()
    pin = manifest["ghostty"]
    if pin["url"] not in package or pin["sha256"] not in package:
        raise ValueError(
            "GhosttyKit changed: audit the new artifact and update third_party/manifest.json"
        )


def _download(source: dict, cache: Path) -> Path:
    cache.mkdir(parents=True, exist_ok=True)
    destination = cache / source["sha256"]
    if not destination.is_file() or _sha(destination) != source["sha256"]:
        with tempfile.NamedTemporaryFile(dir=cache, delete=False) as temp:
            temporary = Path(temp.name)
        try:
            subprocess.run(
                [
                    "curl",
                    "--fail",
                    "--location",
                    "--silent",
                    "--show-error",
                    "--retry",
                    "2",
                    "--output",
                    str(temporary),
                    source["url"],
                ],
                check=True,
            )
            if _sha(temporary) != source["sha256"]:
                raise ValueError(f"Source checksum mismatch: {source['url']}")
            temporary.replace(destination)
        finally:
            temporary.unlink(missing_ok=True)
    return destination


def _crate_packages(root: Path) -> list[dict]:
    metadata = json.loads(_run(["cargo", "metadata", "--locked", "--format-version", "1"], root))
    runtime = set()
    for target in TARGETS:
        tree = _run(
            [
                "cargo",
                "tree",
                "--locked",
                "-p",
                "loopflow",
                "--target",
                target,
                "--edges",
                "normal",
                "--prefix",
                "none",
                "--format",
                "{p}",
            ],
            root,
        )
        runtime.update(re.findall(r"^(\S+) v(\S+)", tree, re.M))
    return sorted(
        (p for p in metadata["packages"] if p["source"] and (p["name"], p["version"]) in runtime),
        key=lambda p: (p["name"], p["version"]),
    )


def _crate_sources(root: Path) -> dict[str, dict]:
    # Cargo.lock is Cargo's authority; these registry packages have no git sources.
    sources = {}
    for entry in (root / "Cargo.lock").read_text().split("[[package]]")[1:]:
        fields = dict(re.findall(r'^(name|version|source|checksum) = "([^"]+)"$', entry, re.M))
        if fields.get("source", "").startswith("registry+"):
            name, version = fields["name"], fields["version"]
            sources[f"{name}@{version}"] = {
                "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
                "sha256": fields["checksum"],
            }
    return sources


def _crate_notices(package: dict, manifest: dict, root: Path) -> str:
    name = package["name"] + "@" + package["version"]
    directory = Path(package["manifest_path"]).parent
    files = sorted(
        p
        for p in directory.rglob("*")
        if p.is_file()
        and p.name.lower().startswith(
            ("license", "licence", "copying", "notice", "copyright", "unlicense")
        )
    )
    if name in manifest["crate_notices"]:
        return _reviewed_notice(root, manifest["crate_notices"][name])
    if not files:
        raise ValueError(f"{name}: crate has no license text; add a pinned upstream notice")
    # Native crates such as ring include their vendored notices here too.
    return "\n\n".join(
        str(p.relative_to(directory)) + "\n\n" + p.read_text(errors="replace") for p in files
    )


def _reviewed_notice(root: Path, notice: dict) -> str:
    path = root / notice["path"]
    if _sha(path) != notice["sha256"]:
        raise ValueError(f"Reviewed notice changed: {notice['path']}; update its manifest digest")
    return path.read_text()


def refresh(root: Path = ROOT) -> None:
    manifest = _json(root / MANIFEST)
    _verify_pin(root, manifest)
    components = []
    sections = [
        "Open Source Licenses\n====================\n\n"
        "Loopflow includes the components below. Copyrights and license terms remain "
        "with their respective authors. Source paths distinguish bundled code from "
        "ancillary notices retained from upstream source packages.\n\n"
        "FreeType is used under the FreeType License: portions of this software are "
        "copyright © The FreeType Project (www.freetype.org). All rights reserved.\n\n"
        "GhosttyKit statically combines GNU libintl (LGPL-2.1-or-later). Its uncombined "
        "libraries, source, and rebuild instructions accompany packaged artifacts in "
        "ThirdPartySources. You may modify and reverse engineer the application to "
        "debug modifications to that library. See REBUILD.md in that directory.\n\n"
        "GPL shell integration scripts are shipped as source. MPL-covered z2d and "
        "option-ext sources also accompany the application. System frameworks are "
        "provided by the operating system. ViewInspector is test-only.\n",
    ]
    for component in manifest["components"]:
        text = "\n\n".join(_reviewed_notice(root, n) for n in component["notices"])
        components.append(component)
        sections.append(f"{component['id']} ({component['scope']})\n" + "-" * 72 + "\n" + text)
    # Keep existing theme attributions and GPL/MIT script headers, without copying
    # a second manually maintained list of themes or shell files.
    resources = []
    for base in ["swift/Loopflow/Themes", "swift/LoopflowMac/GhosttyResources"]:
        for path in sorted((root / base).rglob("*")):
            if path.is_file() and (
                "Licenses" in path.parts
                or path.name == "Sources.md"
                or "shell-integration" in path.parts
            ):
                resources.append(str(path.relative_to(root)))
                sections.append(str(path.relative_to(root)) + "\n\n" + path.read_text())
    components.append({"id": "bundled-resources", "scope": "Desktop", "files": resources})
    sources = _crate_sources(root)
    for package in _crate_packages(root):
        name = package["name"] + "@" + package["version"]
        components.append(
            {"id": name, "scope": "lf", "license": package["license"], "source": sources[name]}
        )
        sections.append(name + "\n" + "-" * 72 + "\n" + _crate_notices(package, manifest, root))
    output = root / NOTICES
    output.mkdir(parents=True, exist_ok=True)
    (output / "NOTICES.txt").write_text("\n\n".join(sections))
    _write_json(
        output / "inventory.json",
        {
            "inputs": _inputs(root),
            "ghostty": manifest["ghostty"],
            "components": components,
            "notices_sha256": _sha(output / "NOTICES.txt"),
        },
    )
    print(f"Generated notices for {len(components)} components")


def check(root: Path = ROOT) -> dict:
    inventory = _json(root / NOTICES / "inventory.json")
    _verify_pin(root, _json(root / MANIFEST))
    if inventory["inputs"] != _inputs(root):
        raise ValueError(
            "Third-party inputs changed; run uv run python scripts/third_party.py refresh"
        )
    if inventory["notices_sha256"] != _sha(root / NOTICES / "NOTICES.txt"):
        raise ValueError("Generated notices changed; regenerate the inventory")
    return inventory


def _source_snapshot(root: Path, destination: Path) -> None:
    paths = _run(["git", "ls-files", "-z"], root).split("\0")
    # Newly authored code is included in local verification before its first commit.
    paths += _run(
        [
            "git",
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "scripts",
            "third_party",
            "swift",
        ],
        root,
    ).split("\0")
    with tarfile.open(destination, "w:gz") as archive:
        for name in sorted(set(paths)):
            if not name or name.startswith(("scratch/", ".lf/")):
                continue
            path = root / name
            if path.is_file():
                archive.add(path, arcname="loopflow/" + name, recursive=False)


def _split_libintl(library: Path, members: list[str], output: Path) -> None:
    present = _run(["ar", "t", str(library)]).splitlines()
    if any(present.count(name) != 1 for name in members):
        raise ValueError(
            "GhosttyKit libintl objects changed; audit pkg/libintl/build.zig and archive"
        )
    with tempfile.TemporaryDirectory() as temp:
        directory = Path(temp)
        for name in members:
            (directory / name).write_bytes(subprocess.check_output(["ar", "p", str(library), name]))
        subprocess.run(["ar", "qc", str(output), *members], cwd=directory, check=True)
        subprocess.run(["ranlib", str(output)], check=True)


def _package_source(source: Path, destination: Path, specification: dict) -> None:
    packaged = specification.get("packaged")
    if packaged is None:
        shutil.copyfile(source, destination)
        return
    omitted = set(packaged["omit_binary_fixtures"])
    found = set()
    with (
        tarfile.open(source) as upstream,
        destination.open("wb") as raw,
        gzip.GzipFile(filename="", fileobj=raw, mode="wb", compresslevel=9, mtime=0) as compressed,
        tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive,
    ):
        for member in upstream:
            if member.name in omitted:
                found.add(member.name)
                continue
            archive.addfile(member, upstream.extractfile(member) if member.isfile() else None)
    if found != omitted or _sha(destination) != packaged["sha256"]:
        raise ValueError("Packaged source differs from its audited fixture omissions")


def _package_sources(root: Path, output: Path, inventory: dict, framework: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    cache = root / ".lf/tmp/third-party"
    for component in inventory["components"]:
        if "source" not in component:
            continue
        source = _download(component["source"], cache)
        _package_source(source, output / (component["id"] + ".tar.gz"), component["source"])
    with zipfile.ZipFile(output / "GhosttyKit.zip", "w", zipfile.ZIP_DEFLATED) as original:
        for path in sorted(framework.rglob("*")):
            relative = path.relative_to(framework)
            if path.is_file() and not relative.parts[0].startswith("ThirdParty"):
                original.write(path, str(Path("GhosttyKit.xcframework") / relative))
    _source_snapshot(root, output / "loopflow.tar.gz")
    shutil.copyfile(root / "third_party/REBUILD.md", output / "REBUILD.md")
    shutil.copyfile(root / "scripts/rebuild_third_party.py", output / "rebuild.py")
    shutil.copyfile(root / "scripts/app_bundle.py", output / "app_bundle.py")
    libraries = sorted(framework.glob("*/libghostty.a"))
    if len(libraries) != 1:
        raise ValueError("Expected one universal macOS GhosttyKit library")
    architectures = _run(["lipo", "-archs", str(libraries[0])]).split()
    for architecture in architectures:
        with tempfile.TemporaryDirectory() as temp:
            thin = Path(temp) / "ghostty.a"
            if len(architectures) == 1:
                shutil.copyfile(libraries[0], thin)
            else:
                subprocess.run(
                    ["lipo", str(libraries[0]), "-thin", architecture, "-output", str(thin)],
                    check=True,
                )
            _split_libintl(
                thin, inventory["ghostty"]["libintl_members"], output / f"libintl-{architecture}.a"
            )


def package(destination: Path, framework: Path, root: Path = ROOT) -> None:
    """Assemble notice/source material before signing an app or zipping a framework."""
    destination = destination.resolve()
    framework = framework.resolve()
    inventory = check(root)
    for name in ["ThirdPartyNotices", "ThirdPartySources"]:
        if (destination / name).exists():
            shutil.rmtree(destination / name)
    shutil.copytree(root / NOTICES, destination / "ThirdPartyNotices")
    _package_sources(root, destination / "ThirdPartySources", inventory, framework)
    files = {
        str(p.relative_to(destination)): _sha(p)
        for name in ["ThirdPartyNotices", "ThirdPartySources"]
        for p in sorted((destination / name).rglob("*"))
        if p.is_file()
    }
    _write_json(destination / "ThirdPartyNotices" / "contents.json", files)
    verify(destination)


def verify(destination: Path) -> None:
    """Validate the final artifact, including missing or corrupted source material."""
    receipt = _json(destination / "ThirdPartyNotices/contents.json")
    inventory = _json(destination / "ThirdPartyNotices/inventory.json")
    required = {
        "ThirdPartyNotices/NOTICES.txt",
        "ThirdPartyNotices/inventory.json",
        "ThirdPartySources/REBUILD.md",
        "ThirdPartySources/rebuild.py",
        "ThirdPartySources/app_bundle.py",
        "ThirdPartySources/loopflow.tar.gz",
        "ThirdPartySources/GhosttyKit.zip",
        "ThirdPartySources/libintl-arm64.a",
        "ThirdPartySources/libintl-x86_64.a",
    }
    required.update(
        "ThirdPartySources/" + c["id"] + ".tar.gz" for c in inventory["components"] if "source" in c
    )
    if not required <= receipt.keys():
        raise ValueError("Incomplete third-party packaging receipt")
    for name, checksum in receipt.items():
        path = destination / name
        if not path.is_file() or _sha(path) != checksum:
            raise ValueError(f"Missing or corrupted third-party material: {name}")
    for component in inventory["components"]:
        if "source" in component:
            path = destination / "ThirdPartySources" / (component["id"] + ".tar.gz")
            source = component["source"]
            if _sha(path) != source.get("packaged", source)["sha256"]:
                raise ValueError(f"Source does not match inventory: {component['id']}")
    print(f"Verified {len(receipt)} third-party files in {destination}")


def verify_archive(archive: Path) -> None:
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        with zipfile.ZipFile(archive) as package_zip:
            package_zip.extractall(root)
        frameworks = list(root.glob("*.xcframework"))
        if len(frameworks) != 1:
            raise ValueError("Expected one GhosttyKit framework in archive")
        verify(frameworks[0])
        # The relink copy must contain the same library and headers as the
        # distributed framework, including after final archive assembly.
        with zipfile.ZipFile(frameworks[0] / "ThirdPartySources/GhosttyKit.zip") as relink:
            for path in frameworks[0].rglob("*"):
                relative = path.relative_to(frameworks[0])
                if path.is_file() and not relative.parts[0].startswith("ThirdParty"):
                    name = str(Path(frameworks[0].name) / relative)
                    if hashlib.sha256(relink.read(name)).hexdigest() != _sha(path):
                        raise ValueError(f"Relink framework differs from shipped framework: {name}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("refresh")
    sub.add_parser("check")
    pack = sub.add_parser("package")
    pack.add_argument("destination", type=Path)
    pack.add_argument("--framework", type=Path)
    verify_parser = sub.add_parser("verify")
    verify_parser.add_argument("destination", type=Path)
    archive = sub.add_parser("verify-archive")
    archive.add_argument("archive", type=Path)
    args = parser.parse_args()
    if args.command == "refresh":
        refresh()
    elif args.command == "check":
        check()
    elif args.command == "verify":
        verify(args.destination)
    elif args.command == "verify-archive":
        verify_archive(args.archive)
    else:
        if args.framework:
            package(args.destination, args.framework)
        else:
            pin = _json(ROOT / MANIFEST)["ghostty"]
            cached = _download(pin, ROOT / ".lf/tmp/third-party")
            with tempfile.TemporaryDirectory() as temp:
                with zipfile.ZipFile(cached) as source:
                    source.extractall(temp)
                package(args.destination, Path(temp) / "GhosttyKit.xcframework")


if __name__ == "__main__":
    main()
