# LOO-923: provider formats leave execution as the first sibling crate

## Outcome

`loopflow-providers` is a new private path crate beside storage: what each
native client (Claude, Codex, OpenCode, Pi) writes and where. It holds the
stream decoders, the provider facts they produce, transcript readers and the
harness identity. `loopflow-execution` keeps launching and recording and
consumes it.

Two things change for whoever edits Rust here:

- Fixing a decoder or transcript reader compiles and tests one small crate.
  `cargo test -p loopflow-providers` never builds execution or the application.
- Every other execution edit stops recompiling that code, and execution's test
  binary stops carrying its tests.

No behavior, wire shape, saved data or migration byte changes.

## Demo

```sh
# a provider changed its output: fix the decoder, run its tests
$EDITOR rust/loopflow-providers/src/claude_decode.rs
cargo test -p loopflow-providers claude_decode      # one workspace crate compiles
cargo tree -p loopflow-providers -e normal,dev | grep -c loopflow-execution   # 0
open scripts/benchmarks/crate-rebuild/results/loo-923.html
```

The results page shows base `7bb879aff4` against the branch for five paired
cohorts: an ops edit hits the providers cache, a decoder edit misses providers,
execution and the application and hits storage, and the owner's test build.

## Why this cut

Findings, with how to get them again:

- **Execution is one cycle.** 50 of 64 top-level modules (112k of ~115k
  production lines) form a single strongly connected component by `crate::`
  references; at a threshold of more than five references per edge, 20 modules
  and 92k lines still do. No sibling exists today; every extraction starts by
  removing upward references. (Module graph over `rust/loopflow-execution/src`,
  test modules cut at `#[cfg(test)] mod tests`.)
- **PRs are whole-feature journeys.** `cochange.py --count 300` at `7bb879aff4`,
  minus the three split PRs and two rename PRs (#1969, #1643), leaves 228 PRs
  with production Rust churn, 215 of them in today's execution code. Given a
  PR touches any execution group, it also touches the application 77–97% of the
  time and storage 58–96%. No two execution groups co-change above 71% except
  with `ops`, so the history argues for no merger of candidates and shows no
  hidden shared concept between them.
- **`ops` is the consumer, not a sibling.** It is in 110 of 184 grouped
  execution PRs, 34 of them with no other execution group. Making it a crate
  downstream of the rest would spare an ops-only edit about 70% of execution's
  ~30 s compile, and it is passed over: it lengthens the chain
  (storage → execution → operations → application), 17 lower modules reference
  `ops` (42 references, `engine` and `machine` another 78), and as siblings
  leave, what remains of execution converges on `ops` plus the runtime anyway.
- **Siblings skipped by most edits.** Share of execution PRs that leave each
  candidate untouched: git/repository 89%, trackers 86%, transport 73%,
  config/graph/skills/prompt 71%, harness and friends 63%.
- **Inside the harness there are two things.** Format files (decoders, facts,
  `transcript/`) change in 23 of 215 execution PRs; launch files in 51. 17 of
  the 23 format PRs also touch launch, but only 17 of the 51 launch PRs touch
  format. Format is the stable lower half that launch reads. One trait,
  `Harness`, carries both today: `open`/`driver` beside `transcript_root`,
  `provider_env`, `model_rungs`. `transcript/` calls `.harness()` 24 times for
  format facts only. That trait is the missing concept split.
- **The decoders are already leaves.** `claude_decode`, `codex_decode`,
  `opencode_decode`, `pi_decode`, `fact` and `lf_tag` (4.4k lines with tests)
  reference nothing else in the crate.
- **Timings to beat** (LOO-650, `results/loo-650.json`, one Cargo job,
  `incremental = false`): application edit 27.4 s, execution edit 57.5 s,
  storage edit 65.5 s; peak RSS 3.4 GiB when execution compiles. A crate is the
  whole unit of rebuild and of cache.

Predictions the measurement checks, not results: the ops-edit build drops from
57.5 s by providers' compile share (a few seconds); a decoder-edit build costs
about what an execution edit costs now; the owner's test build falls from
execution's full test binary to a crate of roughly 15k lines.

## What moves

Into `rust/loopflow-providers/src/`, with their unit tests and
`harness/testdata`:

- `harness/{claude,codex,opencode,pi}_decode.rs`, `fact.rs`, `lf_tag.rs`
- `transcript/` (see rows 2, 4 and 5 below for the three files with upward
  references)
- `HarnessKind`, `Capabilities` and the format half of `Harness`: identity,
  provider, env names, model rungs, sign-in advice, recognition, and every
  `transcript_*` and expiry method

Execution keeps `open`, `driver` and `validate_agent_process_close`, the four
launch implementations, `fact_writer`, `native_session`, `native_exit`,
`context`, `agent_process`, `task_updates`, `conformance_tests` (it drives
decoders through `FactWriter` against a store) and the launch matrix.

Choice made for review: the format trait keeps the name `Harness` in
providers, and execution's launch half becomes `harness::Launcher` reached by
`harness::launcher(kind)`. Passed over: renaming the format half and keeping
`Harness` for launch, which rewrites more call sites for no gain.

## Upward references to remove

| # | Reference | Resolution |
|---|-----------|------------|
| 1 | `HarnessKind::harness()` returns the launch implementation (≈60 callers in execution, 4 in the application) | Format callers keep `kind.harness()`, now resolved in providers; the callers that launch use `harness::launcher(kind)` |
| 2 | `transcript::adopt_native` opens the store from the environment (`transcript/mod.rs:183`) | Takes the store from its two callers (`harness/claude.rs`, `harness/codex.rs`) |
| 3 | `transcript/claude.rs:117` calls `harness::claude::usage` | `usage` moves with the Claude format |
| 4 | `process::OsProcess` in `Harness::recognizes`, `conversation_owners` and `transcript/raw.rs` | Recognition takes the command it reads; if `raw.rs` needs the process table itself it stays in execution beside `observe` |
| 5 | `transcript/watch.rs` uses `observe::Cache` | Stays in execution; `observe` reads providers, never the reverse |
| 6 | `crate::store::{ProviderAccountId, StoreError, …}` re-exports | Named from `loopflow_storage` directly |
| 7 | Tests open stores with execution's `store::opening::open_ephemeral` | `loopflow_storage::test_fixture::open` through storage's `test-support` dev feature |
| 8 | `pub(crate)` items execution still reads (`fact`, decoders) | `pub` in providers; execution's `harness` re-exports nothing |

Not in this Task: `provider_auth` (its `Provider` already lives in storage; the
service holds tracker credentials too, needs the async store, and #1968 is
editing it), the launch implementations, `observe`.

## Callers and files outside Rust source

- `Cargo.toml` workspace members; execution, application and test-support manifests
- `scripts/check_architecture.py:258` and `scripts/check_migrations.py:188` crate tuples; `scripts/test.py` suite reach; `scripts/macos_inputs.py`
- `scripts/benchmarks/crate-rebuild/probe.py` member list; `run.py` cases
- `AGENTS.md` harness rule path, `rust/loopflow-execution/README.md`,
  `harness/README.md`, `docs/dev/{codebase,execution,reference-code}.md`,
  a new `rust/loopflow-providers/README.md`

## What must keep working

- Native provider conversations and hooks: the launch matrix and
  `conformance_tests` pass unchanged in execution.
- One application launch or resume test and one moved fixture run after the
  move; compilation alone misses runtime script paths.
- Served and remote commands, process identity, Desktop and iOS inputs
  (`swift/ios.yml` reads `rust/loopflow-execution/tests/fixtures/dto/server`,
  which does not move).
- A second build with no edit stays fresh.
- `loopflow-storage` and every migration file are untouched.

## Acceptance check at gate

```sh
cargo test -p loopflow-providers
cargo tree -p loopflow-providers -e normal,dev | grep -c loopflow-execution    # 0
uv run python scripts/test.py --reuse-passing --rust-filter 'test(harness::)'
uv run python scripts/benchmarks/crate-rebuild/run.py --base 7bb879aff4 --candidate HEAD --sccache <path> --output /tmp/lf-923
```

`run.py` gains two cases. `provider_format` edits a decoder message at each
revision's own path and requires providers, execution and application misses
with a storage hit. `provider_test` builds the owner's test binary after that
edit (`--no-run`) and records its compiler closure and link. The existing
`execution` case additionally requires a providers hit on the candidate. The
report keeps wall and CPU seconds, peak RSS and closures, as LOO-650's did.

## Sequencing and overlap

- LOO-921 (cache and test baseline) is idle on an open question and LOO-922
  (fresh-worktree cost) is unstarted. The extraction does not depend on either;
  the comparison against their results does. Choice made for review: build the
  slices now, measure base against candidate with `run.py`'s private caches,
  and state in the PR which predecessor numbers did not exist. Before `run.py`
  runs, `lf task status LOO-921` and `LOO-922` show neither is measuring: one
  expensive measurement at a time.
- Open PRs editing the files that move: #1967 (eight `transcript/` files,
  `claude_decode`), #1976 and #1972 (`transcript/raw.rs`, `transcript/mod.rs`,
  `harness/mod.rs`), #1968 (`harness/native_session.rs`). Whichever merges
  first, the other rebases across a rename; this branch takes their content and
  changes none of their behavior.

## Slices

1. **Spike: the leaves.** Create the crate with the four decoders, `fact`,
   `lf_tag`, their tests and `testdata`; execution depends on it; the old files
   are gone. Run `cargo test -p loopflow-providers`, execution's
   `harness::conformance_tests` (the mixed fixture: decoders across the crate
   line into `FactWriter` and a store) and one application launch test. Time
   the owner's test build before and after with a single message edit. This is
   the slice that can prove the plan wrong: it shows whether a sibling's test
   loop is as short as predicted, how much surface must become `pub`, and
   whether mixed fixtures compile without a shim.
2. **Split the trait, move transcripts.** `HarnessKind` and the format half of
   `Harness` move; execution gets `Launcher`; rows 1–7 of the table are
   resolved; `transcript/` moves.
3. **Measure and document.** `run.py` cases, `results/loo-923.html`, scripts,
   READMEs and `AGENTS.md` paths.

## After this Task

Ranked by the same evidence, each its own linked Task once this one's
measurements say what a sibling of this size buys:

- Trackers: `linear`, `asana`, `ops/linear`. 86% of execution PRs skip them;
  about 11 upward references outside `ops/linear`.
- Definition resolution into `loopflow-definitions`: `config`, `graph`,
  `skills`, `definition`, `prompt`, `task_graph`. 71% skip; about 70 upward
  references, `config → store` first.
- Git and repository facts. 89% skip; blocked on `observe`, which reads
  transcripts.
- Provider logins and the launch half, joining `loopflow-providers`.

If the measured drop on the ops edit is inside run-to-run noise (LOO-650's
range was under 1 s), siblings this size are not worth their boundary and the
next Task is chosen by measured compile share, not by this list.
