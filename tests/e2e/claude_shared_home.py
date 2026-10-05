# /// script
# requires-python = ">=3.10"
# ///
"""Loopflow and plain Claude share one home, signed in as one stored account.

Runs a real Claude against a local endpoint with synthetic logins. The native
home is a temporary config directory: on macOS its login is that directory's
own Keychain item, removed afterwards; elsewhere it is `.credentials.json`.
"""

import argparse
import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

PROVIDER_ENV = ("CLAUDE_CONFIG_DIR", "CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY")
MACOS = sys.platform == "darwin"


class Messages(ThreadingHTTPServer):
    def __init__(self) -> None:
        super().__init__(("127.0.0.1", 0), Handler)
        self.bearers: list[str] = []


class Handler(BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:
        pass

    def do_POST(self) -> None:
        self.rfile.read(int(self.headers.get("Content-Length") or 0))
        if self.path.startswith("/v1/messages"):
            self.server.bearers.append(self.headers.get("Authorization", ""))
        body = json.dumps(
            {"type": "error", "error": {"type": "invalid_request_error", "message": "fixture"}}
        ).encode()
        self.send_response(400)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    do_GET = do_POST


def _login(name: str) -> str:
    return json.dumps(
        {
            "claudeAiOauth": {
                "accessToken": f"sk-ant-oat01-fixture-{name}",
                "refreshToken": f"sk-ant-ort01-fixture-{name}",
                "expiresAt": 4102444800000,
                "scopes": ["user:inference", "user:profile"],
                "subscriptionType": "max",
            }
        }
    )


def _bearer(name: str) -> str:
    return f"Bearer sk-ant-oat01-fixture-{name}"


def _keychain_service(native: Path) -> str:
    return "Claude Code-credentials-" + hashlib.sha256(str(native).encode()).hexdigest()[:8]


def _native_login(native: Path) -> str | None:
    """The login a Claude started in `native` reads."""
    if MACOS:
        found = subprocess.run(
            ["security", "find-generic-password", "-s", _keychain_service(native), "-w"],
            capture_output=True,
            text=True,
        )
        if found.returncode == 0:
            return found.stdout.rstrip("\n")
    file = native / ".credentials.json"
    return file.read_text() if file.exists() else None


def _forget_native_login(native: Path) -> None:
    if MACOS:
        subprocess.run(
            ["security", "delete-generic-password", "-s", _keychain_service(native)],
            capture_output=True,
        )


def _probe(claude: Path, directory: Path) -> Path:
    """A `claude` that records the environment Loopflow launched it with."""
    directory.mkdir()
    probe = directory / "claude"
    probe.write_text(
        f"#!{sys.executable}\n"
        "import json, os, sys\n"
        "if record := os.environ.get('LF_PROBE_LAUNCHES'):\n"
        "    with open(record, 'a') as launches:\n"
        f"        launch = {{name: os.environ.get(name) for name in {PROVIDER_ENV!r}}}\n"
        "        launches.write(json.dumps(launch) + '\\n')\n"
        f"os.execv({str(claude)!r}, [{str(claude)!r}, *sys.argv[1:]])\n"
    )
    probe.chmod(0o755)
    return probe


def _contract(binary: Path, claude: Path, root: Path, server: Messages, results: dict) -> None:
    work = root / "work"
    work.mkdir()
    native = (root / "native-claude").resolve()
    native.mkdir()
    lf_home = root / "lf"
    profiles = lf_home / "accounts" / "claude"
    record = root / "launches.jsonl"
    record.touch()
    probe = _probe(claude, root / "bin")
    # macOS finds the login Keychain through the person's own home.
    home = Path.home() if MACOS else root
    env = {
        "HOME": str(home),
        "USER": os.environ.get("USER", "fixture"),
        "PATH": f"{probe.parent}{os.pathsep}{os.environ['PATH']}",
        "LF_HOME": str(lf_home),
        "CLAUDE_CONFIG_DIR": str(native),
        "ANTHROPIC_BASE_URL": f"http://127.0.0.1:{server.server_port}",
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1",
        "DISABLE_TELEMETRY": "1",
        "LF_PROBE_LAUNCHES": str(record),
    }
    subprocess.run(["git", "init", "--quiet", str(work)], env=env, check=True)
    subprocess.run(
        ["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.test"]
        + ["commit", "--allow-empty", "--quiet", "-m", "fixture"],
        cwd=work,
        env=env,
        check=True,
    )

    def lf(*args: str, check: bool = True) -> subprocess.CompletedProcess:
        done = subprocess.run(
            [str(binary), *args],
            cwd=work,
            env=env,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
            timeout=180,
        )
        assert not check or done.returncode == 0, (args, done.stdout, done.stderr)
        return done

    def use(name: str) -> str:
        return lf("account", "claude", "use", f"{name}@example.com").stdout

    def plain_claude() -> str:
        """Claude as a person runs it in the native home; the login it sent."""
        before = len(server.bearers)
        plain = {key: value for key, value in env.items() if not key.startswith("LF_")}
        plain["PATH"] = os.environ["PATH"]
        subprocess.run(
            [str(claude), "-p", "say hi"],
            cwd=work,
            env=plain,
            capture_output=True,
            stdin=subprocess.DEVNULL,
            timeout=180,
        )
        sent = set(server.bearers[before:])
        assert len(sent) == 1, sent
        return sent.pop()

    def converse(*flags: str) -> tuple[dict, str]:
        """One headless Loopflow conversation: its launch and the login it sent."""
        launched, before = len(record.read_text().splitlines()), len(server.bearers)
        # The fixture endpoint refuses every turn; only the launch is under test.
        done = lf(*flags, "--mode", "batch", "--model", "claude", ":", "say hi", check=False)
        launches = [json.loads(line) for line in record.read_text().splitlines()[launched:]]
        sent = set(server.bearers[before:])
        assert launches and len(sent) == 1, (launches, sent, done.stdout, done.stderr)
        return launches[-1], sent.pop()

    def rows(query: str, *values: object) -> list[tuple]:
        with sqlite3.connect(lf_home / "loopflow.db") as database:
            return database.execute(query, values).fetchall()

    lf("session", "list", "--json")
    now = int(time.time())
    with sqlite3.connect(lf_home / "loopflow.db") as database:
        for name in ("first", "second"):
            profile = profiles / name
            profile.mkdir(parents=True)
            (profile / ".credentials.json").write_text(_login(name))
            database.execute(
                "INSERT INTO provider_accounts(provider,account_id,home,login_email,"
                "credential_state,routing_state,created_at,updated_at,observed_email,"
                "observed_subject,observed_credential_digest) "
                "VALUES('claude',?,?,?,'connected','automatic',?,?,?,?,?)",
                (
                    name,
                    str(profile),
                    f"{name}@example.com",
                    now,
                    now,
                    f"{name}@example.com",
                    name,
                    hashlib.sha256(_login(name).encode()).hexdigest(),
                ),
            )

    # A Claude started after a switch runs on the login Loopflow installed.
    use("first")
    assert _native_login(native) == _login("first")
    assert plain_claude() == _bearer("first")
    use("second")
    assert plain_claude() == _bearer("second")
    use("first")
    assert plain_claude() == _bearer("first")
    assert "already" in use("first")
    results["store"] = "keychain" if MACOS else "file"
    results["plain_claude_follows_switch"] = True

    # A launch naming no account, or the active one, changes nothing; neither
    # hands Claude an account home or a credential.
    switched = rows("SELECT COUNT(*) FROM provider_account_switches")[0][0]
    for flags in (("--shared",), ("--shared", "--account", "claude=first@example.com")):
        launch, sent = converse(*flags)
        assert launch["CLAUDE_CONFIG_DIR"] == str(native), launch
        assert launch["CLAUDE_CODE_OAUTH_TOKEN"] is None and launch["ANTHROPIC_API_KEY"] is None
        assert sent == _bearer("first"), sent
    assert rows("SELECT COUNT(*) FROM provider_account_switches")[0][0] == switched
    results["shared_launch"] = True

    # Naming another account moves the home, and plain Claude with it.
    _, sent = converse("--shared", "--account", "claude=second@example.com")
    assert sent == _bearer("second"), sent
    assert plain_claude() == _bearer("second")
    results["launch_switches_home"] = True

    # An isolated launch stays in its account's own home and leaves the
    # native login alone.
    launch, sent = converse("--isolate", "--account", "claude=first@example.com")
    assert launch["CLAUDE_CONFIG_DIR"] == str(profiles / "first"), launch
    assert sent == _bearer("first"), sent
    assert _native_login(native) == _login("second")
    results["isolated_launch"] = True


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claude", required=True, type=Path)
    parser.add_argument("--lf", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    server = Messages()
    threading.Thread(target=server.serve_forever, daemon=True).start()
    root = Path(tempfile.mkdtemp(prefix="lf-claude-shared-")).resolve()
    binary = root / "candidate" / "lf"
    binary.parent.mkdir()
    shutil.copy2(args.lf.resolve(), binary)
    results: dict = {}
    try:
        _contract(binary, args.claude.resolve(), root, server, results)
    finally:
        _forget_native_login((root / "native-claude").resolve())
        server.shutdown()
        (args.output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
        shutil.rmtree(root, ignore_errors=True)
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
