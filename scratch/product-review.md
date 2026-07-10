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
sync` writes one atomic SQLite snapshot per repo and wave; `lf pm show` reads
that snapshot without a network call and exports the complete typed shape as
JSON. Project and task mutations write Linear, then refresh the snapshot.
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
  editing surface; ordinary CLI and Mac reads stay daemonless and offline.
- Made `PmShowResult` the JSON wire shape. The CLI no longer reconstructs a
  second envelope that can omit fields Swift requires.
- Store full definitions in Linear Project content and render that content in
  the Mac. Linear's 255-character description remains only a short summary.
- Kept the wave identity and schedule in `GOAL.md`; no project representation
  survives in the repository.

## How it fits together

`lf pm sync` resolves a wave's Initiative, fetches its Projects and Issues, and
atomically replaces the `(repo, wave)` SQLite row. `lf pm show` filters that
local snapshot and serializes `PmShowResult`; Swift invokes the same command
through `RegistryQuery`, while agents and humans use its table or JSON output.

Radio follows the same ownership rule at the CLI layer: `radio pub` reaches the
existing INSERT path and `radio sub` reaches the existing forward-poll path.
Chat remains the separate human thread.

## Risks and bottlenecks

- Both old radio spellings are intentionally breaking. External callers must
  move to `lf radio pub` or `lf radio sub`.
- PM reads are only as fresh as the last explicit sync or mutation. Missing
  snapshots fail with an actionable `lf pm sync --wave <wave>` instruction.
- The PM snapshot is machine-local and keyed by canonical repo path. Moving a
  checkout requires another sync.
- The Mac issues one local `lf pm show` query per wave while building its plan
  cache. It does not wait on Linear, but large wave rosters still mean multiple
  short-lived subprocesses.
- Resident cron expressions are UTC. Product's `0 0 8 * * * *` schedule is
  08:00 UTC regardless of host timezone or daylight saving time.

## What's not included

- Compatibility aliases for `lf radio TEXT`, `lf sub`, local project files, or
  label-based task grouping.
- Bus transport, delivery, cursor, retention, or attribution changes.
- Automatic/background Linear refresh, task-history analytics, or remote Mac
  plan reads.
- Timezone-aware cron scheduling or changes to `lf chat`.

## Validation

- `uv run python scripts/test.py --all` — all six CI suites passed: 53 Python
  tests; Rust format, clippy, and 1,336 tests; 59 website tests with 3 skips;
  301 Swift tests plus boundary checks; CLI e2e smoke; signed macOS
  `build-for-testing`.
- After adding Project archival, the changed-aware gate reran Rust format,
  clippy, and 1,338 tests plus the website and Swift suites; all passed.
- `cargo test -p loopflow ops::pm --lib` — 19 PM operation tests passed.
- Focused Linear adapter and parser tests cover `String!` IDs and
  `lf pm project archive`; both pass.
- `cargo run -q -p loopflow --bin lf -- pm sync --wave product --plan` — read
  the live product Initiative, Projects, and Issues without mutation.
- Stale-command and project-file scans found no repository-owned executable
  examples or builtin instructions for the retired surfaces.
- `git diff --check main` passed.
