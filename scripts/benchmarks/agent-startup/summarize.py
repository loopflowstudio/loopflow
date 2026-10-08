"""Summarize sanitized startup rows, including matched-pair uncertainty."""

import argparse
import collections
import json
import random
import statistics
from pathlib import Path


def summarize(path: Path) -> list[dict]:
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    groups = collections.defaultdict(list)
    for row in rows:
        groups[(row.get("provider", "stand-in"), row["path"], row["variant"])].append(row)
    result = []
    for (provider, route, variant), group in sorted(groups.items()):
        key = "ready_ms" if "ready_ms" in group[0] else "handoff_ms"
        values = [row[key] for row in group if row[key] is not None]
        quartiles = statistics.quantiles(values, n=4, method="inclusive")
        entry = {
            "dataset": path.stem,
            "provider": provider,
            "path": route,
            "variant": variant,
            "n": len(values),
            "failures": len(group) - len(values),
            "min_ms": min(values),
            "q1_ms": quartiles[0],
            "median_ms": statistics.median(values),
            "q3_ms": quartiles[2],
            "max_ms": max(values),
        }
        if variant == "candidate":
            baseline = {row["sample"]: row[key] for row in groups[(provider, route, "baseline")]}
            differences = [
                baseline[row["sample"]] - row[key]
                for row in group
                if row[key] is not None and baseline.get(row["sample"]) is not None
            ]
            rng = random.Random(436)
            bootstrap = sorted(
                statistics.median(rng.choices(differences, k=len(differences)))
                for _ in range(10000)
            )
            entry.update(
                paired_improvement_ms=statistics.median(differences),
                paired_median_95pct=[bootstrap[250], bootstrap[9750]],
            )
        result.append(entry)
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path, nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    rows = [row for path in args.inputs for row in summarize(path)]
    args.output.write_text(json.dumps(rows, indent=2) + "\n")


if __name__ == "__main__":
    main()
