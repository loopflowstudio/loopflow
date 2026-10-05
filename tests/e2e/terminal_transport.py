"""Compare retained tmux presentation with raw PTY replay using owned processes."""

import argparse
import fcntl
import json
import os
import platform
import pty
import select
import shlex
import struct
import subprocess
import sys
import tempfile
import termios
import time
import tty
from pathlib import Path

COLOR = b"\x1b[38;2;17;93;201m"
CLIPBOARD = b"\x1b]52;c;?\x07"
PASTE = b"\x1b[200~draft survives detach\x1b[201~"


def _fixture(root: Path) -> None:
    tty.setraw(0)
    (root / "pid").write_text(str(os.getpid()))
    os.write(1, b"\x1b[?2004h" + COLOR + b"HISTORY retained\x1b[0m\r\n")
    draft = b""
    while True:
        if (root / "query").exists():
            (root / "query").unlink()
            os.write(1, CLIPBOARD)
        if not select.select([0], [], [], 0.02)[0]:
            continue
        value = os.read(0, 65536)
        with (root / "input").open("ab") as log:
            log.write(value)
        if b"query" in value:
            os.write(1, CLIPBOARD)
        if b"paint" in value:
            os.write(1, COLOR + b"COLOR retained\x1b[0m\r\n")
        if b"draft survives detach" in value:
            draft = b"draft survives detach"
            os.write(1, b"DRAFT retained: draft survives detach\r\n")
        if b"\r" in value:
            (root / "submitted").write_bytes(draft)
            os.write(1, b"ACCEPTED\r\n")


def _terminal() -> None:
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def _spawn(argv: list[str], env: dict[str, str], width: int = 100) -> tuple[subprocess.Popen, int]:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, width, 0, 0))
    try:
        child = subprocess.Popen(
            argv, stdin=slave, stdout=slave, stderr=slave, env=env, preexec_fn=_terminal
        )
    except BaseException:
        os.close(master)
        raise
    finally:
        os.close(slave)
    return child, master


def _read(master: int, seconds: float = 0.4) -> bytes:
    output = bytearray()
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if select.select([master], [], [], min(0.05, max(0, deadline - time.monotonic())))[0]:
            try:
                value = os.read(master, 65536)
            except OSError:
                break
            if not value:
                break
            output.extend(value)
            # Terminal capability/position replies, not provider input.
            if b"\x1b[6n" in value:
                os.write(master, b"\x1b[1;1R")
            if b"\x1b[c" in value:
                os.write(master, b"\x1b[?1;2c")
    return bytes(output)


def _stop(child: subprocess.Popen, master: int) -> None:
    try:
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=3)
    finally:
        os.close(master)


def _wait_file(path: Path) -> None:
    deadline = time.monotonic() + 5
    while not path.exists():
        if time.monotonic() >= deadline:
            raise TimeoutError(str(path))
        time.sleep(0.01)


def _tmux(root: Path, output: Path, env: dict[str, str], result: dict) -> None:
    socket = root / "tmux.sock"
    base = ["tmux", "-S", str(socket), "-f", "/dev/null"]

    def _command(*args: str, check: bool = True) -> subprocess.CompletedProcess:
        completed = subprocess.run(
            [*base, *args], env=env, capture_output=True, check=False, timeout=5
        )
        if check and completed.returncode:
            raise RuntimeError(f"tmux {args}: {completed.stderr.decode(errors='replace')}")
        return completed

    clients: list[tuple[subprocess.Popen, int]] = []

    def _attach(*flags: str, width: int = 100) -> tuple[subprocess.Popen, int]:
        client = _spawn([*base, "attach-session", *flags, "-t", "=probe"], env, width)
        clients.append(client)
        return client

    def _detach() -> None:
        _command("detach-client", "-s", "probe")
        for child, _ in clients:
            child.wait(timeout=3)

    provider_pid = None
    try:
        _command(
            "new-session",
            "-d",
            "-x",
            "100",
            "-y",
            "30",
            "-s",
            "probe",
            shlex.join([sys.executable, str(Path(__file__).resolve()), "--fixture", str(root)]),
        )
        for args in [
            ("set", "-g", "status", "off"),
            ("set", "-g", "set-clipboard", "on"),
            ("set", "-s", "get-clipboard", "request"),
            ("set", "-g", "allow-passthrough", "on"),
            ("set", "-g", "terminal-features", "xterm*:RGB:clipboard"),
            ("set", "-g", "window-size", "manual"),
        ]:
            _command(*args)
        _wait_file(root / "pid")
        provider_pid = int((root / "pid").read_text())
        owner, terminal = _attach()
        initial = _read(terminal, 1)
        os.write(terminal, PASTE)
        draft = _read(terminal)
        (output / "tmux-initial.bin").write_bytes(initial + draft)
        result["initial_history"] = b"HISTORY retained" in initial
        result["bracketed_paste_exact"] = PASTE in (root / "input").read_bytes()
        result["draft_visible"] = b"DRAFT retained" in draft

        os.write(terminal, b"paint")
        painted = _read(terminal)
        (output / "tmux-color.bin").write_bytes(painted)
        result["truecolor_bytes"] = COLOR in painted
        os.write(terminal, b"query")
        query = _read(terminal)
        (output / "tmux-query.bin").write_bytes(query)
        result["clipboard_query_forwarded"] = b"\x1b]52;" in query
        os.write(terminal, b"\x1b]52;;YWN0aXZlLWNsaXBib2FyZA==\x07")
        _read(terminal)
        result["controller_clipboard_reply_reaches_provider"] = (
            b"YWN0aXZlLWNsaXBib2FyZA==" in (root / "input").read_bytes()
        )

        passive, second = _attach("-r", width=60)
        replay = _read(second, 1)
        (output / "tmux-passive.bin").write_bytes(replay)
        before = (root / "input").read_bytes()
        os.write(second, b"PASSIVE MUST NOT WRITE\r")
        _read(second)
        result["passive_input_blocked"] = (root / "input").read_bytes() == before
        result["late_draft_replay"] = b"draft survives detach" in replay
        result["late_clipboard_query_replayed"] = CLIPBOARD in replay
        result["pane_size_with_smaller_view"] = (
            _command("display-message", "-p", "-t", "probe:0.0", "#{pane_width}x#{pane_height}")
            .stdout.decode()
            .strip()
        )

        # A view-only client must not become the source of clipboard replies.
        (root / "query").touch()
        controller_query = _read(terminal)
        passive_query = _read(second)
        (output / "tmux-controller-query.bin").write_bytes(controller_query)
        (output / "tmux-passive-query.bin").write_bytes(passive_query)
        result["passive_receives_clipboard_query"] = b"\x1b]52;" in passive_query
        if result["passive_receives_clipboard_query"]:
            reply = b"\x1b]52;;cGFzc2l2ZS1jbGlwYm9hcmQ=\x07"
            os.write(second, reply)
            _read(second)
            result["passive_clipboard_reply_reaches_provider"] = (
                b"cGFzc2l2ZS1jbGlwYm9hcmQ=" in (root / "input").read_bytes()
            )
        else:
            result["passive_clipboard_reply_reaches_provider"] = None

        client_names = dict(
            line.split(" ", 1)
            for line in _command("list-clients", "-F", "#{client_pid} #{client_name}")
            .stdout.decode()
            .splitlines()
        )
        _command("refresh-client", "-t", client_names[str(owner.pid)], "-f", "read-only")
        _command("switch-client", "-c", client_names[str(passive.pid)], "-r")
        result["clients_after_transfer"] = (
            _command("list-clients", "-F", "#{client_pid} #{client_readonly} #{client_flags}")
            .stdout.decode()
            .splitlines()
        )
        before_transfer = (root / "input").read_bytes()
        os.write(terminal, b"OLD OWNER MUST NOT WRITE")
        _read(terminal)
        result["old_owner_fenced_after_transfer"] = (root / "input").read_bytes() == before_transfer
        os.write(second, b"NEW OWNER INPUT")
        _read(second)
        result["new_owner_can_write"] = b"NEW OWNER INPUT" in (root / "input").read_bytes()
        (root / "query").touch()
        transferred_query = _read(second)
        result["new_owner_receives_clipboard_query"] = b"\x1b]52;" in transferred_query
        os.write(second, b"\x1b]52;;bmV3LW93bmVy\x07")
        _read(second)
        result["new_owner_clipboard_reply_reaches_provider"] = (
            b"bmV3LW93bmVy" in (root / "input").read_bytes()
        )

        # Detach all clients without sending an input byte to the provider.
        _detach()
        os.kill(provider_pid, 0)
        result["detach_keeps_process"] = True
        for index in range(3):
            _, master = _attach()
            late = _read(master, 0.6)
            (output / f"tmux-reattach-{index}.bin").write_bytes(late)
            if b"draft survives detach" not in late:
                raise AssertionError("reattachment lost retained draft display")
            _detach()
        result["repeated_attachments"] = 3
        result["same_process"] = int((root / "pid").read_text()) == provider_pid
        _, master = _attach()
        _read(master)
        os.write(master, b"\r")
        _wait_file(root / "submitted")
        result["submitted_retained_draft"] = (root / "submitted").read_bytes() == (
            b"draft survives detach"
        )
        result["image_input"] = "unmeasured: fixture has no native clipboard/image decoder"
        result["review_fences"] = "unmeasured: transport fixture is not lf session connect"
    finally:
        cleanup_errors = []
        if (root / "input").exists():
            (output / "tmux-provider-input.bin").write_bytes((root / "input").read_bytes())
        try:
            result["cleanup_status"] = _command("kill-server", check=False).returncode
        except Exception as error:
            cleanup_errors.append(str(error))
        for child, master in clients:
            try:
                _stop(child, master)
            except Exception as error:
                cleanup_errors.append(str(error))
        try:
            result["server_absent_after_cleanup"] = (
                _command("list-sessions", check=False).returncode != 0
            )
        except Exception as error:
            cleanup_errors.append(str(error))
        if provider_pid is not None:
            try:
                os.kill(provider_pid, 0)
                result["provider_absent_after_cleanup"] = False
            except ProcessLookupError:
                result["provider_absent_after_cleanup"] = True
        result["cleanup_errors"] = cleanup_errors


def _relay(root: Path, output: Path, env: dict[str, str], result: dict) -> None:
    child, master = _spawn(
        [sys.executable, str(Path(__file__).resolve()), "--fixture", str(root)], env
    )
    try:
        replay = _read(master, 0.5)
        os.write(master, PASTE)
        replay += _read(master)
        os.write(master, b"query")
        query = _read(master)
        replay += query
        (output / "relay-replay.bin").write_bytes(replay)
        result["truecolor_bytes"] = COLOR in replay
        result["bracketed_paste_exact"] = PASTE in (root / "input").read_bytes()
        result["clipboard_query_forwarded"] = CLIPBOARD in query
        result["late_draft_replay"] = b"draft survives detach" in replay
        result["late_clipboard_query_replayed"] = CLIPBOARD in replay
        # A raw replay consumer answers the historical query again. Even when
        # keystrokes are fenced, treating terminal replies as passive writes leaks
        # the new viewer's clipboard into the original provider's input stream.
        reply = b"\x1b]52;c;cGFzc2l2ZS1jbGlwYm9hcmQ=\x07"
        os.write(master, reply)
        _read(master)
        result["replayed_query_response_reaches_pty"] = reply in (root / "input").read_bytes()
        result["replay_bytes"] = len(replay)
        result["image_input"] = "unmeasured: byte relay does not prove native image decoding"
        result["independent_sizes"] = "not implemented: one PTY has one size"
    finally:
        _stop(child, master)
        result["cleanup_status"] = child.returncode


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--fixture", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.fixture:
        _fixture(args.fixture)
        return
    if args.output is None or args.output.exists():
        parser.error("--output must name a new directory")
    args.output.mkdir(parents=True)
    results = {"host": platform.platform(), "endpoint": "PTY bytes; no compositor or provider"}
    try:
        with tempfile.TemporaryDirectory(prefix="lf-terminal-", dir="/tmp") as directory:
            root = Path(directory)
            env = {key: os.environ[key] for key in ("PATH", "LANG") if key in os.environ}
            env.update(HOME=str(root), TERM="xterm-256color", COLORTERM="truecolor")
            results["tmux_version"] = subprocess.check_output(["tmux", "-V"], text=True).strip()
            for name, probe in [("tmux", _tmux), ("relay", _relay)]:
                fixture = root / name
                fixture.mkdir()
                results[name] = {}
                probe(fixture, args.output, env, results[name])
        required = [
            "initial_history",
            "bracketed_paste_exact",
            "draft_visible",
            "truecolor_bytes",
            "clipboard_query_forwarded",
            "controller_clipboard_reply_reaches_provider",
            "passive_input_blocked",
            "late_draft_replay",
            "detach_keeps_process",
            "same_process",
            "submitted_retained_draft",
            "server_absent_after_cleanup",
            "provider_absent_after_cleanup",
            "old_owner_fenced_after_transfer",
            "new_owner_can_write",
            "new_owner_receives_clipboard_query",
            "new_owner_clipboard_reply_reaches_provider",
        ]
        failures = [name for name in required if not results["tmux"].get(name)]
        if failures:
            raise AssertionError(f"tmux transport behavior failed: {failures}")
        results["suitable_for_production"] = False
        results["remaining"] = (
            "controller-owned terminal queries; bounded screen replay; native provider/image "
            "proof; lf review and driver lifetime integration"
        )
    except BaseException as error:
        results["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        (args.output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
