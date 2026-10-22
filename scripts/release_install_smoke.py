"""Verify a pinned public CLI installation inside a disposable OS container."""

import hashlib
import json
import subprocess
import sys
import tarfile
from pathlib import Path


def verify_installation(directory: Path, home: Path, tag: str) -> dict[str, str]:
    archive = directory / "lf-aarch64-unknown-linux-gnu.tar.gz"
    with tarfile.open(archive, "r:gz") as package:
        members = package.getmembers()
        if len(members) != 1 or members[0].name != "lf" or not members[0].isfile():
            raise RuntimeError("unexpected public Linux package contents")
        source = package.extractfile(members[0])
        if source is None:
            raise RuntimeError("public Linux package has no CLI")
        expected = hashlib.sha256(source.read()).hexdigest()
    active = json.loads((home / ".lf-machine/install/active.json").read_text())
    artifacts = active["selection"]["artifact_set"]["artifacts"]
    cli = [artifact for artifact in artifacts if artifact["role"] == {"kind": "cli"}]
    if len(cli) != 1:
        raise RuntimeError("installation did not select one CLI")
    if (
        cli[0]["sha256"] != expected
        or hashlib.sha256(Path(cli[0]["path"]).read_bytes()).hexdigest() != expected
    ):
        raise RuntimeError("installed CLI differs from the exact public artifact")
    entry = home / ".local/bin/lf"
    reported = subprocess.check_output([str(entry), "--version"], text=True).strip()
    if reported != f"lf {tag.removeprefix('v')}":
        raise RuntimeError(f"installed CLI reported {reported!r}, expected {tag}")
    subprocess.run([str(entry), "--help"], check=True, capture_output=True)
    listing = subprocess.check_output([str(entry), "list", "--json"], text=True)
    json.loads(listing)
    return {"lf-linux": reported}


if __name__ == "__main__":
    print(json.dumps(verify_installation(Path("/proof"), Path.home(), sys.argv[1])))
