#!/usr/bin/env python3
from __future__ import annotations

import argparse
import calendar
import csv
import json
import re
import sys
from dataclasses import dataclass
from datetime import date, datetime
from decimal import ROUND_HALF_UP, Decimal, InvalidOperation
from pathlib import Path
from typing import Any, Iterable, Optional

REPO_ROOT = Path(__file__).resolve().parents[1]
MONEY_QUANT = Decimal("0.01")
DATE_FIELDS = ["date", "posted_at", "postedat", "created_at", "createdat", "timestamp"]
TEXT_FIELDS = [
    "description",
    "merchant",
    "merchant_name",
    "merchantname",
    "counterparty",
    "counterparty_name",
    "counterpartyname",
    "name",
    "memo",
    "note",
]
DEBIT_FIELDS = ["debit", "withdrawal", "withdrawals", "charge", "charges", "spent", "expense"]
CREDIT_FIELDS = ["credit", "deposit", "deposits", "refund", "payment"]
AMOUNT_FIELDS = ["amount", "transaction_amount", "transactionamount", "net_amount", "netamount"]


@dataclass(frozen=True)
class Vendor:
    name: str
    label: str
    match: tuple[str, ...]


@dataclass(frozen=True)
class Config:
    monthly_budget: Decimal
    currency: str
    projection: bool
    vendors: tuple[Vendor, ...]


@dataclass(frozen=True)
class Transaction:
    occurred_on: date
    description: str
    amount: Decimal
    source: str


@dataclass(frozen=True)
class MatchedSpend:
    vendor: Vendor
    transaction: Transaction


@dataclass(frozen=True)
class Report:
    month: str
    budget: Decimal
    actual: Decimal
    projected: Decimal
    projection_enabled: bool
    matched: tuple[MatchedSpend, ...]
    uncategorized: tuple[Transaction, ...]

    @property
    def gate_amount(self) -> Decimal:
        if self.projection_enabled:
            return max(self.actual, self.projected)
        return self.actual

    @property
    def over_budget(self) -> bool:
        return self.gate_amount > self.budget


def _money(value: Decimal) -> str:
    return f"${value.quantize(MONEY_QUANT, rounding=ROUND_HALF_UP)}"


def _normalize_key(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", value.strip().lower()).strip("_")


def _normalize_row(row: dict[str, Any]) -> dict[str, Any]:
    return {_normalize_key(str(key)): value for key, value in row.items()}


def _parse_decimal(value: Any) -> Optional[Decimal]:
    if value is None:
        return None
    if isinstance(value, dict):
        for key in ("amount", "value", "cents"):
            parsed = _parse_decimal(value.get(key))
            if parsed is not None:
                if key == "cents":
                    return parsed / Decimal(100)
                return parsed
        return None
    raw = str(value).strip()
    if not raw:
        return None
    negative = raw.startswith("(") and raw.endswith(")")
    cleaned = raw.strip("()")
    cleaned = cleaned.replace("$", "").replace(",", "")
    cleaned = re.sub(r"[^0-9+\-.]", "", cleaned)
    if cleaned in {"", "+", "-", "."}:
        return None
    try:
        amount = Decimal(cleaned)
    except InvalidOperation:
        return None
    if negative:
        amount = -amount
    return amount


def _parse_date(value: Any) -> Optional[date]:
    if value is None:
        return None
    raw = str(value).strip()
    if not raw:
        return None
    raw = raw[:10]
    for fmt in ("%Y-%m-%d", "%m/%d/%Y", "%m/%d/%y"):
        try:
            return datetime.strptime(raw, fmt).date()
        except ValueError:
            continue
    return None


def _first(row: dict[str, Any], keys: Iterable[str]) -> Optional[Any]:
    for key in keys:
        if key in row and str(row[key]).strip():
            return row[key]
    return None


def _description(row: dict[str, Any]) -> str:
    parts = [str(row[key]).strip() for key in TEXT_FIELDS if key in row and str(row[key]).strip()]
    if parts:
        return " | ".join(parts)
    values = [str(value).strip() for value in row.values() if str(value).strip()]
    return " | ".join(values)


def _spend_amount(row: dict[str, Any]) -> Optional[Decimal]:
    debit = _parse_decimal(_first(row, DEBIT_FIELDS))
    credit = _parse_decimal(_first(row, CREDIT_FIELDS))
    if debit is not None:
        return abs(debit)
    if credit is not None:
        return -abs(credit)

    amount = _parse_decimal(_first(row, AMOUNT_FIELDS))
    if amount is None:
        return None
    return abs(amount)


def _load_config(path: Path) -> Config:
    data = json.loads(path.read_text())
    raw_budget = data.get("monthly_budget_usd")
    budget = _parse_decimal(raw_budget)
    if budget is None or budget <= 0:
        raise ValueError(f"invalid monthly_budget_usd in {path}")

    vendors = []
    for raw_vendor in data.get("vendors", []):
        name = str(raw_vendor["name"])
        label = str(raw_vendor.get("label", name))
        match = tuple(str(term).lower() for term in raw_vendor.get("match", []))
        if not match:
            raise ValueError(f"vendor {name} has no match terms")
        vendors.append(Vendor(name=name, label=label, match=match))

    if not vendors:
        raise ValueError(f"no vendors configured in {path}")

    return Config(
        monthly_budget=budget,
        currency=str(data.get("currency", "USD")),
        projection=bool(data.get("projection", True)),
        vendors=tuple(vendors),
    )


def _load_csv(path: Path) -> list[Transaction]:
    transactions = []
    with path.open(newline="") as handle:
        reader = csv.DictReader(handle)
        for raw_row in reader:
            row = _normalize_row(raw_row)
            occurred_on = _parse_date(_first(row, DATE_FIELDS))
            amount = _spend_amount(row)
            if occurred_on is None or amount is None or amount == 0:
                continue
            transactions.append(
                Transaction(
                    occurred_on=occurred_on,
                    description=_description(row),
                    amount=amount,
                    source=str(path),
                )
            )
    return transactions


def _json_items(data: Any) -> list[dict[str, Any]]:
    if isinstance(data, list):
        return [item for item in data if isinstance(item, dict)]
    if isinstance(data, dict):
        for key in ("transactions", "data", "items", "results"):
            value = data.get(key)
            if isinstance(value, list):
                return [item for item in value if isinstance(item, dict)]
    return []


def _load_json(path: Path) -> list[Transaction]:
    transactions = []
    for raw_row in _json_items(json.loads(path.read_text())):
        row = _normalize_row(raw_row)
        occurred_on = _parse_date(_first(row, DATE_FIELDS))
        amount = _spend_amount(row)
        if occurred_on is None or amount is None or amount == 0:
            continue
        transactions.append(
            Transaction(
                occurred_on=occurred_on,
                description=_description(row),
                amount=amount,
                source=str(path),
            )
        )
    return transactions


def _month_for_today(today: date) -> str:
    return today.strftime("%Y-%m")


def _in_month(value: date, month: str) -> bool:
    return value.strftime("%Y-%m") == month


def _project(actual: Decimal, month: str, today: date) -> Decimal:
    year, month_num = [int(part) for part in month.split("-", maxsplit=1)]
    days = calendar.monthrange(year, month_num)[1]
    if today.strftime("%Y-%m") != month:
        return actual
    elapsed = max(today.day, 1)
    return (actual * Decimal(days) / Decimal(elapsed)).quantize(MONEY_QUANT)


def _classify(config: Config, transaction: Transaction) -> Optional[Vendor]:
    text = transaction.description.lower()
    for vendor in config.vendors:
        if any(term in text for term in vendor.match):
            return vendor
    return None


def build_report(
    config: Config,
    transactions: Iterable[Transaction],
    month: str,
    today: date,
    projection: bool,
) -> Report:
    matched = []
    uncategorized = []
    for transaction in transactions:
        if not _in_month(transaction.occurred_on, month):
            continue
        vendor = _classify(config, transaction)
        if vendor is None:
            uncategorized.append(transaction)
            continue
        matched.append(MatchedSpend(vendor=vendor, transaction=transaction))

    actual = sum((item.transaction.amount for item in matched), Decimal("0.00"))
    projected = _project(actual, month, today) if projection else actual
    return Report(
        month=month,
        budget=config.monthly_budget,
        actual=actual,
        projected=projected,
        projection_enabled=projection,
        matched=tuple(matched),
        uncategorized=tuple(uncategorized),
    )


def _render_text(report: Report) -> str:
    lines = [
        f"month: {report.month}",
        f"budget: {_money(report.budget)}",
        f"actual tracked spend: {_money(report.actual)}",
    ]
    if report.projection_enabled:
        lines.append(f"projected spend: {_money(report.projected)}")
    lines.append(f"gate amount: {_money(report.gate_amount)}")
    lines.append(f"verdict: {'BLOCK' if report.over_budget else 'OK'}")

    totals: dict[str, Decimal] = {}
    labels: dict[str, str] = {}
    for item in report.matched:
        totals[item.vendor.name] = (
            totals.get(item.vendor.name, Decimal("0.00")) + item.transaction.amount
        )
        labels[item.vendor.name] = item.vendor.label
    if totals:
        lines.append("")
        lines.append("by vendor:")
        for name, total in sorted(totals.items()):
            lines.append(f"  {labels[name]}: {_money(total)}")
    return "\n".join(lines)


def _render_json(report: Report) -> str:
    payload = {
        "month": report.month,
        "budget": str(report.budget.quantize(MONEY_QUANT)),
        "actual": str(report.actual.quantize(MONEY_QUANT)),
        "projected": str(report.projected.quantize(MONEY_QUANT)),
        "gate_amount": str(report.gate_amount.quantize(MONEY_QUANT)),
        "over_budget": report.over_budget,
        "matched": [
            {
                "vendor": item.vendor.name,
                "label": item.vendor.label,
                "date": item.transaction.occurred_on.isoformat(),
                "amount": str(item.transaction.amount.quantize(MONEY_QUANT)),
                "description": item.transaction.description,
                "source": item.transaction.source,
            }
            for item in report.matched
        ],
    }
    return json.dumps(payload, indent=2, sort_keys=True)


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Check monthly automation spend against budget.")
    parser.add_argument("--config", type=Path, default=REPO_ROOT / "deploy/budget.json")
    parser.add_argument("--csv", type=Path, action="append", default=[])
    parser.add_argument("--json", type=Path, action="append", default=[])
    parser.add_argument("--month", help="Month to check, YYYY-MM. Defaults to current month.")
    parser.add_argument("--today", help="Override today's date for projection tests, YYYY-MM-DD.")
    parser.add_argument("--no-projection", action="store_true")
    parser.add_argument("--json-output", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = _parse_args()
    today = _parse_date(args.today) if args.today else date.today()
    if today is None:
        raise ValueError("--today must be YYYY-MM-DD")

    config = _load_config(args.config)
    month = args.month or _month_for_today(today)
    projection = config.projection and not args.no_projection

    transactions = []
    for path in args.csv:
        transactions.extend(_load_csv(path))
    for path in args.json:
        transactions.extend(_load_json(path))

    report = build_report(config, transactions, month, today, projection)
    print(_render_json(report) if args.json_output else _render_text(report))
    if report.over_budget:
        print(
            "monthly automation spend exceeds "
            f"{_money(report.budget)}; stop and get human approval",
            file=sys.stderr,
        )
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
