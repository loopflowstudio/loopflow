# LOO-285 slice review

## Verdict

**Iterate. Do not publish as an approved slice or mark the Task complete.**
The implementation advances the reviewed architecture: original due identities,
one execution for frozen catch-up coverage, explicit attempt attribution, one
atomic settlement writer, existing recovery, and independent verification health.
It still falls short of the complete design in both implementation and operational
proof. Two bounded defects reproduced during this review are repaired below.
The remaining work belongs in this Task and serial PR; no sibling Task was opened.

Reviewed starting head: `4fbe6d05131c2272a3a9a30616ed9a75059eb95f`.
The complete, untruncated branch patch was obtained through
`lf task diff LOO-285 --json`. Review follows the changed source, CLI plumbing,
new accounting/history documents, Python publisher, fixtures, and release docs.

## Demonstration and limits

Built the candidate with `cargo build -p loopflow --bin lf` and read actual Home
history through `target/debug/lf release history --wave infrastructure --days 35
--json`. At timestamp 1790218883 it returned 70 physical receipts, 36 failed
telemetry targets, 37 undispositioned failures, no qualifying pair, and a null
observation frontier. Full output: `evidence/review-history.json`. Zero due rows
mean unknown pre-cutover accounting, not a zero denominator. The ambient stale
Wave-id warning recurred; requested file-backed history still returned normally.
No auth/registry repair was performed. `flow show release-run` resolved to the
mechanical `op: release run patch` in this checkout.

This is a real CLI read of retained production evidence. It is not an installed
scheduled execution, publication, or exact-tag smoke. No production schedule,
release, or installation was mutated to manufacture proof. Existing shell-target
and publisher fixtures remain labeled local simulations.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Original due identity and repeated firing | Stable due key and activation; no repeated settlement | Deterministic obligation/due identity; exact attempt fencing | Accounting idempotence and unchanged-sync tests; `observe`, `begin`, `settle` | pass locally |
| Delayed wake and new due during execution | Three misses, one execution; fourth waits | Frozen covered keys and coalescing in one document; explicit next-firing wait | Existing delayed-wake test and real shell cron target test | pass locally |
| Crash during reciprocal linking | No partial links or second owner | Whole obligation document atomically replaces all links | `save`, `begin`; corruption and replacement tests | pass by structure/local proof |
| Preserve candidate through retry | Original owner/tag/commit survives failed attempts and preflight | Preflight originally erased selection on its newest attempt; repaired here | Red then green `unchanged_sync_preserves_observation_and_failed_attempts_on_retry` | pass for reproduced boundary |
| Manual provenance | Trigger and manual repair cannot count unattended | Trigger requests, `Triggered`, interventions, inherited descriptor validation | Prior accounting/history tests; trigger no longer uses `-k` | pass locally; natural/trigger race live proof absent |
| Overlap | One mutation owner; loser has continuation | OS target lock and per-job execution lock; retained overlap receipt | Lock fixture and `record_overlap`; continuation is often prose rather than exact due/active attempt | gap in complete continuation proof |
| Parent death with live child | Side-effect child retains exclusion | Publisher prepare/publish/verify inherit target fd; other mutation subprocesses do not | `ReleaseLock::inherit`, `tag_and_push_ref`, `run_output`, `run_stdout` | gap |
| Crash around tag/publication | Reconcile same exact candidate and all stages | Saved selection and candidate/publisher receipts; public read-back | Existing same-tag and Python crash-before-final-receipt fixtures | partial; full configured interruption proof absent |
| Late evidence | Reject superseded or conflicting settlement | Exact receipt fence and retained rejection input; proof and outcome atomic | Prior accounting preservation tests | pass locally |
| Zero process exit | Cannot imply product success | Successful unverified wrapper becomes `Unverified` | Existing shell cron-target test; `finish_process` | pass locally |
| Failed telemetry despite healthy scheduling | Failures remain visible, block success, have dated owner | Current telemetry gates selection; history retains failures and dispositions | Live CLI: 36 failed targets, 37 unassigned failures | retention passes; ownership/repair remains gap |
| Telemetry for every original covered due | Retain each historical prerequisite and recovery link | Only current due telemetry is looked up; historical receipts are a separate unjoined list | `verify_scheduled_telemetry` uses now and two-day receipt window | gap |
| Checks on exact candidate | Resume cannot borrow current main's pass | Originally did; now `finish_candidate` verifies immutable candidate before build/tag/publish | Red then green new resume integration test | pass for reproduced boundary |
| No-change | Empty target range, complete baseline, current checks | Captured origin SHA, empty-range recheck, hosted/public baseline checks | Source and caller-preservation test; no scheduled no-change integration | gap in joined proof |
| Every publication stage | Public exact hashes, native smoke, UI proof, all configured publication stages | Hashes, versioned DMG, website health, crate version, local pinned installer smoke; missing latest-alias read-back | `verify_release`, `publish_release`, Python fixture | gap for complete stage reconstruction |
| Corrupt persistence | Error names path, last evidence not rewritten | Strict parse and atomic rename | Existing corrupt-record test | pass locally |
| Schedule/timezone/Home changes | Keep denominator and explicit continuation/blocker | Old segments retained/closed; old unfinished attempts get no explicit disposition | `observe`, `close`, `receipt_context` rejects closed obligations | gap for unresolved old work |
| Caller preservation | Branch/index/bytes preserved on every exit | Origin selection avoids reset/stash; owned check worktrees | Existing test covers no-change and verification failure with dirty caller/local commit | pass for those exits; success/recovery matrix incomplete |
| Independent target/repository | No collisions or unintended serialization | Hash target lock under canonical repo, obligation identity includes repo/Home | Independent-target lock test; identity source | pass locally; complete cross-repository execution proof absent |
| Two consecutive configured settlements | Two automatic executions; at least one real artifact publication; no repair | Summary rejects collapsed/manual/unknown/unverified rows; no actual qualifying pair yet | DTO/history tests and live history | gap |

## Reproduced and repaired

1. **Wrong source accepted on resume.** Added a temporary-repository integration
   case where v0.9.1 lacks a file required by the configured verification hook,
   while newer pushed main contains it. Before repair, `release_run` returned
   `Resumed` for the failing tag. Now shared `finish_candidate` verifies the exact
   candidate and appends its proof before hosted work, tag push, or publisher
   mutation. This covers initial and saved-candidate resume plus newly merged
   candidates. The saved-selection caller's duplicate check was removed.
2. **Preflight discarded recovery selection.** Extended the candidate-retry test
   with an intervening placement-authority failure in the same due interval.
   Before repair, the next wake changed opportunity owner because the newest
   failed attempt had `selection: None`. Preflight now carries the prior exact
   selection and target. All failed attempts remain, and the next wake retains
   the original candidate owner. The test failed before and passed after repair.

## Required next slice

Keep the complete design as the north star; do not narrow completion to these
passing cases.

1. Complete the joined scheduled path proof through the actual cron executor,
   CLI flow, and typed release entry with a disposable Home/repository and
   simulated external services. Prove published, no-change, rejected telemetry,
   and post-publication failure in durable history. The current shell substitute
   only proves argv/receipt propagation and unverified zero exit; it cannot prove
   the real flow settles correctly.
2. Retain each covered original telemetry obligation and missing/failed receipt
   association, separately from current recovery proof. Resolve the documented
   bounded-prerequisite retry choice explicitly against the approved design.
   Keep all original failures and lateness. The missing `agent_turns` scorecard
   table remains a demonstrated verification blocker, not a demonstrated new
   artifact-publication failure. Record the owning repair disposition/handoff
   through authorized work before claiming that dependency resolved.
3. Carry operation exclusion through every mutating subprocess, including tag
   push and PR operations, or prove equivalent existing stage ownership. Current
   `run_output`/`run_stdout` create children without `ReleaseLock::inherit`;
   Rust file descriptors close on exec. The passing surviving-shell fixture
   covers an explicitly inherited child only. Add a real isolated parent-death
   test with a surviving mutation child and contender before claiming the full
   exclusion contract. Do not add process-age takeover or another liveness store.
4. Reconcile all configured publisher stages when reconstructing from a retained
   candidate. `publish_release` writes `Loopflow-latest.dmg`, but `verify_release`
   only checks the immutable versioned DMG; a candidate-only receipt can qualify
   without evidence of that configured stage. Keep immutable exact-tag proof
   primary, retain missing stage evidence, and repair through existing publisher
   recovery. This is a source-level proof gap, not a live incident reproduced.
5. Give unresolved closed obligation segments an explicit reason and supported
   continuation/disposition. `observe` can close an unfinished candidate while
   `begin` only examines the new segment. History preserves the old row but can
   leave it `Running` indefinitely. Preserve old Home authority; do not silently
   transfer publication rights. Cover schedule, timezone, and placement changes.
6. Complete the remaining caller-preservation and interruption matrix, then use
   supported install/sync on the owning Home and observe two adjacent real
   scheduled executions. Required UI-host and exact public-artifact smoke remain
   mandatory. No manual trigger or changed date may manufacture the pair.

## Negative architectural proof

- No `sync_main` remains reachable from release selection. Fetching origin and
  source-check worktrees replace the caller-reset path.
- No `PublicationEvidence`, `NoChangeEvidence`, or `record_verification` remains
  in production. `settle` is the one typed product writer; `finish_process` only
  fills missing process failure/unverified evidence and cannot overwrite success.
- Infrastructure's repository flow resolves to one mechanical release op. The
  generic built-in skill remains for explicit use; this checkout's flow does not
  route its scheduled operation through agent prose. Installed cutover unproven.
- No new scheduler, SQLite table, daemon, or generic release service was added.
  Obligation documents own timing/attempt links; schema-1 receipts own physical
  process observations; publisher receipts own artifact stages. Those records
  describe different facts rather than duplicate settlement writers.
- The remaining unprotected mutation-child paths are explicitly NOT accepted as
  satisfying the required single-writer architecture merely because the local
  inherited-publisher test passes.

## Validation

New regression tests were each observed failing before repair and passing after.
Final focused success-path and static checks are recorded below after completion.
No full suite, hosted matrix, UI automation, signing, or live publication ran.

Final checks for this review's repairs:

- `cargo test -p loopflow --test release_tests release_resume_rejects_candidate_that_fails_checks_even_when_origin_passes` — passed after the reproduced failure.
- `cargo test -p loopflow --lib unchanged_sync_preserves_observation_and_failed_attempts_on_retry` — passed after the reproduced owner-loss failure.
- `cargo test -p loopflow --test release_tests release_run_prepares_signed_artifacts_before_pushing_the_version_tag` — passed (12.71s execution).
- `cargo test -p loopflow --test release_tests release_run_resumes_an_existing_explicit_tag` — passed.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` — passed.

The built-CLI history demonstration preceded the two repairs; neither repair
changes that read projection. Final repairs compiled in the focused tests and
all-target Clippy. No refreshed configured runtime proof is claimed. Review
withholds PR publication because the matrix still has implementation gaps, not
because another permission question is required.
