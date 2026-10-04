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
  invocations and a documented report command.

Measurements and method: `scripts/benchmarks/wt-list/README.md`.

## Done on this branch

- Listing: ~370 Git processes → ~66; remote enrichment concurrent, one GitHub
  call, 10 s limit; commit-pair answers remembered in `.git/lf-commit-facts`.
  Output byte-identical to baseline on the live repository.
- Deadlock: fenced dispatch timed on its own thread; history recording leaves
  the runtime worker (`harness/dispatch.rs`).
- Production timing: every `lf wt list` appends one sample to
  `<Home>/perf/wt-list.jsonl` (`ops/wt_timing.rs`): total, startup, local Git,
  remote with its outcome (answered / unavailable / timed out), receipt time and
  unrecorded receipts, outcome including interrupted, `lf` version. Trimmed to
  500 lines at 1,000. Written under a file lock, never through SQLite.
- Report: `lf wt timing [--json]` prints count, median/p95/max, phase spreads,
  failures, interruptions, remote timeouts and unrecorded receipts per
  repository, version and mode. Documented in `docs/troubleshooting.md`,
  `docs/lf.md`, `docs/lf-reference.md` and the benchmark README.
- Receipt contention: one 15 s wait per Exec across both receipts
  (`journal::receipt_wait`, `SqliteStore::record_exec_within`). A start receipt
  that used the wait leaves the finish receipt one attempt, which records the
  whole row if the lock cleared. Still warned once; nothing is dropped silently.
- Completed Session recovery: Task admission and completion allow a closed
  Session whose provider is confirmed dead without a completion receipt; live,
  unknown and unfinished Sessions still block (Jack Heart authorized this in
  the same PR, with the live-database repair and its private backup).

## Remaining

### Before completion

- **Read `lf wt timing` from ordinary use.** No sample from real work exists:
  the only ones are four runs in a disposable Home. This needs a release
  carrying the branch installed on Jack Heart's machine, then enough listings
  to report a p95. Until then the Task is not complete.
- **The captured Flow ends with `task pr land -c`**, which completes the Task
  at landing. That contradicts the line above; landing without `-c` (or
  reopening afterwards) is the outcome Jack Heart's contract describes. The
  running Flow's definition is captured, so editing
  `.lf/flows/pursue-auto.yaml` would not change it. Unresolved.

### By measured cost

1. **The start receipt still precedes the command**, so a held write lock costs
   one 15 s wait. Writing it beside the command lets a child `lf` start before
   its parent's row lands, and `parent_is_recorded` would drop the parent
   link — that needs a design, not a patch. `SqliteStore::new` keeps its own
   15 s timeout; it only reads once a Home is initialized, so it is not bounded
   by the receipt wait.
2. **Admission preamble**: seven `rev-parse` processes per `lf` command, four
   identical. ~0.13–0.28 s on every command; now visible as `startup_ms`.
3. **Text mode** resolves diff stats after the listing (two more processes,
   ~0.3 s under load); it could run beside remote enrichment.
4. **Quiet-host measurement.** Every staged number was taken at load 30–90.
5. **≤1 s online** needs the GitHub round trip (0.9–1.2 s) to stop being on the
   path. No truthful way to do that is identified.
6. OpenCode's fenced HTTP post can hold the fence 10 s. Bounded, unmeasured.

## Delete — do not maintain

- `engine::git::is_squash_merged` stays: `work/wave/relocate.rs` uses it.
- Removed: `worktrees::upstream_branch`, `enrich_worktrees_network`,
  `list_worktrees_local`, `list_worktrees_with_default`,
  `list_worktrees_enriched` (all replaced by `list_worktrees_timed`, whose
  `Listing` carries `pull_requests_known` for prune), `ops::wt_diff_stat`, and
  the report's unprinted `first_at`/`last_at`.

## Checks

- 2026-10-04, after folding `list_worktrees_enriched` into `list_worktrees_timed`
  and dropping `first_at`/`last_at`: `cargo test -p loopflow --lib -- ops::wt_timing engine::worktrees ops::wt`
  with `LF_*` Run variables unset — 34 passed. Earlier passes on this branch
  (journal, dispatch, `--test worktree_tests`, `--bin lf`, fmt, clippy, the
  held-write-lock listing at 17.4 s with `receipts.unrecorded: 2`) were not
  rerun; gate owns the affected suites, with inherited `LF_RUN_ID`/`LF_FLOW_STEP`
  cleared (two `lf::` tests fail only under them).

## Acceptance against the Task brief

- Profile text and JSON with counts and phase split: done (report).
- ≤1 s warm p95: met offline JSON only (0.80 s); online 1.93 s JSON / 2.36 s
  text at load >30. Bottleneck reported: one GitHub round trip.
- Bounded while another worker uses SQLite: one 15 s wait, was two.
- Deadlock regression: done.
- Read-only default, text/JSON contracts, `--sync`: output byte-identical on
  the live repository; remote calls end within 10 s.
- Production timing and report command: built; ordinary-use numbers pending
  an installed release.
