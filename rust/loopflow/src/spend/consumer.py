"""Embedded isolated report reader; stdout is a receipt on the parent's private pipe."""

import hashlib
import json
import os
import sys
from pathlib import Path

request = json.load(sys.stdin)
receipt = request["receipt"]
raw = request["report"].encode()
report = json.loads(raw)
assert hashlib.sha256(raw).hexdigest() == receipt["export_sha256"]
assert report["repo"] == receipt["consumer"]["repo"]
assert report["wave_id"] == receipt["consumer"]["wave_id"]
assert report["period"] == receipt["period"]
assert set(report) == {"period", "repo", "wave_id", "generated_at", "amounts", "totals", "coverage"}
for amount in report["amounts"]:
    assert set(amount) == {
        "source",
        "document_id",
        "revision",
        "fetched_at",
        "generated_at",
        "currency",
        "kind",
        "amount",
        "wave_id",
        "rule",
        "rule_revision",
    }
    assert report["wave_id"] is None or amount["wave_id"] == report["wave_id"]
assert not any(
    key.startswith(("LF_", "DOPPLER_", "AWS_", "OPENAI_", "ANTHROPIC_")) for key in os.environ
)
assert "SSH_AUTH_SOCK" not in os.environ
assert not Path("/var/run/docker.sock").exists()
assert not os.access("/root/.doppler", os.R_OK)
assert not os.access("/root/.aws", os.R_OK)
print(json.dumps(receipt))
