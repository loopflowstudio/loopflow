"""Prove branch data isolation with the real installed and branch CLIs."""

import argparse
import hashlib
import json
import os
import pwd
import re
import sqlite3
import subprocess
import uuid
from pathlib import Path


def _schema(path: Path) -> str:
    with sqlite3.connect(f"{path.as_uri()}?mode=ro", uri=True) as connection:
        schema = connection.execute(
            "SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY name"
        ).fetchall()
        ledgers = {
            name: connection.execute(f"SELECT * FROM {name} ORDER BY 1").fetchall()
            for name in ("schema_migrations", "development_migrations")
            if connection.execute("SELECT 1 FROM sqlite_master WHERE name=?", (name,)).fetchone()
        }
    return hashlib.sha256(json.dumps([schema, ledgers]).encode()).hexdigest()


def _run(binary: Path, arguments: list[str], environment: dict[str, str]) -> str:
    result = subprocess.run(
        [str(binary), *arguments],
        env=environment,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode:
        raise RuntimeError(f"{binary.name} {' '.join(arguments)} failed: {result.stderr}")
    return result.stderr


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lf", type=Path, default=Path("target/debug/lf"))
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    branch = arguments.lf.resolve()
    branch_sha256 = hashlib.sha256(branch.read_bytes()).hexdigest()
    account_home = Path(pwd.getpwuid(os.getuid()).pw_dir)
    state = json.loads((account_home / ".lf-machine/install/active.json").read_text())
    selection = state["selection"]
    installed = Path(
        next(
            artifact["path"]
            for artifact in selection["artifact_set"]["artifacts"]
            if artifact["role"] == {"kind": "cli"}
        )
    )
    source = Path(selection["store"])
    environment = dict(os.environ)
    environment.update(
        {
            "LF_HOME": str(source.parent),
            "LF_DB_PATH": str(source),
            "LF_CONTROL_HOME": str(source.parent),
            "LF_CONTROL_DB_PATH": str(source),
        }
    )
    before = _schema(source)
    report = _run(branch, ["home", "id", "--json"], environment)
    match = re.search(r"Branch lf is using data directory (.*?); installed store", report)
    if match is None:
        raise RuntimeError("branch did not report isolation; no write attempted")
    destination = Path(match[1]) / "loopflow.db"
    if source.samefile(destination):
        raise RuntimeError("branch reported the installed store; no write attempted")
    control_only = dict(environment)
    control_only.pop("LF_HOME")
    control_only.pop("LF_DB_PATH")
    if match[0] not in _run(branch, ["home", "id", "--json"], control_only):
        raise RuntimeError("control-only inheritance selected a different data directory")
    explicit_home = dict(environment)
    explicit_home["LF_HOME"] = str(destination.parent)
    explicit_home.pop("LF_DB_PATH")
    if "Branch lf is using data directory" in _run(branch, ["home", "id", "--json"], explicit_home):
        raise RuntimeError(
            "an explicit private data directory was overridden by inherited control pins"
        )
    marker = f"home_{uuid.uuid4().hex}"
    route = f"ssh://proof@{marker}.invalid"
    _run(branch, ["home", "observe", marker, route, "--json"], environment)
    _run(branch, ["home", "id", "--json"], environment)
    for path, expected in [(destination, (route,)), (source, None)]:
        with sqlite3.connect(f"{path.as_uri()}?mode=ro", uri=True) as connection:
            actual = connection.execute("SELECT route FROM homes WHERE id=?", (marker,)).fetchone()
        if actual != expected:
            raise RuntimeError(f"Home observation mismatch in {path}")
    _run(installed, ["home", "id", "--json"], environment)
    after = _schema(source)
    if before != after:
        raise RuntimeError("installed schema or migration receipts changed during proof")
    if hashlib.sha256(branch.read_bytes()).hexdigest() != branch_sha256:
        raise RuntimeError("branch executable changed during proof; finish all builds and retry")
    receipt = {
        "branch": str(branch),
        "branch_sha256": branch_sha256,
        "installed": str(installed),
        "installed_store": str(source),
        "branch_store": str(destination),
        "installed_schema_and_receipts": after,
        "branch_observation": marker,
        "result": "passed",
        "scope": "Real CLI snapshot and private database write; no Task worker or promotion proof.",
    }
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
