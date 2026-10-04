import json
import re
import subprocess
from pathlib import Path

import pytest


@pytest.mark.parametrize(
    ("rows", "accepted"),
    [
        ([{"scope": "managed", "verification": "accepted"}], True),
        ([{"scope": "managed", "verification": "rejected"}], False),
        ([{"scope": "managed", "verification": "unavailable"}], False),
        ([{"scope": "local", "verification": "accepted"}], False),
        ([{"scope": "forwarded", "verification": "accepted"}], False),
        ([], False),
    ],
)
def test_cron_requires_accepted_managed_evidence(
    rows: list[dict[str, str]], accepted: bool
) -> None:
    script = (Path(__file__).resolve().parents[2] / "scripts/bootstrap-cron-host.sh").read_text()
    expression = re.search(r"jq -e '([^']+)'", script)
    assert expression is not None
    result = subprocess.run(
        ["jq", "-e", expression.group(1)],
        input=json.dumps({"accounts": rows}),
        text=True,
        capture_output=True,
        check=False,
    )
    assert (result.returncode == 0) == accepted
