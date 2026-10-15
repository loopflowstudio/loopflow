"""Compile the patched production shaders without creating a window."""

import argparse
import re
import subprocess
import tempfile
from pathlib import Path


def _expand(path: Path) -> str:
    return re.sub(
        r'(?m)^#include "([^"]+)"',
        lambda match: _expand(path.parent / match[1]),
        path.read_text(),
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    args = parser.parse_args()
    shaders = args.source / "src/renderer/shaders/glsl"
    # Same recursive include expansion and source-owned version as shaders.zig.
    with tempfile.TemporaryDirectory(prefix="ghostty-shaders-") as directory:
        for path in sorted(shaders.glob("*.glsl")):
            stage = "vert" if path.name.endswith(".v.glsl") else "frag"
            if not path.name.endswith((".v.glsl", ".f.glsl")):
                continue
            expanded = Path(directory) / path.name
            expanded.write_text(_expand(path))
            subprocess.run(["glslangValidator", "-S", stage, str(expanded)], check=True)
    subprocess.run(
        ["swift", str(Path(__file__).with_name("ghostty_shader_check.swift")), str(args.source)],
        check=True,
    )


if __name__ == "__main__":
    main()
