# `lf wt list` performance

```sh
uv run python scripts/benchmarks/wt-list/profile.py --lf target/release/lf --repo ~/src/loopflow --samples 20
uv run python scripts/benchmarks/wt-list/profile.py --lf target/release/lf --repo ~/src/loopflow --samples 20 --offline
uv run python scripts/benchmarks/wt-list/profile.py --lf ~/.local/bin/lf --repo ~/src/loopflow --home main   # installed CLI, main Home
uv run python scripts/benchmarks/wt-list/profile.py --lf target/release/lf --repo ~/src/loopflow --store ~/.lf/loopflow.db   # copy of a real store
```

```sh
lf wt timing            # real invocations on this machine: count, median/p95, failures, version
lf wt timing --json
```

`profile.py` stages runs and counts processes. `lf wt timing` reports what
ordinary use measured; see [Production timing](#production-timing).

Each sample is one default (read-only) listing. Git's trace2 stream counts and
times every Git process; a shim times `gh`; `lf home id` on the same executable
and Home gives the cost of process start, Exec admission and both ledger writes
with no repository work. `--home fresh` (default) points `LF_HOME` at a new empty
directory so a branch build never opens the main Home. `--store` copies a
database into that fresh Home first (a copy-on-write clone on APFS), so the run
pays for a store of real size and writes only to the copy. `--offline` routes
every remote call to a closed local port.

## 2026-10-05: GitHub's answer time, installed 0.13.4

`lf wt timing` on the main Home after 0.13.4 was installed (JSON, 55 worktrees):

| Installed | Samples | Median | p95 | Startup | Local Git | Remote | Receipts |
|---|---|---|---|---|---|---|---|
| 0.13.3 | 4 | 8.09 s | 16.67 s | 4.90 s | 0.42 s | 1.64 s | 2.45 s |
| 0.13.4 | 3 | 1.80 s | 2.09 s | 0.14 s | 0.39 s | 1.62 s | 0.00 s |

Phase columns are medians. Three and four samples are not a p95. The listing
now ends about 0.2 s after GitHub answers, so GitHub is what is left.

**GitHub's answer time grew with the branches in one query.** For this
repository's 53 branches, `gh api graphql` took 0.41 s for a trivial query,
0.67 s for branch existence alone, 0.71 s for PR states alone and 0.91–1.11 s
for both (five or six runs each). The same branches split across 2, 4 and 8
requests side by side took 0.69 s, 0.65 s and 0.67 s. The listing now asks 16
branches per request, and reads the Git facts it needs first side by side
instead of in turn. One request that fails or is stopped leaves every branch
unknown, as a failed single call did.

Same host and repository, fresh empty Home, one warm-up discarded, two
alternating rounds of ten samples. `baseline` is installed 0.13.4; `candidate`
is this branch. Raw rows: [20261005/](20261005/) (`github-requests-*`).

| Run | Mode | Samples | Median | p95 | `gh` processes | `gh` each | Load (1 m) |
|---|---|---|---|---|---|---|---|
| baseline | text | 10 + 10 | 1.47 s, 1.53 s | 1.72 s, 1.81 s | 1 | 1.11–1.16 s | 26, 19 |
| baseline | JSON | 10 + 10 | 1.52 s, 1.42 s | 1.73 s, 1.72 s | 1 | 1.08–1.18 s | 26, 16 |
| candidate | text | 10 + 10 | 1.06 s, 1.07 s | 1.40 s, 1.18 s | 4 | 0.69–0.70 s | 22, 15 |
| candidate | JSON | 10 + 10 | 1.02 s, 1.04 s | 1.11 s, 1.82 s | 4 | 0.68–0.69 s | 20, 14 |

- **≤1 s warm p95 is still not met online**: about 1.0–1.1 s median. A request
  costs about 0.4 s before GitHub does any work and 0.7 s with a quarter of the
  branches; the rest is process start, the Git reads before the request, and
  output. Going lower means answering PR state or `remote_gone` from something
  other than the remote, which the listing does not do.
- One JSON round's p95 is a single 1.82 s sample; a request's tail now has four
  chances to land in a listing. Ten samples do not size that.
- A fresh Home, so no store cost: 0.13.4 already measured that at 0.14 s on the
  main Home. Installed figures for this change come from `lf wt timing`.
- After these rows the listing stopped reading branch heads twice: 72 and 68
  Git processes, not 73 and 69. Its one check ran at load 112 (1.73 s and
  1.29 s medians), which sizes nothing; the rows above were not re-measured.

## 2026-10-05: startup against a 1.1 GB store

The first sample from ordinary use (installed 0.13.3, JSON) read 8.09 s total:
4.91 s before the listing began and 1.90 s writing two Exec receipts. The
October 4 runs used an empty Home and could not see this.

**Every store open scanned the whole database.** Opening validated the schema
and then ran `PRAGMA foreign_key_check`, which reads every row: 0.24 s on an
idle copy of the 1,145 MB store, more beside live writers. One `lf home id`
opens the store five times (Wave attribution twice, each Exec receipt, the
command), so a command with no work took 1.4 s on the idle copy and 2.6–4.0 s
on the live Home. A sampled call graph attributes about 1,130 of 1,162 ms to the scan.

Opening now validates the migration ledger and schema only. Every connection
enforces foreign keys, so only a migration can break them, and a migration
already checks before it commits. `lf home doctor` and installation preflight
keep the full scan.

Same host, same repository (58 worktrees), a fresh Home holding a clone of the
1,145 MB store, one warm-up discarded. `baseline` is installed 0.13.3;
`candidate` is this branch. Raw rows: [20261005/](20261005/).

| Run | Mode | Samples | Median | p95 | Git processes | `gh` | `lf home id` on the same Home | Load (1 m) |
|---|---|---|---|---|---|---|---|---|
| baseline | text | 10 | 6.38 s | 11.00 s | 76 | 1.74 s | 10.46 s | 74 |
| baseline | JSON | 10 | 7.87 s | 11.24 s | 72 | 1.80 s | 5.22 s | 82 |
| candidate | text | 10 | 3.49 s | 10.95 s | 76 | 1.73 s | 0.15 s | 79 |
| candidate | JSON | 10 | 2.04 s | 2.31 s | 72 | 1.44 s | 0.06 s | 71 |

With nothing else changed, `lf home id` on the idle copy went from 1.40 s to
0.03–0.04 s (three runs each, load about 30).

- **≤1 s warm p95 is still not met online.** The GitHub round trip alone took
  1.4–1.8 s in these runs; the JSON candidate finishes about 0.6 s after it.
- **The host was busier than on October 4** (load 70–82, other workers plus
  this profile). The text candidate's p95 is one 10.9 s sample; ten samples do
  not separate a tail from noise. Baseline and candidate ran minutes apart, not
  interleaved.
- **A copy is not the live Home.** It has no concurrent writers and takes the
  custom-Home open path; both paths ran the same scan and both now skip it. The
  live figure comes from `lf wt timing` after a release carrying this is
  installed.
- The copy is taken file by file from a live database, so it is a size
  fixture, not a consistent snapshot.

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

| Foreign write lock held | Listing wall, one wait per receipt | Listing wall, one wait per Exec | Exec receipt |
|---|---|---|---|
| 3 s | 4.65 s | not remeasured | recorded |
| 40 s | 33.2 s | 17.4 s (one run, debug build) | not recorded, warned once |

The listing waits for its start receipt before doing any work. An Exec now
waits at most 15 s for the store across both receipts: a start receipt that
used the whole wait leaves the finish receipt one attempt, which lands the
whole row if the lock has cleared. A lock held throughout still costs 15 s and
leaves the Exec unrecorded with one warning; the timing sample is written
beside the store and survives. The holder changed too: the demonstrated lock was a Codex dispatch that held
SQLite's write lock while waiting on a runtime its own reader had stopped, with
no working deadline. It now releases within 2 s of real time whatever the
runtime is doing (`harness/dispatch.rs`, two regression tests that fail against
the previous dispatch).

### Installed v0.13.6 — October 6, 2026

After PR #1456 shipped in v0.13.6, the published installer updated Jack Heart's
CLI and Desktop successfully. On the live Home, 20 alternating text/JSON pairs
listed 54 worktrees. All 40 commands succeeded, with no recorded remote timeout,
interruption or missing receipt. These are ordinary installed reads under the
current host load, not a matched replay of the earlier release populations.

| Surface | Samples | External median / p95 | Instrumented median / p95 |
|---|---:|---:|---:|
| Text | 20 | 1.465 / 1.511 s | 1.420 / 1.471 s |
| JSON | 20 | 1.387 / 1.443 s | 1.348 / 1.404 s |

Production phase median / p95 in milliseconds: text startup 113/118, local Git
1242/1295, remote 872/956, receipts 4/4; JSON startup 114/116, local Git 1232/1283,
remote 815/907, receipts 4/8. Local and remote phases overlap; do not add them.
An additional diagnostic Git trace counted 67 Git processes, including 54 status
reads (summed 1052 ms, median 12.6 ms, max 174.6 ms). Those summed subprocess
times are not the local phase's wall time. SQLite query counts were not measured.

Both surfaces still miss the one-second warm p95 aim. The installed instrumentation
works and identifies local Git as the larger phase, but this does not establish a
hard lower bound or complete the remaining performance investigation. Preserve
unknown statuses and checkout read-only behavior when reducing that cost.
Private receipts: `/tmp/infra-375-installed-0136/samples.json`,
`/tmp/infra-375-installed-after.json`, and
`/tmp/infra-375-installed-git-trace.jsonl`. The timing command remains
`lf wt timing --json`.

### Reading production timing

Each real `lf wt list` appends one line to `<Home>/perf/wt-list.jsonl`:

| Field | Measures |
|---|---|
| `total_ms` | process entry to exit, after the finish receipt was attempted |
| `startup_ms` | process entry until the listing began: routing, Exec admission, start receipt |
| `listing.local_git_ms`, `listing.remote_ms` | wall time of each; they run side by side |
| `listing.remote` | `answered`, `unavailable`, `timed_out` or `not_asked` |
| `receipts.wait_ms`, `receipts.unrecorded` | time writing Exec receipts to SQLite, and how many did not land |
| `outcome`, `version`, `repo`, `json`, `sync`, `at` | `succeeded`, `failed` or `interrupted`; grouping keys |

The file is trimmed to its newest 500 lines when it reaches 1,000. It is
appended under a file lock, never through SQLite, so a contended store cannot
lose the slow sample. Text-mode diff stats are `total − startup − listing`.
A process killed with SIGKILL leaves no sample; time before `main` is not
measured. Numbers from ordinary use are in the dated sections above.

### Limits of this evidence

- One repository, one host, one day; `--home fresh`, so no measurement against
  the main Home's database size or its live writers.
- Summed Git time overlaps across threads; it is CPU-ish cost, not wall time.
- The admission floor comes from a different command (`lf home id`), not from
  instrumenting the listing.
- The offline run fails fast (closed port). A remote that accepts and never
  answers is covered by a unit test of the 10 s limit, not by a listing run.
