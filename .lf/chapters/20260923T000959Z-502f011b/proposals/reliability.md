**Agree with the accepted Infrastructure boundary.** Rename Stability & Security to **Reliability**, retain its stable identity, and concentrate this chapter on dependable releases and usable self-hosting. Reliability owns runtime repairs and their operational outcomes. Intelligence owns the evidence needed to measure those outcomes; Architecture Minimalism owns architectural reduction.

This is a proposal for Gate 2. No PM, Work, charter, repository files, or execution state were changed. No human session, controller, child Run, review rerun, or auth repair was started.

The chapter is `20260923T000959Z-502f011b`. Next review: **2026-10-21T00:09:59.226362+00:00**.

**Project identity and definition**

| Field | Proposed value |
|---|---|
| Linear Project UUID | `03c59a52-dbd4-4e11-a6ac-f6d6359c8e08` — unchanged |
| Project Work | `proj_33676884d672454780209ce4e5b624fa` — unchanged |
| Name | Reliability |
| Derived slug | `reliability`, replacing `stability-security` |
| Parent Wave | Infrastructure |
| Linear Initiative | `218967b6-a760-4b7c-9a46-11d9d61a42c2` — unchanged |

Proposed definition:

> Maintainers can keep Loopflow advancing its own work and delivering verified releases without repeatedly repairing its coordination machinery. Scheduled delivery reaches truthful, recoverable outcomes; planning and execution remain usable across credential expiry, process failure, and upgrades. Recovery preserves work, history, and exact control authority.

Apply the rename to the existing Project. Because names determine slugs, the parent must reconcile name-based references when applying it; recreating the Project would sever useful identity and history.

**Exact proposed KR ledger**

Both KRs begin unchecked. Neither inherits a passing judgment from merged Tasks or successful agent exits.

1. **Release settlement and verification.**  
   “Throughout the chapter, every configured scheduled release opportunity is durably accounted for as on-time, caught up, deferred with a recorded reason and continuation, or failed with an actionable cause. Two consecutive opportunities settle through the configured path as a truthful no-op or complete exact-tag publication; at least one publishes artifacts available to users, all required verification and exact-tag smoke checks pass, and neither settlement requires manual repair. Required scheduled verification failures remain visible and receive an owning repair disposition within one day.”

2. **Real self-hosting availability and continuity.**  
   “Across fourteen consecutive days of real Loopflow self-hosting, every attempted core planning or execution operation reaches observable progress, an intentional declared wait, or one bounded, truthful, actionable failure. Ordinary credential expiry with a usable refresh grant does not interrupt planning; recoverable process or controller failures resume without manual state repair. There are no hidden outages, duplicate advancements, lost durable input, terminal-truth regressions, or false availability claims. Recovery preserves assigned-worktree isolation, caller edits, durable history, and exact process authority.”

For KR 1, “configured” means the actual installed schedule and release path. Current Infrastructure configuration schedules daily release attempts; the old weekly wording cannot silently redefine that denominator. A caught-up obligation must retain its original due opportunity, and a deferred obligation remains outstanding until settled. Two arbitrary manual invocations do not prove consecutive scheduled opportunities.

For KR 2, a truthful error is necessary but does not make a recoverable outage successful. A valid human gate is an intentional wait; expired-but-refreshable credentials are a reliability failure. Unknown outcomes remain unknown and prevent the universal claim from passing. Evidence must identify the fourteen-day population and report progress, waits, failures, and missing observations separately.

**Why these commitments fit the evidence**

The completed baseline found all **35 observed telemetry targets failed**, including the latest fourteen verifications. It also found **32 of 33 release-run targets reported success**, while a September 22 release log said “Release did not complete.” Those observations cannot establish publication.

Current code explains the distinction: `run_cron` in `rust/loopflow/src/ops/cron.rs` records success when its target process exits successfully. The release command separately checks publication state, resumes incomplete releases, and runs configured verification. This supports repairing the outcome boundary; it does not prove that every historical successful receipt concealed a failed release.

The baseline also records expired Linear credentials blocking Technical Architecture, Performance & Efficiency, and Stability & Security. LOO-279’s existing design distinguishes the September 22 observation from retained August 28 failure timestamps. We should preserve that distinction rather than count them automatically as separate incidents. Its retained error text cannot distinguish rejected credentials from transport or provider failure, so concurrent refresh remains a hypothesis.

The frozen status has no Project-owned metrics. The baseline lacks the historical strand, writer-ownership, forwarding, and detection-to-recovery populations required by the old KRs. A new duration window starts when evidence coverage exists; it cannot be backfilled from memory.

**Disposition of every old KR**

| Old KR | Baseline verdict | Proposed disposition |
|---|---|---|
| Seven days without strands, lost/duplicated commands, or terminal regression | Unknown | **Rewrite into KR 2.** Preserve these continuity outcomes, add the experienced planning/execution result, and require an explicit fourteen-day population. |
| Thirty days of one writer and no implementation outside assigned worktrees | Unknown | **Retire as a standalone duration claim.** Preserve isolation as a KR 2 acceptance condition and dispatch invariant. Do not invent a filesystem audit or add a general worktree lease. |
| Thirty days without expiry blocks or excess forwarded secrets | Does not hold | **Rewrite into KR 2.** LOO-279 addresses ordinary expiry. Least-authority forwarding remains mandatory; the absent forwarding audit stays an evidence limitation. |
| Four weekly releases and fourteen nightly verifications without repair | Does not hold | **Replace with KR 1.** Preserve publication and required verification, using the accepted minimum of two consecutive configured-path settlements and at least one publication. |
| Thirty days of pre-human drift detection and state-preserving recovery | Unknown | **Rewrite the operational outcome into KR 2.** Retire the unsupported “before a human” universal claim. Intelligence supplies detection evidence; Reliability repairs actual failures and preserves state. |

**Opening work**

**Carry LOO-279 as the immediate, sole active opening Task.**

Retain issue UUID `15172e70-1ec8-4cb0-a15c-f84d49361381`, Work `task_81bb835a6bd04748bf99e906aa9f4236`, its existing worktree, branch, and serial PR identity `pr_4c170525e45e4cbdb9011fcd130aae4d`.

The frozen status reports uncommitted work, no authored commits, and a working first PR without a GitHub publication. I inspected its [existing design](/Users/jack/src/loopflow.make-linear-oauth-recovery-survive/scratch/make-linear-oauth-recovery-survive.md) read-only. It explicitly says implementation evidence is pending. The inspected resolver still performs refresh followed by an unconditional token upsert and emits blanket reconnect guidance after expired-token refresh failure.

Carry the existing Task rather than create another auth repair. Its completion must demonstrate:

- The installed Project planning path crosses expiry, persists a complete credential rotation, adopts current PM truth, and continues.
- Concurrent or stale refresh results cannot overwrite a newer credential generation.
- Transient failure preserves credentials and produces bounded retry guidance.
- Confirmed unusable credentials stop stale-plan execution with sanitized, Doppler-aware reconnect guidance.
- Refresh grants and client secrets remain on their owning Home.

The design already proposes these behaviors. Its protocol assumptions and proofs belong to implementation validation; this proposal does not certify them.

**Select one subsequent release Task, queued behind LOO-279:** “Account for scheduled release opportunities and settle actual release outcomes.”

This is proposed work without an invented issue ID. Its bounded outcome is one authoritative connection between each due opportunity, its execution attempt, verification, and product result. It must distinguish published, no-change, deferred/resumable, and failed outcomes; account for wake delays and overlap without duplication; and demonstrate the two-settlement publication commitment.

Use existing release recovery and obligation-aware continuity code. Do not rebuild the scheduler or create a new release platform. If investigation reveals an independent publication blocker, name that blocker before splitting work. Keep at most one active Reliability implementation Task initially; this is a planning choice, not a runtime restriction.

The 35 failed telemetry targets remain counterevidence. A successful scheduler firing does not clear them, and required verification cannot be bypassed to obtain a publication receipt.

**Historical Task and Work dispositions**

The frozen Project contains one open PM Task, LOO-279. All other Tasks below are PM-complete. Preserve that fact even where runtime projections disagree.

| Tasks | Proposed disposition |
|---|---|
| LOO-277, LOO-273, LOO-266, LOO-263, LOO-264, LOO-261, LOO-255, LOO-243 | **Keep complete.** Retain their implementation/history evidence; do not recreate runtime Work or treat completion as endurance proof. |
| LOO-262, LOO-240, LOO-226, LOO-229, LOO-219, LOO-214, LOO-212, LOO-37, LOO-55, LOO-57, LOO-102 | **Keep complete; retain Done Work.** Missing old worktrees are historical conditions, not permission to resume or reconstruct them. |
| LOO-17, LOO-29, LOO-32, LOO-39, LOO-62, LOO-104, LOO-105 | **Keep retired/subsumed and PM-complete.** Preserve their stated successor relationships; create no replacement Tasks from old descriptions. |
| LOO-241 | **Keep PM-complete and Work Abandoned.** Preserve merged PR #1248 and the unused second working-PR record. Do not resume it to address new scheduling gaps. |
| LOO-227 | **Keep PM-complete and Work Abandoned.** Preserve its historical attempt; no restart. |
| LOO-265 | **Retire the residual Ready Work**, preserving PM completion, merged PR #1237, and the second working-PR record. Missing checkout prevents claiming the residual attempt completed. |
| LOO-224 | **Retire the residual Ready Work**, preserving PM completion, merged PR #1202, and the second working-PR record. The recorded refusal to restart a completed Task remains correct. |
| LOO-228 | **Retire the residual Ready Work** as absorbed work, retaining its unpublished attempt and PM completion. Do not infer shipped behavior. |

The proposed residual retirements target exact Work IDs:

- LOO-265: `task_c185db93945d4a1cb0be74739ad60fd7`
- LOO-224: `task_1c8661873b4a4f8f97a4e4234b555701`
- LOO-228: `task_d1a5666cb9564576b3944f2bbf5ef67a`

These are Gate 2 dispositions, not operations performed here. They preserve evidence and remove obsolete execution intent without deleting branches, PR records, or history.

The unavailable historical `release-stability` Project Work, `proj_ecb630ef20d156f77c63461acd007741`, belongs to the Wave’s reconciliation scope. Propose retirement with preserved history: its Linear Project is absent from the current snapshot, and its cached unavailable-project entry lists no Tasks. Do not merge that identity into Reliability or manufacture a current PM Project for it.

**Dependencies and parked triggers**

Intelligence should own the availability evidence population: attempted Runs and Work advancements, outcome classifications, missingness, and links to causes. Reliability consumes that evidence and fixes credential, controller, process, promotion, and scheduling failures. Missing instrumentation must not become execution authority or block otherwise valid work.

LOO-257 and `loopflow.wave-agents` belong to Architecture Minimalism. Reliability accepts their continuity outcomes: Task placement remains usable, and architectural reduction preserves advancement, recovery, and terminal truth. Do not create parallel Reliability implementations.

List’s retirement must preserve its exact-authority and safe-signaling obligations. Provider history, telemetry, attribution, and visible PIDs do not grant control authority. Missing, stale, or contradictory ownership evidence must never authorize signaling an unrelated process. Promotion must preserve explicit process ownership and recoverable Work. Architecture Minimalism owns documenting and simplifying these contracts; Reliability owns failures against them. List’s unresolved implementation choices are not settled merely by retiring its Wave.

Park additional work behind evidence:

- Open a focused runtime repair when a configured-path incident demonstrates stranded advancement, unsafe signaling, false liveness, duplicate progress, or state loss.
- Repair telemetry execution in Reliability when scheduler, host, or execution behavior fails; route missing or malformed analytical evidence to Intelligence.
- Consider another release Home only if the current Home demonstrably cannot satisfy obligation accounting and catch-up.
- Keep broad credential-framework changes, background refresh restoration, general deployment infrastructure, and blanket historical-worktree restoration outside this chapter.

**Material questions and limitations for the parent**

The main challenge is evidentiary: two settled release opportunities are attainable proof; a fourteen-day universal availability claim requires coverage not retained today. Gate 2 should name Intelligence’s evidence dependency and preserve “unknown” until that coverage exists.

A second challenge is availability wording. Truthful blocking alone could reward a system that always explains why it cannot work. KR 2 therefore also requires ordinary expiry continuity and automatic recovery from recoverable failures.

Finally, the accepted plan should explicitly ratify the release timing denominator and required verification surfaces. Daily configured attempts, the historical nightly/weekly model, and agent-process receipts are different facts. Preserve required checks and publication proof while choosing the operational cadence deliberately.
