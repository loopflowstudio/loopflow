"""Reproduce security -i replacement failures using disposable synthetic items."""

import json
import subprocess
import uuid


def _run(args: list[str], data: bytes | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["/usr/bin/security", *args],
        input=data,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=15,
        start_new_session=True,
    )


def _value(size: int) -> bytes:
    base = b'{"claudeAiOauth":{"accessToken":"synthetic"},"padding":""}'
    return base[:-2] + b"x" * (size - len(base)) + base[-2:]


def _probe(account: str, size: int) -> dict:
    service = "loopflow-LOO-661-prefix-probe-" + uuid.uuid4().hex
    old, new = _value(64), _value(size)
    report = {"account_length": len(account), "size": size}

    def _write(data: bytes) -> subprocess.CompletedProcess:
        script = f'add-generic-password -U -a "{account}" -s "{service}" -X {data.hex()}\n'
        return _run(["-i"], script.encode())

    def _read() -> subprocess.CompletedProcess:
        return _run(["find-generic-password", "-a", account, "-s", service, "-w"])

    try:
        created = _write(old)
        baseline = _read()
        report["baseline_ok"] = (
            created.returncode == 0
            and baseline.returncode == 0
            and baseline.stdout.rstrip(b"\n") == old
        )
        if report["baseline_ok"]:
            result = _write(new)
            read = _read()
            output = read.stdout.rstrip(b"\n")
            report.update(
                write_exit=result.returncode,
                read_exit=read.returncode,
                read_output_length=len(output),
                old_retained=output == old,
                exact_new=output == new,
            )
    except subprocess.TimeoutExpired:
        report["timed_out"] = True
    finally:
        try:
            report["cleanup_exit"] = _run(
                ["delete-generic-password", "-a", account, "-s", service]
            ).returncode
        except subprocess.TimeoutExpired:
            report["cleanup_timed_out"] = True
        if report.get("cleanup_exit") != 0:
            report["cleanup_service"] = service
    return report


if __name__ == "__main__":
    reports = []
    for account in ("probe", "probe2"):
        for size in (1242, 2048, 8192):
            report = _probe(account, size)
            reports.append(report)
            print(json.dumps(report))
    # Zero means the experiment completed, not that persistence is correct.
    raise SystemExit(
        0 if all(
            row.get("baseline_ok")
            and row.get("cleanup_exit") == 0
            and "write_exit" in row
            and not row.get("timed_out")
            for row in reports
        ) else 1
    )
