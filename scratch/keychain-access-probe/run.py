"""Disposable ACL diagnosis; stdout contains only statuses and comparisons."""

import json
import subprocess
import tempfile
import uuid
from pathlib import Path


ROOT = Path(__file__).parent


def _run(args: list[str], data: bytes | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(args, input=data, capture_output=True, timeout=15)


def _snapshot(probe: Path, service: str) -> dict:
    result = _run([str(probe), "snapshot", service])
    if result.returncode:
        raise RuntimeError("synthetic access snapshot failed")
    return json.loads(result.stdout)


def _read(service: str) -> subprocess.CompletedProcess:
    return _run(["/usr/bin/security", "find-generic-password", "-s", service, "-a", "probe", "-w"])


def main() -> None:
    results = []
    with tempfile.TemporaryDirectory(prefix="loo661-explicit-") as directory:
        probe = Path(directory) / "probe"
        compiled = _run(["clang", "-Wno-deprecated-declarations", str(ROOT / "probe.c"),
            "-framework", "Security", "-framework", "CoreFoundation", "-o", str(probe)])
        if compiled.returncode:
            raise RuntimeError("probe compilation failed: " + compiled.stderr.decode())
        for mode in ("rebuild_without_integrity", "rebuild_without_partition", "rebuild_without_both", "explicit_security"):
            for size in (2048, 8192):
                prefix = "loopflow-LOO-661-explicit-probe-" + uuid.uuid4().hex
                original, candidate = prefix + "-old", prefix + "-new"
                old = b'{"padding":"old-synthetic-value"}'
                expected = b'{"padding":"' + b"x" * (size - 14) + b'"}'
                result = {"mode": mode, "size": size}
                try:
                    seed = _run(["/usr/bin/security", "-i"],
                        f"add-generic-password -a probe -s {original} -X {old.hex()}\n".encode())
                    if seed.returncode or _read(original).stdout != old + b"\n":
                        raise RuntimeError("synthetic seed failed")
                    before = _snapshot(probe, original)
                    result["original_apple_tool"] = before["apple_tool"]
                    run = _run([str(probe), mode, original, candidate, str(size)])
                    result["probe_exit"] = run.returncode
                    if run.returncode == 0:
                        result["api"] = json.loads(run.stdout)
                        if result["api"]["create"] == 0:
                            access = _snapshot(probe, candidate)
                            result["candidate_partition_decoded"] = access["decoded"] == access["partition_count"] == 1
                            result["candidate_apple_tool"] = access["apple_tool"]
                    result["original_access_exact"] = _snapshot(probe, original) == before
                    for service, label, value in ((original, "old", old), (candidate, "new", expected)):
                        try:
                            read = _read(service)
                            result[label + "_read_exit"] = read.returncode
                            result[label + "_exact"] = read.stdout == value + b"\n"
                        except subprocess.TimeoutExpired:
                            result[label + "_read_timeout"] = True
                except subprocess.TimeoutExpired:
                    result["probe_timeout"] = True
                finally:
                    for service, label in ((candidate, "new"), (original, "old")):
                        cleanup = _run(["/usr/bin/security", "delete-generic-password", "-s", service, "-a", "probe"])
                        result[label + "_cleanup_exit"] = cleanup.returncode
                results.append(result)
                print(json.dumps(result), flush=True)
                (ROOT / "results.json").write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
