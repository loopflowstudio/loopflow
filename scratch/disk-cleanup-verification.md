# LOO-722 inspection repair and remaining acceptance

## Outcome and accepted policy — Jack Heart, 2026-11-04

Jack Heart's outcome is reliable hourly coding cleanup, recurrence prevention and
successive measurements across repositories and tooling. Publication alone does
not finish this Task. Preserve its identity and prior delivery history: PR #1820
and PR #1821 (merged as `3e3a653b2b19243b69c11c6843e09779aa846360`).

Accepted: 4h/1h/15m idle windows, 24h minimum owner termination, 24h
successful evidence and seven-day failed/unknown evidence retention. Source,
unpublished work, durable history/databases, active dependencies, saved evidence,
project reservations and Chrome owners remain protected. Non-coding cleanup needs
explicit approval. Preserve complete owner evidence and PID/start verification;
unknown inspection fails closed, without retries or partial-inventory deletion.

Jack Heart also accepted occasional 10–30 minute inactive-worktree rebuilds,
disposal of unused reproducible dependencies/downloads and native cache maintenance
even with plentiful free space. Implemented caps are uv 8 GiB per cache, sccache
10 GiB and Docker build cache 8 GiB; activity and reservations can defer them.
Docker volumes and original data remain protected. Only separately approved
unused Chrome signing copies older than 24h are non-coding deletion candidates;
newest/in-use copies stay. The runner remains machine opt-in, outside product
packages and installation. Accounting labels never authorize deletion.

## Implemented inspection repair — 2026-11-04

The diff replaces `_schedule`'s Background classification with Standard and removes
`_processes`/`_activity`'s special 10/30-second bounds. Both now use `_command`'s
two-minute limit, capped by the remaining run budget. The same hourly job, clean
environment, versioned installation, locking, overall budget and owner teardown
remain. No new deletion category or broader authority is introduced.

## Evidence behind the repair

The ordinary **2026-11-04 17:17:09–17:19:28 PST** cycle on merged main exited 1:
initial `ps` exceeded ten seconds, before discovery or native maintenance. No
candidates, signals or removed allocation; protection checks passed. Free space
fell independently by 2,995,564,544 bytes to 193,018,949,632 bytes, with zero target
shortfall. Accounting measured 29 of 134 roots, left 105 unknown and compared 27
last-known values, five growing. The earlier successful manual run and 101-test
gate for discovery/Docker repair did not establish ordinary-calendar acceptance.

Read-only reproduction under build contention: `taskpolicy -b` made both exact
ps/lsof commands exceed 30 seconds; foreground probes finished in 0.68/1.22 seconds.
A clean-environment launchd probe reproduced the ten-second ps timeout with
Background; Standard completed ps/lsof in 2.49/15.69 seconds. A second Standard
probe took 8.68 seconds for ps but exceeded 30 seconds for lsof. Real teardown also
hit ten seconds on a two-PID ps query. Observed load exceeded 200. This supports
removing both background throttling and the short bounds; it does not prove every
timeout has that cause. Standard's light limits are documented by local
`launchd.plist(5)`.

Installed repaired files match source. The unchanged minute-17 job's manual
kickstart ran **17:55:40–17:58:52 PST**, exit zero: 291 candidate decisions, no
failures, signals or removed allocation. Six protected file hashes, three root
identities, private configuration, schedule and all five installed digests passed
checks. Free space fell independently by 2,951,847,936 bytes to 175,365,357,568 bytes;
shortfall remained zero. Accounting measured 32 roots, left 102 unknown and compared
32 last-known values: ten grew, none shrank. Coverage, intervals and overlapping
roots prevent a containment conclusion.

The real owner-tree test verifies graceful/forced descendant termination while
preserving separate history. LOO-725 comment
`1a4fd20f-9bf3-43fa-aaea-9a76629d0be7` concerns a provider reparented *before*
inspection, with no remaining runtime handle or discoverable ancestry. Current
inspection cannot authorize its termination; shared history or repository ancestry
provides no substitute. Descendants discovered *during* teardown retain their
identities. Transient hook PIDs missing from ps still fail closed. LOO-725 owns
runtime changes; this repair does not redesign ownership.

Reconciliation at 18:04 PST confirmed that all five installed files still match
source, Standard scheduling and the minute-17 calendar remain configured, and the
latest receipt is the successful manual run above. No ordinary cycle on these
bytes has finished yet. The current source includes `origin/main` at `c59cfcc631`,
including PR #1821's discovery and unknown-age Docker repair; this inspection
repair is additional unpublished work. LOO-723 installation pairing and LOO-724
upgrade hooks remain separate from this runner's deployment. No Wave is selected.

## Concept review — 2026-11-04

Outcome: no design change and no unresolved product choice. Jack Heart's accepted
experience remains one explicitly installed hourly cleaner, private receipts and
bounded growth history. Standard scheduling and the shared command deadline repair
that experience under load; they introduce no new action or deletion authority.
The existing usage guide already describes the resulting interaction.

Actions map directly to the existing model: `install` owns a versioned machine
copy and launchd schedule; `run` derives eligible `Candidate` values from fresh
`Activity`/`Process` evidence; `uninstall` disables recurrence. `_schedule`,
`Cleaner.run`, `_reason`, `_stop_owners` and `_clean` own those transitions.
Receipts record effects; usage observations record allocation and uncertainty.
Neither a successful receipt nor an accounting label grants deletion permission
or proves sustained containment. No additional persistent lifecycle is needed.

Normal path: an old ignored build is selected, its retention and ownership are
rechecked, and its measured allocation is credited only after removal. Held output
uses the accepted minimum age and verified teardown; active dependencies and
reserved state stay. Failure path: unavailable process evidence stops deletion
with an incomplete receipt. Recovery: the lock releases and the next scheduled
run inspects afresh, without replaying an old inventory. The repair retains all
three paths and the total wall-clock budget.

Counterexamples remain explicit: missing ownership evidence cannot be replaced
by shared repository ancestry; a clean manual run cannot establish ordinary
calendar reliability; falling free space alongside zero reclaimed allocation
cannot establish either cleanup failure or sustained containment. The remaining
proof below addresses operational evidence, not a policy decision. Leave code,
publication and Graph navigation to their authored steps.

## Remaining acceptance

- Ordinary **2026-11-04 18:17 PST** execution on the repaired bytes, its receipt
  and protection checks remain unverified. The manual run does not replace this.
- Successive comparable scheduled observations must establish growth and containment
  across repositories/tooling. The latest observation leaves 102 roots unknown;
  ten measured roots grew. These results establish neither containment nor the
  generator responsible. Repository-owned prevention remains required wherever
  recurring growth is demonstrated, including outside Loopflow. Existing
  Manabot/Kata prevention and completed one-time sweeps are historical evidence,
  not work to repeat.
- Gate owns affected suites; the authored delivery and verification steps own
  publication and ordinary-cycle acceptance. Sustained containment remains
  unverified after publication or one successful cycle. No new policy decision is
  needed; LOO-725's separate runtime ownership limitation remains as stated above.

## Evidence locations

Private evidence root:
`~/.lf-machine/maintenance/disk-cleanup/evidence/20261104-LOO-722/`.
`discovery-repair/verify-pass-2/` holds the failed ordinary receipt and protection
checks. `inspection-repair/` holds probes and prior notes; `manual/` holds the repaired
receipt, usage history, launchd status, schedule and preservation summary.
`inspection-repair/verification-before-compress.md` preserves the full uncommitted
note before consolidation. Personal inventories remain private.

`inspection-repair/capture.py` collects a completed receipt, installed digests and
protection evidence without initiating cleanup. Its `ordinary 1793845020`
arguments require a receipt starting no earlier than November 4 at 18:17 PST.

Earlier accepted design/demo and gate detail remain at
`7e969d4f6e:scratch/loo-722.md`,
`7e969d4f6e:scratch/disk-cleanup-demo.md`, and
`8086c21a2d:scratch/disk-cleanup-verification.md`.

Check: `uv run python -` read-only installed/receipt/plist audit passed; `git diff --check` and `lf context` passed. Prior focused pytest (five passed) and Ruff results remain applicable to unchanged code; gate owns affected suites and verify owns the pending ordinary cycle.
