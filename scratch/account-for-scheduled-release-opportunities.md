# Account for scheduled release opportunities and settle product outcomes

## Problem

LOO-285 serves Reliability's release-settlement KR. Maintainers need to know
what happened to every configured release opportunity: when it was due, which
execution attempted it, what verification passed or failed, and what users can
download. Today these facts have separate owners but no durable connection.
`CronReceipt::Succeeded` means the scheduled target process exited zero. For
Infrastructure that process is an agent running the `release-run` skill. Its
successful exit does not establish release completion.

The accepted chapter requires two consecutive configured opportunities to
settle as truthful no-change or complete publication, with at least one
publication, required verification and exact-tag smoke passing, and no manual
repair. Deferred opportunities remain outstanding. Successful scheduling never
clears failed telemetry. This design preserves that entire finish line.

This is the kickoff artifact at source base
`88cf10641b0e88fc4bfcf504ef62bed4aa057539`. It specifies implementation and
operational proof; neither has been completed by this kickoff. No schedule,
release, installation, PM state, or Task disposition was changed.

## The demo

After two real configured firings, run
`lf release history --wave infrastructure --days 3 --json` (new read surface).
It shows consecutive original due times, exact cron and release attempts,
verification evidence, one published exact tag with downloadable verified
artifacts, and the next publication or verified empty release range. A delayed
wake retains the missed due times; an overlap shows a reason and continuation
instead of a second publisher. The same history view retains failed scheduled
verification and its repair disposition even when `lf doctor --json` reports
healthy scheduling continuity.

## Approach

Extend the existing cron receipt storage and release operation. Keep launchd as
the wake source, the installed obligation as the timing authority, the release
operation as the settlement writer, and the configured publisher as the product
side-effect owner. Add no daemon, queue service, general execution lease, or
parallel release implementation.

### Existing owners and integration points

| Surface | Current authority | Change |
| --- | --- | --- |
| `wave/infrastructure/GOAL.md`, installed launchd plist | Daily 09:00 telemetry and 10:00 release in Home-local time; durable activation | Preserve cadence. Persist obligation history and timezone evidence before replacement/removal. |
| `ops/cron.rs` | Physical launch receipts under `<LF_HOME>/cron/receipts` | Enumerate due release opportunities, retain wake/trigger provenance, associate each launch with attempts. |
| `lf/commands/doctor.rs` | Latest-due-interval scheduling continuity | Share its calendar calculation with accounting; retain its scheduling meaning. Release history separately projects settlement and verification health. |
| `.lf/flows/release-run.yaml` (new) | Repository selection of the scheduled target | One mechanical `op: release run patch`; replace the scheduled agent wrapper. |
| `ops/flow.rs`, `lf/commands/flow.rs` | Mechanical flow execution already supports release operations | Carry an explicit optional cron receipt reference to the release operation. |
| `ops/release.rs` | Selection, PR/candidate/tag recovery, stage leases, publication check | Persist typed attempts, verification, continuation, and terminal settlement through one shared entry point. |
| `scripts/publish_release.py` | Candidate hashes/stages and final publisher receipt | Retain those receipts; add exact public-artifact read-back evidence. |
| `.github/workflows/release.yml`, `package-build.yml` | Exact candidate native matrix and smoke | Keep required jobs; bind their workflow run and commit into settlement. |

The built-in `release-run` skill remains useful for explicit human/agent use in
other repositories. It ceases to be Infrastructure's scheduled execution path.
Catalog resolution and installed `target_kind` must converge to the repository
flow at ordinary cron sync. No fallback to the skill is allowed if that flow is
missing. The public cadence and `release-run` name remain unchanged.

### Durable identities and records

Use the current private Home cron storage with versioned JSON and atomic
replacement. Add `cron/obligations/` and `cron/opportunities/` beside existing
`cron/receipts/`; these contain different facts, not a second copy of execution
truth. Do not introduce SQLite tables just to join three local file records.

An obligation snapshot identifies the canonical repository, stable Home and
Wave identity, job name, schedule, activation/end times, and observed local
timezone. Preserve the existing activation across an unchanged sync. A changed
schedule, placement, or timezone closes one interval and opens the next;
replacement must persist the closed snapshot before unloading the old job.
Record target kind and resolved release operation/configuration as execution
provenance. The skill-to-flow cutover continues the same daily obligation rather
than dropping a due time or counting it twice.

An opportunity key is `(obligation identity, due_at UTC)`. Retain local date,
offset, and the next due time for inspection. Use a digest of canonical identity
fields for filenames; do not rely on the current lossy `safe_component` mapping
to distinguish identities. An opportunity is independent of its wake time,
attempt count, selected tag, and process PID.

`ReleaseOpportunity` contains:

- Obligation key and original due interval; discovery time and coverage origin
  (`observed` or explicitly incomplete historical reconstruction).
- Append-only attempt entries: attempt id, physical cron receipt id, optional
  attributed Run id, start/end, launch source, explicit intervention records,
  target, frozen selection evidence, current stage and typed outcome.
- Required verification references: exact command/config digest and subject
  commit, result, start/end, retained evidence path/digest or hosted run URL/id.
  A missing check is distinct from a failed check or a check not applicable to
  this outcome.
- One current disposition and a terminal settlement referencing the attempt
  that proved it. Repeated reconciliation of the same settlement is idempotent;
  a conflicting settlement is an actionable error and preserves both evidence
  inputs without overwriting the accepted result.
- Failure disposition when needed: cause, next action, owning Task/Work
  reference, disposition time, and linked successful retry if later repaired.
  LOO-285 is the disposition owner for release-accounting failures during this
  work; record that fact with the disposition command below, not a hardcoded
  production issue id. Ownership assignment is evidence, never automatic
  permission to start another Task.

Add `lf cron disposition <receipt-or-opportunity-id> --owner <work-id> --reason <text>` to
record an explicit repair disposition beside an existing failed receipt. The
write retains prior dispositions and their timestamps; it does not edit the
receipt or invoke PM. Release history joins this evidence for prerequisite
failures as well as release attempts. Missing/unresolved owner ids remain
unassigned in the report; an owner name in prose is not an accepted remote Task
handoff. For prelaunch opportunity failures with no receipt, the opportunity
record itself retains the disposition through the same storage operation.

Keep `CronReceipt` as the physical process result. The opportunity points to it;
there is no need to rewrite old cron receipts with inferred success or duplicate
their whole contents. A child release operation can settle before its wrapper
exits; later wrapper failure remains visible as a distinct fact. Publish
evidence may exist even when the release invocation fails after publication.

Retain the existing schema-1 cron reader because these are durable historical
receipts. Historical absence of opportunity, intervention, verification, or
timezone evidence is explicitly unknown, never a serde default that means
success. New wire DTOs have required or optional fields and fixture coverage.

### Enumerate, attempt, settle

1. At scheduled entry, materialize every due opportunity since the retained
   accounting cursor, using the existing daily calendar semantics. A bounded
   page is fine; advancing the cursor past an unwritten due time is not. The
   cursor is optimization only: deterministic keys permit replay after a crash.
   History readers also calculate missing due entries without writing, so a
   failed or absent wake cannot hide the denominator.
2. Preserve installed Home placement checks and preflight-failure receipts.
   Failed preflight accounts for a due opportunity with its cause; it never
   authorizes execution on the wrong Home. If persistence itself fails, return
   nonzero with the failed path. Readers must still expose the missing entry.
3. Execute due work oldest first through the existing mechanical flow. Carry
   the physical receipt reference explicitly in the flow execution context and
   typed release entry point; do not infer process role from an ambient env
   variable or parse agent stdout. Validate that reference against the installed
   job, repository, Home, and due record before adding evidence. Attribution
   never grants publication or process-control authority.
4. Acquire one narrow repository/target release-operation lock before release
   selection or mutation. Scheduled and manual `lf release run` share it. Use
   the existing OS file-lock primitive and retain existing generated-worktree
   stage leases and publisher lock. Acquisition of the target lock does not
   supersede a surviving stage lease. Record overlap as deferred, linked to the
   active attempt, without starting another publisher or bumping another tag.
   Standalone tag/publish mutations must honor the same exclusion; a publisher's
   nested `lf release publish` reuses its inherited exact lock descriptor rather
   than deadlocking on reacquisition or trusting an environment flag.
5. Reconcile the last exact candidate/tag and any durable intermediate before
   selecting new work. Write the attempt's selection and each completed stage
   before proceeding to the next consequential operation. Replace opportunity
   records under a short file lock and fence updates by exact attempt id.
   An old attempt cannot overwrite a retry or a terminal settlement.
6. After each attempt, rescan due opportunities that accumulated while it ran.
   Drain the discovered backlog serially until no due work remains or the
   existing bounded stage wait returns a deferred/failed result. On a stop,
   retain all remaining opportunities as deferred to the exact continuation and
   next configured firing. Do not busy-loop or add another timer.

Use a short per-job accounting lock for materialization and record updates;
never hold it across a network call or whole release. The longer target lock
owns release mutation. Use a fixed lock order and release the accounting lock
before attempting the target lock. Children performing side effects must retain
the release lock for their lifetime (inherited lock descriptor, or equivalently
the same existing stage lease covering their operation). Prove parent death
with a live publisher child: an unlocked parent PID or six-hour age is not
permission to publish concurrently. This is a release lock, not a new process
liveness registry. Never kill a process to make progress.

One wake may catch up multiple opportunities, but each requires its own
selection/verification decision. After the oldest publishes, the next can
settle no-change against the now-published tag. Do not copy one publication
receipt across all missed days and call them separate successes. If two
opportunities resume the same incomplete release, only the opportunity that
owns that release attempt receives that publication settlement; the later one
then makes its own decision.

Timing is orthogonal to product outcome. Retain `due_at`, first attempt time,
settled time, and delay seconds. For presentation, `on_time` means the first
attempt began in the due calendar minute; later starts are `caught_up`.
This is a reporting convention, not permission to omit late work or an SLA
that grants control authority. Product dispositions are:

| State | Required evidence | Continuation |
| --- | --- | --- |
| Pending/running | Original due key and current attempt/stage | Observed work, never a settled success |
| Published | Exact tag/commit, successful required verification, prepared hashes, publisher stages, public read-back/smoke | Terminal success; later external deletion is a new observation, not rewritten history |
| No-change | Previous completed tag, captured origin tip, exact empty target-scoped commit range, required verification for this opportunity | Terminal success; unavailable GitHub or unknown range is not empty |
| Deferred | Typed reason, retained PR/candidate/tag/stage, next configured retry time or exact active attempt | Outstanding; deadline expiring without progress remains visible |
| Failed | Named stage/cause, evidence, actionable repair and owner | Failed attempt remains immutable; repair may append a retry on the same opportunity |

Do not classify arbitrary error strings as retryable. Introduce typed release
failures only for actual release stages: queued/running external work with a
known continuation is deferred; a rejected verification, unavailable authority,
corrupt record, or unclassified command failure is failed. Existing one-hour
workflow/PR waits remain bounded. A failed verification is not disguised as an
intentional wait even if retrying later may succeed.

### Truthful verification and publication

All paths, including no-change and resume, enter one settlement routine. Today
`NoChanges` and early `resume_existing_release` return before `target.verify`;
the new entry must run applicable required checks or attach retained successful
evidence for the exact immutable subject and check definition. Current-state
checks such as the scheduled telemetry target cannot reuse yesterday's pass.

Preserve and identify separately:

1. Scheduled telemetry (`doctor` and `__telemetry-scorecard`) for the applicable
   daily obligation. A delayed release links the telemetry obligation preceding
   its original due time and any current prerequisite checks, not whichever
   old green receipt is convenient. Missing or failed verification blocks a
   qualifying settlement. Recovery is a new linked attempt of that verification;
   it does not erase the failed target.
2. Repository `release.targets.default.verify`, currently the migration check.
3. Required release-PR checks, hosted release acceptance, all four native
   packages, and their exact-version CLI/daemon smoke checks on the candidate.
4. Publisher candidate validation, signing/notarization, installer and website
   checks, and every configured publication stage.
5. Public exact-tag artifacts read back after publication: expected asset set,
   hashes matching the prepared receipt, version-pinned installer/package
   smoke in an isolated Home, and the immutable versioned DMG. Keep platform
   matrix smoke evidence from hosted builds; never claim cross-platform
   execution from one host's smoke. Website/deployment checks retain the exact
   release identity; mutable `latest` URLs alone cannot prove this tag.

The repository also documents `scripts/test.py --ui-host` as required. It is
not established by `--all` or package smoke. Implementation must inventory its
required evidence at the candidate/release boundary and include it; absent
capability is an explicit blocker, never an exemption invented here. This
kickoff did not run UI automation or prove a current permission gap.

For a missing/failed prerequisite, the release wake may make one bounded retry
through the existing cron target executor, carrying the original telemetry due
identity and recording an automatic catch-up attempt. It then observes the
result before release mutation. This permits recovery after a repaired check
without requiring an operator to manufacture a new receipt. It adds no timer
or independent worker. Limit prerequisite retry to once per prerequisite per
wake; if it still fails, retain the failure/owner and stop with the next
configured firing as continuation. Old failure evidence and late disposition
remain visible even after a linked successful retry.

No-change does not require rebuilding an unchanged published artifact, but it
must prove the empty range, the baseline tag's complete publication evidence,
and current required scheduled verification. A missing artifact/check for that
baseline means repair/resume, not no-change. For a resumed immutable candidate,
reuse exact matching recorded checks; if a required result or artifact has
expired or is absent, execute the necessary check again before settlement.

Keep the Python candidate and publisher receipts as their stage evidence.
Persist their digest and content needed for settlement before temporary
artifact cleanup. Add a publisher read-back/smoke mode rather than making Rust
reimplement the asset manifest. A crash after external publication but before
local settlement reconciles that exact tag and hashes and finishes the same
opportunity. It must not bump a new version or invoke credentialed publication
again merely to obtain a receipt. If no final publisher receipt was written,
reconstruct only the stages that can be independently checked, retry remaining
idempotent stages through existing recovery, and leave missing proof explicit.

“Published externally” and “qualifying complete publication” may differ. If
assets became visible but post-publication smoke failed, retain the external
effect and classify the attempt failed at smoke. Never roll back the fact that
users could already download the assets.

### Wake provenance, history, and cutover

`lf cron trigger` currently calls `launchctl kickstart -k` and the launched
plist always supplies `--scheduled`. That both obscures manual provenance and
can replace an active job. Remove `-k` for the ordinary trigger. Persist the
trigger request id/time before kickstart; associate or conservatively mark
overlapping observed firings as operator-triggered. An unused request expires
with a recorded disposition. An ambiguous trigger/natural firing cannot count
as autonomous proof. Direct manual runs remain manual. Neither creates a new
due time; an explicitly requested repair of an existing opportunity remains
marked intervention and disqualifies that pair from the no-manual-repair KR.

Use the existing local calendar rule: first occurrence of an ambiguous local
time, no occurrence for a nonexistent local time. Test DST and timezone
changes against the shared function. Record the effective zone/offset with
each due key and obligation segment. Unobserved historical timezone changes
cannot be reconstructed from today's zone; mark the affected denominator
uncertain. Never silently reinterpret prior local schedules as UTC.

At cutover, preserve the old installed obligation, receipts, and known due
times. Import unambiguous receipt associations as evidence only. Historical
successful agent exits remain `outcome_unknown` unless exact product evidence
can be linked. Begin complete observation at an explicit frontier while still
showing all prior chapter gaps/unknowns. This frontier does not waive the
chapter's universal accounting KR or manufacture historical no-change results.
Old Home obligations stay historical after placement changes; do not invent
cross-Home publication coordination or discard outstanding work. If an old
Home's active attempt cannot be reconciled, record the named placement blocker.

### Preserve caller work

`release_run` currently calls `sync_main`, which can reset another checkout,
stash overlapping edits, and leave their recovery to a human. That does not
satisfy this Task's unchanged caller branch/index/working-bytes contract.
Select from fetched `origin/<default>` without resetting the caller or sibling
main. Run checks requiring a source checkout in the existing owned release
worktree at the selected commit. Continue using the existing generated-worktree
classification and stage leases for reuse and cleanup. No general rewrite of
`sync_main` or new worktree platform is needed.

## De-risking

| Question | Finding | Impact on design |
| --- | --- | --- |
| Are the 35 telemetry failures merely old continuity gaps? | Fresh 35-day read has 36/36 failed telemetry targets. Latest log passes continuity then fails scorecard on missing `agent_turns`; a read-only SQL prepare reproduces the missing table. | Retain original 35 and the new failure. Name the schema-mismatched scorecard as a concrete verification blocker; scheduling repair alone cannot green the KR. |
| Is publication still blocked at v0.12.14? | Current `lf release status` reports v0.12.19, successful workflow 35894647999, and a GitHub Release. Existing publisher receipt contains exact commit, hashes, and all stages. | Do not resurrect LOO-261 or infer that all successful agent exits hid failures. Current publication is positive evidence, not the required consecutive pair. |
| Can launchd identify every missed opportunity? | Apple's calendar semantics coalesce missed sleep intervals; power-off waits for a later designated firing. The plist passes no original due time. | Enumerate retained obligations at wake; actual start is never opportunity identity. [Apple guide](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/ScheduledJobs.html), [Apple launchd manual source](https://github.com/apple-oss-distributions/launchd/blob/main/man/launchd.plist.5). |
| Can existing continuity be reused? | Activation is preserved across unchanged sync; `latest_due_interval` uses local calendar rules and exact scheduled receipt identity. It checks one current interval, intentionally accepting failed targets as proof of firing. | Extract its calendar function; retain that scheduling meaning and add separate health checks rather than changing firing into publication truth. |
| Does release recovery already own durable intermediates? | Exact candidate refs, prepared artifacts, prepare/publish worktree classification, and OS-backed stage leases are implemented and covered by targeted Rust fixtures. | Reuse these mechanisms. Add operation-level selection exclusion and settlement, not a second recovery machine. |
| Is the mechanical flow possible without a new cron target type? | Existing `Op` parser and `execute_flow_ops` already dispatch `release run`. | Use a repo flow and explicit context plumbing; no new scheduler command language. |
| Are manual launchd triggers distinguishable? | Current trigger uses `kickstart -k`; plist identifies all resulting starts as scheduled. | Record trigger provenance and remove forced replacement before claiming unattended proof. |
| Does publication prove artifact accessibility? | Rust completion reads `isDraft`; publisher writes local hashes/stages. Neither alone is retained public download/smoke evidence. | Add exact public read-back to the publisher's existing verification responsibility. |
| Can required checks be skipped on resume/no-change? | Source has early returns before `target.verify`; no-change has only target/tag in its result. | Funnel outcomes through one checker and retain the exact range and required evidence. |
| Are caller bytes protected on every exit? | `sync_main` explicitly resets and can retain overlapping changes in a stash; the tests expect that behavior. | Avoid that helper in release selection. Keep branch/index/bytes untouched on every exit. |

Primary observations and limits are in [research.md](research.md), with captured
CLI receipts under [evidence/](evidence/). Six existing publisher tests passed;
they validate current local preparation behavior, not this design or live
publication. No independent current artifact-publication failure was reproduced.

## Alternatives considered

| Approach | Tradeoff | Why not |
| --- | --- | --- |
| Correlate cron starts, agent text, and latest release after the fact | Almost no execution change | Ambiguous manual runs, delayed wakes, and overlapping releases make the join non-authoritative. Cannot prove no-change or crashed settlement. |
| New release queue/controller and transactional store | Centralizes all scheduling and release state | Duplicates launchd, existing obligations, stage recovery, and publication machinery. The migration/ownership problem is as large as the requested fix. |
| Extend existing cron evidence plus the mechanical release operation (chosen) | Requires narrow identity/context plumbing and explicit settlement | Keeps owners intact and reaches the user's observable outcome. Durable records are not allowed to dispatch work independently. |

## Key decisions

- Success is product evidence. An agent/process exit and a GitHub tag are
  supporting facts, never settlement shortcuts.
- Attempts can fail repeatedly; due identity is stable. Retry history and
  external partial publication survive recovery.
- Retain failure ownership and its age independently of later green checks.
  Release history reports undispositioned failures past one day. The latest
  scheduled failure stays visible even when continuity passes. Do not feed
  this retrospective health projection into `doctor`'s exit status: telemetry
  contains `doctor`, so making a prior unsettled release fail that prerequisite
  creates a cycle. Existing doctor checks and scorecard failures still gate
  telemetry; the new history view does not change their verification contract.
- The scorecard's obsolete SQL is an observed verification dependency. LOO-285
  retains responsibility for the release acceptance blocker; Intelligence owns
  correction of analytical evidence, consistent with the chapter boundary.
  Record a concrete handoff before any separate Task is selected. No new Task,
  parallel implementation slot, or remote assignment is created by this doc.
- Historical UI capability notes are not a fresh diagnosis. Missing current
  required evidence blocks the claim; a real probe must establish the cause.
- Wild success: a wake after a long sleep shows every missed day, finishes the
  existing candidate, and leaves a verifiable publication followed by a true
  no-change with no operator archaeology.
- Wild failure: a green accounting screen masks red verification, every retry
  invents a new release, or a stale process deletes another attempt's artifacts.
  The proof matrix below specifically rejects those outcomes.

## Scope

- In scope: due opportunity accounting; typed release outcomes; explicit cron
  linkage; delayed wake and overlap; bounded recovery; verification failures
  and disposition; public exact-tag proof; caller preservation; CLI history,
  shared doctor calendar calculation, repo flow, and matching docs/fixtures.
- Out of scope: scheduler replacement, arbitrary cron grammar, new resident
  services, multi-product deployment framework, general process liveness or
  worktree leases, PM redesign, resurrecting completed historical Tasks, and
  Intelligence's broader usage/operation-population implementation.
- The fourteen-day availability KR is separate. This work supplies release
  evidence to it but does not claim that duration window.

## Done when

### Focused behavioral proof

Add behavioral cases to existing cron, doctor, release integration, flow, and
publisher tests. Use real temporary repositories/files/OS locks and simulated
external services. Assert durable outcomes and preserved bytes, not mock calls.

| Counterexample | Required observation |
| --- | --- |
| One on-time firing, unchanged sync, repeated same-minute firing | One due key, preserved activation, one accepted terminal settlement |
| Wake after three due times; release lasts across another due time | All original due times remain; independent decisions, ordered attempts, no duplicate publication |
| Manual run or `cron trigger` near a natural firing | Manual/intervention provenance retained; no fabricated opportunity or qualifying autonomous pair |
| Overlapping scheduled/manual release selection | One target mutation owner; loser is deferred to a real continuation |
| Parent dies while publisher child remains active | No second publisher or cleanup of the active stage |
| Crash before launch, after tag, after public release, before settlement | Unsettled opportunity remains visible; exact same candidate/tag resumes or reconciles once |
| Old attempt returns after retry or terminal settlement | No terminal regression; rejected evidence retained diagnostically |
| Agent exits zero without release evidence | No publication/no-change settlement; mechanical path does not depend on agent prose |
| Successful continuity, failed telemetry | Independent red verification and dated owning disposition; no qualifying success |
| Failed/missing exact-candidate check on no-change or resume | Failure/deferral with evidence, never a bypass |
| Draft release, wrong commit/hash, absent asset, smoke failure | No complete publication settlement; any external effects remain recorded |
| Corrupt JSON or interrupted atomic replacement | Fail with path/cause, preserve last valid evidence; never turn a missing row into no-change |
| Schedule change/removal, timezone/DST, changed Home | No invented/dropped due times; uncertainty explicit where history is absent |
| Staged/unstaged/untracked caller edits and local commits, including overlapping upstream paths | Exact caller HEAD/branch/index/working bytes unchanged after every failure, retry, and success |
| Independent target/repository | No opportunity collision or unintended shared publication lock |

Suggested affected-suite commands after implementation:

```sh
cargo test -p loopflow --lib ops::cron::tests
cargo test -p loopflow --lib lf::commands::doctor::tests
cargo test -p loopflow --test release_tests
uv run pytest python/tests/test_release_publisher.py python/tests/test_release_automation.py
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Also run the actual added flow/context and DTO fixture tests and the design's
caller-preservation cases; update commands to their final names. Do not claim
a successful compiler/static check as configured operational proof.

### Configured-path acceptance

1. Capture installed obligation, Home, binary/source digest, effective timezone,
   release config and required-check inventory, and unresolved historical
   failures. Record cutover and any unaccounted chapter intervals.
2. Install through the supported release/install path and sync cron using `lf`.
   Preserve the 09:00/10:00 cadence. Record actual installed flow resolution.
3. Observe two adjacent real due opportunities. Do not manually trigger them,
   alter their dates, or select two nonadjacent green rows. Delayed automatic
   catch-up is eligible when it retains original consecutive due identities.
4. Retain exact attempts, verification records, release workflow/check URLs,
   tag/commit, publisher manifest, asset download hashes, isolated exact-version
   smoke results, and intervention provenance. At least one opportunity must
   publish. Both must settle no-change/publication without manual repair.
5. Inspect `lf release history` and `lf doctor` from the installed CLI. Prove
   every due opportunity is present or explicitly missing/uncertain, all
   deferred work names its continuation, and failures name an owner within one
   day. A late disposition must remain visibly late.

If verification remains blocked, complete the code proof but keep operational
acceptance open with the named blocker. Do not mark the Task/KR complete on a
fixture, on the v0.12.19 observation, or on two arbitrary manual runs. A pending
configured opportunity is a declared wait with its exact next due time, not a
reason to keep an agent sleeping for a day. The pinned lifecycle owns landing
and final Task disposition; this kickoff does not invoke it manually.

## Forbidden outcomes

- A scheduler/process receipt promoted into product success.
- Failed telemetry disappearing because scheduling continuity is green.
- A new tag used to escape a resumable incomplete tag or missing proof.
- One publication counted repeatedly across due opportunities.
- Manual trigger/repair counted as unattended success.
- Missing verification treated as pass, or required UI/host checks silently
  dropped to obtain a publication receipt.
- A timeout, stale PID, or unclaimed process interpreted as mutation authority.
- A second release state machine, daemon, generic deployment platform, or
  historical receipt rewrite.
- Caller edits left only in a stash, changed index state, reset local commits,
  or cleanup of dirty/divergent/live-owned generated worktrees.
- A new observation frontier that silently removes earlier chapter failures
  or uncertain due opportunities from the denominator.

## Internal slices

These are ordered cuts within one coherent PR, not separate Tasks or deployable
half-contracts.

1. Extract calendar calculation; add retained obligation/opportunity records,
   migration of historical evidence, atomic persistence, and identity tests.
2. Replace scheduled agent wrapping with the mechanical flow; carry explicit
   receipt context, preserve trigger provenance, and account for wake/backlog.
3. Converge manual/scheduled release paths on operation locking, typed outcomes,
   exact-stage recovery, caller-preserving selection, and single settlement.
4. Bind required checks and publisher read-back; add the release-history view,
   failure disposition, DTO fixtures, and user docs. Exercise preservation and
   interruption tests through the joined path.
5. Run configured acceptance. Name and retain any independent blocker before
   selecting further work; do not split around required proof.

## This slice

Kickoff: map current authorities, reproduce the leading verification blocker,
capture baseline facts, and specify the joined outcome/preservation proof.
Next executable implementation cut is slice 1, carried through slices 2–4
before claiming the feature works. Slice 5 is required operational acceptance.

## Slice ledger

- 2026-09-23 local / 2026-09-24 UTC: read assigned Wave memory, accepted
  Reliability chapter proposal, source and installed cron/release surfaces.
- Captured 70 physical receipts: 36 failed telemetry, 33 successful release
  targets, one failed release target. This extends rather than erases the
  accepted 35-failure counterevidence.
- Observed v0.12.19 publication via `lf release status` and existing publisher
  receipt. No new publication or live artifact smoke performed.
- Read-only SQLite prepare reproduced `no such table: agent_turns`.
- Existing publisher tests: `uv run pytest python/tests/test_release_publisher.py
  -q` — 6 passed in 1.10s. No Rust, full CI, or UI suite run for this design-only
  change.
- Simulated code review changed the design: separate timing from product
  outcome; preserve manual trigger provenance; reject main-reset/stash as
  caller preservation; keep parent-death/child-lock and post-publication-crash
  cases; distinguish a real published tag from a qualifying KR settlement;
  remove the proposed doctor health gate that would create a telemetry/release
  dependency cycle.

## Measure

Use the Reliability KR's actual due population, not successful process counts.
Report configured due count, accounted count, unresolved/unknown count,
on-time/caught-up attempts, product outcomes, maximum failure-disposition age,
and consecutive qualifying settlements. Always retain the original due times
and intervention flags. Baseline is 70 physical receipts with no authoritative
opportunity join; it cannot supply a truthful historical settlement percentage.
