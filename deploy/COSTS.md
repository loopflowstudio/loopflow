# Cost guardrails

Track automation spend before it surprises the release host.

```bash
scripts/check_monthly_spend.py --csv ~/.lf/costs/mercury-2026-06.csv
scripts/check_monthly_spend.py --json ~/.lf/costs/mercury-transactions-2026-06.json
```

The monthly automation budget is **$100**. If actual or projected spend crosses that line, stop and get human approval before adding spend.

## Source of truth

Use the company card/bank feed as the source of truth. Provider dashboards are useful early warnings, but card transactions are what decide the budget gate.

Keep exports outside the repo:

```bash
mkdir -p ~/.lf/costs
# Save Mercury CSV/API exports here, not under the checkout.
scripts/check_monthly_spend.py --csv ~/.lf/costs/mercury-$(date +%Y-%m).csv
```

## Vendors covered

`deploy/budget.yaml` categorizes charges for:

- AWS
- Fly.io
- Claude / Anthropic
- Codex / OpenAI
- OpenCode
- Doppler

Add a match term when a statement line uses a new merchant spelling. Do not add card numbers, account IDs, tokens, or private billing details.

## Payment policy

Use the company card for AWS, Fly.io, Claude/Anthropic, OpenAI/Codex, OpenCode, Doppler, and release-host services whenever the vendor supports card billing. If a service needs invoice or bank transfer billing, record the transaction export the same way and keep credentials outside the repo.

## Gate behavior

```bash
scripts/check_monthly_spend.py --csv ~/.lf/costs/mercury-2026-06.csv --month 2026-06
```

The script exits:

| Exit | Meaning |
|------|---------|
| `0` | Actual/projected spend is within budget |
| `2` | Actual/projected spend exceeds budget; stop for approval |

Projection is on by default for the current month. Disable it for closed months:

```bash
scripts/check_monthly_spend.py --csv ~/.lf/costs/mercury-2026-05.csv --month 2026-05 --no-projection
```

## Mercury path

Use either CSV exports or transaction JSON from Mercury. The script accepts both because live bank credentials should not be required for local validation.

```bash
scripts/check_monthly_spend.py --csv ~/.lf/costs/mercury.csv
scripts/check_monthly_spend.py --json ~/.lf/costs/mercury-transactions.json
```

Later adapters can fetch Mercury, AWS Cost Explorer, Fly usage, or provider API usage directly. Keep the card transaction check as the final gate.
