## Try it!

Build this branch's CLI, then inspect the same evidence used by the dashboard:

```bash
cargo build -p loopflow --bin lf
export PATH="$PWD/target/debug:$PATH"

lf doctor
lf runs
lf usage
lf usage --json --days 30
lf tokens
lf tokens --days 365
```

On the real local ledger, all six doctor checks are green and eight usage
boundaries reconcile exactly: 20,010 JSON tokens equal the 20,010-token human
summary. At this revision, the live tree and the final historical snapshot also
agree exactly at 183,599 lines and 1,791,384 tokens; the cached 92-snapshot year
loads in 2.55 seconds.

Exercise the Mac dashboard:

```bash
uv run python scripts/loopflow-dev.py run-debug
```

Open **Go → Telemetry (⌘1)**. The page shows ledger health, tokens by skill and
model, cache reuse, codebase growth, and a zoomable code-weight icicle.

Exercise wave chat in two terminals:

```bash
lf wave intelligence --no-flowloop
lf wavechat intelligence
```

The second pane replays recent history, follows live events, accepts messages,
and provides `/status`, `/help`, and `/quit`.

## Intent

Make local agent work observable from one trustworthy record. Canonical process
identity and cumulative usage boundaries now support audits, trace trees, spend
reconciliation, codebase-weight history, and a native dashboard without a
parallel telemetry service or a second interpretation of the data.

## Assumptions

- Repository journals remain available if old events ever need a deliberate
  import after migration 057 clears the ambiguous pre-contract ledger.
- Provider usage snapshots are cumulative within a process; `own_spend` turns
  adjacent boundaries into attributable increments.
- Git's tracked blob is the source of truth for historical code weight,
  including symlink link text rather than the target's contents.
- The Mac app bundles the `lf` built from the same revision. The development
  installer establishes that invariant.
- Local SQLite and Git history are sufficient for this wave; remote telemetry
  is outside the current bet.

## Key decisions

- Treat each run as a trace and each launched process as a span, with explicit
  parent identity.
- Use the same boundary rows for JSON and human usage summaries, including
  flows that launch multiple providers or models.
- Read telemetry directly in Rust and keep Swift as a JSON CLI client instead
  of maintaining an lfd HTTP mirror.
- Cache historical token counts by blob SHA and make the live scanner match Git
  semantics for tracked symlinks.
- Align the local Loopflow UI check with CI's `build-for-testing` merge gate.

## Not included

The eval harvester, PR delivery records, `escalated` emission, wavechat
pause/resume/interrupt verbs, remote telemetry, and automatic backfill of
pre-contract events remain future work.

