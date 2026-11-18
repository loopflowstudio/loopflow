#!/usr/bin/env python3
"""Count the stored model: what a change adds to it or removes from it.

    uv run python scripts/model_size.py

Tables and columns come from replaying the migrations. Storage `pub fn`s and SQL
call sites outside `loopflow-storage` count production code only: files named
`*test*` and the `#[cfg(test)]` modules that trail a file are skipped. Paste the
before and after into the PR body.

Stdlib only.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_migrations  # noqa: E402

RUST = check_migrations.REPO_ROOT / "rust"
STORAGE = "loopflow-storage"
PUB_FN = re.compile(r"^\s*pub(?:\([^)]*\))?\s+(?:async\s+)?(?:unsafe\s+)?fn\s", re.MULTILINE)
SQL_CALL = re.compile(r"\.(?:execute|execute_batch|prepare|prepare_cached|query_row)\(")


def _production(path: Path) -> str:
    return path.read_text().split("#[cfg(test)]\nmod ", 1)[0]


def _sources(crates: list[Path]) -> list[Path]:
    return sorted(
        path
        for crate in crates
        for path in (crate / "src").rglob("*.rs")
        if "test" not in path.stem and "test" not in path.parent.name
    )


def measure() -> dict[str, str]:
    local = check_migrations._local_migrations()
    schema = check_migrations._schema(check_migrations.Catalog.of_origin_main(local).replay())
    storage_fns = sum(len(PUB_FN.findall(_production(path))) for path in _sources([RUST / STORAGE]))
    others = [crate for crate in sorted(RUST.iterdir()) if (crate / "src").is_dir()]
    sites = {
        path: count
        for path in _sources([crate for crate in others if crate.name != STORAGE])
        if (count := len(SQL_CALL.findall(_production(path))))
    }
    return {
        "tables": str(len(schema.tables)),
        "columns": str(sum(len(columns) for columns in schema.tables.values())),
        "storage pub fns (non-test)": str(storage_fns),
        "SQL call sites outside storage": f"{sum(sites.values())} in {len(sites)} files",
    }


def main() -> None:
    for name, value in measure().items():
        print(f"{name:<31}{value}")


if __name__ == "__main__":
    main()
