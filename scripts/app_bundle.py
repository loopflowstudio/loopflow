"""Embed the frameworks Loopflow.app links, and prove its executable loads them.

SwiftPM links Sparkle beside its build products with only an `@loader_path`
rpath. Copied into `Contents/MacOS` alone, that binary aborts in dyld at launch
("Library not loaded: @rpath/Sparkle.framework/..."). Every bundle builder
embeds the framework here, which checks the result before signing.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

SPARKLE_FRAMEWORK = "Sparkle.framework"
FRAMEWORKS_RPATH = "@executable_path/../Frameworks"


def embed_frameworks(build_dir: Path, executable: Path) -> None:
    """Copy SwiftPM's Sparkle.framework into Contents/Frameworks, point the executable at it,
    and verify every `@rpath` library it links now resolves."""
    source = build_dir / SPARKLE_FRAMEWORK
    if not source.is_dir():
        raise RuntimeError(f"Missing built {SPARKLE_FRAMEWORK}: {source}")
    destination = executable.parent.parent / "Frameworks" / SPARKLE_FRAMEWORK
    if destination.exists():
        shutil.rmtree(destination)
    destination.parent.mkdir(exist_ok=True)
    # Framework bundles are symlink trees; copying their targets breaks the signature.
    shutil.copytree(source, destination, symlinks=True)
    if FRAMEWORKS_RPATH not in _rpaths(executable):
        _run_tool(["install_name_tool", "-add_rpath", FRAMEWORKS_RPATH, str(executable)])
    verify_rpath_libraries(executable)


def verify_rpath_libraries(executable: Path) -> None:
    """Raise unless each `@rpath` library the executable links resolves inside the bundle."""
    loader_dir = executable.parent
    search = [
        Path(
            rpath.replace("@executable_path", str(loader_dir)).replace(
                "@loader_path", str(loader_dir)
            )
        )
        for rpath in _rpaths(executable)
    ]
    missing = [
        library
        for library in _rpath_libraries(executable)
        if not any((directory / library).exists() for directory in search)
    ]
    if missing:
        raise RuntimeError(
            f"{executable} cannot load {', '.join(missing)} "
            f"from its rpaths ({', '.join(map(str, search)) or 'none'})"
        )


def _rpaths(executable: Path) -> list[str]:
    lines = _run_tool(["otool", "-l", str(executable)]).splitlines()
    rpaths = []
    for index, line in enumerate(lines):
        if line.strip() == "cmd LC_RPATH":
            for field in lines[index + 1 : index + 3]:
                field = field.strip()
                if field.startswith("path "):
                    rpaths.append(field.removeprefix("path ").rsplit(" (offset", 1)[0])
    return rpaths


def _rpath_libraries(executable: Path) -> list[str]:
    output = _run_tool(["otool", "-L", str(executable)])
    return [
        line.strip().split(" (compatibility", 1)[0].removeprefix("@rpath/")
        for line in output.splitlines()[1:]
        if line.strip().startswith("@rpath/")
    ]


def _run_tool(cmd: list[str]) -> str:
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"{cmd[0]} failed on {cmd[-1]}: {result.stderr.strip()}")
    return result.stdout
