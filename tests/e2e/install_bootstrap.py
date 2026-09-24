"""Exercise candidate installation in a disposable Linux container, never on a host.

Run with uv after copying a release-shaped lf/lfd pair to /fixture/bin and the
shell installer to /fixture/install.sh. Draft migrations must be materialized
in a disposable source snapshot before building with published authority.
Optionally put a checksum-verified older release pair in /fixture/prior.

The HTTPS release transport is local; binaries, checksum checks, promotion,
store initialization, activation and recovery are real. Each case owns a fresh
OS account because HOME overrides cannot isolate machine installation state.
"""

from __future__ import annotations

import hashlib
import http.server
import json
import os
import shutil
import sqlite3
import ssl
import subprocess
import tarfile
import threading
from pathlib import Path

FIXTURE = Path("/fixture")
ASSETS = FIXTURE / "release"
CLI = FIXTURE / "bin/lf"
DAEMON = FIXTURE / "bin/lfd"
REQUESTS: list[str] = []


def run(*args: str, cwd: Path | None = None) -> str:
    result = subprocess.run(args, cwd=cwd, capture_output=True, text=True, timeout=180)
    if result.returncode:
        raise RuntimeError(f"{args}: {result.stdout}\n{result.stderr}")
    return result.stdout.strip()


def invoke(user: str, cwd: Path, *args: str, succeeds: bool = True) -> str:
    result = subprocess.run(
        [
            "runuser",
            "-u",
            user,
            "--",
            "env",
            "-i",
            f"HOME=/home/{user}",
            "PATH=/usr/bin:/bin",
            "NO_COLOR=1",
            "SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt",
            *args,
        ],
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=180,
    )
    output = result.stdout + result.stderr
    print(f"$ ({user}) {' '.join(args)}\n{output}", flush=True)
    assert (result.returncode == 0) == succeeds, output
    return output


def account(name: str) -> Path:
    run("useradd", "--create-home", name)
    return Path("/home") / name


def snapshot(root: Path) -> dict[str, str]:
    return {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in root.rglob("*")
        if path.is_file()
    }


def release_server(version: str) -> http.server.HTTPServer:
    ASSETS.mkdir()
    shutil.copy2(FIXTURE / "install.sh", ASSETS / "install.sh")
    architecture = run("uname", "-m")
    archive = ASSETS / f"lf-{architecture}-unknown-linux-gnu.tar.gz"
    with tarfile.open(archive, "w:gz", compresslevel=1) as package:
        package.add(CLI, arcname="lf")
        package.add(DAEMON, arcname="lfd")
    with tarfile.open(archive) as package:
        for source in [CLI, DAEMON]:
            member = package.extractfile(source.name)
            assert member is not None
            assert (
                hashlib.sha256(member.read()).digest()
                == hashlib.sha256(source.read_bytes()).digest()
            )
    (ASSETS / "SHA256SUMS").write_text(
        "".join(
            f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n"
            for path in [ASSETS / "install.sh", archive]
        )
    )
    certificate = Path("/usr/local/share/ca-certificates/lf-install-fixture.crt")
    ca_key = FIXTURE / "ca.key"
    key = FIXTURE / "tls.key"
    run(
        "openssl",
        "req",
        "-x509",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-days",
        "1",
        "-keyout",
        str(ca_key),
        "-out",
        str(certificate),
        "-subj",
        "/CN=Loopflow installation fixture CA",
    )
    request = FIXTURE / "tls.csr"
    leaf = FIXTURE / "tls.crt"
    extensions = FIXTURE / "tls.ext"
    extensions.write_text(
        "basicConstraints=CA:FALSE\nsubjectAltName=DNS:github.com\n"
        "keyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n"
    )
    run(
        "openssl",
        "req",
        "-new",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-keyout",
        str(key),
        "-out",
        str(request),
        "-subj",
        "/CN=github.com",
    )
    run(
        "openssl",
        "x509",
        "-req",
        "-in",
        str(request),
        "-CA",
        str(certificate),
        "-CAkey",
        str(ca_key),
        "-CAcreateserial",
        "-days",
        "1",
        "-out",
        str(leaf),
        "-extfile",
        str(extensions),
    )
    run("update-ca-certificates")
    with Path("/etc/hosts").open("a") as hosts:
        hosts.write("\n127.0.0.1 github.com\n")
    base = "/loopflowstudio/loopflow/releases"

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_HEAD(self) -> None:
            self.do_GET()

        def do_GET(self) -> None:
            REQUESTS.append(self.path)
            if self.path == f"{base}/latest":
                location = f"{base}/tag/v{version}"
            elif self.path.startswith(f"{base}/latest/download/"):
                location = f"{base}/download/v{version}/{self.path.rsplit('/', 1)[-1]}"
            else:
                location = None
            if location:
                self.send_response(302)
                self.send_header("Location", location)
                self.end_headers()
                return
            if self.path == f"{base}/tag/v{version}":
                content = b"isolated candidate release fixture"
            elif self.path.startswith(f"{base}/download/v{version}/"):
                path = ASSETS / self.path.rsplit("/", 1)[-1]
                if not path.is_file():
                    self.send_error(404)
                    return
                content = path.read_bytes()
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Length", str(len(content)))
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(content)

    server = http.server.HTTPServer(("127.0.0.1", 443), Handler)
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(leaf, key)
    server.socket = context.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def main() -> None:
    if not Path("/.dockerenv").exists() or os.getuid() != 0:
        raise SystemExit("Run only as root in a disposable container without host Home mounts.")
    version = run(str(CLI), "--version").removeprefix("lf ")
    fresh = account("lf-fresh")
    recovery = account("lf-recovery")
    old = account("lf-upgrade")
    checkout = fresh / "project"
    invoke("lf-fresh", fresh, "git", "init", str(checkout))
    invoke("lf-fresh", checkout, "git", "config", "user.name", "Install fixture")
    invoke("lf-fresh", checkout, "git", "config", "user.email", "fixture@example.invalid")
    (checkout / "tracked").write_text("tracked\n")
    invoke("lf-fresh", checkout, "git", "add", "tracked")
    invoke("lf-fresh", checkout, "git", "commit", "-m", "fixture")
    (checkout / "tracked").write_text("dirty\n")
    (checkout / "untracked").write_text("untracked\n")
    before = snapshot(checkout)
    shutil.move("/usr/bin/git", FIXTURE / "git-disabled")
    assert shutil.which("git") is None
    server = release_server(version)
    try:
        invoke("lf-fresh", fresh, str(CLI), "install")
        installed = fresh / ".local/bin/lf"
        assert invoke("lf-fresh", fresh, str(installed), "--version").strip() == f"lf {version}"
        assert (fresh / ".lf/loopflow.db").is_file()
        assert (fresh / ".lf-machine/install/active.json").is_file()
        REQUESTS.clear()
        assert "already installed" in invoke("lf-fresh", checkout, str(installed), "install")
        assert not any("/download/" in path for path in REQUESTS)
        with sqlite3.connect(fresh / ".lf/loopflow.db") as store:
            frontier = store.execute("SELECT * FROM schema_migrations").fetchall()
        (fresh / ".local/bin/lfd").unlink()
        invoke("lf-fresh", checkout, str(installed), "install")
        assert (
            invoke("lf-fresh", fresh, str(fresh / ".local/bin/lfd"), "--version").strip()
            == f"lfd {version}"
        )
        with sqlite3.connect(fresh / ".lf/loopflow.db") as store:
            assert store.execute("SELECT * FROM schema_migrations").fetchall() == frontier
        assert snapshot(checkout) == before
        assert not (checkout / ".lf").exists()

        blocked = recovery / "blocked"
        blocked.write_text("not a directory")
        invoke(
            "lf-recovery",
            recovery,
            str(CLI),
            "install",
            "promote",
            "--cli-target",
            str(blocked / "lf"),
            "--daemon-source",
            str(DAEMON),
            "--daemon-target",
            str(blocked / "lfd"),
            succeeds=False,
        )
        receipt_path = recovery / ".lf-machine/install/switch.json"
        receipt = json.loads(receipt_path.read_text())
        assert receipt["prior"] is None and receipt["published_fallback"] is None
        assert receipt["target_store_advanced"] and not receipt["active_selection_committed"]
        assert not (receipt_path.parent / "active.json").exists()
        candidate = receipt["candidate"]["path"]
        blocked.unlink()
        invoke(
            "lf-recovery",
            recovery,
            candidate,
            "install",
            "recover-switch",
            "--switch",
            receipt["id"],
        )
        assert not receipt_path.exists()
        assert (
            invoke("lf-recovery", recovery, str(blocked / "lf"), "--version").strip()
            == f"lf {version}"
        )

        prior = FIXTURE / "prior"
        if prior.is_dir():
            (old / ".local/bin").mkdir(parents=True)
            for name in ["lf", "lfd"]:
                shutil.copy2(prior / name, old / ".local/bin" / name)
            run("chown", "-R", "lf-upgrade:lf-upgrade", str(old / ".local"))
            invoke("lf-upgrade", old, str(old / ".local/bin/lf"), "--version")
            invoke("lf-upgrade", old, "/bin/sh", str(FIXTURE / "install.sh"))
            assert (
                invoke("lf-upgrade", old, str(old / ".local/bin/lf"), "--version").strip()
                == f"lf {version}"
            )
        print(
            "PASS: candidate install, repeat, repair, checkout preservation, "
            "and first-install recovery."
        )
        print("Transport is a local HTTPS fixture; this is not a public release-channel pass.")
    finally:
        server.shutdown()


if __name__ == "__main__":
    main()
