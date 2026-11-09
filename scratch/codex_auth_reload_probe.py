# /// script
# dependencies = ["websockets>=15,<16"]
# ///
"""Observe managed-auth reload using synthetic logins and loopback discovery."""

import argparse
import base64
import datetime
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tests/e2e"))
from codex_connect import Client


def _auth(home: Path, name: str, email: str | None = None) -> bytes:
    claims = {
        "email": email or name + "@example.test",
        "exp": 4102444800,
        "https://api.openai.com/auth": {
            "chatgpt_account_id": name,
            "chatgpt_user_id": name,
            "chatgpt_plan_type": "plus",
        },
    }
    token = (
        "e30."
        + base64.urlsafe_b64encode(json.dumps(claims).encode()).decode().rstrip("=")
        + ".fixture"
    )
    payload = json.dumps(
        {
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": token,
                "access_token": token,
                "refresh_token": "fixture-" + name,
                "account_id": name,
            },
            "last_refresh": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        }
    ).encode()
    (home / "auth.json").write_bytes(payload)
    return payload


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if os.environ.get("LOOPFLOW_NETWORK_ISOLATED") != "1":
        raise RuntimeError("run through scripts/test_network.py")
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    requests = []

    class Backend(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_GET(self):
            account = self.headers.get("chatgpt-account-id")
            requests.append({"path": self.path, "account": account})
            payload = (
                {
                    "accounts": [
                        {
                            "id": name,
                            "name": name,
                            "workspace_backend_origin": "https://fixture.example.test",
                            "account_routing_override": "NO_CONSTRAINT",
                        }
                        for name in ("first", "second")
                    ]
                }
                if "/accounts/check" in self.path
                else {}
            )
            body = json.dumps(payload).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    backend = ThreadingHTTPServer(("127.0.0.1", 0), Backend)
    worker = threading.Thread(target=backend.serve_forever)
    worker.start()
    process = None
    client = None
    results = {}
    try:
        with tempfile.TemporaryDirectory(prefix="loo811-auth-", dir="/tmp") as temporary:
            root = Path(temporary)
            home = root / "codex"
            home.mkdir()
            (home / "config.toml").write_text(
                'cli_auth_credentials_store="file"\n'
                f'chatgpt_base_url="http://127.0.0.1:{backend.server_port}/backend-api/"\n'
            )
            _auth(home, "first")
            endpoint = root / "agent.sock"
            env = {
                "HOME": str(root),
                "CODEX_HOME": str(home),
                "PATH": "/usr/bin:/bin",
                "RUST_LOG": "warn",
            }
            with (out / "agent.log").open("w") as log:
                process = subprocess.Popen(
                    [
                        str(args.codex.resolve()),
                        "app-server",
                        "--listen",
                        "unix://" + str(endpoint),
                    ],
                    cwd=root,
                    env=env,
                    stdin=subprocess.DEVNULL,
                    stdout=log,
                    stderr=log,
                    start_new_session=True,
                )
                try:
                    deadline = time.monotonic() + 10
                    while (
                        not endpoint.exists()
                        and time.monotonic() < deadline
                        and process.poll() is None
                    ):
                        time.sleep(0.05)
                    client = Client(endpoint)

                    def read(label, refresh):
                        try:
                            results[label] = client.call("account/read", {"refreshToken": refresh})
                        except AssertionError as error:
                            results[label] = error.args[0].get("error")

                    read("before", False)
                    _auth(home, "first", "rotated@example.test")
                    read("same_account_refresh", True)
                    saved = _auth(home, "second")
                    read("after_file_switch", False)
                    read("after_forced_refresh", True)
                    results["file_still_second"] = (home / "auth.json").read_bytes() == saved
                    results["provider_alive"] = process.poll() is None
                    results["requests"] = requests
                    assert results["before"]["account"]["email"] == "first@example.test"
                    assert (
                        results["same_account_refresh"]["account"]["email"]
                        == "rotated@example.test"
                    )
                    results["cross_account_reload"] = (
                        results["after_forced_refresh"]["account"]["email"] == "second@example.test"
                    )
                finally:
                    if client is not None:
                        client.close()
                    process.terminate()
                    try:
                        process.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
                    results["provider_reaped"] = process.poll() is not None
    finally:
        backend.shutdown()
        backend.server_close()
        worker.join(timeout=3)
        results["http_worker_reaped"] = not worker.is_alive()
        (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
        print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
