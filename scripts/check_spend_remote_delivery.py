"""Exercise the remote controller with synthetic Home identity and real Docker isolation."""

import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import uuid
from pathlib import Path


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    docker = shutil.which("docker")
    if docker is None:
        raise SystemExit("Docker unavailable; run on capable gate/CI")
    endpoint = subprocess.check_output(
        [docker, "context", "inspect", "--format", "{{.Endpoints.docker.Host}}"],
        env={"PATH": os.defpath}, timeout=5,
    ).decode().strip()
    if not endpoint.startswith("unix:///"):
        raise SystemExit("A local Docker socket is required")
    consumer = (root / "rust/loopflow/src/spend/consumer.py").read_text()
    controller = root / "rust/loopflow/src/spend/remote_consumer.py"
    with tempfile.TemporaryDirectory(prefix="spend-remote-") as directory:
        home = Path(directory)
        identity = "home_" + uuid.uuid4().hex
        lf = home / ".local/bin/lf"
        lf.parent.mkdir(parents=True)
        lf.write_text("#!/bin/sh\nprintf '%s\\n' '" + json.dumps({"id": identity}) + "'\n")
        lf.chmod(0o700)
        config = home / ".docker"
        subprocess.run(
            [docker, "--config", str(config), "context", "create", "fixture", "--docker", f"host={endpoint}"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5, check=True,
        )
        (config / "config.json").write_text(json.dumps({"currentContext": "fixture"}))
        raw = json.dumps({"period": "2026-09", "repo": "example/two", "wave_id": None,
                          "generated_at": 0, "amounts": [], "totals": [], "coverage": []})
        receipt = {"invocation": str(uuid.uuid4()), "binding": {"home": identity,
                   "requirement": "fixture", "revision": "r1"}, "consumer": {
                   "repo": "example/two", "wave_id": None}, "period": "2026-09",
                   "export_sha256": hashlib.sha256(raw.encode()).hexdigest()}
        name = "lf-spend-" + receipt["invocation"]
        payload = {"home": identity, "name": name, "args": [
            "run", "--rm", "--pull", "never", "--name", name, "-i", "--network", "none",
            "--read-only", "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
            "--user", "65534:65534", "--pids-limit", "32", "--memory", "128m",
            "--entrypoint", "/usr/bin/python3", "rust:bookworm", "-I", "-c", consumer],
            "request": json.dumps({"receipt": receipt, "report": raw})}
        script = controller.read_text().split('if __name__ == "__main__":')[0]
        script += "\ndeliver(Path(" + repr(str(home)) + "))\n"
        for remote_home, succeeds in ((identity, True), ("home_" + uuid.uuid4().hex, False)):
            payload["home"] = remote_home
            result = subprocess.run(
                ["/usr/bin/python3", "-I", "-c", script], input=json.dumps(payload).encode(),
                env={"PATH": os.defpath, "DOPPLER_FIXTURE": "non-secret-sentinel"},
                capture_output=True, timeout=85,
            )
            lines = result.stdout.splitlines()
            assert json.loads(lines[0]) == identity
            if succeeds:
                assert result.returncode == 0, "remote controller failed"
                assert json.loads(lines[1]) == receipt
            else:
                assert result.returncode != 0 and len(lines) == 1
        print("Remote controller: isolated consumption passed; mismatched Home rejected (synthetic identity, no SSH authentication exercised)")


if __name__ == "__main__":
    main()
