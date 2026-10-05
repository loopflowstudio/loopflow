"""Probe pathname exclusion and inode sealing for capture maintenance.

This is an exclusion experiment, not a converter or installation acceptance.
The alias counterexample runs unprivileged in temporary directories. Released
CLI checks require a disposable Linux container with checksum-verified v0.13.3
at /fixture/prior/lf and no host Home, installation, or credential mounts.
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
import tempfile
from contextlib import closing
from pathlib import Path

CLI = Path("/fixture/prior/lf")
ACCOUNT = "lf-capture-exclusion"


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


def _open_writer(path: Path) -> subprocess.Popen[str]:
    return subprocess.Popen(
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
        descriptor = os.open(path, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    return saved


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--released-sha256")
    modes.add_argument("--probe-aliases", action="store_true")
    modes.add_argument("--write-aliases", type=Path)
    parser.add_argument("--excluded", action="store_true")
    args = parser.parse_args()
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
    assert hashlib.sha256(CLI.read_bytes()).hexdigest() == args.released_sha256
    version = subprocess.check_output([str(CLI), "--version"], text=True).strip()
    assert version == "lf 0.13.3", version
    subprocess.run(["useradd", "--create-home", ACCOUNT], check=True)
    account = pwd.getpwnam(ACCOUNT)
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
    writer = _open_writer(payload)
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
    # Directory exclusion still permits newly opened aliases and external paths.
    _attempt_alias_writes(home, aliases, excluded=False)
    writer = _open_writer(aliases / "events.jsonl")
    try:
        _await_open(writer)
        saved = _seal_fixture_inodes([home, external])
    finally:
        writer.communicate("write\n", timeout=10)
        assert writer.returncode == 0
    # Inode permissions also cannot revoke an existing writable descriptor.
    # Snapshot only after this known writer exits; production quiescence remains
    # a separate requirement, including writable mappings and inherited handles.
    assert payload.read_text().endswith("retained descriptor\n")
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
