"""Run the ignored whole-observer fixture in fresh processes, serially."""

import argparse
import hashlib
import json
import os
import subprocess
import tempfile
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--rounds", default=5, type=int)
    parser.add_argument("--profile", action="store_true")
    args = parser.parse_args()
    binary = args.binary.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    sources = [
        Path("rust/loopflow/src/performance.rs"),
        Path("rust/loopflow/src/performance/observer_measurement.rs"),
        Path("rust/loopflow/src/harness/codex_connection.rs"),
        Path("rust/loopflow/src/harness/observer_measurement.rs"),
        Path(__file__),
    ]
    metadata = {
        "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
        "binary": str(binary),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "harness_sha256": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources},
        "profile": args.profile,
        "rounds": args.rounds,
    }
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    workloads = [
        (kind, count, active)
        for kind in ("native", "codex")
        for count in (1, 16)
        for active in (0, 1)
    ]
    if args.profile:
        workloads = [(kind, 16, 0) for kind in ("native", "codex")]
    receipts = []
    for repeat in range(args.rounds):
        order = workloads if repeat % 2 == 0 else list(reversed(workloads))
        for kind, count, active in order:
            label = f"{repeat + 1}-{kind}-{count}-{active}"
            env = {k: v for k, v in os.environ.items() if not k.startswith("LF_")}
            env.update(
                OBSERVER_KIND=kind,
                OBSERVER_COUNT=str(count),
                OBSERVER_ACTIVE=str(active),
                OBSERVER_PROFILE=str(int(args.profile)),
            )
            command = [
                str(binary),
                "observer_baseline",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ]
            with tempfile.TemporaryDirectory(prefix="observer-home-") as home:
                env["HOME"] = home
                with (args.output / f"{label}.log").open("w") as log:
                    process = subprocess.Popen(
                        command,
                        env=env,
                        cwd=home,
                        stdin=subprocess.DEVNULL,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.STDOUT,
                        text=True,
                    )
                    result = None
                    for line in process.stdout:
                        log.write(line)
                        log.flush()
                        if args.profile and "OBSERVER_READY" in line:
                            subprocess.run(
                                [
                                    "/usr/bin/sample",
                                    str(process.pid),
                                    "2",
                                    "1",
                                    "-file",
                                    str((args.output / f"{label}.sample").resolve()),
                                ],
                                stdout=subprocess.DEVNULL,
                                stderr=subprocess.DEVNULL,
                                check=True,
                            )
                        if "OBSERVER_RESULT " in line:
                            result = json.loads(line.split("OBSERVER_RESULT ", 1)[1])
                    status = process.wait()
            if status != 0 or result is None:
                raise SystemExit(f"{label} failed ({status}); see {args.output / f'{label}.log'}")
            if receipts and result["output_sha256"] != receipts[0]["output_sha256"]:
                raise SystemExit(f"{label}: delivered output differs from the first sample")
            result.update(repeat=repeat + 1, command=command, label=label)
            receipts.append(result)
            (args.output / "results.json").write_text(json.dumps(receipts, indent=2) + "\n")
            print(
                f"{label}: {result['work']['cpu_ms']:.3f} CPU ms, "
                f"{result['work']['polls']} polls, {result['work']['vm_steps']} VM steps",
                flush=True,
            )
    print(f"{len(receipts)} completed samples; {args.output}", flush=True)


if __name__ == "__main__":
    main()
