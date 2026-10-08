"""Alternate native launch readiness without submitting measured turns."""

import argparse
import json
import subprocess
from pathlib import Path

from fixture import read_session_ids, require_fixture
from measure import build_environment, measure


def record_readiness(output: Path, row: dict) -> None:
    with (output / "numbers.jsonl").open("a") as numbers:
        numbers.write(json.dumps(row) + "\n")
    print(json.dumps(row), flush=True)
    if row["status"] != "ready" or row["returncode"] != 0:
        raise RuntimeError(
            f"{row['path']}/{row['variant']} did not reach readiness and exit cleanly"
        )


def prepare_session(lf: Path, home: Path, repo: Path, output: Path, provider: str) -> str:
    before = read_session_ids(home)
    output.mkdir(parents=True, exist_ok=True)
    if provider == "codex":
        # Codex needs one persisted turn before native reconnect is possible.
        with (output / "seed.private").open("wb") as log:
            seed = subprocess.run(
                [str(lf), "-b", "-a", "codex", ":", "Reply only OK. Do not call tools."],
                cwd=repo,
                env=build_environment(home, lf),
                stdout=log,
                stderr=log,
                timeout=90,
            )
        ready = seed.returncode == 0
    else:
        result = measure(
            [str(lf)], repo, build_environment(home, lf), output, provider, trust_fixture=True
        )
        ready = result["status"] == "ready" and result["returncode"] == 0
    created = read_session_ids(home) - before
    if not ready or len(created) != 1:
        raise RuntimeError("Preparation must create exactly one usable benchmark Session")
    return created.pop()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--home", type=Path, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=12)
    parser.add_argument("--provider", choices=["claude", "codex"], required=True)
    parser.add_argument("--profile", action="store_true")
    args = parser.parse_args()
    require_fixture(args.home)
    args.output.mkdir(parents=True, exist_ok=False)
    provider = args.provider
    variants = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    (args.home / "config.yaml").write_text(f"agent: {provider}\n")
    session = prepare_session(
        variants["baseline"], args.home, args.repo, args.output / "prepare", provider
    )
    for i in range(args.samples):
        paths = ["direct", "bare", "connect"]
        if i % 2:
            paths.reverse()
        for path in paths:
            order = list(variants) if i % 2 == 0 else list(reversed(variants))
            if path == "direct":
                order = ["direct"]
            for variant in order:
                lf = variants.get(variant)
                command = [str(lf)] if lf else [provider]
                if path == "connect":
                    command += ["session", "connect", session]
                result = measure(
                    command,
                    args.repo,
                    build_environment(args.home, lf),
                    args.output / f"{path}-{i}-{variant}",
                    provider,
                    profile=args.profile,
                    trust_fixture=True,
                )
                row = {
                    "provider": provider,
                    "path": path,
                    "variant": variant,
                    "sample": i,
                    **result,
                }
                record_readiness(args.output, row)


if __name__ == "__main__":
    main()
