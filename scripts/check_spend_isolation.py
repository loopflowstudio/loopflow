"""Prove designated report consumption without the administrative Home or credentials."""

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

_CONSUMER = r"""
import json
import os
from pathlib import Path
import shutil

report = json.loads(Path("/report.json").read_text())
assert report["repo"] == "example/two"
assert report["totals"][0]["billed"] == "30.00"
assert len(report["amounts"]) == 1
assert "invoices" not in report
credential_prefixes = ("DOPPLER_", "LF_", "AWS_", "ANTHROPIC_", "OPENAI_")
assert not any(k.startswith(credential_prefixes) for k in os.environ)
assert "SSH_AUTH_SOCK" not in os.environ
assert shutil.which("doppler") is None
for path in json.loads(os.environ["FORBIDDEN_PATHS"]):
    assert not Path(path).exists(), "administrative path visible"
assert not Path("/var/run/docker.sock").exists()
try:
    Path("/report.json").write_text("modified")
except OSError:
    pass
else:
    raise AssertionError("report mount is writable")
print("Designated report: USD 30.00; administrative paths and inherited authority unavailable")
"""


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lf", type=Path, default=Path("target/debug/lf"))
    parser.add_argument("--image", default="rust:bookworm")
    args = parser.parse_args()
    executable = args.lf.resolve(strict=True)
    docker = shutil.which("docker")
    if docker is None:
        raise SystemExit("Docker unavailable; run this check on capable gate/CI")
    root = Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix="spend-isolation-") as directory:
        home = Path(directory)
        environment = {"PATH": os.defpath, "LF_HOME": str(home)}

        def run_lf(*arguments: str) -> None:
            result = subprocess.run(
                [str(executable), *arguments],
                env=environment,
                capture_output=True,
                timeout=60,
                check=False,
            )
            if result.returncode:
                raise RuntimeError("isolated fixture CLI failed")

        run_lf("auth", "inventory", "import", str(root / "tests/fixtures/dto/spend/inventory.json"))
        run_lf(
            "auth",
            "source",
            "import",
            "fixture",
            "--period",
            "2026-09",
            "--file",
            str(root / "tests/fixtures/dto/spend/invoice.json"),
        )
        report = home / "report.json"
        run_lf(
            "auth",
            "export",
            "--period",
            "2026-09",
            "--repo",
            "example/two",
            "--output",
            str(report),
        )
        # Sentinel metadata is deliberately non-secret. These locations are never mounted.
        for relative in (".doppler/config.yaml", ".aws/credentials", ".ssh/agent.sock"):
            path = home / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("administrative fixture sentinel")
        forbidden = [
            str(home / name)
            for name in (
                "loopflow.db",
                ".doppler/config.yaml",
                ".aws/credentials",
                ".ssh/agent.sock",
            )
        ]
        result = subprocess.run(
            [
                docker,
                "run",
                "--rm",
                "-i",
                "--network",
                "none",
                "--read-only",
                "--cap-drop",
                "ALL",
                "--security-opt",
                "no-new-privileges",
                "--user",
                "65534:65534",
                "--pids-limit",
                "32",
                "--memory",
                "128m",
                "--mount",
                f"type=bind,source={report},target=/report.json,readonly",
                "--env",
                f"FORBIDDEN_PATHS={json.dumps(forbidden)}",
                "--entrypoint",
                "/usr/bin/python3",
                args.image,
                "-I",
                "-",
            ],
            input=_CONSUMER,
            text=True,
            capture_output=True,
            timeout=120,
            check=False,
        )
        if result.returncode:
            raise SystemExit(
                "Container isolation check failed; verify Docker and the Python-equipped image"
            )
        print(result.stdout.strip())


if __name__ == "__main__":
    main()
