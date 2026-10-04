# `lf wt list` performance

```sh
uv run python scripts/benchmarks/wt-list/profile.py --lf target/release/lf --repo ~/src/loopflow --samples 20
uv run python scripts/benchmarks/wt-list/profile.py --lf target/release/lf --repo ~/src/loopflow --samples 20 --offline
uv run python scripts/benchmarks/wt-list/profile.py --lf ~/.local/bin/lf --repo ~/src/loopflow --home main   # installed CLI, main Home
```

Each sample is one default (read-only) listing. Git's trace2 stream counts and
times every Git process; a shim times `gh`; `lf home id` on the same executable
and Home gives the cost of process start, Exec admission and both ledger writes
with no repository work. `--home fresh` (default) points `LF_HOME` at a new empty
directory so a branch build never opens the main Home. `--offline` routes every
remote call to a closed local port.

## 2026-10-04: 51 worktrees, Jack Heart's loopflow repository

Same host, same repository, fresh Home, one warm-up run discarded.
`baseline` is `cd344891b`; `candidate` is this branch. Raw rows: [20261004/](20261004/).

| Run | Mode | Samples | Median | p95 | Git processes | Local Git (summed) | Remote (`gh` + `ls-remote`) | Admission + ledger floor | Load (1 m) |
|---|---|---|---|---|---|---|---|---|---|
| baseline | text | 10 | 12.83 s | 17.00 s | 419 | 6.15 s | 0.79 s + 1.16 s | 0.35 s | not recorded |
| baseline | JSON | 10 | 10.47 s | 17.85 s | 370 | 4.97 s | 0.79 s + 1.12 s | 0.32 s | not recorded |
| baseline, rechecked | JSON | 5 | 6.60 s | 9.08 s | 378 | 3.30 s | 0.76 s + 1.25 s | 0.29 s | 33 |
| candidate | text | 20 | 1.92 s | 2.36 s | 70 | 2.93 s | 1.19 s | 0.25 s | 31 |
| candidate | JSON | 20 | 1.63 s | 1.93 s | 66 | 2.61 s | 1.05 s | 0.27 s | 32 |
| candidate, offline | text | 20 | 0.88 s | 1.31 s | 71 | 2.68 s | 0.11 s (fails) | 0.25 s | 42 |
| candidate, offline | JSON | 20 | 0.71 s | 0.80 s | 67 | 2.58 s | 0.10 s (fails) | 0.32 s | 38 |

Text and JSON output of baseline and candidate were byte-identical on the same
repository state.

### What the time was

- **Starting processes, not Git's work.** Each Git process took 3–30 ms inside
  Git but 25–45 ms to start from `lf` (`/usr/bin/git` is the Xcode shim). The
  baseline ran about 370 of them, almost all one after another: per worktree an
  upstream lookup, two `rev-parse`, two `merge-base` and a `status`; per branch
  a `merge-tree` plus a `rev-parse`; another `rev-parse` per branch for the PR
  query; and in text mode a `diff --shortstat` per row.
- **Remote enrichment ran after local work**, and asked twice: one GitHub query
  and one `git ls-remote`.

### What changed

- Branch heads and upstreams come from one `for-each-ref`; branches with no
  commits beyond the target from one `for-each-ref --merged`.
- `status` runs in every worktree on 16 threads.
- Squash-merge and diff-stat answers depend only on two commit ids, so they are
  remembered in `.git/lf-commit-facts` for the current target commit. When
  `origin/<default>` moves, each branch is asked once more (one `merge-tree`
  each, concurrently); the first candidate run after that took 4.9 s under load.
- Remote enrichment starts as soon as the worktree list is read and runs beside
  local work. GitHub answers PR state and branch existence in one call;
  `ls-remote` runs only for a non-GitHub remote or when GitHub is unavailable.
- Every remote call ends within 10 s. An unanswered remote reports PR state as
  unknown and `remote_gone` as false, as a failed one always did.

### Where the candidate stands against ≤1 s warm p95

Not met online: 1.93 s JSON, 2.36 s text at a load average above 30. Offline it
is met for JSON (0.80 s) and missed for text (1.31 s).

- **One GitHub round trip is the floor online.** A trivial `gh api graphql` took
  0.42 s here; the listing's query took 0.9–1.2 s. The listing cannot finish
  before it without reporting PR state or `remote_gone` from something other
  than the remote.
- **Before the listing starts**, Exec admission runs seven `rev-parse`
  processes (four identical `--show-toplevel`) and the listing two
  `symbolic-ref`: about 0.13 s idle, 0.28 s at this load.
- **Text mode** resolves diff stats after the listing with two more processes.
- **The host was busy.** Other workers kept the load average between 30 and 90
  throughout, and the same baseline measured 10.5 s and 6.6 s an hour apart.
  Ratios on one row pair are more trustworthy than absolute times; no quiet-host
  measurement exists yet.

### SQLite contention

Measured on the candidate with a fresh Home and a second connection holding
`BEGIN IMMEDIATE`:

| Foreign write lock held | Listing wall | Exec receipt |
|---|---|---|
| 3 s | 4.65 s | recorded |
| 40 s | 33.2 s | not recorded, warned once |

The listing waits for its start receipt before doing any work and for its
finish receipt after, each up to the 15 s busy timeout. That is unchanged. What
changed is the holder: the demonstrated lock was a Codex dispatch that held
SQLite's write lock while waiting on a runtime its own reader had stopped, with
no working deadline. It now releases within 2 s of real time whatever the
runtime is doing (`harness/dispatch.rs`, two regression tests that fail against
the previous dispatch).

### Limits of this evidence

- One repository, one host, one day; `--home fresh`, so no measurement against
  the main Home's database size or its live writers.
- Summed Git time overlaps across threads; it is CPU-ish cost, not wall time.
- The admission floor comes from a different command (`lf home id`), not from
  instrumenting the listing.
- The offline run fails fast (closed port). A remote that accepts and never
  answers is covered by a unit test of the 10 s limit, not by a listing run.
