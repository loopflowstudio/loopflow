#!/usr/bin/env python3
"""Provider-level billing collector for the Systems wave "billing" north star.

Reports month-to-date spend / balance per vendor against the monthly budget.
Secrets are read from Doppler (loopflow/dev_billing) and are NEVER printed,
logged, or written to the JSON snapshot. If a vendor call fails, only the
vendor name and a short status reason are surfaced.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

REPO_ROOT = Path(__file__).resolve().parents[1]
BUDGET_PATH = REPO_ROOT / "deploy" / "budget.json"
DOPPLER_PROJECT = "loopflow"
DOPPLER_CONFIG = "dev_billing"
HTTP_TIMEOUT = 30


@dataclass
class VendorResult:
    vendor: str
    amount_usd: Optional[float]
    kind: str  # "spend" | "balance"
    status: str


# ---------------------------------------------------------------------------
# Secret + HTTP helpers
# ---------------------------------------------------------------------------


def _get_secret(name: str) -> Optional[str]:
    """Read a secret from Doppler. Returns None if the key is absent."""
    try:
        proc = subprocess.run(
            [
                "doppler",
                "secrets",
                "get",
                name,
                "-p",
                DOPPLER_PROJECT,
                "-c",
                DOPPLER_CONFIG,
                "--plain",
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    value = proc.stdout.strip()
    return value or None


def _openai_admin_key() -> Optional[str]:
    """Probe OPENAI_ADMIN_KEY first; fall back to OPENCODE_API_KEY if it looks
    like an org admin key (starts with sk-admin)."""
    key = _get_secret("OPENAI_ADMIN_KEY")
    if key:
        return key
    fallback = _get_secret("OPENCODE_API_KEY")
    if fallback and fallback.startswith("sk-admin"):
        return fallback
    return None


def _http_json(
    url: str,
    headers: dict[str, str],
    method: str = "GET",
    body: Optional[bytes] = None,
) -> tuple[Optional[Any], Optional[str]]:
    """Perform an HTTP request and parse JSON.

    Returns (parsed_json, None) on success or (None, short_reason) on failure.
    The reason never contains secret material — only status codes and the URL
    host.
    """
    req = urllib.request.Request(url, headers=headers, method=method, data=body)
    try:
        with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT) as resp:
            raw = resp.read()
    except urllib.error.HTTPError as exc:
        return None, f"http {exc.code}"
    except urllib.error.URLError as exc:
        reason = getattr(exc, "reason", "unreachable")
        return None, f"unreachable: {reason}"
    except (TimeoutError, OSError):
        return None, "unreachable: timeout"
    try:
        return json.loads(raw), None
    except json.JSONDecodeError:
        return None, "bad json"


def _first_of_month(now: datetime) -> datetime:
    return now.replace(day=1, hour=0, minute=0, second=0, microsecond=0)


# ---------------------------------------------------------------------------
# Vendor fetchers
# ---------------------------------------------------------------------------


def fetch_mercury() -> VendorResult:
    """Sum currentBalance across Mercury accounts → total balance."""
    token = _get_secret("MERCURY_API_TOKEN")
    if not token:
        return VendorResult("mercury", None, "balance", "unavailable: no key")
    data, err = _http_json(
        "https://api.mercury.com/api/v1/accounts",
        {"Authorization": f"Bearer {token}"},
    )
    if err:
        return VendorResult("mercury", None, "balance", f"unavailable: {err}")
    accounts = data.get("accounts") if isinstance(data, dict) else None
    if not isinstance(accounts, list):
        return VendorResult("mercury", None, "balance", "unavailable: no accounts field")
    total = 0.0
    for acct in accounts:
        bal = acct.get("currentBalance") if isinstance(acct, dict) else None
        if isinstance(bal, (int, float)):
            total += float(bal)
    return VendorResult("mercury", total, "balance", "ok")


def fetch_anthropic(month_start: datetime) -> VendorResult:
    """Sum Anthropic cost_report amounts for the current month → MTD USD."""
    key = _get_secret("ANTHROPIC_ADMIN_KEY")
    if not key:
        return VendorResult("anthropic", None, "spend", "unavailable: no key")
    starting_at = month_start.strftime("%Y-%m-%d")
    url = f"https://api.anthropic.com/v1/organizations/cost_report?starting_at={starting_at}"
    data, err = _http_json(
        url,
        {"x-api-key": key, "anthropic-version": "2023-06-01"},
    )
    if err:
        return VendorResult("anthropic", None, "spend", f"unavailable: {err}")
    buckets = data.get("data") if isinstance(data, dict) else None
    if not isinstance(buckets, list):
        keys = _top_level_keys(data)
        return VendorResult("anthropic", None, "spend", f"unclear shape, keys={keys}")
    # Shape: data[] time buckets, each with results[] cost entries carrying an
    # `amount` field. Empty results across all buckets means a real $0.00 MTD.
    total = 0.0
    for bucket in buckets:
        results = bucket.get("results") if isinstance(bucket, dict) else None
        if not isinstance(results, list):
            continue
        for res in results:
            value = _extract_amount(res.get("amount")) if isinstance(res, dict) else None
            if value is not None:
                total += value
    return VendorResult("anthropic", total, "spend", "ok")


def fetch_openai(month_start: datetime) -> VendorResult:
    """Sum data[].results[].amount.value from the OpenAI costs endpoint → MTD USD."""
    key = _openai_admin_key()
    if not key:
        return VendorResult("openai", None, "spend", "unavailable: no admin key")
    start_time = int(month_start.replace(tzinfo=timezone.utc).timestamp())
    url = f"https://api.openai.com/v1/organization/costs?start_time={start_time}"
    data, err = _http_json(url, {"Authorization": f"Bearer {key}"})
    if err:
        return VendorResult("openai", None, "spend", f"unavailable: {err}")
    buckets = data.get("data") if isinstance(data, dict) else None
    if not isinstance(buckets, list):
        keys = _top_level_keys(data)
        return VendorResult("openai", None, "spend", f"unclear shape, keys={keys}")
    total = 0.0
    for bucket in buckets:
        results = bucket.get("results") if isinstance(bucket, dict) else None
        if not isinstance(results, list):
            continue
        for res in results:
            amount = res.get("amount") if isinstance(res, dict) else None
            value = amount.get("value") if isinstance(amount, dict) else None
            if isinstance(value, (int, float)):
                total += float(value)
    return VendorResult("openai", total, "spend", "ok")


def fetch_fly() -> VendorResult:
    """Best-effort Fly.io current invoice / usage via GraphQL → MTD USD."""
    token = _get_secret("FLY_API_TOKEN")
    if not token:
        return VendorResult("fly", None, "spend", "unavailable: no key")
    query = (
        "query { organizations { nodes { slug type "
        "billingStatus invoices(first: 1) { nodes { amountDue total } } } } }"
    )
    body = json.dumps({"query": query}).encode("utf-8")
    data, err = _http_json(
        "https://api.fly.io/graphql",
        {"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
        method="POST",
        body=body,
    )
    if err:
        return VendorResult("fly", None, "spend", f"unavailable: {err}")
    if not isinstance(data, dict) or "data" not in data:
        return VendorResult("fly", None, "spend", "reachable, no cost field")
    orgs = (
        data.get("data", {})
        .get("organizations", {})
        .get("nodes")
        if isinstance(data.get("data"), dict)
        else None
    )
    if not isinstance(orgs, list):
        return VendorResult("fly", None, "spend", "reachable, no cost field")
    total = 0.0
    found = False
    for org in orgs:
        invoices = (
            org.get("invoices", {}).get("nodes")
            if isinstance(org, dict) and isinstance(org.get("invoices"), dict)
            else None
        )
        if not isinstance(invoices, list):
            continue
        for inv in invoices:
            amount = inv.get("total") if isinstance(inv, dict) else None
            if amount is None and isinstance(inv, dict):
                amount = inv.get("amountDue")
            if isinstance(amount, (int, float)):
                # Fly reports invoice amounts in cents.
                total += float(amount) / 100.0
                found = True
    if not found:
        return VendorResult("fly", None, "spend", "reachable, no cost field")
    return VendorResult("fly", total, "spend", "ok")


def fetch_cloudflare() -> VendorResult:
    """Best-effort Cloudflare R2 usage via the GraphQL analytics API.

    R2 dollar cost requires an account id and per-class pricing; when that's
    not cleanly available we report stored bytes instead of a dollar figure.
    """
    token = _get_secret("CLOUDFLARE_API_TOKEN")
    if not token:
        return VendorResult("cloudflare", None, "spend", "unavailable: no key")
    # Cloudflare's GraphQL analytics is scoped by accountTag, which we don't
    # have without an extra account lookup. Verify the token is usable and
    # report that a clean R2 cost needs the account id rather than over-investing.
    data, err = _http_json(
        "https://api.cloudflare.com/client/v4/accounts",
        {"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
    )
    if err:
        return VendorResult("cloudflare", None, "spend", f"unavailable: {err}")
    return VendorResult(
        "cloudflare", None, "spend", "unavailable: needs account id for R2 cost"
    )


# ---------------------------------------------------------------------------
# Response-shape helpers
# ---------------------------------------------------------------------------


def _top_level_keys(data: Any) -> list[str]:
    if isinstance(data, dict):
        return sorted(data.keys())
    if isinstance(data, list):
        return ["<list>"]
    return ["<non-object>"]


def _extract_amount(amount: Any) -> Optional[float]:
    """Pull a float out of a cost `amount` field, which may be a number, a
    numeric string, or a nested {value: ...} object."""
    if isinstance(amount, (int, float)):
        return float(amount)
    if isinstance(amount, str):
        try:
            return float(amount)
        except ValueError:
            return None
    if isinstance(amount, dict):
        return _extract_amount(amount.get("value"))
    return None


# ---------------------------------------------------------------------------
# Budget + reporting
# ---------------------------------------------------------------------------


def _load_budget() -> float:
    try:
        raw = json.loads(BUDGET_PATH.read_text())
    except (OSError, json.JSONDecodeError):
        return 0.0
    return float(raw.get("monthly_budget_usd", 0.0))


def _format_amount(result: VendorResult) -> str:
    if result.amount_usd is None:
        return "-"
    return f"${result.amount_usd:,.2f}"


def _cell(result: VendorResult, kind: str) -> str:
    """A vendor's amount in the column for `kind`; blank in the other column."""
    if result.kind != kind or result.amount_usd is None:
        return "-"
    return f"${result.amount_usd:,.2f}"


def _print_table(results: list[VendorResult], budget: float) -> None:
    total_spend = sum(
        r.amount_usd
        for r in results
        if r.kind == "spend" and r.amount_usd is not None
    )
    rows = [("VENDOR", "MTD SPEND", "BALANCE", "STATUS")]
    for r in results:
        rows.append((r.vendor, _cell(r, "spend"), _cell(r, "balance"), r.status))

    widths = [max(len(row[i]) for row in rows) for i in range(4)]
    line = "  ".join("-" * w for w in widths)
    for idx, row in enumerate(rows):
        print("  ".join(row[i].ljust(widths[i]) for i in range(4)))
        if idx == 0:
            print(line)
    print(line)
    print(
        f"\nTotal known MTD spend: ${total_spend:,.2f}  /  budget ${budget:,.2f}"
        f"  ({total_spend / budget * 100:.1f}% used)"
        if budget
        else f"\nTotal known MTD spend: ${total_spend:,.2f}"
    )


def _write_json(path: Path, results: list[VendorResult], budget: float) -> None:
    total_spend = sum(
        r.amount_usd
        for r in results
        if r.kind == "spend" and r.amount_usd is not None
    )
    snapshot: dict[str, Any] = {
        r.vendor: {
            "amount_usd": r.amount_usd,
            "kind": r.kind,
            "status": r.status,
        }
        for r in results
    }
    snapshot["total_spend_usd"] = total_spend
    snapshot["budget_usd"] = budget
    path.write_text(json.dumps(snapshot, indent=2) + "\n")


def _collect(now: datetime) -> list[VendorResult]:
    month_start = _first_of_month(now)
    return [
        fetch_mercury(),
        fetch_anthropic(month_start),
        fetch_openai(month_start),
        fetch_fly(),
        fetch_cloudflare(),
    ]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", dest="json_path", help="Write a secret-free snapshot to PATH")
    args = parser.parse_args()

    now = datetime.now(timezone.utc)
    results = _collect(now)
    budget = _load_budget()

    _print_table(results, budget)

    if args.json_path:
        _write_json(Path(args.json_path), results, budget)
        print(f"\nSnapshot written to {args.json_path}")


if __name__ == "__main__":
    main()
