# LOO-375: make `lf wt list` fast under real worktree load

Draft plan, written during implementation. Jack Heart requested autonomous
delivery (Linear steers, 2026-10-04); no design review happened.

Measurements and method: `scripts/benchmarks/wt-list/README.md`.

## Done on this branch

- Listing: ~370 Git processes → ~66; remote enrichment concurrent, one GitHub
  call, 10 s limit; commit-pair answers remembered in `.git/lf-commit-facts`.
  Output byte-identical to baseline on the live repository.
- Deadlock: fenced dispatch timed on its own thread; history recording leaves
  the runtime worker (`harness/dispatch.rs`).

## Remaining, by measured cost

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
