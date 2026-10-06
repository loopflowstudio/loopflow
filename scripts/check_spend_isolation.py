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
    args = parser.parse_args()
    executable = args.lf.resolve(strict=True)
    docker = shutil.which("docker")
    if docker is None:
        raise SystemExit("Docker unavailable; run this check on capable gate/CI")
    root = Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix="spend-isolation-") as directory:
        home = Path(directory)
        environment = {"PATH": os.defpath, "LF_HOME": str(home)}

        def run_lf(*arguments: str, administrative: bool = True) -> str:
            result = subprocess.run(
                [str(executable), *arguments],
                env=environment if administrative else {"PATH": os.defpath},
                capture_output=True,
                timeout=75,
                check=False,
            )
            if result.returncode:
                raise RuntimeError("isolated fixture CLI failed")
            return result.stdout.decode()

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
                "rust:bookworm",
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

        def consume(repo: str) -> str:
            return run_lf(
                "auth",
                "access",
                "consume",
                "--file",
                str(report),
                "--repo",
                repo,
                "--period",
                "2026-09",
                administrative=False,
            )

        consumed = json.loads(consume("example/two"))
        assert consumed["totals"][0]["billed"] == "30.00"
        try:
            consume("example/one")
        except RuntimeError:
            pass
        else:
            raise AssertionError("wrong recipient accepted")
        inventory = json.loads((root / "tests/fixtures/dto/spend/inventory.json").read_text())
        local_home = json.loads(run_lf("home", "id", "--json"))["id"]
        inventory["environments"][0]["home_id"] = local_home
        requirement = inventory["requirements"][0]
        requirement.update(
            credential=None,
            tool="auth.export",
            revision="2",
            report_consumer={"repo": "example/two", "wave_id": None},
        )
        inventory_path = home / "consumer-inventory.json"

        def import_inventory() -> None:
            inventory_path.write_text(json.dumps(inventory))
            run_lf("auth", "inventory", "import", str(inventory_path))

        def verify(export: Path) -> dict:
            return json.loads(
                run_lf(
                    "auth",
                    "access",
                    "verify",
                    "laptop",
                    "--period",
                    "2026-09",
                    "--export",
                    str(export),
                    "--json",
                )
            )

        import_inventory()
        verified = verify(report)
        observation = verified["observations"][0]
        assert observation["outcome"] == "success"
        assert observation["executed_home"] == local_home
        assert observation["scope_evidence"] is None
        receipt = observation["report_receipt"]
        assert receipt["consumer"] == requirement["report_consumer"]
        assert receipt["period"] == "2026-09"
        assert len(receipt["export_sha256"]) == 64
        again = verify(report)["observations"][0]["report_receipt"]
        assert again["invocation"] != receipt["invocation"]
        assert again["export_sha256"] == receipt["export_sha256"]
        wrong = home / "wrong.json"
        run_lf(
            "auth", "export", "--period", "2026-09", "--repo", "example/one", "--output", str(wrong)
        )
        denied = verify(wrong)["observations"][0]
        assert denied["outcome"] == "denied" and denied["report_receipt"] is None
        inspection = json.loads(run_lf("auth", "access", "show", "laptop", "--json"))
        assert any(o["report_receipt"] == receipt for o in inspection["observations"])
        inventory["environments"][0]["home_id"] = None
        import_inventory()
        remote = verify(report)["observations"][0]
        assert remote["outcome"] == "unavailable" and remote["report_receipt"] is None
        print(result.stdout.strip())
        print(
            "Environment-bound receipts: local success, wrong-recipient denial, remote unavailable"
        )


if __name__ == "__main__":
    main()
