"""Upgrade a live 0.14.3 Session in a disposable account; provider and viewer are fixtures."""

import hashlib
import json
import os
import signal
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

from repository_upgrade import PRIOR, _fetch_prior, _run

PROVIDER = r"""#!/usr/bin/python3
import json
import os
import subprocess
import sys
import time
from pathlib import Path

home = Path.home()
args = sys.argv[1:]
if args == ["--version"]:
    print("1.0.0 fixture")
    raise SystemExit(0)
resumed = "--resume" in args
flag = "--resume" if resumed else "--session-id"
native = args[args.index(flag) + 1]
history = home / "native-history"
if resumed:
    assert history.read_text() == native + "\nsaved conversation\n"
else:
    history.write_text(native + "\nsaved conversation\n")
settings = json.loads(Path(args[args.index("--settings") + 1]).read_text())
binary = os.environ["LF_BIN"]
checks = []
for command in ([binary, "session", "list", "--json"],
                [binary, "flow", "nested-proof", "--batch"]):
    result = subprocess.run(command, capture_output=True, text=True, timeout=30)
    checks.append({"command": command[1:], "code": result.returncode,
                   "stderr": result.stderr})
hook = settings["hooks"]["SessionStart"][0]["hooks"][0]["command"]
result = subprocess.run(hook, shell=True, input=json.dumps({
    "session_id": native, "cwd": os.getcwd(), "hook_event_name": "SessionStart",
    "source": "resume" if resumed else "startup"}),
    capture_output=True, text=True, timeout=30)
checks.append({"hook": hook, "code": result.returncode, "stderr": result.stderr})
record = {"pid": os.getpid(), "native": native, "resumed": resumed,
          "binary": binary, "cwd": os.getcwd(), "args": args, "checks": checks}
with (home / "native-launches.jsonl").open("a") as output:
    output.write(json.dumps(record) + "\n")
print("Recovered saved conversation" if resumed else "Original conversation", flush=True)
while True:
    time.sleep(1)
"""


def _wait(predicate, description: str, timeout: int = 45):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if result := predicate():
            return result
        time.sleep(0.1)
    raise AssertionError(f"Timed out: {description}")


def main() -> None:
    assert Path("/.dockerenv").exists() and os.getuid() == 0, "requires disposable container"
    if sys.argv[1] == "--fetch-prior":
        _fetch_prior()
        return
    candidate = str(Path(sys.argv[1]).resolve())
    _scenario(candidate, False)
    _scenario(candidate, True)


def _scenario(candidate: str, missed_recovery: bool) -> None:
    user = "lf-session-upgrade-retry" if missed_recovery else "lf-session-upgrade"
    _run("useradd", "--create-home", user)
    home = Path("/home") / user
    repo = home / "retained"
    repo.mkdir()
    bin_dir = home / ".local/bin"
    bin_dir.mkdir(parents=True)
    provider = bin_dir / "claude"
    provider.write_text(PROVIDER)
    provider.chmod(0o755)
    terminal = bin_dir / "fixture-terminal"
    terminal.write_text(
        '#!/bin/sh\nprintf "opened\\n" >> "$HOME/presentations"\n'
        'shift\nexec "$@" > "$HOME/viewer-output"\n'
    )
    terminal.chmod(0o755)
    lf_home = home / ".lf"
    lf_home.mkdir()
    (lf_home / "config.yaml").write_text(f"agent: claude\nsession:\n  terminal: {terminal}\n")
    _run("chown", "-R", f"{user}:{user}", str(home))

    def command(binary: str, *args: str, pin: str | None = None) -> list[str]:
        return [
            "runuser",
            "-u",
            user,
            "--",
            "env",
            "-i",
            f"HOME={home}",
            f"PATH={bin_dir}:/usr/bin:/bin",
            "TERM=xterm-256color",
            *([f"LF_BIN={pin}"] if pin else []),
            binary,
            *args,
        ]

    def invoke(binary: str, *args: str) -> str:
        result = subprocess.run(
            command(binary, *args),
            cwd=repo,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=180,
        )
        if result.stderr:
            print(result.stderr, file=sys.stderr)
        assert result.returncode == 0, result.stdout + result.stderr
        return result.stdout

    invoke("/usr/bin/git", "init", "-q")
    invoke(
        "/usr/bin/git",
        "-c",
        "user.name=Proof",
        "-c",
        "user.email=proof@example.invalid",
        "commit",
        "--allow-empty",
        "-qm",
        "Initial",
    )
    entry = str(bin_dir / "lf")
    invoke(str(PRIOR / "lf"), "self", "install", "promote", "--cli-target", entry)
    assert invoke(entry, "--version").strip() == "lf 0.14.3"
    pinned = lf_home / "bin" / f"lf-{hashlib.sha256((PRIOR / 'lf').read_bytes()).hexdigest()}"
    assert pinned.is_file()
    task = json.loads(invoke(entry, "task", "create", "--title", "Session upgrade", "--json"))
    placed = json.loads(invoke(entry, "task", "checkout", task["id"], "--json"))
    repo = Path(placed["worktree"])
    flows = repo / ".lf/flows"
    flows.mkdir(parents=True, exist_ok=True)
    (flows / "nested-proof.yaml").write_text("- cmd: sync --plan\n")
    (flows / "live-proof.yaml").write_text("- cmd: __telemetry-scorecard\n- cmd: sync --plan\n")
    scripts = repo / "scripts"
    scripts.mkdir(exist_ok=True)
    (scripts / "lifecycle_scorecard.py").write_text(
        "import json, pathlib, sys, time\nrepo = pathlib.Path(sys.argv[2])\n"
        "with repo.joinpath('effects').open('a') as output: output.write('started\\n')\n"
        "repo.joinpath('entered').touch()\ndeadline = time.monotonic() + 120\n"
        "while not repo.joinpath('release').exists():\n"
        "    if time.monotonic() > deadline: raise RuntimeError('release timed out')\n"
        "    time.sleep(0.05)\n"
        "print(json.dumps({'report': {}, 'metric_observations': [], 'text': 'finished'}))\n"
    )
    _run("chown", "-R", f"{user}:{user}", str(repo))
    session = json.loads(invoke(entry, "session", "ensure", "--task", task["id"], "--json"))
    identity = session["id"]
    database = lf_home / "loopflow.db"

    def launches() -> list[dict]:
        path = home / "native-launches.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    log = (home / "original-terminal.log").open("w")
    original = subprocess.Popen(
        command(str(pinned), "session", "connect", identity, pin=str(pinned)),
        cwd=repo,
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=log,
    )
    live = None
    live_identity = None
    try:
        before = _wait(lambda: launches(), "released native launch")[0]
        assert all(check["code"] == 0 for check in before["checks"]), before
        if not missed_recovery:
            live = subprocess.Popen(
                command(str(pinned), "flow", "live-proof", "--batch", pin=str(pinned)),
                cwd=repo,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=log,
            )
            _wait(lambda: (repo / "entered").exists(), "released Flow effect")
            entries = json.loads(invoke(entry, "flow", "list", "--processes", "--all", "--json"))
            live_identity = next(
                row["id"] for row in entries["entries"] if row["name"] == "live-proof"
            )
        invoke(entry, "session", "steer", identity, "Preserve this pending direction")
        steers = json.loads(invoke(entry, "session", "steers", identity, "--json"))
        with sqlite3.connect(database) as db:
            events = db.execute(
                "SELECT seq,payload FROM session_events WHERE session_id=? ORDER BY seq",
                (identity,),
            ).fetchall()
        if missed_recovery:
            mode = pinned.stat().st_mode
            try:
                # Failed executable inspection is not permission to stop the client.
                pinned.chmod(mode & ~0o111)
                invoke(candidate, "self", "install", "promote", "--cli-target", entry)
                assert len(launches()) == 1
                os.kill(before["pid"], 0)
            finally:
                pinned.chmod(mode)
            invoke(candidate, "self", "install", "promote", "--cli-target", entry)
        else:
            # Both installers may observe the old attachment before its exit.
            promotions = [
                subprocess.Popen(
                    command(candidate, "self", "install", "promote", "--cli-target", entry),
                    cwd=repo,
                    stdin=subprocess.DEVNULL,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    text=True,
                )
                for _ in range(2)
            ]
            for promotion in promotions:
                output, _ = promotion.communicate(timeout=180)
                assert promotion.returncode == 0, output
        after = _wait(
            lambda: next((row for row in launches() if row["resumed"]), None),
            "automatic installation recovery",
        )
        assert after["native"] == before["native"] and after["pid"] != before["pid"]
        assert after["binary"] != before["binary"]
        preflight = subprocess.run(
            command(before["binary"], "self", "install", "preflight", "--json"),
            cwd=repo,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=30,
        )
        assert preflight.returncode != 0
        evidence = json.loads(preflight.stdout)
        assert evidence["compatibility"]["kind"] == "incompatible", evidence
        assert after["cwd"] == before["cwd"]
        assert all(check["code"] == 0 for check in after["checks"]), after
        assert "--resume" in after["args"] and "--prompt" not in after["args"]
        original.wait(timeout=15)
        _wait(
            lambda: (
                (home / "viewer-output").exists()
                and "Recovered saved conversation" in (home / "viewer-output").read_text()
            ),
            "automatic viewer presentation",
        )
        with sqlite3.connect(database) as db:
            retained = dict(
                db.execute(
                    "SELECT seq,payload FROM session_events WHERE session_id=?", (identity,)
                ).fetchall()
            )
        assert all(retained[seq] == payload for seq, payload in events)
        retained_steers = json.loads(invoke(entry, "session", "steers", identity, "--json"))
        for item in steers["items"]:
            assert any(row["steer"] == item["steer"] for row in retained_steers["items"])
        retry = invoke(candidate, "self", "install", "promote", "--cli-target", entry)
        assert "already installed" in retry, retry
        assert len(launches()) == 2
        assert (home / "presentations").read_text() == "opened\n"
        assert (
            json.loads(invoke(entry, "session", "ensure", "--task", task["id"], "--json"))["id"]
            == identity
        )
        retained_task = json.loads(invoke(entry, "task", "status", task["id"], "--json"))
        assert retained_task["task"]["id"] == task["id"]
        assert retained_task["execution"]["worktree"] == str(repo)
        if live is not None:
            assert live.poll() is None, "installation interrupted the live Flow"
            observed = json.loads(
                invoke(entry, "flow", "show", live_identity, "--processes", "--json")
            )
            assert observed["entry"]["state"] == "current"
            assert [step["label"] for step in observed["steps"]] == ["__telemetry-scorecard"]
            (repo / "release").touch()
            live.wait(timeout=30)
            retained_flow = json.loads(
                invoke(entry, "flow", "show", live_identity, "--processes", "--json")
            )
            assert retained_flow["entry"]["state"] != "completed"
            (flows / "remaining-proof.yaml").write_text("- cmd: sync --plan\n")
            invoke(entry, "flow", "remaining-proof", "--batch")
            final_flow = json.loads(
                invoke(entry, "flow", "show", live_identity, "--processes", "--json")
            )
            assert final_flow["steps"] == retained_flow["steps"]
            assert (repo / "effects").read_text() == "started\n"
        print(
            f"PASS ({'missed-scheduling retry' if missed_recovery else 'concurrent promotion'}): "
            "real 0.14.3 promotion recovered the same live Session; hooks, nested commands, "
            "history, steers and one viewer survived; compatible reinstall kept its provider. "
            "Provider and terminal executables are fixtures."
        )
    except Exception:
        log.flush()
        print((home / "original-terminal.log").read_text(), file=sys.stderr)
        with sqlite3.connect(database) as db:
            db.row_factory = sqlite3.Row
            for query, parameters in [
                ("SELECT * FROM agent_sessions WHERE id=?", (identity,)),
                ("SELECT * FROM processes WHERE kind='agent'", ()),
            ]:
                print(
                    json.dumps([dict(row) for row in db.execute(query, parameters)], default=str),
                    file=sys.stderr,
                )
        transport_log = lf_home / "logs/transport.log"
        if transport_log.exists():
            print(transport_log.read_text(), file=sys.stderr)
        raise
    finally:
        if live is not None and live.poll() is None:
            (repo / "release").touch()
            live.wait(timeout=15)
        for row in launches():
            try:
                os.kill(row["pid"], signal.SIGTERM)
            except ProcessLookupError:
                pass
        if original.poll() is None:
            original.terminate()
        original.wait(timeout=15)
        log.close()


if __name__ == "__main__":
    main()
