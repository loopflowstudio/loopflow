# Reviewing this branch

Everything below was run, not inferred from the diff.

## What shipped

- **The ledger is an API.** Span identity (`process_id` / `parent_process_id`),
  closed `node`/`event` vocabularies with CHECK, absolute `repo`, `model`,
  cumulative usage. Migration 057 truncated the pre-contract rows; `process_id`
  is `NOT NULL`, no legacy path.
- **`lf doctor` exits 0** on the real ledger, six green checks, wired into the
  intelligence wave's `daily` cron (`.lf/flows/telemetry-daily.yaml`).
- **`lf runs` / `lf trace` / `lf usage`** summarize per process, print the
  process tree with per-span cost and model, and reconcile bar-for-bar with the
  boundary rows. `own_spend` is the single home for the cumulative-diff rule.
- **`lf tokens`** measures the codebase: total tokens on disk, growth over a
  year by extension, an interactive icicle. Migration 058 caches by blob sha.
- **The Mac dashboard** (`TelemetryDashboardView.swift`) renders four cards from
  `lf usage/tokens --json`: tokens by skill, tokens by `provider:model`,
  codebase growth + icicle, cache-hit ratio. `lf doctor` check dots head the
  page as the ledger-health strip.
- **`lf wavechat`** — one pane that steers and monitors a wave.

## Verify it

```bash
export PATH="$PWD/target/debug:$PATH"   # this branch's lf, not ~/.local/bin
lf doctor                 # exit 0, six green checks
lf runs                   # one row per process, TRACE/SPAN columns
lf trace <id>             # the process tree: nesting, per-span cost, model
lf usage --json --days 30 # boundary rows; they must sum to the lf usage table
lf tokens                 # tokens on disk; --days 365 for the growth walk
```

Reconciliation is the contract: the sum of `own_spend` over boundaries equals
`lf usage`'s total (verified at 20,010 = 20,010 on two real skill runs). A chart
that disagrees with the table is wrong.

Corruption check — the CHECK and lineage guards fire:

```bash
sqlite3 ~/.lf/lfd.db "insert into run_events (run_id, process_id, seq, ts, node, event) \
  values ('x','y',0,0,'task','started')"   # rejected by the CHECK
# hand-insert a row with a dangling parent_process_id → lf doctor fails lineage, exits 1
```

## The dashboard

```bash
uv run python scripts/loopflow-dev.py run-debug
```

Builds `lf`, `lfd`, and the app, installs `~/Applications/Loopflow Dev.app`, and
launches it. The app resolves `lf` from its own bundle before `PATH`. Then
**Go → Telemetry (⌘1)**.

If "Telemetry" is missing you are running a stale binary, not this branch:

```bash
D="$HOME/Applications/Loopflow Dev.app/Contents/MacOS"
strings "$D/Loopflow" | grep -c "Tokens by skill"   # 1 = has the charts, 0 = stale
```

Stale-binary causes, in order: (1) a *different* app —
`/Applications/Loopflow.app` predates the dashboard; `run-debug` installs a
separate `Loopflow Dev.app` bundle id. (2) `swift build` names the product
`LoopflowMac` but `Info.plist` declares `CFBundleExecutable = Loopflow`; the
installer now reads the plist and copies to that name. (3) `swift run
LoopflowMac` makes a bare executable and dies on `bundleProxyForCurrentProcess
is nil` — the app needs a real bundle, which `run-debug` builds.

## `lf wavechat`

```bash
lf wave intelligence --no-flowloop    # pane 1: a listener, spends no tokens
lf wavechat intelligence              # pane 2
```

Replays the thread's recent history, then live events scroll past. Type to
speak; `/status` reads health, `/help` lists verbs, `/quit` leaves. Drop
`--no-flowloop` for the resident flowloop.

## One gotcha, which is the design working

A shell launched *by* an `lf` run inherits `LF_RUN_ID`, so every `lf` invoked
from it joins that trace as a child span — correct, and what makes `lf trace`
show nested work. A demo run from such a shell will not mint a fresh trace;
prefix `env -u LF_RUN_ID` when you want a root.

## Not started (folded into the wave's open-work list)

The evals harvester, the PR delivery record, an `escalated` event that fires,
and steering verbs for `lf wavechat`. See `wave/intelligence/MEMORY.md` →
"Open work"; these become Linear tasks once `lf auth linear` is refreshed.
