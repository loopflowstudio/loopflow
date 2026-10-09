"""Probe exact first turns through the native Codex external editor."""

import argparse
import fcntl
import json
import os
import pty
import select
import shlex
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
from context_delivery import Requests
from launch import _codex_config
from request_mapping import _message_texts


def _first_turn_matches(requests: list[dict], prompt: str) -> bool:
    texts = _message_texts(requests[0], "user") if requests else []
    return bool(texts) and texts[-1] == prompt and texts.count(prompt) == 1


@contextmanager
def _terminal(
    command: list[str], root: Path, env: dict[str, str]
) -> Iterator[tuple[subprocess.Popen, int]]:
    with ExitStack() as resources:
        master, slave = pty.openpty()
        resources.callback(os.close, master)
        with ExitStack() as inputs:
            inputs.callback(os.close, slave)
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))

            def _controlling_terminal() -> None:
                os.setsid()
                fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

            process = subprocess.Popen(
                command,
                cwd=root / "work",
                env=env,
                stdin=slave,
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


def _run_editor(
    command: list[str],
    root: Path,
    env: dict[str, str],
    prompt: str,
    server: Requests,
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
                "input_loaded",
                "editor_returned_raw",
                "no_request_before_submit",
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

        def _wait(predicate):
            while not (value := predicate()):
                if process.poll() is not None:
                    raise RuntimeError("terminal exited before the observation")
                if time.monotonic() >= deadline:
                    raise TimeoutError("terminal observation deadline exceeded")
                _pump()
            return value

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
            # The composer echoes before thread startup finishes. External-editor
            # shortcuts are unavailable until the native session footer appears.
            _wait(lambda: _visible("gpt-5.4default"))
            pending.extend(b"zzlfreadyzz")
            _wait(lambda: _visible("zzlfreadyzz"))
            checks["editor_ready"] = True
            # Clear the readiness marker without submitting it.
            pending.extend(b"\x15")
            pending.extend(b"\x07")
            _wait(lambda: (root / "editor.json").exists())
            receipt = json.loads((root / "editor.json").read_text())
            checks["input_loaded"] = receipt["exact_copy"] and receipt["terminal"]
            _wait(lambda: _visible("LOO444_END"))
            attrs = termios.tcgetattr(master)
            checks["editor_returned_raw"] = not attrs[3] & (termios.ECHO | termios.ICANON)
            checks["no_request_before_submit"] = not server.bodies
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
            def _followup() -> list[str] | None:
                for body in server.bodies:
                    texts = _message_texts(body, "user")
                    if texts[-1:] == ["LOO444_FOLLOWUP"]:
                        return texts
                return None

            texts = _wait(_followup)
            checks["followup_preserves_first_turn"] = texts.count(prompt) == 1
            pending.extend(b"/quit")
            _wait(lambda: _visible("/quit"))
            _submit()
            _wait(lambda: not pending)
            while process.poll() is None and time.monotonic() < deadline:
                _pump()
            checks["clean_exit"] = process.poll() == 0
        except (OSError, RuntimeError, TimeoutError) as error:
            (root / "terminal.error").write_text(str(error))
    return process.returncode, bytes(output), checks


def _editor_command(root: Path) -> str:
    script = root / "editor.py"
    script.write_text(
        "import json, os, sys\nfrom pathlib import Path\n"
        "root = Path(__file__).parent\n"
        "target = Path(sys.argv[1])\n"
        "source = (root / 'prompt.txt').read_bytes()\n"
        "target.write_bytes(source)\n"
        "receipt = {'exact_copy': target.read_bytes() == source, "
        "'terminal': all(os.isatty(fd) for fd in (0, 1, 2)), "
        "'max_argument_bytes': max(len(arg.encode()) for arg in sys.argv)}\n"
        "pending = root / 'editor.pending'\n"
        "pending.write_text(json.dumps(receipt))\n"
        "pending.rename(root / 'editor.json')\n"
    )
    return shlex.join([sys.executable, str(script)])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--case",
        choices=["unicode", "carriage-return", "paste-marker", "trailing-whitespace"],
        default="unicode",
    )
    args = parser.parse_args()
    executable = shutil.which("codex")
    if executable is None:
        raise SystemExit("Codex is required")
    executable = str(Path(executable).resolve())
    args.output.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="editor-", dir=args.output)).resolve()
    for name in ["home", "native", "work"]:
        (root / name).mkdir()
    prompt = "LOO444_BEGIN\n" + "all first-turn bytes survive. λ 🐙\n" * 7500 + "LOO444_END"
    if args.case == "carriage-return":
        prompt = prompt.replace("LOO444_BEGIN\n", "LOO444_BEGIN\r\n")
    elif args.case == "paste-marker":
        prompt = prompt.replace("LOO444_END", "literal \x1b[201~ LOO444_END")
    elif args.case == "trailing-whitespace":
        prompt += " \t\r\n"
    (root / "prompt.txt").write_bytes(prompt.encode())
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
        env["VISUAL"] = _editor_command(root)
        command = [executable, "--no-alt-screen", "--no-daemon"]
        status, output, checks = _run_editor(command, root, env, prompt, server)
        checks["complete_first_turn"] = _first_turn_matches(server.bodies, prompt)
        max_argument_bytes = max(len(arg.encode()) for arg in command)
        if (root / "editor.json").exists():
            receipt = json.loads((root / "editor.json").read_text())
            max_argument_bytes = max(max_argument_bytes, receipt["max_argument_bytes"])
        result = {
            "version": version,
            "evidence": str(root),
            "exit": status,
            "case": args.case,
            "model_requests": len(server.bodies),
            "prompt_bytes": len(prompt.encode()),
            "max_argument_bytes": max_argument_bytes,
            "checks": checks,
        }
        (root / "terminal.output").write_bytes(output)
        (root / "requests.json").write_text(json.dumps(server.bodies, indent=2))
        (root / "result.json").write_text(json.dumps(result, indent=2))
        print(json.dumps(result, indent=2))
        return 0 if all(checks.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
