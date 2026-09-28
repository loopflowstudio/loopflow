"""Expose the line-oriented provider fixture over its advertised Unix WebSocket."""

import base64
import hashlib
import os
import socket
import struct
import subprocess
import sys
import threading


def _read(stream: socket.socket, length: int) -> bytes:
    data = bytearray()
    while len(data) < length:
        part = stream.recv(length - len(data))
        if not part:
            raise EOFError
        data.extend(part)
    return bytes(data)


def _message(stream: socket.socket) -> bytes:
    first, second = _read(stream, 2)
    size = second & 127
    if size == 126:
        size = struct.unpack("!H", _read(stream, 2))[0]
    elif size == 127:
        size = struct.unpack("!Q", _read(stream, 8))[0]
    mask = _read(stream, 4) if second & 128 else None
    payload = _read(stream, size)
    if first & 15 == 8:
        raise EOFError
    return bytes(value ^ mask[i % 4] for i, value in enumerate(payload)) if mask else payload


def _reply(stream: socket.socket, payload: bytes) -> None:
    size = len(payload)
    length = bytes([size]) if size < 126 else b"\x7e" + struct.pack("!H", size)
    stream.sendall(b"\x81" + length + payload)


def main() -> None:
    script, *args = sys.argv[1:]
    endpoint = args[args.index("--listen") + 1].removeprefix("unix://")
    with socket.socket(socket.AF_UNIX) as listener:
        listener.bind(endpoint)
        listener.listen(1)
        listener.settimeout(30)
        client, _ = listener.accept()
        with client:
            header = bytearray()
            while not header.endswith(b"\r\n\r\n"):
                header.extend(_read(client, 1))
            key = next(
                line.split(b":", 1)[1].strip()
                for line in header.split(b"\r\n")
                if line.lower().startswith(b"sec-websocket-key:")
            )
            accept = base64.b64encode(
                hashlib.sha1(key + b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11").digest()
            )
            client.sendall(
                b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n"
                b"Connection: Upgrade\r\nSec-WebSocket-Accept: " + accept + b"\r\n\r\n"
            )
            env = dict(os.environ, LF_TEST_CODEX_STDIO="1")
            with subprocess.Popen(
                [script, *args], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE
            ) as child:
                assert child.stdin is not None and child.stdout is not None

                def replies() -> None:
                    try:
                        for line in child.stdout:
                            _reply(client, line.rstrip(b"\n"))
                    except OSError:
                        pass
                    finally:
                        client.shutdown(socket.SHUT_RDWR)

                reader = threading.Thread(target=replies, daemon=True)
                reader.start()
                try:
                    while True:
                        child.stdin.write(_message(client) + b"\n")
                        child.stdin.flush()
                except (EOFError, OSError):
                    pass
                finally:
                    child.stdin.close()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait()
                    reader.join(timeout=5)


if __name__ == "__main__":
    main()
