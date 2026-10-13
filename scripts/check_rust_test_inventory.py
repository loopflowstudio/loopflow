"""Reject integration-test source files that Cargo cannot discover."""

import re
from pathlib import Path

import tomllib


def main() -> int:
    crate = Path(__file__).resolve().parents[1] / "rust/loopflow"
    manifest = tomllib.loads((crate / "Cargo.toml").read_text())
    if manifest["package"].get("autotests", True):
        return 0

    registered = {
        crate / target.get("path", f"tests/{target['name']}.rs")
        for target in manifest.get("test", [])
    }
    integration = crate / "tests/integration.rs"
    if integration in registered:
        registered.update(
            integration.parent / f"{name}.rs"
            for name in re.findall(r"^mod (\w+);$", integration.read_text(), re.MULTILINE)
        )

    missing = sorted(set((crate / "tests").glob("*.rs")) - registered)
    for path in missing:
        print(f"Unregistered Rust test source: {path.relative_to(crate)}")
    return int(bool(missing))


if __name__ == "__main__":
    raise SystemExit(main())
