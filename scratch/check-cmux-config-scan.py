"""Run with uv run python scratch/check-cmux-config-scan.py WRAPPER PATCH."""

import argparse
import subprocess
import tempfile
import time
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wrapper", type=Path)
    parser.add_argument("patch", type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="cmux-config-scan-") as directory:
        candidate = Path(directory) / "wrapper"
        candidate.write_text(args.wrapper.read_text())
        subprocess.run(
            ["patch", str(candidate), str(args.patch.resolve())], check=True
        )
        source = candidate.read_text()
        scanner = source[
            source.index("codex_launch_has_subrouter_provider()") : source.index(
                "# Find the real codex binary"
            )
        ]
        cases = [
            (["-c", 'model_provider="subrouter"'], 0),
            (["--config=model_provider=subrouter"], 0),
            (["-c", " model_provider = 'subrouter' "], 0),
            (["-c", "model_provider=subrouter", "-c", "model_provider=openai"], 1),
            (["-c", "model_provider=openai", "--config", "model_provider=subrouter"], 0),
            (["--", "-c", "model_provider=subrouter"], 1),
            (["-c", 'developer_instructions="model_provider=subrouter"'], 1),
            (["-c", "model_provider=subrouter", "-c", "hooks={ x = 1 }"], 0),
            (["-c", 'developer_instructions="' + "large context " * 6_000 + '"'], 1),
        ]
        started = time.monotonic()
        for arguments, expected in cases:
            result = subprocess.run(
                [
                    "/bin/bash", "-c",
                    scanner + '\ncodex_launch_has_subrouter_provider "$@"',
                    "probe", *arguments,
                ],
                capture_output=True,
                timeout=3,
            )
            assert result.returncode == expected, result.stderr.decode()
        print(f"{len(cases)} cases passed in {time.monotonic() - started:.3f}s")


if __name__ == "__main__":
    main()
