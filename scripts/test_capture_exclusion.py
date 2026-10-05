"""Run the released-writer exclusion experiment without host mounts or credentials."""

import io
import subprocess
import tarfile
import uuid
from pathlib import Path


def main() -> None:
    try:
        subprocess.run(["docker", "info", "--format", "{{.ServerVersion}}"], check=True, timeout=10)
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, FileNotFoundError):
        raise SystemExit("Docker is unavailable; no exclusion experiment was run.") from None

    fixture = Path(__file__).resolve().parents[1] / "tests/e2e"
    # Send only the two fixture sources. Neither Docker's context nor the running
    # container receives a host Home, installation, environment or credentials.
    context = io.BytesIO()
    with tarfile.open(fileobj=context, mode="w") as archive:
        archive.add(fixture / "capture_exclusion.Dockerfile", arcname="Dockerfile")
        archive.add(fixture / "capture_exclusion.py", arcname="capture_exclusion.py")
    identity = f"lf-capture-exclusion-{uuid.uuid4().hex}"
    subprocess.run(
        ["docker", "build", "--tag", identity, "-"],
        input=context.getvalue(),
        check=True,
        timeout=600,
    )
    try:
        # Foreground run propagates the experiment's exit status directly.
        subprocess.run(
            [
                "docker",
                "run",
                "--name",
                identity,
                "--network",
                "none",
                "--cap-drop",
                "ALL",
                "--cap-add",
                "CHOWN",
                "--cap-add",
                "DAC_OVERRIDE",
                "--cap-add",
                "FOWNER",
                "--cap-add",
                "SETUID",
                "--cap-add",
                "SETGID",
                "--security-opt",
                "no-new-privileges",
                "--pids-limit",
                "128",
                identity,
            ],
            check=True,
            timeout=300,
        )
    finally:
        try:
            subprocess.run(["docker", "rm", "--force", identity], check=True, timeout=30)
        finally:
            subprocess.run(["docker", "image", "rm", identity], check=True, timeout=30)


if __name__ == "__main__":
    main()
