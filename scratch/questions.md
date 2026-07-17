# Open questions — W2-280

## 0. PR boundary after the failed recovery (CLOSED 2026-07-17)

**Resolved.** #1031 is CLOSED (`gh api pulls/1031` → `closed_at 01:04:18Z, merged=false`);
#1036 MERGED at 01:38:10Z as `c649a4db4`. One identity throughout, no split. The
Task rotated to PR 3 for the remaining work.

The freeze this section triggered was itself the lesson: a directive held work
pending #1031's state on the grounds that no Loopflow surface could refresh it.
True — `lf pr status` takes no argument, and `lf task status` reports a *stored*
observation (sequence 1's was frozen at 00:46:01.988, the instant of its own
`abandoned_at`, recording a head the branch no longer had; its `result: fresh`
meant the read succeeded, not that the record was current). But that is a fact
about lf, not about #1031 — GitHub answered in one call. Absence in one
projection is evidence about the projection. Recorded in wave MEMORY.

Original analysis retained below.

## 0a. Original: do not mint a duplicate (decided)

State as of epoch 2, verified rather than inferred:

- **PR #1031** — branch `…-recorders`, head `931dee16a`, **OPEN** on GitHub,
  carries both design files. The ledger marked it abandoned at 00:46:01 during a
  recovery that then failed on a dirty worktree — the abandonment is an artifact
  of that error, not a decision.
- **Sequence 2** — branch `…-recorders-2`, head `194c1d462`, base `6b659d92b`,
  tree clean, ledger-active, unpublished. `git diff 931dee16a HEAD -- scratch/`
  is **empty**: it carries the identical revised design.

So sequence 2 is a **duplicate**, not the empty successor the ENG-20 rule warns
about — the rotation carried the design forward and nothing was lost.

**Decided: do not publish sequence 2 as a second design PR.** Two open PRs with
byte-identical content is the noise that rule exists to prevent. #1031 stays the
design of record and keeps the review thread; sequence 2 is the serial branch
that PR1's *implementation* goes into, and it gets published when it has
implementation to show. #1031 is then superseded naturally, by content rather
than by ceremony.

Not attempted here: repairing the ledger's abandoned/open split-brain. W2-286
(`reconcile-a-reopened-task-pr`, PR #1032) owns exactly that and is in review.
Racing it from inside the Task it broke would be the same mistake twice.

## 1. Directive Done-When #4 is unachievable as written (decided: reframed)

> "lf usage output before and after the cut is identical on a migrated copy of a
> production ledger (same totals, same rows)"

Measurement says this cannot hold, and should not:

- **Grain changes.** `lf usage --json` emits one row per `run_events` boundary
  (94 rows live). Turn grain is per-turn (1,040 rows). "Same rows" would require
  re-aggregating turns back to process boundaries — reintroducing the coarse
  grain the merger exists to remove.
- **Totals differ ~4.3%** (claude cost $63.42 vs $60.68; output 469,873 vs
  448,953), from launches whose capture never cleanly finished.
- **Bit-identity would preserve a bug.** `run_events` attributes spend to the
  *last* agent a process launched. Process `2eb82575` launched claude then
  opencode; its 5,197 tokens and $0.99 are recorded as `provider=opencode`,
  `model=opencode/glm-5.2`. `agent_turns` attributes them correctly to claude.
  57 of 358 processes launched >1 agent. Reproducing today's output exactly
  means reproducing the misattribution.

**Assumption taken (headless, proceeding):** Done-When #4 becomes *"totals
reconcile with every divergence attributed to a named cause, none unexplained"*,
and where the stores disagree, `agent_turns` is treated as correct. If Jack wants
literal bit-identity, the task is not "retire a recorder" but "freeze the bug",
and the design needs to change.

## 2. Directive's premise #1 is inverted (decided: design follows the measurement)

Directive: *"task/project session runners drop TurnUsage ... so their spend
reaches agent_turns via capture only, never the ledger."*

Measured: their spend reaches **neither** store. Both runners drive the vendor
CLI in-process via a `Harness` they own and never construct a `CaptureHandle`;
the subprocess is Anthropic's `claude` binary, not a child `lf`, so nothing
records on their behalf. Every one of the 70 `run_events` spend processes has a
launch — i.e. all recorded spend comes from the five capture sites, none from the
session runners.

Consequence: Task/Project Session spend — the dominant workload — is invisible in
`lf usage` **today**, before any cut. That makes PR1 (writers) the substance of
this task, and it is additive, which is in tension with Done-When #5 (net LOC
negative). Design measures net LOC across both PRs, not per-PR.

## 3. Directive's premise #2 is inverted (decided: cheaper fix than specified)

Directive: *"agent_turns ... misses opencode entirely"* and *"opencode launches
capture turns"* is listed as work to do.

Measured: opencode launches **already** capture turns (8 launches, 8 turns). Its
usage is parsed correctly by `engine/stream.rs`. It is then **destroyed** by
`opencode_mapping::complete_turn` unconditionally emitting a defaulted
`TurnUsage`, which `trace.rs` clones over the accumulated real usage. The fix is
one line at the source plus a value-asserting conformance test — not a new
capture path.

## 4. Migration ordinal race (decided: take 0.11.027 — closed)

`ls` showed max `0.11.025` while then-open PR #1028 already shipped
`0.11.026_lineage_boundary.sql` — the wave MEMORY hazard ("scan open PRs for the
table, not the ordinal"), live. #1028 has since merged; a re-scan confirms main
tops out at `0.11.026` and **no open PR ships any migration**, so `0.11.027` is
uncontested.

Keep the discipline, not the specific claim: re-scan open PRs at land time. The
scan is what settled this both times — including retracting a stale collision I
had flagged against PR #1018, which ships no migration at all (its files are
`child_control.rs`, `task/runner.rs`, `ci_fix_lifecycle_tests.rs`). A false
hazard left in a design doc is read later as true.

## 5. SQLite contention vs adding a writer (DECIDED: PR1 lands after ENG-7)

Wave MEMORY records the fleet-wide `database is locked` incident (2026-07-16).
ENG-7 owns deterministic concurrent ledger writes and is in kickoff; W2-284,
W2-285 and W2-287 are parked failed on the lock right now and are deliberately
not being resumed into it. PR1 adds ledger writers to that store.

**Settled: PR1 does not land until ENG-7 lands.** Not conditional on how the
contention looks at the time. The mitigating measurement — capture writes one
UPDATE per *turn boundary*, not per stream event, the rate `flowloop/wave.rs`
already sustains across 1,030 launches — argues the pressure is additive, not
multiplicative. It is still an assumption about a store that is stranding
Sessions, and the cost of being wrong is more dead bodies. Implement and review
PR1 freely; the gate is on landing only.

**Corollary: do not arm auto-merge on PR1.** `lf pr land` arms GitHub
auto-merge, which answers only to required checks and would cross this gate the
moment CI goes green (wave MEMORY, 2026-07-16: a completed review gate did not
stop an armed auto-merge on #1024). Publish PR1; land it by hand after ENG-7.

## 6. Deliberately out of scope: the parallel *parsers* (filed)

The deeper duplication is not two stores but two parser stacks — `StreamEvent`
(`engine/stream.rs`) and `ConversationEvent` (`harness/*_mapping.rs`) — both
alive, with `trace.rs` consuming both under *conflicting* semantics (accumulate
vs clobber). That conflict is the root cause of finding #3. Retiring the store
does not retire the parsers.

Filed as `92e0f253-f551-47d8-b74d-21a37e5dd551` — retiring the parallel usage
parsers. It must not start before both of this task's PRs land.
