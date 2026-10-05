"""Probe pathname exclusion and inode sealing for capture maintenance.

This is an exclusion experiment, not a converter or installation acceptance.
The alias counterexample runs unprivileged in temporary directories. Released
CLI checks require a disposable Linux container with checksum-verified v0.13.3
at /fixture/prior/lf and no host Home, installation, or credential mounts.
"""

import argparse
import hashlib
import json
import os
import pwd
import select
import signal
import sqlite3
import stat
import subprocess
import sys
import tempfile
from collections.abc import Iterator
from contextlib import closing, contextmanager
from pathlib import Path

CLI = Path("/fixture/prior/lf")
ACCOUNT = "lf-capture-exclusion"


def _sync(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _metadata_boundary(stage: str, interrupt_at: str | None, pause_at: str | None) -> None:
    if stage == interrupt_at:
        os.kill(os.getpid(), signal.SIGKILL)
    if stage == pause_at:
        print("opened", flush=True)
        if sys.stdin.readline().strip() != "continue":
            raise SystemExit("fixture boundary was not released")


def _recover_metadata(
    receipt: Path, interrupt_at: str | None = None, pause_at: str | None = None
) -> None:
    entries = json.loads(receipt.read_text())
    # Check every identity before restoring any access. A pathname alone cannot
    # safely identify an object after an interrupted namespace exclusion.
    # The concurrent-swap case deliberately disproves this check as a fence.
    for entry in entries:
        metadata = Path(entry["path"]).lstat()
        if (metadata.st_dev, metadata.st_ino) != (entry["dev"], entry["ino"]):
            raise SystemExit("recovery target changed; metadata left untouched")
    _metadata_boundary("checked", interrupt_at, pause_at)
    for entry in reversed(entries):
        path = Path(entry["path"])
        if os.geteuid() == 0:
            os.chown(path, entry["uid"], entry["gid"])
        _metadata_boundary("restore-owner", interrupt_at, pause_at)
        path.chmod(entry["mode"])
        _metadata_boundary("restore-mode", interrupt_at, pause_at)
        _sync(path)


def _run_metadata_worker(
    operation: str, receipt: Path, *args: str
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, __file__, operation, str(receipt), *args],
        capture_output=True,
        text=True,
        timeout=10,
    )


def _probe_metadata_boundary(boundary: str) -> None:
    # This narrow counterexample deliberately leaves the shared parent writable.
    # The privileged variant uses root-owned inodes and an unprivileged renamer;
    # the portable variant proves pathname replacement, not ownership exclusion.
    privileged = os.geteuid() == 0
    account = pwd.getpwnam(ACCOUNT) if privileged else None
    with tempfile.TemporaryDirectory(prefix="lf-capture-recovery-") as temporary:
        root = Path(temporary)
        root.chmod(0o755)
        shared = root / "shared"
        shared.mkdir()
        if account:
            os.chown(shared, account.pw_uid, account.pw_gid)
        storage = shared / "payloads"
        storage.mkdir()
        payload = storage / "events.jsonl"
        payload.write_text("retained history\n")
        payload.chmod(0o644)
        storage.chmod(0o755)
        if account:
            for path in (storage, payload):
                os.chown(path, account.pw_uid, account.pw_gid)
        receipt = root / "restoration.json"
        entries = []
        # File first: a crash leaves the directory unsealed. Its restoration
        # record is still required before the first change to either inode.
        for path in (payload, storage):
            metadata = path.stat()
            entries.append(
                dict(
                    path=str(path),
                    dev=metadata.st_dev,
                    ino=metadata.st_ino,
                    uid=metadata.st_uid,
                    gid=metadata.st_gid,
                    mode=stat.S_IMODE(metadata.st_mode),
                )
            )
        receipt.write_text(json.dumps(entries))
        receipt.chmod(0o600)
        _sync(receipt)
        _sync(root)
        seal_boundary = boundary if boundary.startswith("seal-") else "seal-mode"
        worker = _run_metadata_worker(
            "--interrupt-metadata", receipt, "--interrupt-at", seal_boundary
        )
        assert worker.returncode == -signal.SIGKILL, worker.stderr
        assert stat.S_IMODE(payload.stat().st_mode) == (
            entries[0]["mode"] if seal_boundary == "seal-owner" else 0o400
        )
        assert stat.S_IMODE(storage.stat().st_mode) == entries[1]["mode"]
        # In the race case, recovery is alive and has accepted all identities
        # before another process replaces the path. No timing sleep is involved.
        recovery_process = None
        try:
            if boundary == "checked":
                recovery_process = subprocess.Popen(
                    [
                        sys.executable,
                        __file__,
                        "--recover-metadata",
                        str(receipt),
                        "--pause-at",
                        "checked",
                    ],
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                )
                _await_open(recovery_process)
            replacement = subprocess.run(
                [
                    *(["runuser", "-u", ACCOUNT, "--"] if privileged else []),
                    sys.executable,
                    "-c",
                    "import sys; from pathlib import Path; p=Path(sys.argv[1]); "
                    "(p/'unrelated').write_text('before'); "
                    "(p/'payloads').rename(p/'displaced'); (p/'payloads').mkdir(); "
                    "(p/'payloads/events.jsonl').write_text('replacement'); "
                    "(p/'payloads/events.jsonl').chmod(0o600); "
                    "(p/'unrelated').write_text('after')",
                    str(shared),
                ],
                capture_output=True,
                text=True,
                timeout=10,
            )
            assert replacement.returncode == 0, replacement.stderr
            before = _snapshot(shared)
            if recovery_process is not None:
                _, error = recovery_process.communicate("continue\n", timeout=10)
                assert recovery_process.returncode == 0, error
                assert _snapshot(shared) != before, "race did not alter replacement metadata"
                assert stat.S_IMODE(payload.stat().st_mode) == entries[0]["mode"]
                assert stat.S_IMODE((shared / "displaced/events.jsonl").stat().st_mode) == 0o400
            else:
                recovery = _run_metadata_worker("--recover-metadata", receipt)
                assert recovery.returncode != 0 and "target changed" in recovery.stderr
                assert _snapshot(shared) == before
        finally:
            if recovery_process is not None and recovery_process.poll() is None:
                recovery_process.kill()
                recovery_process.communicate(timeout=10)
        assert payload.read_text() == "replacement"
        assert (shared / "unrelated").read_text() == "after"
        assert (shared / "displaced/events.jsonl").read_text() == "retained history\n"
        # Only the fixture restores the known namespace. Recovery then uses the
        # on-disk receipt alone and is repeatable in separate processes.
        payload.unlink()
        storage.rmdir()
        (shared / "displaced").rename(storage)
        if boundary.startswith("restore-"):
            if privileged:
                os.chown(storage, 0, 0)
            storage.chmod(0o700)
            recovery = _run_metadata_worker(
                "--recover-metadata", receipt, "--interrupt-at", boundary
            )
            assert recovery.returncode == -signal.SIGKILL, recovery.stderr
            assert stat.S_IMODE(storage.stat().st_mode) == (
                0o700 if boundary == "restore-owner" else entries[1]["mode"]
            )
            assert stat.S_IMODE(payload.stat().st_mode) == 0o400
        for _ in range(2):
            recovery = _run_metadata_worker("--recover-metadata", receipt)
            assert recovery.returncode == 0, recovery.stderr
        for entry in entries:
            metadata = Path(entry["path"]).stat()
            assert (metadata.st_uid, metadata.st_gid, stat.S_IMODE(metadata.st_mode)) == (
                entry["uid"],
                entry["gid"],
                entry["mode"],
            )
        assert payload.read_text() == "retained history\n"
    print(f"Metadata boundary {boundary}: counterexample and fixture recovery verified.")


def _probe_interrupted_metadata() -> None:
    for boundary in ("seal-owner", "seal-mode", "checked", "restore-owner", "restore-mode"):
        _probe_metadata_boundary(boundary)


def _probe_retained_aliases() -> None:
    if os.geteuid() == 0:
        raise SystemExit("Run the alias counterexample as an unprivileged account.")
    with tempfile.TemporaryDirectory(prefix="lf-capture-alias-") as temporary:
        root = Path(temporary)
        home = root / "home"
        home.mkdir()
        payload = home / "events.jsonl"
        payload.write_text("before\n")
        database = home / "loopflow.db"
        with closing(sqlite3.connect(database)) as store:
            store.execute("PRAGMA journal_mode=WAL")
            store.execute("CREATE TABLE evidence (value TEXT NOT NULL)")
            store.execute("INSERT INTO evidence VALUES ('before')")
            store.commit()
        payload_alias = root / "retained-events.jsonl"
        database_alias = root / "retained.db"
        os.link(payload, payload_alias)
        os.link(database, database_alias)
        original_mode = stat.S_IMODE(home.stat().st_mode)
        home.chmod(0)
        try:
            try:
                payload.read_bytes()
            except PermissionError:
                pass
            else:
                raise AssertionError("the original pathname is still accessible")
            # Both writers open after exclusion; no retained descriptor or
            # process is required to reach these same inodes through aliases.
            with payload_alias.open("a") as output:
                output.write("after exclusion\n")
            with closing(sqlite3.connect(database_alias)) as store:
                store.execute("INSERT INTO evidence VALUES ('after exclusion')")
                store.commit()
                assert store.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone() == (0, 0, 0)
        finally:
            home.chmod(original_mode)
        assert payload.read_text() == "before\nafter exclusion\n"
        with closing(sqlite3.connect(database)) as store:
            assert store.execute("SELECT value FROM evidence ORDER BY rowid").fetchall() == [
                ("before",),
                ("after exclusion",),
            ]
    print("Counterexample confirmed: retained aliases bypass Home pathname exclusion.")


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
        if stat.S_ISLNK(metadata.st_mode):
            content = f"link:{path.readlink()}"
        elif stat.S_ISREG(metadata.st_mode):
            content = hashlib.sha256(path.read_bytes()).hexdigest()
        else:
            content = "directory"
        result[str(path.relative_to(home))] = (
            f"{metadata.st_mode}:{metadata.st_uid}:{metadata.st_gid}:{content}"
        )
    return result


def _exec_count(home: Path) -> int:
    with closing(sqlite3.connect(f"file:{home}/.lf/loopflow.db?mode=ro", uri=True)) as store:
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


def _write_aliases(aliases: Path, excluded: bool) -> None:
    for path in (aliases / "events.jsonl", aliases / "loopflow.db"):
        try:
            path.chmod(0o600)
        except PermissionError:
            assert excluded
        else:
            assert not excluded, "alias can restore write permission"
    try:
        with (aliases / "events.jsonl").open("a") as output:
            output.write("alias write\n")
    except PermissionError:
        assert excluded
    else:
        assert not excluded, "payload alias remains writable"
    try:
        with closing(sqlite3.connect(aliases / "loopflow.db")) as store:
            store.execute("CREATE TABLE IF NOT EXISTS alias_probe (value TEXT)")
            store.execute("INSERT INTO alias_probe VALUES ('alias write')")
            store.commit()
            assert store.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone() == (0, 0, 0)
    except sqlite3.OperationalError:
        assert excluded
    else:
        assert not excluded, "database alias remains writable"


def _attempt_alias_writes(home: Path, aliases: Path, excluded: bool) -> None:
    result = subprocess.run(
        [
            "runuser",
            "-u",
            ACCOUNT,
            "--",
            sys.executable,
            __file__,
            "--write-aliases",
            str(aliases),
            *(["--excluded"] if excluded else []),
        ],
        cwd="/tmp",
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert result.returncode == 0, result.stderr
    # Exercise the released journal opener through a different LF_HOME whose
    # database is a hard link. The alias directory itself remains writable.
    before = _exec_count(home)
    _invoke(home, "doctor", "--json", lf_home=aliases)
    if excluded:
        assert _exec_count(home) == before
    else:
        assert _exec_count(home) > before


@contextmanager
def _retained_writer(path: Path) -> Iterator[None]:
    before = path.read_bytes()
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
            str(path),
        ],
        cwd="/tmp",
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
    )
    try:
        _await_open(writer)
        yield
    finally:
        # Finish this known writer before any frozen snapshot. This establishes
        # fixture ownership, not production process discovery or quiescence.
        writer.communicate("write\n", timeout=10)
        assert writer.returncode == 0, "retained-descriptor fixture failed"
        assert path.read_bytes() == before + b"retained descriptor\n"


def _attempt_storage_replacement(storage: Path, excluded: bool) -> None:
    result = subprocess.run(
        [
            "runuser",
            "-u",
            ACCOUNT,
            "--",
            sys.executable,
            "-c",
            "import sys; from pathlib import Path; p=Path(sys.argv[1]); "
            "q=p.with_name('replaced'); p.rename(q); q.rename(p)",
            str(storage),
        ],
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert (result.returncode != 0) == excluded, result.stderr


def _seal_fixture_inodes(roots: list[Path]) -> list[tuple[Path, os.stat_result]]:
    # Explicit fixture roots include the external storage's replacement boundary.
    # This is not production discovery: ACLs, mount semantics and arbitrary
    # symlink targets require their own inventory and preservation contract.
    saved = [(path, path.lstat()) for root in roots for path in [root, *root.rglob("*")]]
    for path, metadata in saved:
        if stat.S_ISLNK(metadata.st_mode):
            continue
        os.chown(path, 0, 0)
        path.chmod(0o700 if stat.S_ISDIR(metadata.st_mode) else 0o600)
        _sync(path)
    return saved


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--released-sha256")
    modes.add_argument("--probe-aliases", action="store_true")
    modes.add_argument("--write-aliases", type=Path)
    modes.add_argument("--probe-recovery", action="store_true")
    modes.add_argument("--interrupt-metadata", type=Path)
    modes.add_argument("--recover-metadata", type=Path)
    parser.add_argument("--excluded", action="store_true")
    parser.add_argument(
        "--interrupt-at", choices=("seal-owner", "seal-mode", "restore-owner", "restore-mode")
    )
    parser.add_argument("--pause-at", choices=("checked",))
    args = parser.parse_args()
    if os.geteuid() == 0 and not Path("/.dockerenv").exists():
        raise SystemExit("Privileged probes require the disposable container.")
    if args.interrupt_metadata:
        entry = json.loads(args.interrupt_metadata.read_text())[0]
        path = Path(entry["path"])
        if os.geteuid() == 0:
            os.chown(path, 0, 0)
        _metadata_boundary("seal-owner", args.interrupt_at, None)
        path.chmod(0o400)
        _metadata_boundary("seal-mode", args.interrupt_at, None)
        _sync(path)
        os.kill(os.getpid(), signal.SIGKILL)
    if args.recover_metadata:
        _recover_metadata(args.recover_metadata, args.interrupt_at, args.pause_at)
        return
    if args.probe_recovery:
        if os.geteuid() == 0:
            raise SystemExit("Run the portable recovery probe unprivileged.")
        _probe_interrupted_metadata()
        return
    if args.write_aliases:
        if not Path("/.dockerenv").exists() or os.geteuid() == 0:
            raise SystemExit("Alias writes require the disposable container's fixture account.")
        _write_aliases(args.write_aliases, args.excluded)
        return
    if args.probe_aliases:
        _probe_retained_aliases()
        return
    if not Path("/.dockerenv").exists() or os.geteuid() != 0:
        raise SystemExit("Requires root inside a disposable container without host mounts.")
    _probe_released_writers(args.released_sha256)


def _probe_released_writers(released_sha256: str) -> None:
    assert hashlib.sha256(CLI.read_bytes()).hexdigest() == released_sha256
    version = subprocess.check_output([str(CLI), "--version"], text=True).strip()
    assert version == "lf 0.13.3", version
    subprocess.run(["useradd", "--create-home", ACCOUNT], check=True)
    account = pwd.getpwnam(ACCOUNT)
    _probe_interrupted_metadata()
    subprocess.run(
        ["runuser", "-u", ACCOUNT, "--", sys.executable, __file__, "--probe-aliases"],
        check=True,
        timeout=30,
        cwd="/tmp",
    )
    home = Path(account.pw_dir)
    home_metadata = home.stat()
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

    external = Path("/fixture/external")
    external.mkdir()
    os.chown(external, account.pw_uid, account.pw_gid)
    storage = external / "payloads"
    storage.mkdir()
    os.chown(storage, account.pw_uid, account.pw_gid)
    (home / ".lf/runs").symlink_to(storage, target_is_directory=True)
    payload = storage / "aa/aa-exclusion/events.jsonl"
    payload.parent.mkdir(parents=True)
    payload.write_text("synthetic payload; not a populated Session fixture\n")
    os.chown(payload, account.pw_uid, account.pw_gid)
    aliases = Path("/tmp/lf-capture-aliases")
    aliases.mkdir()
    os.chown(aliases, account.pw_uid, account.pw_gid)
    os.link(payload, aliases / "events.jsonl")
    os.link(home / ".lf/loopflow.db", aliases / "loopflow.db")
    _attempt_alias_writes(home, aliases, excluded=False)
    _attempt_storage_replacement(storage, excluded=False)
    # Retained descriptors defeat pathname permissions. Exercise that limitation
    # before declaring the fixture quiescent, rather than hiding it with a stub.
    with _retained_writer(payload):
        # The root-owned parent prevents the account replacing its Home. Locking
        # only .lf would leave its writable parent able to rename and recreate it.
        os.chown(home, 0, 0)
        home.chmod(0o700)
        _sync(home)
    _attempt_released_writes(home, excluded=True)
    # Directory exclusion still permits newly opened aliases and external paths.
    _attempt_alias_writes(home, aliases, excluded=False)
    with _retained_writer(aliases / "events.jsonl"):
        saved = _seal_fixture_inodes([home, external])
    # Inode permissions also cannot revoke an existing writable descriptor.
    # Snapshot only after this known writer exits; production quiescence remains
    # a separate requirement, including writable mappings and inherited handles.
    frozen = (_snapshot(home), _snapshot(external))
    _attempt_alias_writes(home, aliases, excluded=True)
    assert (_snapshot(home), _snapshot(external)) == frozen

    _attempt_storage_replacement(storage, excluded=True)
    assert (_snapshot(home), _snapshot(external)) == frozen

    # Kill a privileged worker while it has the intended target open. The
    # kernel's persisted inode ownership must outlive that worker. This
    # deliberately does not impersonate the missing candidate recovery API.
    worker = subprocess.Popen(
        [
            sys.executable,
            "-c",
            "import sqlite3,sys; "
            "db=sqlite3.connect('file:'+sys.argv[1]+'?mode=ro&immutable=1', uri=True); "
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
        _attempt_alias_writes(home, aliases, excluded=True)
    finally:
        worker.kill()
        worker.communicate(timeout=10)
    assert home.stat().st_uid == 0
    assert stat.S_IMODE(home.stat().st_mode) == 0o700
    _attempt_released_writes(home, excluded=True)
    _attempt_alias_writes(home, aliases, excluded=True)
    _attempt_storage_replacement(storage, excluded=True)
    assert (_snapshot(home), _snapshot(external)) == frozen
    # Recovery here restores only this fixture's metadata. There is no durable
    # candidate receipt or layout conversion, so this proves neither of those.
    for path, metadata in reversed(saved):
        if stat.S_ISLNK(metadata.st_mode):
            continue
        os.chown(path, metadata.st_uid, metadata.st_gid)
        path.chmod(stat.S_IMODE(metadata.st_mode))
        restored = path.stat()
        assert (restored.st_uid, restored.st_gid, restored.st_mode) == (
            metadata.st_uid,
            metadata.st_gid,
            metadata.st_mode,
        )
    os.chown(home, home_metadata.st_uid, home_metadata.st_gid)
    home.chmod(stat.S_IMODE(home_metadata.st_mode))
    _attempt_alias_writes(home, aliases, excluded=False)
    _attempt_storage_replacement(storage, excluded=False)
    _attempt_released_writes(home, excluded=False)
    print(
        "Inode probe passed for fixture aliases and external storage; conversion remains unproved."
    )


if __name__ == "__main__":
    main()
