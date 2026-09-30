"""Repeat the branch's production-prefix estimate against the recorded main ref."""

import difflib
import json
import re
import subprocess
from pathlib import Path


def _read(revision: str, path: str) -> str:
    result = subprocess.run(["git", "show", f"{revision}:{path}"], text=True, capture_output=True)
    return result.stdout if result.returncode == 0 else ""


def _source(text: str) -> list[str]:
    parts = re.split(r"(?m)^#\[cfg\(test\)\]\nmod \w*tests\s*\{", text, maxsplit=1)
    prefix = parts[0]
    marker = "#[cfg(not(test))]\npub(crate) async fn start_home_session("
    if len(parts) > 1 and marker in parts[1]:
        prefix += parts[1][parts[1].index(marker) :]
    return prefix.splitlines()


def main() -> None:
    base, head = [
        subprocess.check_output(["git", "rev-parse", ref], text=True).strip()
        for ref in ("origin/main", "HEAD")
    ]
    totals = {name: {"added": 0, "removed": 0} for name in ("rust_swift", "python_shell", "sql")}
    files = {}
    for path in subprocess.check_output(
        ["git", "diff", "--no-renames", "--name-only", base, head], text=True
    ).splitlines():
        p = Path(path)
        if (
            any(
                part in p.parts for part in ("scratch", "tests", "LoopflowTests", "LoopflowUITests")
            )
            or p.name.startswith("test_")
            or p.name.endswith(("_test.rs", "_tests.rs"))
        ):
            continue
        kind = (
            "rust_swift"
            if p.suffix in (".rs", ".swift")
            else "sql"
            if p.suffix == ".sql"
            else "python_shell"
            if p.suffix in (".py", ".sh") and p.parts[0] in ("scripts", "release")
            else None
        )
        if kind is None:
            continue
        diff = list(difflib.unified_diff(_source(_read(base, path)), _source(_read(head, path))))
        counts = {
            "added": sum(line.startswith("+") and not line.startswith("+++") for line in diff),
            "removed": sum(line.startswith("-") and not line.startswith("---") for line in diff),
        }
        files[path] = counts
        for key, count in counts.items():
            totals[kind][key] += count
    result = {
        "base": base,
        "head": head,
        "method": (
            "production-prefix estimate; no rename detection; excludes tests/docs/scratch; "
            "excludes trailing inline Rust test modules"
        ),
        "totals": totals,
        "files": files,
    }
    output = Path(".lf/tmp/density-final/code-delta.json")
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: v for k, v in result.items() if k != "files"}, indent=2))


if __name__ == "__main__":
    main()
