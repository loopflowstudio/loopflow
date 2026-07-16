# Unified usage: one grain, subscription-first

## Demo

```
lf usage
```

prints, in under a second:

```
ACCOUNTS
PROFILE                 PROVIDER  PLAN  SESSION      WEEKLY        STATUS
jack@loopflow.studio    claude    max   22% → 22:09  12% → Tue     ok
jackstah@gmail.com      claude    max   —            —             needs re-login
loopflow-eng@…          codex     pro   —            78% → Thu     ok

SPEND (30 days)
REPO       PROVIDER  INPUT    OUTPUT   CACHE READ  TOTAL    SHARE
loopflow   claude    1,328    333,168  42,199,400  334,496  62%
…
```

The headline is *percent of subscription used per account*, polled live when
stale, persisted when the harness reports it mid-run. Dollar cost leaves the
human table (it was fiction: codex reports none, claude's is API-equivalent
dollars nobody pays on a subscription). `--json` keeps cost for machines.

## What exists today (measured, 2026-07-16)

Three stores record the same tokens, none reconcile:

1. **run_events** (ledger): cumulative readings per boundary, diffed at read
   time by `own_spend`/`boundary_spans`. Complete coverage — every provider,
   every run. The *grain* is the problem, not the coverage.
2. **agent_turns** (trace): per-turn, richest fields — but codex rows are
   cumulative-in-thread snapshots (live DB sums to 9.6B "input" tokens),
   opencode never lands there, claude coverage is partial. `lf runs` raw-sums
   this table and therefore lies for codex.
3. **wave journal**: per-turn narration. Fine — it's a log, not a query
   source. Untouched by this design.

Four read surfaces, four aggregation rules: `lf usage` (own_spend diffing),
`lf runs` (agent_turns raw sums), `lf trace` (terminal cumulative readings),
`lf top` (output-only buckets + a codex-session-JSONL side channel).
`lf doctor` exists partly to referee disagreements between stores.

Cross-provider semantics differ: codex `inputTokens` includes cached reads
(hence 204M "input" vs claude's 1.7K), and codex cache reads mostly never
reach the ledger (`lf usage` shows 0; agent_turns shows 93% cached).

Subscription state: `record_rate_limit` stores a bare percent + cooldown, only
when a *routed* run happens to see a rate-limit event. Nothing polls on
demand; three of five managed accounts currently hold expired or revoked
tokens and no surface says so.

104 `.tmp*` repos pollute the live ledger — residue from integration tests
that predate the store isolation in #908. The leak is fixed; the junk remains.

## Design

### One spend grain: per-boundary deltas in run_events

The write path (journal `PendingUsage`) tracks last-emitted totals and writes
the *delta* at each boundary. Readers sum rows; no read-time diffing anywhere.
`own_spend`/`boundary_spans` and the trace-side fold die. The `--json` wire
shape (SpanDto rows, already deltas) is unchanged — the Swift dashboard keeps
working without edits.

`agent_turns` stays as trace/debug data (context pressure, per-turn forensics)
but is no longer a spend source: `lf runs` and the doctor stop summing it for
tokens/cost. The doctor's cross-store reconciliation is deleted — with one
store there is nothing to referee.

Migration (one SQL file):
- convert existing cumulative usage columns to deltas via `lag()` over
  `(process_id)` ordered by `seq`;
- normalize historical codex input: `input -= cache_read` where both present;
- delete rows whose repo basename matches `^\.tmp[A-Za-z0-9]{6}$` (test
  residue).

### Comparable tokens across providers

At extraction, codex input is stored net of `cachedInputTokens`; cache reads
land in `cache_read_tokens` like claude's. TOTAL stays input+output, cache
its own column — same meaning in every row.

### Subscription limits: one table, two feeders, one poller

```
provider_account_limits(
  provider, account_id, window,   -- 'session' | 'weekly' | 'weekly:<model>'
  used_percent, resets_at, plan, observed_at, source,  -- 'stream' | 'poll'
  PRIMARY KEY (provider, account_id, window)
)
```

Feeders:
- **Harness streams** (free, mid-run): widen the existing rate-limit signal to
  persist full snapshots — codex `rateLimits` windows arrive on every turn;
  claude `rate_limit_event` gives session utilization + reset.
- **On-demand poll** (when a window is stale or `--refresh`):
  - claude: refresh the OAuth token if expired
    (`console.anthropic.com/v1/oauth/token`, public client id, write the
    rotated pair back to the account home's `.credentials.json`), then
    `GET api.anthropic.com/api/oauth/usage` — returns `limits[]` with
    session/weekly/per-model percent + resets_at. Proven live.
  - codex: one-shot `codex app-server` with `CODEX_HOME` set, JSON-RPC
    `account/rateLimits/read` — returns windows + plan. Proven live
    (manabot-eng: 78% weekly, pro).
- Revoked/expired-beyond-refresh tokens surface as `needs re-login`, not a
  silent blank. (`token_invalidated` is exactly what two accounts return
  today.)

`lf auth accounts` reads the same table for its `NN% used` detail. The old
single-percent `utilization_percent` column on provider_accounts is replaced
by this table (cooldown stays where it is — it's routing state, not usage).

### Surfaces after the cut

| surface | source | rule |
|---|---|---|
| `lf usage` | provider_account_limits + run_events | accounts headline; token table sums deltas; SHARE = row/total tokens |
| `lf usage --json` | run_events | stored delta rows, SpanDto shape (cost included) |
| `lf runs` | run_events (join by run) | per-run sums of deltas |
| `lf trace` | run_events | per-process GROUP BY sums |
| `lf top` | run_events | bucket deltas by ts; codex JSONL side channel deleted |
| `lf doctor` | run_events | integrity only; cross-store checks deleted |

## Non-goals

- Dollar-cost modeling for codex (no pricing table; subscription percent is
  the real budget).
- Wave journal changes.
- Backfilling opencode into agent_turns.

## Open questions (executive picks, flag if wrong)

- `lf usage` default window: 30 days (was: all history). `--days 0` = all.
- COST column: dropped from human tables, kept in `--json`. If a human
  surface wants it back it's one formatter away.
