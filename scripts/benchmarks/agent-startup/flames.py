"""Render sanitized, demangled main-thread macOS sample stacks with Inferno."""

import argparse
import collections
import re
import subprocess
import tempfile
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--label", required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for path in ("bare", "connect"):
        for variant in ("baseline", "candidate"):
            stacks: collections.Counter[str] = collections.Counter()
            for sample in sorted(args.input.glob(f"{path}-*-{variant}/sample.txt")):
                folded = subprocess.check_output(
                    [str(args.tools / "inferno-collapse-sample"), "--no-modules", str(sample)]
                )
                with tempfile.TemporaryFile() as symbols:
                    symbols.write(folded)
                    symbols.seek(0)
                    demangled = subprocess.check_output(
                        [str(args.tools / "rustfilt")], stdin=symbols, timeout=30
                    ).decode()
                for line in demangled.splitlines():
                    stack, count = line.rsplit(" ", 1)
                    if not re.match(r"Thread_\d+: main;", stack):
                        continue
                    stack = re.sub(r"^Thread_\d+: main;", "lf main;", stack)
                    # Keep symbols only: sample headers, filesystem paths,
                    # addresses and Session thread names are never published.
                    stack = re.sub(r"0x[0-9a-fA-F]+", "address", stack)
                    stacks[stack] += int(count)
            if not stacks:
                raise RuntimeError(f"No main-thread samples for {path}/{variant}")
            folded_path = args.output / f"{args.label}-{path}-{variant}.folded"
            folded_path.write_text("".join(f"{s} {n}\n" for s, n in sorted(stacks.items())))
            with folded_path.with_suffix(".svg").open("wb") as output:
                subprocess.run(
                    [
                        str(args.tools / "inferno-flamegraph"),
                        "--deterministic",
                        "--width",
                        "1600",
                        "--title",
                        f"lf {path}: {args.label} — {variant}",
                        "--subtitle",
                        "Release + debuginfo; macOS sample, 1 ms; "
                        "main thread only; startup + shutdown",
                        "--notes",
                        "Aggregated diagnostic samples, including waits. "
                        "Not elapsed-time accounting. Attach misses initial startup; "
                        "excludes provider and Git child CPU. Hover or click to inspect.",
                        str(folded_path),
                    ],
                    stdout=output,
                    check=True,
                )


if __name__ == "__main__":
    main()
