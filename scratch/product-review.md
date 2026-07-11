# Review: explicit radio and Linear-owned planning

## What was implemented

The agent bus now has one explicit namespace:

```text
lf radio pub [TEXT] [--channel NAME | --parent] [--from NAME]
lf radio sub [CHANNEL] [--json]
```

The old `lf radio TEXT` and top-level `lf sub` forms fail during parsing.
Repository callers, webhook execution, docs, prompt goldens, and builtin skills
all use the new grammar.

Linear now owns Projects, their definitions and KRs, and their Issues. `lf pm
sync` writes one atomic SQLite snapshot per repo and wave; `lf pm show` exports
that complete typed shape as JSON. Human reads serve fresh snapshots and make
one bounded refresh attempt when stale; `--no-sync` keeps agent and app reads
cache-only. Project and task mutations write Linear, then refresh the snapshot.
Repository `projects/*.md` mirrors and legacy task-label migration paths are
gone.

The Mac reads the objective from `GOAL.md`, then maps the SQLite-backed `lf pm
show --json` export into `WavePlan`. It renders the full Linear Project
definition and KR proof state, while the backlog uses each Issue's native
Project association. Product, intelligence, and infrastructure are bound to
their Linear Initiatives; product also runs the standard wave flow daily at
08:00 UTC.

## Key choices

- Kept radio's store bus unchanged. Nested Clap subcommands change command
  ownership without changing delivery, cursor, retention, or byline semantics.
- Reserved retired top-level `sub` as a hidden parser error so external-skill
  fallback cannot reinterpret the removed command.
- Made Linear the sole planning author. SQLite is an atomic read model, not an
  editing surface. Human CLI reads use a bounded freshness policy; agents and
  the Mac pass `--no-sync` so headless work and UI rendering stay offline.
- Made `PmShowResult` the JSON wire shape. The CLI no longer reconstructs a
  second envelope that can omit fields Swift requires.
- Store full definitions in Linear Project content and render that content in
  the Mac. Linear's 255-character description remains only a short summary.
- Kept the wave identity and schedule in `GOAL.md`; no project representation
  survives in the repository.

## How it fits together

`lf pm sync` resolves a wave's Initiative, fetches its Projects and Issues, and
atomically replaces the `(repo, wave)` SQLite row. `lf pm show` filters that
local snapshot and serializes `PmShowResult`. Auto mode serves snapshots under
one hour, makes a five-second refresh attempt after that, and refuses a stale
fallback after one week; Swift and builtin agents select cache-only mode.

Radio follows the same ownership rule at the CLI layer: `radio pub` reaches the
existing INSERT path and `radio sub` reaches the existing forward-poll path.
Chat remains the separate human thread.

## Risks and bottlenecks

- Both old radio spellings are intentionally breaking. External callers must
  move to `lf radio pub` or `lf radio sub`.
- Cache-only PM reads are only as fresh as the last sync or mutation. Missing
  snapshots fail with an actionable `lf pm sync --wave <wave>` instruction;
  human auto reads may wait up to five seconds when the cache is stale.
- The PM snapshot is machine-local and keyed by canonical repo path. Moving a
  checkout requires another sync.
- The Mac issues one local `lf pm show` query per wave while building its plan
  cache. Every invocation uses `--no-sync`, but large wave rosters still mean
  multiple short-lived subprocesses.
- Product and intelligence both currently carry migrations numbered `061`.
  Their version strings are distinct, but cross-branch ordering remains a
  coordination risk recorded in `scratch/questions.md`.
- Resident cron expressions are UTC. Product's `0 0 8 * * * *` schedule is
  08:00 UTC regardless of host timezone or daylight saving time.

## What's not included

- Compatibility aliases for `lf radio TEXT`, `lf sub`, local project files, or
  label-based task grouping.
- Bus transport, delivery, cursor, retention, or attribution changes.
- Background refresh beyond the resident sync cron, task-history analytics, or
  remote Mac plan reads.
- Timezone-aware cron scheduling or changes to `lf chat`.

## Validation

- `uv run python scripts/test.py --all` — all six CI suites passed: 53 Python
  tests; Rust format, clippy, and 1,339 tests; 59 website tests with 3 skips;
  301 Swift tests plus boundary checks; CLI e2e smoke; signed macOS
  `build-for-testing`.
- `cargo test -p loopflow pm_ --lib` — 16 focused PM, parser, provider, and
  snapshot tests passed.
- Focused Linear adapter and parser tests cover `String!` IDs and
  `lf pm project archive`; both pass.
- `cargo run -q -p loopflow --bin lf -- pm sync --wave product --plan` — read
  the live product Initiative, Projects, and Issues without mutation.
- Stale-command and project-file scans found no repository-owned executable
  examples or builtin instructions for the retired surfaces.
- Every builtin PM read uses `--no-sync`; Swift tests pin cache-only argv for
  both backlog and plan queries.
- `git diff --check` passed.

## Wave alignment

This advances the product wave's loopflow-api, mac-surface-ux, and
product-performance bets: one typed planning export feeds the CLI and Mac,
agent reads cannot unexpectedly reach Linear, and the wave list keeps its
network-free first-paint path. No bus semantics, remote plan transport, or iOS
surface was added.
