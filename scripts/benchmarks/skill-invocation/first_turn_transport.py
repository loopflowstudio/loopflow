"""Probe exact first turns through native Codex stdin and terminal paste transports."""

import argparse
import fcntl
import json
import os
import pty
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
from collections.abc import Iterator
from contextlib import ExitStack, contextmanager
from pathlib import Path

import pyte
from context_delivery import Requests, _codex_config
from request_mapping import _message_texts


def _assess(requests: list[dict], prompt: str) -> dict[str, bool]:
    texts = _message_texts(requests[0], "user") if requests else []
    return {
        "one_model_request": len(requests) == 1,
        "complete_first_turn": bool(texts) and texts[-1] == prompt and texts.count(prompt) == 1,
    }


@contextmanager
def _terminal(
    command: list[str], root: Path, env: dict[str, str], *, file_stdin: bool = False
) -> Iterator[tuple[subprocess.Popen, int]]:
    with ExitStack() as resources:
        master, slave = pty.openpty()
        resources.callback(os.close, master)
        with ExitStack() as inputs:
            inputs.callback(os.close, slave)
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
            source = inputs.enter_context((root / "prompt.txt").open("rb")) if file_stdin else slave

            def _controlling_terminal() -> None:
                os.setsid()
                # stdout stays a controlling terminal even when stdin is the prompt file.
                fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

            process = subprocess.Popen(
                command,
                cwd=root / "work",
                env=env,
                stdin=source,
                stdout=slave,
                stderr=slave,
                preexec_fn=_controlling_terminal,
            )
        try:
            yield process, master
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)


def _terminal_replies(output: bytearray, start: int) -> bytes:
    # Include the preceding bytes only for queries straddling this read boundary.
    return b"".join(
        reply * output.count(query, max(0, start - len(query) + 1))
        for query, reply in [(b"\x1b[6n", b"\x1b[1;1R"), (b"\x1b[c", b"\x1b[?1;2c")]
    )


def _run(command: list[str], root: Path, env: dict[str, str]) -> tuple[int, bytes, bool]:
    output = bytearray()
    deadline = time.monotonic() + 30
    timed_out = False
    with _terminal(command, root, env, file_stdin=True) as (process, master):
        while True:
            if time.monotonic() >= deadline:
                timed_out = True
                break
            if not select.select([master], [], [], 0.1)[0]:
                if process.poll() is not None:
                    break
                continue
            try:
                data = os.read(master, 65536)
            except OSError:
                break
            if not data:
                break
            start = len(output)
            output.extend(data)
            if replies := _terminal_replies(output, start):
                os.write(master, replies)
        try:
            process.wait(timeout=max(0.1, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            timed_out = True
    return process.returncode, bytes(output), timed_out


def _run_paste(
    command: list[str], root: Path, env: dict[str, str], prompt: str, server: Requests
) -> tuple[int, bytes, dict[str, bool]]:
    with _terminal(command, root, env) as (process, master):
        os.set_blocking(master, False)
        output = bytearray()
        screen = pyte.Screen(160, 40)
        stream = pyte.ByteStream(screen)
        pending = bytearray()
        deadline = time.monotonic() + 45
        checks = {
            key: False
            for key in [
                "editor_ready",
                "paste_drained",
                "resized",
                "followup_preserves_first_turn",
                "clean_exit",
            ]
        }

        def _pump() -> None:
            readable, writable, _ = select.select([master], [master] if pending else [], [], 0.02)
            if readable:
                try:
                    data = os.read(master, 65536)
                except BlockingIOError:
                    data = b""
                start = len(output)
                output.extend(data)
                stream.feed(data)
                pending.extend(_terminal_replies(output, start))
            if writable:
                try:
                    count = os.write(master, pending[:4096])
                    del pending[:count]
                except BlockingIOError:
                    pass

        def _wait(predicate) -> None:
            while not predicate():
                if process.poll() is not None:
                    raise RuntimeError("terminal exited before the observation")
                if time.monotonic() >= deadline:
                    raise TimeoutError("terminal observation deadline exceeded")
                _pump()

        def _submit() -> None:
            # Codex 0.161.0 treats Enter within its 120 ms paste-burst window as
            # a newline. Separate this key from the rendered paste/typing; never
            # retry submission, which could conceal a duplicate first turn.
            ready = time.monotonic() + 0.15
            _wait(lambda: time.monotonic() >= ready)
            pending.extend(b"\r")

        def _visible(text: str) -> bool:
            return text in "".join("".join(screen.display).split())

        try:
            _wait(lambda: _visible("AskCodextodoanything"))
            attrs = termios.tcgetattr(master)
            if attrs[3] & (termios.ECHO | termios.ICANON):
                raise RuntimeError("editor is not in raw mode")
            pending.extend(b"zzlfreadyzz")
            _wait(lambda: _visible("zzlfreadyzz"))
            checks["editor_ready"] = True
            # Remove the unsubmitted readiness marker, then send one bracketed paste.
            pending.extend(b"\x15\x1b[200~" + prompt.encode() + b"\x1b[201~")
            _wait(lambda: not pending)
            checks["paste_drained"] = True
            _wait(lambda: _visible("Pasted") or _visible("LOO444_END"))
            # Resize while the first turn is still editable; the same PTY owns input.
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 120, 0, 0))
            screen.resize(32, 120)
            checks["resized"] = termios.tcgetwinsize(master) == (32, 120)
            _submit()
            _wait(lambda: _visible("fixtureresponse"))
            pending.extend(b"LOO444_FOLLOWUP")
            _wait(lambda: _visible("LOO444_FOLLOWUP"))
            _submit()
            # Codex can make an independent title-generation request between turns.
            _wait(
                lambda: any(
                    _message_texts(body, "user")[-1:] == ["LOO444_FOLLOWUP"]
                    for body in server.bodies
                )
            )
            texts = next(
                _message_texts(body, "user")
                for body in server.bodies
                if _message_texts(body, "user")[-1:] == ["LOO444_FOLLOWUP"]
            )
            checks["followup_preserves_first_turn"] = (
                texts.count(prompt) == 1 and texts[-1] == "LOO444_FOLLOWUP"
            )
            pending.extend(b"/quit")
            _wait(lambda: _visible("/quit"))
            _submit()
            _wait(lambda: not pending)
            while process.poll() is None and time.monotonic() < deadline:
                _pump()
            checks["clean_exit"] = process.poll() == 0
        except (OSError, RuntimeError, TimeoutError) as error:
            (root / "paste.error").write_text(str(error))
    return process.returncode, bytes(output), checks


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--transport", choices=["stdin", "paste"], default="stdin")
    parser.add_argument(
        "--case", choices=["unicode", "carriage-return", "paste-marker"], default="unicode"
    )
    args = parser.parse_args()
    executable = shutil.which("codex")
    if executable is None:
        raise SystemExit("Codex is required")
    executable = str(Path(executable).resolve())
    args.output.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix=f"{args.transport}-", dir=args.output)).resolve()
    for name in ["home", "native", "work"]:
        (root / name).mkdir()
    prompt = "LOO444_BEGIN\n" + "all first-turn bytes survive. λ 🐙\n" * 7500 + "LOO444_END"
    if args.case == "carriage-return":
        prompt = prompt.replace("LOO444_BEGIN\n", "LOO444_BEGIN\r\n")
    elif args.case == "paste-marker":
        prompt = prompt.replace("LOO444_END", "literal \x1b[201~ LOO444_END")
    (root / "prompt.txt").write_text(prompt)
    env = {
        "PATH": os.environ["PATH"],
        "HOME": str(root / "home"),
        "CODEX_HOME": str(root / "native"),
        "TERM": "xterm-256color",
    }
    version = subprocess.run(
        [executable, "--version"],
        cwd=root / "work",
        env=env,
        capture_output=True,
        text=True,
        timeout=10,
        check=True,
    ).stdout.strip()
    with Requests("codex") as server:
        (root / "native/config.toml").write_text(
            _codex_config(server.server_port)
            + f'''
[projects."{root / "work"}"]
trust_level = "trusted"
'''
        )
        if args.transport == "paste":
            command = [executable, "--no-alt-screen", "--no-daemon"]
            status, output, checks = _run_paste(command, root, env, prompt, server)
            checks["complete_first_turn"] = _assess(server.bodies, prompt)["complete_first_turn"]
            result = {
                "version": version,
                "evidence": str(root),
                "exit": status,
                "case": args.case,
                "model_requests": len(server.bodies),
                "prompt_bytes": len(prompt.encode()),
                "max_argument_bytes": max(len(arg.encode()) for arg in command),
                "checks": checks,
            }
            (root / "terminal.output").write_bytes(output)
            (root / "requests.json").write_text(json.dumps(server.bodies, indent=2))
            (root / "result.json").write_text(json.dumps(result, indent=2))
            print(json.dumps(result, indent=2))
            return 0 if all(checks.values()) else 1
        observations = {}
        for surface, options in [
            ("headless", ["exec", "--skip-git-repo-check", "--json", "-"]),
            ("terminal", ["--no-alt-screen", "--no-daemon", "-"]),
        ]:
            command = [executable, *options]
            status, output, timed_out = _run(command, root, env)
            bodies = list(server.bodies)
            (root / f"{surface}.output").write_bytes(output)
            (root / f"{surface}.requests.json").write_text(json.dumps(bodies, indent=2))
            observations[surface] = {
                "exit": status,
                "timed_out": timed_out,
                "max_argument_bytes": max(len(arg.encode()) for arg in command),
                "checks": _assess(bodies, prompt),
            }
            server.bodies.clear()
        result = {
            "version": version,
            "evidence": str(root),
            "prompt_bytes": len(prompt.encode()),
            **observations,
        }
        (root / "result.json").write_text(json.dumps(result, indent=2))
        print(json.dumps(result, indent=2))
        return (
            0
            if all(
                result["exit"] == 0 and not result["timed_out"] and all(result["checks"].values())
                for result in observations.values()
            )
            else 1
        )


if __name__ == "__main__":
    sys.exit(main())
