import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/check_monthly_spend.py"
CONFIG = ROOT / "deploy/budget.json"


def run_budget(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), "--config", str(CONFIG), *args],
        text=True,
        capture_output=True,
    )


def test_budget_guardrail_allows_spend_under_projected_budget(tmp_path: Path):
    export = tmp_path / "mercury.csv"
    export.write_text(
        "Date,Description,Amount\n"
        "2026-06-01,Amazon Web Services,10.00\n"
        "2026-06-02,Fly.io,5.00\n"
        "2026-06-03,Unrelated coffee,7.00\n"
    )

    result = run_budget("--csv", str(export), "--month", "2026-06", "--today", "2026-06-30")

    assert result.returncode == 0
    assert "budget: $100.00" in result.stdout
    assert "actual tracked spend: $15.00" in result.stdout
    assert "verdict: OK" in result.stdout
    assert "Unrelated coffee" not in result.stdout


def test_budget_guardrail_blocks_projected_spend_over_budget(tmp_path: Path):
    export = tmp_path / "mercury.csv"
    export.write_text("Date,Description,Amount\n2026-06-15,Anthropic Claude,55.00\n")

    result = run_budget("--csv", str(export), "--month", "2026-06", "--today", "2026-06-15")

    assert result.returncode == 2
    assert "actual tracked spend: $55.00" in result.stdout
    assert "projected spend: $110.00" in result.stdout
    assert "verdict: BLOCK" in result.stdout
    assert "stop and get human approval" in result.stderr


def test_budget_guardrail_supports_debit_credit_exports(tmp_path: Path):
    export = tmp_path / "mercury.csv"
    export.write_text(
        "Date,Description,Debit,Credit\n"
        "2026-06-01,OpenAI Codex,30.00,\n"
        "2026-06-03,OpenAI refund,,5.00\n"
    )

    result = run_budget(
        "--csv",
        str(export),
        "--month",
        "2026-06",
        "--today",
        "2026-06-30",
        "--json-output",
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["actual"] == "25.00"
    assert payload["over_budget"] is False


def test_budget_guardrail_supports_transaction_json(tmp_path: Path):
    export = tmp_path / "transactions.json"
    export.write_text(
        json.dumps(
            {
                "transactions": [
                    {
                        "postedAt": "2026-06-04T12:00:00Z",
                        "merchantName": "Fly.io",
                        "amount": {"amount": "12.34"},
                    }
                ]
            }
        )
    )

    result = run_budget(
        "--json",
        str(export),
        "--month",
        "2026-06",
        "--today",
        "2026-06-30",
        "--json-output",
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["actual"] == "12.34"
    assert payload["matched"][0]["vendor"] == "fly"
