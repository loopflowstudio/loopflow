import importlib.util
import io
import os
from pathlib import Path
from unittest.mock import MagicMock

import pytest


def test_early_provider_exit_does_not_fail_bridge_cleanup(monkeypatch):
    path = (
        Path(__file__).resolve().parents[2]
        / "rust/loopflow-execution/tests/support/codex_socket.py"
    )
    spec = importlib.util.spec_from_file_location("codex_socket", path)
    bridge = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(bridge)

    listener = MagicMock()
    listener.__enter__.return_value = listener
    client = MagicMock()
    client.__enter__.return_value = client
    listener.accept.return_value = (client, None)
    header = iter(b"Sec-WebSocket-Key: fixture\r\n\r\n")
    monkeypatch.setattr(bridge, "_read", lambda stream, length: bytes([next(header)]))
    monkeypatch.setattr(bridge, "_message", lambda stream: b"{}")
    monkeypatch.setattr(bridge.socket, "socket", lambda *args: listener)

    # The provider has closed stdin before the bridge forwards initialize.
    read_fd, write_fd = os.pipe()
    os.close(read_fd)
    with os.fdopen(write_fd, "wb") as stdin:
        child = MagicMock()
        child.__enter__.return_value = child
        child.stdin = stdin
        child.stdout = io.BytesIO()
        monkeypatch.setattr(bridge.subprocess, "Popen", lambda *args, **kwargs: child)
        monkeypatch.setattr(
            bridge.sys, "argv", ["bridge", "provider", "--listen", "unix://fixture"]
        )

        class FixtureIdle(Exception):
            pass

        def idle():
            raise FixtureIdle

        monkeypatch.setattr(bridge.signal, "pause", idle)
        with pytest.raises(FixtureIdle):
            bridge.main()
