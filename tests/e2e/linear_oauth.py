# /// script
# requires-python = ">=3.10"
# dependencies = ["cryptography>=44"]
# ///
"""Exercise an installed candidate against synthetic Linear HTTPS in a disposable container."""

import argparse
import base64
import hashlib
import json
import os
import socket
import sqlite3
import ssl
import subprocess
import tempfile
import threading
import time
import uuid
from datetime import datetime, timedelta, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.x509.oid import NameOID

REPOSITORY_CLAIM = "<!-- loopflow-repository: loopflowstudio/fixture -->"


class LinearServer(ThreadingHTTPServer):
    mode = "recover"
    exchanges = 0
    reads = 0
    invalid_requests = 0


class LinearHandler(BaseHTTPRequestHandler):
    def log_message(self, format: str, *args: object) -> None:
        pass  # Never log request bodies or authorization headers.

    def do_POST(self) -> None:
        server = self.server
        body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
        status = 200
        payload = {}
        if self.path == "/oauth/token":
            server.exchanges += 1
            if parse_qs(body.decode()) != {
                "grant_type": ["refresh_token"],
                "client_id": ["fixture-client"],
                "refresh_token": ["synthetic-R1"],
            }:
                server.invalid_requests += 1
                status = 400
            elif server.mode == "reject":
                status = 400
                payload = {"error": "invalid_grant", "error_description": "synthetic-secret"}
            elif server.exchanges == 1:
                status = 503
            else:
                payload = {
                    "access_token": "synthetic-A2",
                    "refresh_token": "synthetic-R2",
                    "expires_in": 86400,
                }
        elif self.path == "/graphql":
            server.reads += 1
            if self.headers.get("Authorization") != "Bearer synthetic-A2":
                server.invalid_requests += 1
                status = 401
            else:
                query = json.loads(body)["query"]
                page = {"hasNextPage": False, "endCursor": None}
                if "query ListTeams" in query:
                    payload = {
                        "data": {
                            "teams": {
                                "nodes": [
                                    {
                                        "id": "team-1",
                                        "name": "Fixture",
                                        "key": "FIX",
                                        "description": REPOSITORY_CLAIM,
                                    }
                                ]
                            }
                        }
                    }
                elif "query ListInitiativeProjects" in query:
                    payload = {
                        "data": {
                            "initiative": {
                                "projects": {
                                    "nodes": [
                                        {
                                            "id": "project-1",
                                            "name": "Product — Reliability",
                                            "description": "",
                                            "content": "## Definition\n\nFresh definition.\n\n"
                                            "## KRs\n\n- [ ] Fresh proof",
                                            "initiatives": {"nodes": [{"id": "initiative-1"}]},
                                            "teams": {"nodes": [{"id": "team-1"}]},
                                        }
                                    ],
                                    "pageInfo": page,
                                }
                            }
                        }
                    }
                elif "query ListProjectIssues" in query:
                    payload = {
                        "data": {
                            "project": {
                                "issues": {
                                    "nodes": [],
                                    "pageInfo": page,
                                }
                            }
                        }
                    }
                else:
                    server.invalid_requests += 1
                    status = 400
        else:
            server.invalid_requests += 1
            status = 404
        encoded = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)


def _encrypt(key: bytes, value: str) -> str:
    nonce = os.urandom(12)
    return (
        base64.b64encode(nonce + AESGCM(key).encrypt(nonce, value.encode(), None))
        .decode()
        .rstrip("=")
    )


def _decrypt(key: bytes, value: str) -> str:
    raw = base64.b64decode(value + "=" * (-len(value) % 4))
    return AESGCM(key).decrypt(raw[:12], raw[12:], None).decode()


def _run(args: list[str], **kwargs: object) -> subprocess.CompletedProcess:
    return subprocess.run(args, capture_output=True, text=True, timeout=15, **kwargs)


def _tls_context(root: Path) -> ssl.SSLContext:
    key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    issuer = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "Linear fixture CA")])
    now = datetime.now(timezone.utc)
    for ca, filename in [(True, "certificate.pem"), (False, "server.pem")]:
        subject = (
            issuer if ca else x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "api.linear.app")])
        )
        cert = (
            x509.CertificateBuilder()
            .subject_name(subject)
            .issuer_name(issuer)
            .public_key(key.public_key())
            .serial_number(x509.random_serial_number())
            .not_valid_before(now - timedelta(minutes=1))
            .not_valid_after(now + timedelta(days=1))
            .add_extension(x509.BasicConstraints(ca=ca, path_length=None), critical=True)
            .add_extension(
                x509.SubjectAlternativeName([x509.DNSName("api.linear.app")]), critical=False
            )
            .sign(key, hashes.SHA256())
        )
        (root / filename).write_bytes(cert.public_bytes(serialization.Encoding.PEM))
    private_key = root / "certificate.key"
    private_key.write_bytes(
        key.private_bytes(
            serialization.Encoding.PEM,
            serialization.PrivateFormat.PKCS8,
            serialization.NoEncryption(),
        )
    )
    private_key.chmod(0o600)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(root / "server.pem", private_key)
    return context


def _exercise(lf: Path, root: Path, selection: dict, server: LinearServer) -> list[dict]:
    repo = root / "repo"
    (repo / ".lf").mkdir(parents=True)
    (repo / "wave/product").mkdir(parents=True)
    for args in [
        ["init", "-q"],
        ["remote", "add", "origin", "https://github.com/loopflowstudio/fixture.git"],
    ]:
        assert _run(["git", *args], cwd=repo).returncode == 0
    (repo / ".lf/config.yaml").write_text("pm:\n  provider: linear\n  linear_team: team-1\n")
    (repo / "wave/product/GOAL.md").write_text(
        "---\npm:\n  linear_initiative: initiative-1\n---\nKeep working.\n"
    )
    key = AESGCM.generate_key(bit_length=256)
    key_path = root / "fixture.key"
    key_path.write_bytes(base64.b64encode(key).rstrip(b"="))
    key_path.chmod(0o600)
    # A minimal child environment excludes all ambient Run and forwarded-token authority.
    env = {
        "PATH": os.environ["PATH"],
        "HOME": str(Path.home()),
        "SSL_CERT_FILE": str(root / "certificate.pem"),
        "SSL_CERT_DIR": str(root / "empty-certificates"),
        "LF_PROVIDER_TOKEN_KEY_PATH": str(key_path),
    }
    (root / "empty-certificates").mkdir()
    wave_id = str(uuid.uuid4())
    with sqlite3.connect(selection["store"]) as db:
        db.execute("PRAGMA foreign_keys = ON")
        db.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?,?,?,?)",
            (wave_id, "product", str(repo), int(time.time())),
        )
        db.commit()
        wave_before = db.execute("SELECT * FROM waves WHERE id=?", (wave_id,)).fetchone()
        receipts = []
        for mode, cached in [("recover", True), ("reject", True), ("reject", False)]:
            server.mode, server.exchanges, server.reads = mode, 0, 0
            db.execute("DELETE FROM provider_tokens WHERE provider='linear'")
            db.execute(
                "INSERT INTO provider_tokens VALUES(?,?,?,?,?,?,?,?,?)",
                (
                    "linear",
                    _encrypt(key, "synthetic-A1"),
                    _encrypt(key, "synthetic-R1"),
                    "fixture-client",
                    int(time.time()) - 1,
                    "fixture",
                    int(time.time()),
                    "oauth",
                    1,
                ),
            )
            db.execute("DELETE FROM pm_snapshots WHERE wave_id=?", (wave_id,))
            if cached:
                db.execute(
                    "INSERT INTO pm_snapshots VALUES(?,?,?,?,?)",
                    (
                        wave_id,
                        "linear",
                        "initiative-1",
                        1,
                        json.dumps({"projects": [], "items": []}),
                    ),
                )
            db.commit()
            token_before = db.execute("SELECT * FROM provider_tokens").fetchone()
            snapshot_before = db.execute(
                "SELECT * FROM pm_snapshots WHERE wave_id=?", (wave_id,)
            ).fetchall()
            started = time.monotonic()
            result = _run(
                [str(lf), "pm", "show", "--wave", "product", "--sync", "--json"], cwd=repo, env=env
            )
            elapsed = time.monotonic() - started
            assert elapsed < 8, "planning read exceeded its bounded failure allowance"
            assert not any(
                marker in result.stdout + result.stderr
                for marker in ["synthetic-A", "synthetic-R", "synthetic-secret", "fixture-client"]
            )
            current = db.execute("SELECT * FROM provider_tokens").fetchone()
            assert (
                db.execute("SELECT * FROM waves WHERE id=?", (wave_id,)).fetchone() == wave_before
            )
            if mode == "recover":
                assert result.returncode == 0, result.stderr
                shown = json.loads(result.stdout)
                assert shown["projects"][0]["definition"] == "Fresh definition."
                assert shown["projects"][0]["krs"][0]["text"] == "Fresh proof"
                snapshot = json.loads(
                    db.execute(
                        "SELECT payload FROM pm_snapshots WHERE wave_id=?", (wave_id,)
                    ).fetchone()[0]
                )
                assert snapshot["projects"] == shown["projects"]
                assert current[8] == 1
                assert _decrypt(key, current[1]) == "synthetic-A2"
                assert _decrypt(key, current[2]) == "synthetic-R2"
                assert current[3] == "fixture-client" and current[5] == "fixture"
                assert current[4] > time.time() and server.exchanges == 2 and server.reads == 3
                assert "reconnect" not in result.stderr
            else:
                assert result.returncode != 0
                assert not result.stdout.strip(), "failed fresh read returned planning output"
                assert "invalid_grant" in result.stderr
                assert "doppler run -- lf auth linear" in result.stderr
                assert current == token_before
                assert (
                    db.execute("SELECT * FROM pm_snapshots WHERE wave_id=?", (wave_id,)).fetchall()
                    == snapshot_before
                )
                assert server.exchanges == 1 and server.reads == 0
            assert server.invalid_requests == 0
            receipts.append(
                {
                    "case": mode,
                    "cached": cached,
                    "exit_code": result.returncode,
                    "seconds": round(elapsed, 3),
                    "exchanges": server.exchanges,
                    "graphql_reads": server.reads,
                }
            )
    return receipts


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lf", type=Path, default=Path("/root/.local/bin/lf"))
    args = parser.parse_args()
    # This fixture seeds the selected installed Home; never run against a user's account.
    if not Path("/.dockerenv").exists() or socket.gethostbyname("api.linear.app") != "127.0.0.1":
        parser.error("run in a disposable Linux container with api.linear.app mapped to 127.0.0.1")
    active = json.loads((Path.home() / ".lf-machine/install/active.json").read_text())
    selection = active["selection"]
    assert selection["source"] == "development", "install the development candidate first"
    version = _run([str(args.lf), "--version"])
    assert version.returncode == 0
    artifacts = selection["artifact_set"]["artifacts"]
    cli = next(artifact for artifact in artifacts if artifact["role"] == {"kind": "cli"})
    assert hashlib.sha256(Path(cli["path"]).read_bytes()).hexdigest() == cli["sha256"]
    with tempfile.TemporaryDirectory(prefix="linear-oauth-") as directory:
        root = Path(directory)
        context = _tls_context(root)
        with LinearServer(("127.0.0.1", 443), LinearHandler) as server:
            server.socket = context.wrap_socket(server.socket, server_side=True)
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                cases = _exercise(args.lf.resolve(), root, selection, server)
            finally:
                server.shutdown()
                thread.join()
    print(
        json.dumps(
            {
                "source_revision": selection["artifact_set"]["source_revision"],
                "entry_point": str(args.lf),
                "entry_target": str(args.lf.resolve()),
                "version": version.stdout.strip(),
                "cli_sha256": cli["sha256"],
                "installation": selection["installation_id"],
                "provider": "simulated HTTPS",
                "cases": cases,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
