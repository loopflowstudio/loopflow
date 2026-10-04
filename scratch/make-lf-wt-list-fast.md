# LOO-375: make `lf wt list` fast under real worktree load

Draft plan; no design review happened. Decisions by Jack Heart (Linear steers,
2026-10-04):

- Autonomous delivery, no interactive Sessions or demo; several slices are
  expected and a partial slice is progress, not completion.
- Landing is allowed after autonomous correctness, regression, build/static
  checks and honest benchmark evidence. On-machine experience and a quiet-host
  benchmark are post-merge validation, not a gate. Test failures, data loss,
  broken authority and known regressions still need repair.
- The Task is not complete without production timing of real `lf wt list`
  invocations and a documented report command. If a slice lands without it,
  it stays as remaining work on this Task.

Measurements and method: `scripts/benchmarks/wt-list/README.md`.

## Done on this branch

- Listing: ~370 Git processes → ~66; remote enrichment concurrent, one GitHub
  call, 10 s limit; commit-pair answers remembered in `.git/lf-commit-facts`.
  Output byte-identical to baseline on the live repository.
- Deadlock: fenced dispatch timed on its own thread; history recording leaves
  the runtime worker (`harness/dispatch.rs`).

## Remaining

### Production timing (required for completion; nothing built yet)

No code on this branch times real invocations. The benchmark script needs a
staged run and trace2, so it does not satisfy this.

- Record per invocation: total duration, local Git, remote enrichment and
  database (receipt) phases, outcome including remote timeout, and `lf` version.
- One documented command reports sample count, median/p95, failures/timeouts
  and version for this machine, without reading raw traces.
- Bounded retention; no secrets, paths beyond the repository, or conversation
  contents.
- Lead, unverified: every invocation already has an `execs` row with `command`,
  `started_at`, `completed_at` and `outcome`, which may supply totals and
  failures. It has no phases or version, its time resolution is unchecked, and
  under SQLite contention the row is the thing that goes missing (item 1 below),
  so the slowest samples would be the ones lost.
- Unchosen: where phase timings live (Exec row, a small bounded local file, or
  the existing `studio.loopflow`/`perf` signposts, which are Desktop-side).

### By measured cost

1. **Receipts serialize the command behind SQLite contention.** 3 s foreign lock
   → 4.65 s listing; permanent lock → two 15 s waits. Options not yet chosen:
   one receipt deadline per process instead of one per write; writing the start
   receipt beside the command. The second lets a child `lf` start before its
   parent's row lands, and `parent_is_recorded` would then drop the parent
   link — that needs a design, not a patch.
2. **Admission preamble**: seven `rev-parse` processes per `lf` command, four
   identical. ~0.13–0.28 s on every command, not only this one.
3. **Text mode** resolves diff stats after the listing (two more processes,
   ~0.3 s under load); it could run beside remote enrichment.
4. **Quiet-host measurement.** Every number so far was taken at load 30–90.
5. **≤1 s online** needs the GitHub round trip (0.9–1.2 s) to stop being on the
   path. No truthful way to do that is identified.
6. OpenCode's fenced HTTP post can hold the fence 10 s. Bounded, unmeasured.

## Delete — do not maintain

- `engine::git::is_squash_merged` stays: `work/wave/relocate.rs` uses it.
- Removed: `worktrees::upstream_branch`, `enrich_worktrees_network`,
  `list_worktrees_local`, `ops::wt_diff_stat`. `lf wt delete` needed only
  paths and branches, so it reads `list_porcelain`; the integration tests use
  `list_worktrees`.

## Checks

- After sync with main: `cargo test -p loopflow --lib -- harness::dispatch` — 3 passed.
- `cargo test -p loopflow --lib -- engine::worktrees harness::dispatch ops::wt`:
  32 passed; `--test worktree_tests`: 26 passed; clippy `-D warnings` clean.
  Both dispatch tests fail (deadlock detected) with the previous `block_on` +
  worker-blocking reader restored.

- After main sync: `cargo test -p loopflow --test worktree_tests` — 26 passed.

## Completed Session recovery (done on this branch)

Jack Heart authorized fixing the delivery blocker in this PR and repairing the
live database. Task admission and completion allow a closed Session whose
provider is confirmed dead even when its native turn has no completion receipt;
live, unknown and unfinished Sessions still block, using the existing
process-identity evidence. Native history is untouched. The live repair kept
the original start and recorded an administrative interruption, not
provider-reported success, with a private backup of the original rows; the
regression does not depend on it.

Check: `cargo test -p loopflow --lib completed_session_with_exited_provider_does_not_block_task_work`
— 4 passed; `cargo fmt --check` and clippy `-D warnings` clean.

## Acceptance against the Task brief

- Profile text and JSON with counts and phase split: done (report).
- ≤1 s warm p95: met offline JSON only (0.80 s); online 1.93 s JSON / 2.36 s
  text at load >30. Bottleneck reported: one GitHub round trip.
- Bounded while another worker uses SQLite: not met — item 1 under "By measured cost".
- Deadlock regression: done.
- Read-only default, text/JSON contracts, `--sync`: output byte-identical on
  the live repository; remote calls end within 10 s.
- Production timing and report command: not started.
