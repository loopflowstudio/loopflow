"""Trusted SSH controller; only the export request enters the isolated reader."""

import json
import shutil
import signal
import subprocess
import sys
from pathlib import Path


def deliver(home_dir: Path) -> None:
    env = {"HOME": str(home_dir), "PATH": "/usr/local/bin:/usr/bin:/bin"}
    lf = next(
        (str(path) for path in (home_dir / ".local/bin/lf", Path("/usr/local/bin/lf"), Path("/usr/bin/lf")) if path.is_file()),
        None,
    )
    assert lf is not None
    home = subprocess.run(
        [lf, "home", "id", "--json"], env=env, stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=10, check=True,
    )
    identity = json.loads(home.stdout)["id"]
    print(json.dumps(identity), flush=True)
    payload = json.loads(sys.stdin.buffer.read(20 * 1024 * 1024))
    assert identity == payload["home"]
    docker = shutil.which("docker", path=env["PATH"])
    assert docker is not None
    endpoint = subprocess.run(
        [docker, "context", "inspect", "--format", "{{.Endpoints.docker.Host}}"],
        env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL, timeout=5, check=True,
    ).stdout.decode().strip()
    assert endpoint.startswith("unix:///")
    command = [docker, "--host", endpoint]
    try:
        subprocess.run(
            command + payload["args"], input=payload["request"].encode(),
            env=env, stderr=subprocess.DEVNULL, timeout=60, check=True,
        )
    finally:
        subprocess.run(
            command + ["rm", "--force", payload["name"]], env=env,
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL, timeout=5, check=False,
        )


if __name__ == "__main__":
    # Finish bounded container cleanup even if the SSH client disconnects.
    signal.signal(signal.SIGHUP, signal.SIG_IGN)
    try:
        deliver(Path.home())
    except Exception:
        sys.exit(1)
