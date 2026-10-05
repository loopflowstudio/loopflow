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
    built = False
    created = False
    try:
        subprocess.run(
            ["docker", "build", "--tag", identity, "-"],
            input=context.getvalue(),
            check=True,
            timeout=600,
        )
        built = True
        subprocess.run(
            [
                "docker",
                "create",
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
            timeout=30,
        )
        created = True
        subprocess.run(["docker", "start", "--attach", identity], check=True, timeout=300)
        # Attach success alone does not establish the container's exit status.
        result = subprocess.check_output(["docker", "wait", identity], text=True, timeout=10)
        if result.strip() != "0":
            raise SystemExit(f"Capture exclusion experiment failed (exit {result.strip()}).")
    finally:
        if created:
            subprocess.run(["docker", "rm", "--force", identity], check=True, timeout=30)
        if built:
            subprocess.run(["docker", "image", "rm", identity], check=True, timeout=30)


if __name__ == "__main__":
    main()
