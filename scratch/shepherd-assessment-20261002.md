# LOO-285 shepherd assessment — 2026-10-02

Continue the existing Task, but reconcile its accounting with the current release
controller before adding another isolated slice. The smallest useful next outcome
is a joined local proof that a scheduled opportunity survives invalid-candidate
recovery, retains its original failures and due identity, and settles only the
verified successor. Reuse run-records' recovery; do not build it again here.
This is a recommended implementation boundary, not permission to publish or a
reduction of the accepted full Task outcome.

## Scope and evidence

Jack requested a bounded read-only assessment. No implementation, sync, worker,
PM operation, schedule change, install, release, or delivery operation ran.
Only this new assessment was written. Existing design and review artifacts were
read as evidence, not rewritten or treated as current execution instructions.

The comparison uses locally available Git objects; no fetch or live provider
query was performed:

- Task HEAD: `7458ab8d6cd920b1de1c391757a151d72ea638a9`.
- Both local `main` and `origin/main`: `f4dfc620188aa16b78331c17e68cff2b00b32949`.
- Common ancestor: `88cf10641b0e88fc4bfcf504ef62bed4aa057539`;
  44 Task-only commits and 118 main-only commits.
- run-records: `6acd2389e878bbc47c1b80bdab8591ebf06a46fe`, containing
  `424958e9d90462dbc152416d4b5d81d10776e544`, simplification `dc7ef8341`, and
  main integration `4f323c768`. Its checkout was clean when inspected.
  `424958e9d` is not in the observed main. This does not establish current
  hosted PR or deployment state.
- This checkout has twelve modified tracked files and six existing untracked
  scratch artifacts. Their contents are part of the assessed implementation,
  not incidental dirt. A SHA-256 manifest of 2,000 tracked/untracked files,
  the index, and HEAD was compared after writing: every pre-existing file,
  index and HEAD remained unchanged; this assessment was the only added file.

Source inspection focused on cron/accounting/history, Flow attribution, release
selection/settlement, publisher validation, queue handling, and their named
tests. Earlier test passes below are retained reports, not fresh verification.
No tests were run in this read-only pass. The supplied installed-0.12.29 restart
failure on an unrelated foreign-Team Project is a reported execution blocker;
this assessment did not reproduce it or establish that main fixes it.

## What exists, and which notes are stale

The authored path is coherent: cron retains physical process receipts;
`ops/cron/accounting.rs` owns obligation segments, original dues, frozen catch-up
coverage and attempts; explicit `CronExecution` reaches
`release_run_with_cron`; the release operation owns selection and verification;
`settle` writes product outcomes. `finish_process` cannot promote a zero exit
to publication. `ops/cron/history.rs` joins retained failures and dispositions
and separately judges qualifying pairs. No new scheduler is required.

| Topic | Current evidence | Assessment |
| --- | --- | --- |
| Telemetry recovery | `release.rs:1245` freezes historical prerequisites and reserves one Recovery receipt; cron bounds observation and retains child exclusion | Older notes saying there is no retry, no bounded wait, or no historical telemetry segments are superseded |
| Historical schedules | Dirty `accounting.rs` retains telemetry segments and maps original release dues to them | Do not reimplement segment retention from the older review action lists |
| Closed obligations | `closed_unsettled`, disposition/history readers, and `scheduled_release_tests.rs:116` expose old owners outside the display window and retain dated assignment | Repair ownership exists; execution continuation still does not |
| Overlap | `accounting.rs:1066` calculates an exact next due from the retained calendar; `release.rs:685` uses it on scheduled target-lock contention | The generic-overlap gap in `review-closed-disposition.md` is superseded by `release-overlap-continuation.md` and current code |
| Caller and child preservation | `release_run_inner` fetches origin and verifies in owned source checkouts; `release_lock_tests.rs` covers surviving tag, publication, hook, Git, notes, lockfile and Task-revocation children | Substantial authored protection must survive integration; this pass does not certify every interruption boundary |
| Mechanical activation | Jack's September 28 steer records v0.12.24 installation and skill-to-Flow cron sync at the unchanged 10:00 schedule | Blanket claims that no activation ever occurred are stale; that activation does not prove this dirty implementation was installed or any qualifying outcome |
| Telemetry diagnosis | `scratch/evidence/baseline-summary.json` dates 36 failures and the missing `agent_turns` probe to September 24 UTC | Preserve all 36, including the original 35; do not describe that diagnosis as freshly reproduced today |

The later overlap report also explicitly excludes telemetry waits that cross
their saved next-due boundary. `verify_scheduled_telemetry` still derives its
retry text from the entry snapshot (`release.rs:1350` onward). Exact target-lock
continuation does not establish accuracy of every other wait message.

Closed execution is a real remaining boundary: `begin` (`accounting.rs:763`)
selects owners within one obligation, `receipt_context` (`:1037`) rejects a closed
segment, and `record_telemetry` skips closed segments. A previously authorized
attempt can still settle through its exact fence. Assigning a repair owner does
not resume an old candidate, move its Home, or authorize a new execution.
The accepted follow-up is same-context continuation; an old Home remains an
explicit blocker when authority cannot be preserved.

## Main and run-records change the next move

Main now uses `cmd: repo release run patch`, and telemetry contains `cmd: doctor`,
`cmd: __telemetry-scorecard`, and `cmd: usage --weekly`. Its Flow command executor,
Session/Exec model, Home selection and Task/PR interfaces differ from this
branch's older `op:` and Run-era integration. Port explicit receipt/descriptor
attribution through those current owners; replacing whole files would regress
unrelated work. Main's cron also records placement-preflight failures when Home
authority cannot be read (`record_cron_preflight_failure`); accounting must
preserve that path rather than require a successful launch to expose a due.

Main's `scripts/lifecycle_scorecard.py` consumes supplied SessionHistory JSON and
current Task PR facts. It contains neither the `agent_turns` nor
`agent_invocations` query still present in this checkout. The analytics repair
described as a proposed Intelligence handoff in old notes must first be compared
with that landed implementation (`aab595e6a`, followed by later history changes).
There is no evidence here for commissioning a duplicate repair. Source progress
does not clear historical failures or prove installed telemetry works.

The independent publication blocker is concrete: exact incident commit
`8cbd0c5b1c99a12151908c59f923b0128706a34f` contains both
`0.12.30.001_release.sql` and `drafts/remove_ask.sql` (confirmed with `git ls-tree`).
run-records' incident report records packaged preflight rejecting that candidate.
Its current absence from all publication providers was **not** checked here.
Main's publisher CLI compatibility fix (#1406) addresses a different failure.

run-records owns the implemented response: inspect the exact integrated source,
cut a successor when additional preparation is needed, preserve invalid tags,
require affirmative unpublished evidence before replacing a tagged candidate,
recover the minor's corrected closing patch, and freshly validate packaged
artifacts in a disposable `LF_HOME`, including cached reuse. Its
`inspect_source` checks GitHub, crates.io and versioned R2 publication and fails
on unknown external state. Its current notes retain live recovery/installation
and the full concurrent-merge-to-install proof as uncompleted obligations.

LOO-285 directly overlaps at three interfaces:

1. **Saved selection versus successor recovery.** `release_run_with_cron`
   (`release.rs:707` onward) sends a saved selection straight to
   `finish_candidate`, bypassing fresh selection. `accounting::begin` carries
   that selection into a retry; `accounting::select` (`:1210`) forbids replacing
   its tag/commit. Simply copying run-records' new-selection loop would leave
   scheduled recovery pinned to the invalid candidate. Integrate an explicit,
   evidenced recovery transition that retains the previous attempt/candidate,
   original due owner and failed proof. A successor cannot become a second
   opportunity or an extra success for the same wake.
2. **Packaged acceptance versus public proof.** This branch's
   `_validate_release_candidate` (`scripts/publish_release.py:187`) still ignores
   preflight rejection if authority is published, and uses the retired
   `LF_CONTROL_DB_PATH`. Run-records fixes that. LOO-285 separately adds UI-host,
   public hash/stage read-back and exact-version smoke. Both are necessary;
   neither receipt set substitutes for the other.
3. **Queue recovery and mutation ownership.** This branch's
   `wait_for_pr_merge` (`release.rs:2851`) treats BEHIND/DIRTY independently of
   queue membership. Current main/run-records use `observe_pr_merge` and
   `merge_needs_integration`: queued work waits, and AwaitingQueue does not
   require integration merely because it is behind. Main's `fac48dd22` also
   rechecks merged state after repair. Retain these refinements while carrying
   LOO-285's exact lock inheritance and typed Deferred outcomes through them.

The opposite integration hazard matters equally: run-records still calls
`sync_main` in minor/single release selection. Wholesale replacement of LOO-285's
controller would discard its caller-byte preservation. Reuse the recovery
decisions within the preservation boundary, not one complete controller over
the other. No parallel release implementation or new Task is warranted.

## Smallest remaining coherent outcome and next action

Keep LOO-285 / `task_863fc808db714040ab5993cd0d88d43e`, this checkout, its branch,
all authored commits and dirty content. The failed managed-Flow restart does not
justify recreating the Task, reopening planning elsewhere, or launching a
replacement worker. Its execution-path repair remains separately visible.

After the current read-only boundary, use the existing serial owner to reconcile
against current main and the run-records repair through the supported Loopflow
workflow. Do not run competing delivery or release operations. The next bounded
proof should start with an opportunity already holding a rejected candidate,
then demonstrate either a safely evidenced successor or a truthful unresolved
publication blocker, with the following observable invariants:

- Original dues, frozen coverage, failed checks, candidate/tag history and
  manual provenance survive; the opportunity receives at most one settlement.
- Unknown/partial external publication prevents replacement. A valid interrupted
  candidate resumes unchanged. Cached artifacts still undergo packaged acceptance.
- Queue waiting and repair use current main's evidence, while child exclusion
  and exact caller HEAD/index/staged/unstaged/untracked bytes survive.
- Required telemetry, repository, UI-host and public proof cannot be bypassed
  on either the old candidate or the successor.

That joined result is the smallest useful review checkpoint. It is not a
proposal to split the existing design's one coherent PR or to declare the Task
complete. Closed same-Home continuation, remaining interruption proofs and
affected-suite integration still follow. Existing focused tests in
`scheduled_release_tests.rs`, `release_lock_tests.rs`, run-records'
`release_tests.rs` successor/partial-publication cases, and publisher tests are
the concrete starting evidence; no new pass is asserted here.

## Acceptance and human judgment

The full finish line remains two adjacent original configured due opportunities,
settled by **two distinct automatic executions**, at least one with artifact
publication, all required checks, and no manual repair. Collapsed misses provide
accounting coverage but no extra qualifying settlement. `history.rs:318–400`
enforces adjacency, intervention exclusion and required proof; simulated dates,
manual release recovery, v0.12.24 activation and a later product release cannot
be substituted for the actual pair.

Current main's doctor still accepts only Scheduled receipts for continuity.
Recovery success cannot fill a missing natural receipt. Exercise the actual
telemetry Flow after integration and retain any resulting blocker; do not waive
doctor to make catch-up green. Record dated repair ownership through the supported
surface when authorized, preserving late assignment and the original 36 failures.
No accepted remote Intelligence handoff or current installed pass was established.
UI-host proof, public exact-tag evidence and supported installation remain real
acceptance obligations. There is no basis to choose a next due timestamp from
old snapshots without a current installed-obligation read.

No new product judgment is required to reconcile these already accepted
contracts or to make the local preservation proof. Jack already accepted the
one-execution catch-up policy and, in run-records' retained acceptance, safe
invalid-candidate recovery. The tension between immutable attempt evidence and
successor selection is an implementation/proof obligation, not permission to
overwrite history.

Human judgment would be required if continuation needs to transfer an old Home's
authority, if an analytics repair needs a newly accepted ownership handoff, or
if anyone proposes weakening verification, counting a collapsed/manual execution,
or changing the coherent-PR/acceptance boundary. None is selected or necessary
for the recommended local next outcome. External delivery, installation and
schedule actions remain outside this assessment's authorization; return their
concrete evidence and requested action at that boundary. The reported foreign-Team
restart failure should be repaired or routed through a supported Task-preserving
execution path, not worked around by mutating the unrelated Project.
