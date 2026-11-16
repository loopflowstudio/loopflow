"""Replay the authored migration demonstrations in disposable directories."""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]


def _run(command: list[str], cwd: Path, succeeds: bool = True) -> str:
    started = time.monotonic()
    result = subprocess.run(
        command, cwd=cwd, stdin=subprocess.DEVNULL, capture_output=True, text=True,
        env={key: value for key, value in os.environ.items() if not key.startswith("LF_")},
    )
    print(f"{' '.join(command)}: exit {result.returncode}, {time.monotonic() - started:.3f}s")
    if (result.returncode == 0) != succeeds:
        raise RuntimeError(result.stdout + result.stderr)
    return result.stdout + result.stderr


def main() -> None:
    sources = [REPO / ".lf/skills/implement.md"]
    sources.extend(REPO / "rust/loopflow/src/builtins/task/skill" / f"{name}.md"
                   for name in ("design", "kickoff", "implement", "compress",
                                "concept-review", "realign", "loop-or-next"))
    for path in sources:
        print(f"{path.relative_to(REPO)} sha256={hashlib.sha256(path.read_bytes()).hexdigest()}")

    with tempfile.TemporaryDirectory(prefix="journey-") as directory:
        root = Path(directory)
        ownership = root / "ownership"
        shutil.copytree(ROOT / "ownership", ownership,
                        ignore=shutil.ignore_patterns("target", "Cargo.lock"))
        consumers = [ownership / path for path in ("src/lib.rs", "src/main.rs", "tests/cli.rs")]
        (ownership / "src/definitions.rs").rename(ownership / "src/runtime.rs")
        for path in consumers:
            path.write_text(path.read_text().replace("definitions", "runtime"))
        command = ["cargo", "test", "--test", "cli"]
        _run(command, ownership)

        # Try production and tests together before growing the new owner.
        (ownership / "src/runtime.rs").rename(ownership / "src/definitions.rs")
        for path in consumers[:2]:
            path.write_text(path.read_text().replace("runtime", "definitions"))
        failure = _run(command, ownership, succeeds=False)
        if "unresolved import" not in failure or "runtime" not in failure:
            raise RuntimeError(f"Expected the missed test consumer, got: {failure}")
        consumers[2].write_text(consumers[2].read_text().replace("runtime", "definitions"))
        _run(command, ownership)
        print("Ownership: CLI/library complete; Graph definitions and exports pending.")

        state = root / "state.py"
        source = (ROOT / "state.py").read_text()
        incomplete = source.replace(
            '        state = self.store.complete(task, allowed)\n'
            '        for consumer in consumers:\n'
            '            consumer.receive(state)',
            '        if not allowed:\n'
            '            raise PermissionError("completion refused")\n'
            '        self.tasks.remove(task)\n'
            '        self.selected = next(iter(self.tasks), None)',
        )
        if incomplete == source:
            raise RuntimeError("Reducer-only negative control no longer matches the fixture")
        state.write_text(incomplete)
        failure = _run(["uv", "run", "--no-project", "python", "state.py"], root, succeeds=False)
        if "AssertionError" not in failure:
            raise RuntimeError(f"Expected missing consumer behavior, got: {failure}")
        state.write_text(source)
        _run(["uv", "run", "--no-project", "python", "state.py"], root)
        print("Complete: both lists, persistence, stale reply, restart, refusal and empty list pass.")
        print("Create/Reopen pending; native timing unmeasured. No application build claim.")


if __name__ == "__main__":
    main()
