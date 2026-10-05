"""Probe a privileged capture-maintenance boundary in a disposable Linux container.

This is an exclusion experiment, not a converter or installation acceptance.
Copy the checksum-verified v0.13.3 Linux CLI to /fixture/prior/lf. No host Home,
installation, or credentials may be mounted. The container must be discarded.
"""

import argparse
import hashlib
import os
import pwd
import select
import sqlite3
import stat
import subprocess
import sys
from pathlib import Path

CLI = Path("/fixture/prior/lf")
ACCOUNT = "lf-capture-exclusion"


def _invoke(
    home: Path, *args: str, lf_home: Path | None = None
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "runuser",
            "-u",
            ACCOUNT,
            "--",
            "env",
            "-i",
            f"HOME={home}",
            "PATH=/usr/bin:/bin",
            "NO_COLOR=1",
            *([f"LF_HOME={lf_home}"] if lf_home is not None else []),
            str(CLI),
            *args,
        ],
        cwd="/tmp",
        capture_output=True,
        text=True,
        timeout=30,
    )


def _snapshot(home: Path) -> dict[str, str]:
    result = {}
    for path in [home, *home.rglob("*")]:
        metadata = path.lstat()
        if path.is_symlink():
            content = f"link:{path.readlink()}"
        elif path.is_file():
            content = hashlib.sha256(path.read_bytes()).hexdigest()
        else:
            content = "directory"
        result[str(path.relative_to(home))] = (
            f"{metadata.st_mode}:{metadata.st_uid}:{metadata.st_gid}:{content}"
        )
    return result


def _exec_count(home: Path) -> int:
    with sqlite3.connect(f"file:{home}/.lf/loopflow.db?mode=ro", uri=True) as store:
        return store.execute("SELECT count(*) FROM execs").fetchone()[0]


def _await_open(process: subprocess.Popen[str]) -> None:
    assert process.stdout is not None
    ready, _, _ = select.select([process.stdout], [], [], 10)
    assert ready, "fixture process did not open its target within 10 seconds"
    assert process.stdout.readline().strip() == "opened"


def _attempt_released_writes(home: Path, excluded: bool) -> None:
    commands = [
        ("install", "preflight", "--json"),
        ("doctor", "--json"),
        # A missing local source exercises early journaling without a renderer.
        ("screenshot", "/fixture/absent.html", "-o", "/tmp/absent.png"),
    ]
    for command in commands:
        before = _snapshot(home) if excluded else _exec_count(home)
        result = _invoke(home, *command)
        # Observation failure is intentionally nonfatal. Exit status alone says
        # nothing about whether an old process wrote the protected database.
        if excluded:
            assert _snapshot(home) == before, f"protected bytes changed: {command}"
        else:
            assert _exec_count(home) > before, f"baseline did not journal: {command}"
        print(f"excluded={excluded} {command}: exit={result.returncode}", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--released-sha256", required=True)
    args = parser.parse_args()
    if not Path("/.dockerenv").exists() or os.geteuid() != 0:
        raise SystemExit("Requires root inside a disposable container without host mounts.")
    assert hashlib.sha256(CLI.read_bytes()).hexdigest() == args.released_sha256
    version = subprocess.check_output([str(CLI), "--version"], text=True).strip()
    assert version == "lf 0.13.3", version
    subprocess.run(["useradd", "--create-home", ACCOUNT], check=True)
    account = pwd.getpwnam(ACCOUNT)
    home = Path(account.pw_dir)
    parent = home.parent.stat()
    assert parent.st_uid == 0 and not parent.st_mode & 0o022
    # Ordinary commands cannot initialize the installation-owned main Home.
    # Seed an empty disposable store with the released CLI, then place it at
    # the fixture account's default path before exercising bypass commands.
    seed = home / ".seed"
    initialized = _invoke(home, "home", "id", "--json", lf_home=seed)
    assert initialized.returncode == 0, initialized.stderr
    seed.rename(home / ".lf")
    _attempt_released_writes(home, excluded=False)

    payload = home / ".lf/runs/aa/aa-exclusion/events.jsonl"
    payload.parent.mkdir(parents=True)
    payload.write_text("synthetic payload; not a populated Session fixture\n")
    os.chown(payload, account.pw_uid, account.pw_gid)
    # Retained descriptors defeat pathname permissions. Exercise that limitation
    # before declaring the fixture quiescent, rather than hiding it with a stub.
    writer = subprocess.Popen(
        [
            "runuser",
            "-u",
            ACCOUNT,
            "--",
            sys.executable,
            "-c",
            "import sys; f=open(sys.argv[1], 'a'); print('opened', flush=True); "
            "sys.stdin.readline(); f.write('retained descriptor' + chr(10)); f.close()",
            str(payload),
        ],
        cwd="/tmp",
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
    )
    try:
        _await_open(writer)
        # The root-owned parent prevents the account replacing its Home. Locking
        # only .lf would leave its writable parent able to rename and recreate it.
        os.chown(home, 0, 0)
        home.chmod(0o700)
        directory = os.open(home, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        # Complete the known writer before the frozen byte snapshot. This is
        # fixture ownership, not a production process-discovery implementation.
        writer.communicate("write\n", timeout=10)
        if writer.returncode != 0:
            raise RuntimeError("retained-descriptor fixture failed")
    assert payload.read_text().endswith("retained descriptor\n")
    _attempt_released_writes(home, excluded=True)

    # Kill a privileged worker while it has the intended target open. The
    # kernel's persisted directory ownership must outlive that worker. This
    # deliberately does not impersonate the missing candidate recovery API.
    worker = subprocess.Popen(
        [
            sys.executable,
            "-c",
            "import sqlite3,sys; "
            "db=sqlite3.connect(sys.argv[1]); "
            "db.execute('SELECT count(*) FROM execs').fetchone(); "
            "print('opened', flush=True); sys.stdin.read()",
            str(home / ".lf/loopflow.db"),
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
    )
    try:
        _await_open(worker)
        _attempt_released_writes(home, excluded=True)
    finally:
        worker.kill()
        worker.communicate(timeout=10)
    assert home.stat().st_uid == 0
    assert stat.S_IMODE(home.stat().st_mode) == 0o700
    _attempt_released_writes(home, excluded=True)
    print("Exclusion probe passed; candidate targeting and conversion remain unproved.")


if __name__ == "__main__":
    main()
