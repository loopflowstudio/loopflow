#!/usr/bin/env python3
"""Run credential-free CLI fixtures; PTY evidence is not Ghostty rendering proof."""

from __future__ import annotations

import argparse
import base64
import errno
import fcntl
import hashlib
import json
import os
import pty
import re
import select
import shlex
import shutil
import signal
import socket
import sqlite3
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path

LIMIT = 512 * 1024


def _write(path: Path, text: str, executable: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    if executable:
        path.chmod(0o755)


def _json(path: Path, value: object) -> None:
    _write(path, json.dumps(value, indent=2, ensure_ascii=True) + "\n")


def _sha(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def _tty() -> None:
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def _process_table() -> dict[int, tuple[int, str]]:
    result = subprocess.run(
        ["/bin/ps", "-axo", "pid=,ppid=,lstart="],
        capture_output=True,
        text=True,
        timeout=5,
        check=True,
    )
    rows = {}
    for line in result.stdout.splitlines():
        parts = line.split(maxsplit=2)
        if len(parts) == 3:
            rows[int(parts[0])] = (int(parts[1]), parts[2])
    return rows


def _track_children(owned: dict[int, str], leader: int) -> None:
    rows = _process_table()
    if leader in rows and leader not in owned:
        owned[leader] = rows[leader][1]
    changed = True
    while changed:
        changed = False
        for pid, (parent, birth) in rows.items():
            if (
                pid not in owned
                and parent in owned
                and parent in rows
                and rows[parent][1] == owned[parent]
            ):
                owned[pid] = birth
                changed = True


def _clean_children(owned: dict[int, str]) -> list[int]:
    rows = _process_table()
    for pid, birth in owned.items():
        if pid in rows and rows[pid][1] == birth:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        rows = _process_table()
        remaining = [pid for pid, birth in owned.items() if pid in rows and rows[pid][1] == birth]
        if not remaining:
            return []
        time.sleep(0.05)
    return remaining


class _Trial:
    def __init__(self, root: Path, lf: Path, output: Path) -> None:
        self.root = root
        self.output = output
        self.repo = root / "repo"
        self.home = root / "home"
        self.bin = root / "bin"
        for path in (self.repo, self.home, self.bin, root / "tmp"):
            path.mkdir()
        shutil.copy2(lf, self.bin / "lf")
        self.python = Path(sys.executable).resolve()
        self.git = Path("/opt/homebrew/bin/git").resolve()
        if not self.git.exists():
            raise RuntimeError("trial requires a standalone Git binary, not the Xcode launcher")
        self.env = {
            "HOME": str(self.home),
            "LF_HOME": str(self.home / "lf"),
            "LF_BIN": str(self.bin / "lf"),
            "PATH": str(self.bin),
            "TMPDIR": str(root / "tmp"),
            "SHELL": str(self.bin / "shell"),
            "XDG_CONFIG_HOME": str(self.home / "config"),
            "XDG_STATE_HOME": str(self.home / "state"),
            "XDG_CACHE_HOME": str(self.home / "cache"),
            "XDG_DATA_HOME": str(self.home / "data"),
            "CLAUDE_CONFIG_DIR": str(self.home / "claude"),
            "CODEX_HOME": str(self.home / "codex"),
            "TERM": "xterm-256color",
            "LANG": "en_US.UTF-8",
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_TERMINAL_PROMPT": "0",
            "GIT_AUTHOR_NAME": "Trial fixture",
            "GIT_COMMITTER_NAME": "Trial fixture",
            "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
            "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
        }
        (self.bin / "git").symlink_to(self.git)
        (self.bin / "sh").symlink_to("/bin/sh")
        _write(self.bin / "ps", f"#!{self.python}\n" + PROCESS_AGE, True)
        _write(self.bin / "shell", '#!/bin/sh\nexec /bin/zsh -df "$@"\n', True)
        _write(self.bin / "claude", f"#!{self.python}\n" + PROVIDER, True)
        for name in ("gh", "codex", "opencode", "security", "doppler", "ssh", "curl", "open"):
            _write(
                self.bin / name,
                f"#!/bin/sh\necho 'TRIAL_BLOCKED_HELPER:{name}' >&2\nexit 77\n",
                True,
            )
        self.profile = root / "runtime.sb"
        # No user directory is readable except the private fixture and Python runtime.
        reads = [
            root,
            Path(sys.base_prefix),
            self.git.parent.parent,
            Path("/System"),
            Path("/usr/lib"),
            Path("/usr/share"),
            Path("/dev"),
            Path("/opt/homebrew/Cellar"),
        ]
        executables = [self.bin, self.git.parent.parent / "libexec/git-core"]
        self.profile.write_text(
            "(version 1)\n(allow default)\n"
            "(deny file-read*)\n(deny file-write*)\n(deny process-exec)\n"
            "(deny network*)\n"
            '(deny mach-lookup (global-name "com.apple.securityd") '
            '(global-name "com.apple.securityd.xpc") (global-name "com.apple.secd"))\n'
            '(allow file-read-metadata)\n(allow file-read* (literal "/"))\n'
            + "".join(f"(allow file-read* (subpath {json.dumps(str(p))}))\n" for p in reads)
            + '(allow file-read* (literal "/bin/sh") '
            '(literal "/bin/bash") (literal "/bin/zsh") '
            '(literal "/private/etc/localtime") (literal "/private/etc/zshenv"))\n'
            + f'(allow file-write* (subpath {json.dumps(str(root))}) (subpath "/dev"))\n'
            + "".join(f"(allow process-exec (subpath {json.dumps(str(p))}))\n" for p in executables)
            + f"(allow process-exec (literal {json.dumps(str(self.python))}) "
            f'(literal {json.dumps(str(self.git))}) (literal "/bin/sh") '
            '(literal "/bin/bash") (literal "/bin/zsh"))\n'
            + f"(allow network* (local unix-socket (subpath {json.dumps(str(root))})))\n"
            + f"(allow network* (remote unix-socket (subpath {json.dumps(str(root))})))\n"
        )
        self.receipts: list[dict] = []

    def _command(self, argv: list[str]) -> list[str]:
        return ["/usr/bin/sandbox-exec", "-f", str(self.profile), *argv]

    def _record(
        self, name: str, argv: list[str], cwd: Path | None, reply: bool, **observations: object
    ) -> dict:
        receipt = {
            "id": name,
            "evidence": "fixture",
            "observer": "terminal_host_trial.py",
            "argv": argv,
            "cwd": str(cwd or self.repo),
            "launch_mode": "headless" if "-b" in argv else "interactive" if reply else "command",
            "task_binding": "TRIAL-1 (seeded)"
            if name.startswith("task-") or name == "checkout-publish"
            else None,
            "provider": "local synthetic Claude protocol substitute",
            "shims": False,
            "attention_list": None,
            "notifications": None,
            "colors_rendered": None,
            **observations,
        }
        self.receipts.append(receipt)
        _json(self.output / f"{name}.json", receipt)
        return receipt

    def _run(
        self,
        name: str,
        argv: list[str],
        *,
        cwd: Path | None = None,
        reply: bool = False,
        timeout: float = 30,
    ) -> dict:
        start = time.monotonic()
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 28, 100, 0, 0))
        process = subprocess.Popen(
            self._command(argv),
            cwd=cwd or self.repo,
            env=self.env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            preexec_fn=_tty,
        )
        os.close(slave)
        data = bytearray()
        sent = False
        first = None
        problem = None
        owned: dict[int, str] = {}
        try:
            while time.monotonic() - start < timeout:
                _track_children(owned, process.pid)
                if select.select([master], [], [], 0.05)[0]:
                    try:
                        chunk = os.read(master, 8192)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            break
                        raise
                    if not chunk:
                        break
                    data.extend(chunk)
                    if first is None and b"SYNTHETIC_RESULT:" in data:
                        first = round(time.monotonic() - start, 3)
                    if reply and not sent and b"SYNTHETIC_INPUT>" in data:
                        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 120, 0, 0))
                        os.write(master, b"fixture-return\n")
                        sent = True
                    if len(data) > LIMIT:
                        problem = "capture-limit"
                        del data[LIMIT:]
                        break
                if process.poll() is not None and not select.select([master], [], [], 0)[0]:
                    break
            else:
                problem = "timeout"
        finally:
            if process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                except PermissionError:
                    process.kill()
            process.wait(timeout=5)
            os.close(master)
            survivors = _clean_children(owned)
        try:
            os.killpg(process.pid, 0)
            cleaned = False
        except ProcessLookupError:
            cleaned = True
        except PermissionError:
            cleaned = False
        raw = bytes(data)
        return self._record(
            name,
            argv,
            cwd,
            reply,
            host="pty",
            pane_id=None,
            returncode=process.returncode,
            problem=problem,
            elapsed_seconds=round(time.monotonic() - start, 3),
            first_useful_fixture_seconds=first,
            input_return_sent=sent,
            process_group=process.pid,
            process_group_cleaned=cleaned and not survivors,
            owned_processes=sorted(owned),
            surviving_owned_processes=survivors,
            host_recognition=None,
            host_state=None,
            title=None,
            resize="PTY 100x28 to 120x36" if sent else None,
            scrollback=None,
            output_bytes=len(raw),
            pty_base64=base64.b64encode(raw).decode(),
            text=raw.decode("utf-8", errors="replace"),
        )

    def _api(self, method: str, params: dict | None = None) -> dict:
        with socket.socket(socket.AF_UNIX) as client:
            client.settimeout(5)
            client.connect(str(self.root / "herdr.sock"))
            client.sendall(
                (
                    json.dumps({"id": "trial", "method": method, "params": params or {}}) + "\n"
                ).encode()
            )
            data = bytearray()
            while b"\n" not in data:
                chunk = client.recv(8192)
                if not chunk:
                    raise RuntimeError(f"herdr closed {method} without a response")
                data.extend(chunk)
                if len(data) > LIMIT:
                    raise RuntimeError(f"herdr {method} exceeded capture limit")
        response = json.loads(data.split(b"\n", 1)[0])
        if "error" in response:
            raise RuntimeError(f"herdr {method}: {response['error']}")
        return response["result"]

    def _start_herdr(self, binary: Path) -> None:
        shutil.copy2(binary, self.bin / "herdr")
        self.env.update(
            HERDR_SOCKET_PATH=str(self.root / "herdr.sock"),
            HERDR_SESSION=self.root.parent.name,
            HERDR_CONFIG_PATH=str(self.home / "herdr.toml"),
        )
        version = self._run("herdr-version", [str(self.bin / "herdr"), "--version"])
        if version["returncode"]:
            raise RuntimeError("herdr version command failed")
        self.host_processes = {}
        self.host_bytes = bytearray()
        self.server = subprocess.Popen(
            self._command([str(self.bin / "herdr"), "server"]),
            env=self.env,
            cwd=self.repo,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        self.client = None
        self.host_master = None
        deadline = time.monotonic() + 30
        while not (self.root / "herdr.sock").exists():
            self._poll_host()
            if self.server.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("herdr server failed to expose its owned socket")
            time.sleep(0.05)
        self._api("ping")
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 28, 100, 0, 0))
        self.client = subprocess.Popen(
            self._command([str(self.bin / "herdr")]),
            env=self.env,
            cwd=self.repo,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            preexec_fn=_tty,
        )
        os.close(slave)
        self.host_master = master
        self._poll_host()
        _json(self.output / "herdr-start.json", self._api("session.snapshot"))

    def _poll_host(self) -> None:
        _track_children(self.host_processes, self.server.pid)
        if self.client is not None:
            _track_children(self.host_processes, self.client.pid)
        descriptors = [self.server.stdout.fileno()]
        if self.host_master is not None:
            descriptors.append(self.host_master)
        for descriptor in select.select(descriptors, [], [], 0)[0]:
            try:
                self.host_bytes.extend(os.read(descriptor, 8192))
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
        if len(self.host_bytes) > LIMIT:
            del self.host_bytes[LIMIT:]
            raise RuntimeError("herdr client/server capture limit")

    def _stop_herdr(self) -> dict:
        try:
            self._poll_host()
            self._api("server.stop")
        except (OSError, RuntimeError):
            pass
        # Release the client terminal before waiting for terminal teardown.
        if self.host_master is not None:
            os.close(self.host_master)
            self.host_master = None
        errors = []
        for process in (self.client, self.server):
            if process is not None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                if process.poll() is None:
                    process.kill()
                try:
                    process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    errors.append(f"process {process.pid} did not exit in 30 seconds")
        survivors = _clean_children(self.host_processes)
        self.server.stdout.close()
        receipt = {
            "owned_processes": sorted(self.host_processes),
            "surviving_owned_processes": survivors,
            "errors": errors,
            "output_base64": base64.b64encode(self.host_bytes).decode(),
        }
        _json(self.output / "herdr-cleanup.json", receipt)
        return receipt

    def _scenario(
        self, host: str, name: str, args: list[str], *, cwd: Path | None = None, reply: bool = False
    ) -> dict:
        argv = [str(self.bin / "lf"), *args]
        run = self._run_in_herdr if host == "herdr" else self._run
        try:
            receipt = run(name, argv, cwd=cwd, reply=reply)
        except (OSError, RuntimeError, KeyError, ValueError, subprocess.SubprocessError) as error:
            self._record(
                name,
                argv,
                cwd,
                reply,
                host=host,
                outcome="failure",
                returncode=None,
                problem=str(error),
                text="",
                process_group_cleaned=None,
                cleanup_receipt="herdr-cleanup.json" if host == "herdr" else None,
            )
            raise
        receipt["outcome"] = _outcome(receipt)
        _json(self.output / f"{name}.json", receipt)
        return receipt

    def _run_in_herdr(
        self, name: str, argv: list[str], *, cwd: Path | None = None, reply: bool = False
    ) -> dict:
        start = time.monotonic()
        workspace = self._api("workspace.create", {"cwd": str(cwd or self.repo)})
        _json(self.output / f"{name}-workspace.json", workspace)
        # Resolve the created workspace's actual pane through public reads.
        workspace_id = workspace["workspace"]["workspace_id"]
        pane_id = workspace["root_pane"]["pane_id"]
        self._api("workspace.focus", {"workspace_id": workspace_id})
        stages = [{"stage": "before", "snapshot": self._api("session.snapshot")}]
        _json(self.output / f"{name}-host-states.json", stages)
        command = f"cd {shlex.quote(str(cwd or self.repo))} && {shlex.join(argv)}"
        command += f"; printf '\\nTRIAL_EXIT:{name}:%d\\n' $?"
        self._api("pane.send_text", {"pane_id": pane_id, "text": command + "\n"})
        text = ""
        sent = False
        first = None
        problem = None
        code = None
        while time.monotonic() - start < 120:
            self._poll_host()
            reading = self._api(
                "pane.read", {"pane_id": pane_id, "source": "recent_unwrapped", "lines": 2000}
            )
            _json(self.output / f"{name}-pane-read.json", reading)
            text = reading["read"]["text"]
            if reading["read"]["truncated"]:
                problem = "host-truncated"
                break
            if len(text.encode()) > LIMIT:
                text = text[:LIMIT]
                problem = "capture-limit"
                break
            if len(stages) == 1:
                stages.append({"stage": "active", "snapshot": self._api("session.snapshot")})
            if first is None and "SYNTHETIC_RESULT:" in text:
                first = round(time.monotonic() - start, 3)
            if (
                "SYNTHETIC_RESULT:trial-one" in text
                and "SYNTHETIC_RESULT:trial-two" not in text
                and "-b" in argv
                and not any(s["stage"] == "between-steps" for s in stages)
            ):
                stages.append({"stage": "between-steps", "snapshot": self._api("session.snapshot")})
            if "SYNTHETIC_RETURN:fixture-return" in text and not any(
                s["stage"] == "after-return" for s in stages
            ):
                stages.append({"stage": "after-return", "snapshot": self._api("session.snapshot")})
            if reply and not sent and "SYNTHETIC_INPUT>" in text:
                stages.append(
                    {"stage": "awaiting-input", "snapshot": self._api("session.snapshot")}
                )
                time.sleep(0.5)
                fcntl.ioctl(
                    self.host_master, termios.TIOCSWINSZ, struct.pack("HHHH", 36, 120, 0, 0)
                )
                self._api("pane.send_text", {"pane_id": pane_id, "text": "fixture-return\n"})
                sent = True
            match = re.search(rf"TRIAL_EXIT:{re.escape(name)}:(\d+)", text)
            if match:
                code = int(match.group(1))
                break
            time.sleep(0.1)
        else:
            problem = "timeout"
        stages.append(
            {
                "stage": "exited" if code is not None else "stopped",
                "snapshot": self._api("session.snapshot"),
            }
        )
        _json(self.output / f"{name}-host-states.json", stages)
        final = stages[-1]["snapshot"]["snapshot"]
        pane = next(p for p in final["panes"] if p["pane_id"] == pane_id)
        shown_workspace = next(w for w in final["workspaces"] if w["workspace_id"] == workspace_id)
        receipt = self._record(
            name,
            argv,
            cwd,
            reply,
            host="herdr",
            pane_id=pane_id,
            workspace_id=workspace_id,
            returncode=code,
            problem=problem,
            elapsed_seconds=round(time.monotonic() - start, 3),
            first_useful_fixture_seconds=first,
            input_return_sent=sent,
            process_group_cleaned=None,
            cleanup_receipt="herdr-cleanup.json",
            host_recognition=[a for a in final["agents"] if a.get("pane_id") == pane_id],
            host_state=pane["agent_status"],
            title=shown_workspace["label"],
            terminal_title=pane.get("terminal_title"),
            resize="client PTY 100x28 to 120x36" if sent else None,
            scrollback="pane.read recent-unwrapped",
            output_bytes=len(text.encode()),
            text=text,
            host_states_receipt=f"{name}-host-states.json",
        )
        self._api("workspace.close", {"workspace_id": workspace_id})
        return receipt

    def _lf(self, name: str, args: list[str]) -> dict:
        return self._run(name, [str(self.bin / "lf"), *args])

    def _confine(self, sentinel: Path) -> bool:
        probe = self.root / "probe.py"
        _write(probe, PROBE)
        with (
            socket.socket() as listener,
            socket.socket(socket.AF_UNIX) as inside,
            socket.socket(socket.AF_UNIX) as outside,
        ):
            inside_path = self.root / "probe.sock"
            outside_path = sentinel.parent / "outside.sock"
            inside.bind(str(inside_path))
            outside.bind(str(outside_path))
            inside.listen()
            outside.listen()
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            receipt = self._run(
                "confinement",
                [
                    str(self.python),
                    str(probe),
                    str(sentinel),
                    str(listener.getsockname()[1]),
                    str(inside_path),
                    str(outside_path),
                ],
            )
        return (
            receipt["returncode"] == 0
            and "CONFINED" in receipt["text"]
            and sentinel.read_text() == "synthetic; never a credential"
        )

    def _prepare(self) -> None:
        for args in (["init", "-b", "main"], ["commit", "--allow-empty", "-m", "Fixture"]):
            receipt = self._run("prepare-git-" + args[0], [str(self.git), *args])
            if receipt["returncode"]:
                raise RuntimeError("fixture git preparation failed")
        self._run("prepare-origin", [str(self.git), "remote", "add", "origin", str(self.repo)])
        _write(self.repo / ".lf/config.yaml", "agent: claude\n")
        for name in ("trial-one", "trial-two"):
            _write(self.repo / f".lf/skills/{name}.md", f"Return synthetic marker {name}.\n")
        _write(self.repo / ".lf/flows/trial.yaml", "- trial-one\n- trial-two\n")
        receipt = self._lf("prepare-commit", ["commit", "-m", "Add synthetic trial skills"])
        if receipt["returncode"]:
            raise RuntimeError("fixture lf commit failed")

    def _register_task(self) -> Path:
        _write(
            self.repo / ".lf/config.yaml",
            "agent: claude\npm:\n  provider: linear\n  linear_team: trial-team\n",
        )
        _write(
            self.repo / "wave/trial/GOAL.md",
            "---\npm:\n  linear_initiative: trial-initiative\n---\nSynthetic trial.\n",
        )
        planning = self._lf("prepare-planning", ["commit", "-m", "Bind synthetic planning"])
        if planning["returncode"]:
            raise RuntimeError("fixture planning checkpoint failed")
        created = self._lf("prepare-worktree", ["wt", "create", "trial-task"])
        if created["returncode"]:
            raise RuntimeError("fixture worktree preparation failed")
        checkout = self.root / "repo.trial-task"
        branch = self._run(
            "prepare-branch", [str(self.git), "branch", "--show-current"], cwd=checkout
        )["text"].strip()
        head = self._run("prepare-head", [str(self.git), "rev-parse", "HEAD"], cwd=checkout)[
            "text"
        ].strip()
        now = int(time.time())
        project = dict(
            id="trial-project",
            revision="2026-10-07T00:00:00Z",
            slug="trial",
            name="Synthetic trial",
            summary="",
            metric_targets=[],
            workflow="",
            status="started",
            krs=[],
            initiative_ids=["trial-initiative"],
            team_ids=["trial-team"],
        )
        item = dict(
            id="trial-issue",
            revision="2026-10-07T00:00:00Z",
            identifier="TRIAL-1",
            branch_name=branch,
            url=None,
            name="Synthetic terminal trial",
            description="Fixture only",
            rank=1,
            completed=False,
            completed_at=None,
            state="started",
            project_id="trial-project",
            project="trial",
            team_id="trial-team",
            assignee=None,
        )
        # Only the freshly initialized private database is seeded. This is registered
        # placement, not a demonstration of first-time Task adoption without Linear.
        with sqlite3.connect(self.home / "lf/loopflow.db") as connection:
            connection.execute("PRAGMA foreign_keys=ON")
            connection.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?,?,?,?)",
                ("00000000-0000-4000-8000-000000000421", "trial", str(self.repo), now),
            )
            connection.execute(
                "INSERT INTO projects(id,wave_id,external_project_id,created_at,"
                "project_slug,project_name,project_prompt_context,"
                "pm_snapshot_synced_at,updated_at) "
                "VALUES(?,?,?,?,?,?,?,?,?)",
                (
                    "proj_00000000000040008000000000000421",
                    "00000000-0000-4000-8000-000000000421",
                    "trial-project",
                    now,
                    "trial",
                    "Synthetic trial",
                    "Synthetic planning only",
                    now,
                    now,
                ),
            )
            connection.execute(
                "UPDATE waves SET current_project_id='proj_00000000000040008000000000000421' "
                "WHERE id='00000000-0000-4000-8000-000000000421'"
            )
            connection.execute(
                "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,"
                "created_at,updated_at,issue_title,issue_description,pm_snapshot_synced_at,"
                "pm_writeback_json,worktree,workspace_slug,started_at) "
                "VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)",
                (
                    "task_00000000000040008000000000000421",
                    "proj_00000000000040008000000000000421",
                    "trial-issue",
                    "TRIAL-1",
                    now,
                    now,
                    "Synthetic terminal trial",
                    "Fixture only",
                    now,
                    '{"state":"current"}',
                    str(checkout),
                    "trial-task",
                    None,
                ),
            )
            connection.execute(
                "INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,"
                "created_at,updated_at) "
                "VALUES(?,?,?,?,?,?,?,?)",
                (
                    "tpr_00000000000040008000000000000421",
                    "task_00000000000040008000000000000421",
                    1,
                    "trial-task",
                    branch,
                    head,
                    now,
                    now,
                ),
            )
            connection.execute(
                "INSERT INTO pm_projects(repo,provider,id,observed_at,body) VALUES(?,?,?,?,?)",
                (str(self.repo), "linear", "trial-project", now, json.dumps(project)),
            )
            connection.execute(
                "INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) "
                "VALUES(?,?,?,?,?,?,?)",
                (
                    str(self.repo),
                    "linear",
                    "trial-issue",
                    "TRIAL-1",
                    "trial-project",
                    now,
                    json.dumps(item),
                ),
            )
        return checkout

    def _inspect(self, prefix: str) -> None:
        for suffix, args in (
            (
                "sessions",
                ["session", "list", "--interactive", "all", "--history", "--all", "--json"],
            ),
            ("flows", ["flow", "list", "--sessions", "--all", "--json"]),
            ("processes", ["ps", "--json"]),
        ):
            receipt = self._lf(f"{prefix}-{suffix}", args)
            if suffix != "processes" and receipt["returncode"]:
                raise RuntimeError(f"{prefix}: {suffix} evidence read failed")
            if suffix == "flows" and prefix in ("flow", "task-flow"):
                for entry in json.loads(receipt["text"])["entries"]:
                    self._lf(
                        f"{prefix}-flow-{entry['id']}",
                        ["flow", "show", entry["id"], "--sessions", "--json"],
                    )


# macOS refuses setuid /bin/ps under sandbox-exec. Read actual process birth
# through libproc for lf's age query; unsupported inventory is explicitly absent.
PROCESS_AGE = """import ctypes
import struct
import sys
import time

args = sys.argv[1:]
if len(args) != 4 or args[0] != "-p" or args[2:] != ["-o", "etime="]:
    print("TRIAL: process inventory unavailable in confinement", file=sys.stderr)
    raise SystemExit(77)
buffer = ctypes.create_string_buffer(136)
lib = ctypes.CDLL("/usr/lib/libproc.dylib")
size = lib.proc_pidinfo(int(args[1]), 3, 0, buffer, len(buffer))
if size != len(buffer):
    raise SystemExit(1)
started = struct.unpack_from("Q", buffer.raw, 120)[0]
seconds = max(0, int(time.time()) - started)
print(f"{seconds // 3600:02}:{seconds // 60 % 60:02}:{seconds % 60:02}")
"""


PROBE = """import pathlib
import socket
import subprocess
import sys

sentinel = pathlib.Path(sys.argv[1])
try:
    sentinel.read_bytes()
except PermissionError:
    print("outside_read_denied", flush=True)
else:
    raise SystemExit("outside_read_allowed")
try:
    sentinel.write_text("changed")
except PermissionError:
    print("outside_write_denied", flush=True)
else:
    raise SystemExit("outside_write_allowed")
try:
    subprocess.run(["/usr/bin/true"], check=True)
except PermissionError:
    print("unlisted_executable_denied", flush=True)
else:
    raise SystemExit("unlisted_executable_allowed")
with socket.socket() as client:
    client.settimeout(1)
    try:
        client.connect(("127.0.0.1", int(sys.argv[2])))
    except PermissionError:
        print("loopback_denied", flush=True)
    else:
        raise SystemExit("loopback_not_denied")
with socket.socket(socket.AF_UNIX) as client:
    client.settimeout(1)
    client.connect(sys.argv[3])
    print("owned_socket_allowed", flush=True)
with socket.socket(socket.AF_UNIX) as client:
    client.settimeout(1)
    try:
        client.connect(sys.argv[4])
    except PermissionError:
        print("outside_socket_denied", flush=True)
    else:
        raise SystemExit("outside_socket_allowed")
print("CONFINED", flush=True)
"""

PROVIDER = """import json
import os
import sys
import time
from pathlib import Path

args = sys.argv[1:]
if "--version" in args:
    print("synthetic-trial-provider 1")
    raise SystemExit(0)
stream = "--output-format" in args
if stream and "--input-format" in args:
    while True:
        line = sys.stdin.readline()
        if not line:
            raise SystemExit(2)
        message = json.loads(line)
        if message.get("type") == "user":
            prompt = " ".join(args) + line
            break
        if message.get("type") == "control_request":
            print(
                json.dumps(
                    {
                        "type": "control_response",
                        "response": {
                            "subtype": "success",
                            "request_id": message["request_id"],
                            "response": {},
                        },
                    }
                ),
                flush=True,
            )
elif stream:
    prompt = " ".join(args) + sys.stdin.read()
else:
    prompt = " ".join(args)
if "--append-system-prompt-file" in args:
    prompt += Path(args[args.index("--append-system-prompt-file") + 1]).read_text()
marker = "trial-two" if "Return synthetic marker trial-two." in prompt else "trial-one"
text = "SYNTHETIC_RESULT:" + marker
time.sleep(0.35)
if stream:
    print(
        json.dumps(
            {"type": "system", "subtype": "init", "session_id": "trial-" + str(os.getpid())}
        ),
        flush=True,
    )
    print(
        json.dumps({"type": "assistant", "message": {"content": [{"type": "text", "text": text}]}}),
        flush=True,
    )
    print(
        json.dumps(
            {
                "type": "result",
                "subtype": "success",
                "is_error": False,
                "result": text,
                "session_id": "trial-" + str(os.getpid()),
            }
        ),
        flush=True,
    )
else:
    print(text, flush=True)
    print("SYNTHETIC_INPUT>", flush=True)
    answer = input()
    print("SYNTHETIC_RETURN:" + answer, flush=True)
    time.sleep(0.25)
"""


def _ordered_steps(text: str) -> bool:
    first = text.find("SYNTHETIC_RESULT:trial-one")
    second = text.find("SYNTHETIC_RESULT:trial-two")
    return 0 <= first < second


def _outcome(receipt: dict) -> str:
    name, text = receipt["id"], receipt["text"]
    if receipt["problem"] is not None or receipt["returncode"] is None:
        return "failure"
    boundaries = {
        "fresh-checkout": "pm.linear_team",
        "publish": "default branch cannot open a PR",
        "checkout-publish": "gh CLI not found",
        "task-flow": "a connected managed account is required",
    }
    if receipt["returncode"] != 0:
        return "blocked" if name in boundaries and boundaries[name] in text else "failure"
    if name in ("skill", "conversation") and "SYNTHETIC_RETURN:fixture-return" not in text:
        return "failure"
    if name in ("flow", "task-flow") and not _ordered_steps(text):
        return "failure"
    return "success"


def _exercise(args: argparse.Namespace, host: str, output: Path) -> dict:
    output.mkdir()
    started = time.monotonic()
    summary = {
        "host": host,
        "outcome": "blocked",
        "errors": [],
        "attempts": [],
        "lf_source": str(args.lf.resolve()),
        "lf_sha256": None,
        "lf_revision": None,
        "install_to_useful_seconds": None,
        "host_build": dict(zip(("system", "node", "release", "version", "machine"), os.uname())),
    }
    trial = None
    try:
        summary["lf_sha256"] = _sha(args.lf)
        if sys.platform != "darwin" or not Path("/usr/bin/sandbox-exec").exists():
            raise RuntimeError("runtime confinement currently requires macOS sandbox-exec")
        with tempfile.TemporaryDirectory(prefix="lf-host-trial-", dir="/tmp") as temporary:
            parent = Path(temporary).resolve()
            root = parent / "allowed"
            root.mkdir()
            sentinel = parent / "outside-sentinel"
            sentinel.write_text("synthetic; never a credential")
            trial = _Trial(root, args.lf.resolve(), output)
            summary["environment_keys"] = sorted(trial.env)
            summary["process_observer"] = "libproc age query; full lf ps inventory unavailable"
            summary["executables"] = {
                "lf": str(trial.bin / "lf"),
                "claude": str(trial.bin / "claude"),
                "provider_sha256": _sha(trial.bin / "claude"),
                "git": str(trial.git),
                "gh": "blocked substitute",
            }
            _write(output / "runtime.sb", trial.profile.read_text())
            summary["confinement"] = trial._confine(sentinel)
            if not summary["confinement"]:
                raise RuntimeError("confinement not established; no host or lf trial launched")
            summary["lf_version"] = trial._lf("lf-version", ["--version"])["text"].strip()
            summary["lf_binary_ready_seconds"] = round(time.monotonic() - started, 3)
            trial._prepare()
            if args.self_test and host == "pty":
                overflow = trial._run(
                    "self-test-overflow", [str(trial.python), "-c", "print('x' * 600000)"]
                )
                timed = trial._run(
                    "self-test-timeout",
                    [
                        str(trial.python),
                        "-c",
                        "import subprocess,sys,time; "
                        "subprocess.Popen([sys.executable,'-c','import time; time.sleep(5)'], "
                        "start_new_session=True); time.sleep(5)",
                    ],
                    timeout=0.3,
                )
                if overflow["problem"] != "capture-limit" or timed["problem"] != "timeout":
                    raise RuntimeError("capture bound or timeout did not fail as expected")
                if not overflow["process_group_cleaned"] or not timed["process_group_cleaned"]:
                    raise RuntimeError("owned process survived a forced stop")
            try:
                if host == "herdr":
                    if args.herdr is None or not args.herdr.is_file():
                        raise RuntimeError("--herdr must name the separately prepared binary")
                    summary["herdr_sha256"] = _sha(args.herdr)
                    summary["herdr_source"] = str(args.herdr.resolve())
                    trial._start_herdr(args.herdr)
                    summary["herdr_binary_ready_seconds"] = round(time.monotonic() - started, 3)
                for name, argv, reply in (
                    ("skill", ["-m", "claude", "trial-one"], True),
                    ("conversation", ["-m", "claude"], True),
                    ("flow", ["-b", "-m", "claude", "run", "trial"], False),
                    ("fresh-checkout", ["task", "checkout", "TRIAL-1"], False),
                    ("publish", ["pr", "publish", "--title", "Synthetic trial"], False),
                ):
                    if name == "skill":
                        summary["first_command_seconds"] = round(time.monotonic() - started, 3)
                    receipt = trial._scenario(host, name, argv, reply=reply)
                    if name == "skill" and receipt["first_useful_fixture_seconds"] is not None:
                        summary["first_useful_fixture_seconds"] = round(
                            summary["first_command_seconds"]
                            + receipt["first_useful_fixture_seconds"],
                            3,
                        )
                    if name == "fresh-checkout":
                        summary["planning_stall_observed_seconds"] = round(
                            time.monotonic() - started, 3
                        )
                    if receipt["outcome"] == "failure":
                        summary["errors"].append(name + ": unexpected CLI result")
                    trial._inspect(name)
                checkout = trial._register_task()
                for name, argv in (
                    ("task-checkout", ["task", "checkout", "TRIAL-1"]),
                    ("task-flow", ["-b", "-m", "claude", "task", "run", "TRIAL-1", "trial"]),
                    ("checkout-publish", ["pr", "publish", "--title", "Synthetic trial"]),
                ):
                    receipt = trial._scenario(host, name, argv, cwd=checkout)
                    if receipt["outcome"] == "failure":
                        summary["errors"].append(name + ": unexpected CLI result")
                    trial._inspect(name)
            finally:
                if hasattr(trial, "server"):
                    cleanup = trial._stop_herdr()
                    for receipt in trial.receipts:
                        if receipt["host"] == "herdr":
                            receipt["process_group_cleaned"] = not (
                                cleanup["surviving_owned_processes"] or cleanup["errors"]
                            )
                            _json(output / f"{receipt['id']}.json", receipt)
                    if cleanup["surviving_owned_processes"] or cleanup["errors"]:
                        summary["errors"].append("herdr cleanup")
            for receipt in trial.receipts:
                if not receipt["id"].startswith("self-test-") and (
                    receipt["problem"] or not receipt["process_group_cleaned"]
                ):
                    summary["errors"].append(receipt["id"] + ": capture/cleanup")
            summary["outcome"] = "failure" if summary["errors"] else "success"
    except (
        OSError,
        RuntimeError,
        ValueError,
        KeyError,
        sqlite3.Error,
        subprocess.SubprocessError,
    ) as error:
        summary["errors"].append(str(error))
    finally:
        if trial is not None:
            summary["attempts"] = [r["id"] for r in trial.receipts]
    _json(output / "summary.json", summary)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", action="append", choices=["pty", "herdr"])
    parser.add_argument("--lf", type=Path, default=Path(shutil.which("lf") or "lf"))
    parser.add_argument("--herdr", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    output = (args.output or Path(tempfile.mkdtemp(prefix="lf-trial-receipts-"))).resolve()
    output.mkdir(parents=True, exist_ok=True)
    if any(output.iterdir()):
        parser.error("choose a new output directory; earlier attempts are retained")
    results = [_exercise(args, host, output / host) for host in dict.fromkeys(args.host or ["pty"])]
    _json(output / "summary.json", results)
    print(
        json.dumps(
            {
                "output": str(output),
                "hosts": [
                    {"host": r["host"], "outcome": r["outcome"], "errors": r["errors"]}
                    for r in results
                ],
            }
        )
    )
    return 0 if all(r["outcome"] == "success" for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
