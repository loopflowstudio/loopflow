#!/usr/bin/env python3
"""Measure Rust source size so a deletion can be proved rather than estimated.

    uv run python scripts/measure_source.py
    uv run python scripts/measure_source.py --baseline 144210 --min-reduction 10000

The metric is physical lines under `rust/loopflow/src` — every line of every
`.rs` file, comments and blanks included. It is deliberately the dumbest
definition that cannot drift: no comment stripping, no `#[cfg(test)]` parsing,
no tokenizer. A number nobody can reproduce is not a ceiling, and the previous
architecture-ledger figure (121,818 physical / 119,126 normalized) matches no
measurement of any scope in the tree.

Baseline is `ae1344a57` (PR #1073, the Run spine) at 144,210 lines.

Stdlib only, so no Python environment is needed to check a size claim.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent
SOURCE_ROOT = REPO_ROOT / "rust/loopflow/src"


def measure(root: Path) -> tuple[int, dict[str, int]]:
    """Return total physical lines and a per-top-level-module breakdown."""
    per_module: dict[str, int] = {}
    total = 0
    for path in sorted(root.rglob("*.rs")):
        lines = len(path.read_text(encoding="utf-8").splitlines())
        total += lines
        relative = path.relative_to(root)
        module = relative.parts[0] if len(relative.parts) > 1 else relative.name
        per_module[module] = per_module.get(module, 0) + lines
    return total, per_module


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--baseline",
        type=int,
        help="expected starting size; enables the reduction gate",
    )
    parser.add_argument(
        "--min-reduction",
        type=int,
        default=0,
        help="required lines removed relative to --baseline",
    )
    parser.add_argument(
        "--breakdown",
        action="store_true",
        help="print per-module line counts, largest first",
    )
    args = parser.parse_args()

    if not SOURCE_ROOT.is_dir():
        print(f"no source root at {SOURCE_ROOT}", file=sys.stderr)
        return 2

    total, per_module = measure(SOURCE_ROOT)
    print(f"rust/loopflow/src: {total:,} physical lines")

    if args.breakdown:
        for module, lines in sorted(per_module.items(), key=lambda kv: -kv[1]):
            print(f"  {lines:>7,}  {module}")

    if args.baseline is None:
        return 0

    reduction = args.baseline - total
    print(f"baseline: {args.baseline:,}  reduction: {reduction:+,}")
    if reduction < args.min_reduction:
        print(
            f"FAIL: need at least {args.min_reduction:,} lines removed, got {reduction:,}",
            file=sys.stderr,
        )
        return 1
    print(f"OK: reduction meets the {args.min_reduction:,}-line floor")
    return 0


if __name__ == "__main__":
    sys.exit(main())
