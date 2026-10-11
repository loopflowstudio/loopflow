"""Measure first launch in fresh dense fixtures, without evicting OS/provider caches."""

import argparse
import shutil
import subprocess
import sys
from pathlib import Path

from fixture import prepare, read_session_ids, require_fixture
from measure import build_environment, measure
from native import prepare_session, record_readiness


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "candidate", "home", "repo", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    parser.add_argument("--provider", choices=["claude", "codex"], required=True)
    parser.add_argument("--samples", type=int, default=5)
    args = parser.parse_args()
    require_fixture(args.home)
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    seed = args.output / "seed"
    prepare(args.home / "loopflow.db", seed, False)
    variants = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}

    def record(path: str, variant: str, sample: int, result: dict) -> None:
        row = {
            "provider": args.provider,
            "phase": "first_use",
            "path": path,
            "variant": variant,
            "sample": sample,
            **result,
        }
        record_readiness(args.output, row)

    for i in range(args.samples):
        record(
            "direct",
            "direct",
            i,
            measure(
                [args.provider],
                args.repo,
                build_environment(seed, None),
                args.output / f"direct-{i}",
                args.provider,
                trust_fixture=True,
            ),
        )
        order = list(variants) if i % 2 == 0 else list(reversed(variants))
        for variant in order:
            home = args.output / f"home-{i}-{variant}"
            home.mkdir(mode=0o700)
            if sys.platform == "darwin":
                subprocess.run(
                    ["cp", "-c", str(seed / "loopflow.db"), str(home / "loopflow.db")], check=True
                )
            else:
                shutil.copyfile(seed / "loopflow.db", home / "loopflow.db")
            shutil.copyfile(seed / "startup-fixture.json", home / "startup-fixture.json")
            (home / "config.yaml").write_text(f"agent: {args.provider}\n")
            lf = variants[variant]
            before = read_session_ids(home)
            result = measure(
                [str(lf)],
                args.repo,
                build_environment(home, lf),
                args.output / f"bare-{i}-{variant}",
                args.provider,
            )
            record("bare", variant, i, result)
            created = read_session_ids(home) - before
            if len(created) != 1:
                raise RuntimeError("Bare launch must create exactly one benchmark Session")
            session = created.pop()
            if args.provider == "codex":
                session = prepare_session(
                    lf, home, args.repo, args.output / f"prepare-{i}-{variant}", args.provider
                )
            result = measure(
                [str(lf), "session", "connect", session],
                args.repo,
                build_environment(home, lf),
                args.output / f"connect-{i}-{variant}",
                args.provider,
            )
            record("connect", variant, i, result)


if __name__ == "__main__":
    main()
