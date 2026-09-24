# Mutation-child slice review

## Verdict and next slice

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Direct
tag/candidate/publication children now retain target exclusion after controller
death. This advances the full design, but does not complete its execution or
acceptance contract. This review reproduced and fixed one additional provenance
defect. The next implementation slice must continue the remaining ownership
paths rather than treating these passing children as coverage of the whole graph.

1. Carry the existing release exclusion through shared commit/PR operations,
   auto-merge re-arming, hooks, notes, source checkout mutation, and cleanup.
   `prepare_release_in_worktree` still calls `commit_workflow` and
   `finish_arm_after_rebase` without the borrowed lock; `wait_for_pr_merge`
   reaches `enable_auto_merge` the same way. Prove surviving mutation children
   at those boundaries, without a second release implementation or ambient
   process-role inference.
2. Preserve checkout protection through publisher survival too. The target
   lock and `WorktreeLease` protect different scopes. The latter still owns a
   parent-held file; ordinary `worktree_remove` acquires that lease, not the
   release target lock. Parent death can therefore reopen checkout removal while
   a publisher child still holds target exclusion. The tag/GitHub tests do not
   exercise this boundary. Add the actual child-versus-cleanup counterexample.
3. Complete original telemetry prerequisite associations and bounded current
   recovery, with retained failures and dated repair ownership. Current
   `verify_scheduled_telemetry` still uses the current due interval and two-day
   receipt search, with no prerequisite retry. The missing `agent_turns`
   scorecard query remains the observed verification blocker; the Intelligence
   handoff is still unaccepted. No independent publication blocker was newly
   demonstrated here and no sibling Task was opened.
4. Give unfinished closed obligations explicit continuation/disposition without
   transferring Home authority. `close`/`observe` retain their rows but do not
   settle or disposition old Running attempts; `receipt_context` rejects their
   reuse. Finish the remaining interruption/caller-preservation matrix before
   supported installed acceptance, required UI-host/public smoke, and two
   adjacent automatic executions with at least one publication.

## Review boundary and demonstration

Starting head: `5941c1b2c79a3fc5d5adf4e544d14569d0c0d924`. Obtained the complete
500,890-character Task patch through `lf task diff LOO-285 --json`
(`truncated: false`). Reviewed the directive, complete design and preservation
claims, preceding reviews, direct mutation paths, lock acquisition/inheritance,
cron attribution/settlement/history, CLI flow and intervention consumers, Python
descriptor forwarding, and adjacent shared PR/worktree owners.

The built-CLI parent-death demonstration passed both existing cases in 7.23s:
each starts a controller, waits for its mutation child at an explicit barrier,
kills and reaps that exact fixture-owned controller, and observes a contender
defer until the child finishes. The tag case pushes to a real disposable bare
origin and proves another repository remains independent. The publication case
uses a simulated GitHub endpoint. These are local process/OS-lock proofs, not
production publication, launchd scheduling, or configured KR settlements.

## Reproduced and repaired

An inherited descriptor matching the target lock's inode was marked reused
before proving there was existing ownership. Supplying an independently opened,
**unlocked** descriptor let a manual `lf release tag` acquire the lock while
`is_inherited()` suppressed its intervention record. That loses evidence needed
to exclude manual repair from later unattended qualification.

The new CLI regression seeds a pending failed opportunity in a disposable Home,
passes an unlocked descriptor to the real tag command, and inspects durable
evidence. Before the fix the push succeeded but intervention count was zero
instead of one. Acquisition now first tries the independently opened lock file:
when available it keeps that fresh ownership and records the manual action;
when already held it tries the supplied exact descriptor as before. A genuine
held descriptor still permits nested execution without false intervention.

The regression covers both cases, preserves the prior attempts byte-equivalently
as JSON values, and executes real local Git operations. All three lock
integration tests passed after repair (11.81s), including both parent-death
cases. No new persisted field, lock owner, settlement writer, or compatibility
path was introduced. The README's worktree-lease claim now explicitly describes
protection while the runner is alive; it no longer implies demonstrated
surviving-child checkout protection.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Direct mutation survives parent death | One target owner through child exit | Tag/candidate refs, workflow submission and GitHub mutation use the borrowed lock | Replayed built-CLI tag and publication cases; source paths | pass locally; candidate/workflow kill points not independently exercised |
| Independent repository and recovery | Other repository proceeds; same target resumes after exit | Canonical repository/target lock; same-tag recovery remains | Real bare-origin fixture and contender results | pass locally |
| Manual provenance | Only genuine nested ownership suppresses intervention | Unlocked descriptor now gets fresh ownership and manual evidence | New CLI case failed before fix, both descriptor cases passed after | pass for reproduced boundary |
| All mutation/checkout owners survive | Shared PR/hooks and checkout removal honor surviving work | Those paths still omit child inheritance or equivalent surviving ownership | `prepare_release_in_worktree`, `wait_for_pr_merge`, `run_release_hooks`, `WorktreeLease` | gap |
| Original dues and frozen catch-up | Preserve every due; one execution and at most one settlement | Atomic obligation document, saved covered keys, original owner on candidate retry | Source and earlier accounting/joined receipts | retained local evidence; not rerun |
| Exact settlement authority | Late/wrapper evidence cannot overwrite success | Exact attempt fence; atomic outcome plus verification; process zero becomes Unverified | `settle`, `finish_process`, prior preservation receipts | retained local evidence; not rerun |
| Timing/intervention through collapse | Keep first attempt and all manual history | History examines retained coverage and collapsed provenance | History source and prior four passing cases | retained local evidence; not rerun |
| Required telemetry | Each original due retains prerequisite; recovery cannot erase failure | Current telemetry gates mutation; historical associations/retry absent | Current release source; retained 36-failure Home observation | gap |
| Exact candidate/no-change/publication | Required exact-source checks, empty range, all public stages | Shared candidate completion, publisher reconciliation, immutable/public checks | Source and previous joined/Python receipts | local evidence retained; live/UI proof gap |
| Obligation replacement/removal | Preserve denominator and actionable old work | Closed segments retained, unfinished attempts lack continuation | `observe`, `close`, `receipt_context` | gap |
| Caller and interruption preservation | Caller bytes survive all exits | Existing joined five outcomes preserve bytes; full interruption matrix incomplete | Earlier joined receipts; current selection source | partial |
| Corruption/DST/trigger provenance | Explicit error/unknown; no invented dates or automatic success | Strict records, shared calendar, trigger requests retained | Source and earlier focused receipts | local evidence retained; live trigger race unproven |
| Two adjacent configured settlements | Two distinct automatic executions, one publication, no repair | History can reject known ineligible rows; no qualifying configured pair demonstrated | Retained Home history and current acceptance ledger | gap |

## Negative architectural proof

The repository flow still contains one `op: release run patch`; explicit cron
receipt plus job descriptor supplies attribution. The release target descriptor
supplies mutation exclusion. Neither capability was merged into the other.
`settle` remains the sole typed product-success writer; `finish_process` cannot
promote a zero exit or replace an accepted success. Searches found no restored
release-selection `sync_main`, `PublicationEvidence`, `NoChangeEvidence`,
`record_verification`, or separate Python candidate/publish receipt classes.
Historical schema-1 process receipts remain historical evidence.

The remaining shared mutation commands are reachable gaps in the required
ownership contract. They are not an accepted alternative authority. This repair
uses the existing OS lock and existing intervention writer; it adds no daemon,
scheduler, database table, process-age takeover, or independent publisher.

## Validation

- `cargo test -p loopflow --test release_lock_tests -- --nocapture`: initial
  two-case demonstration passed; after adding the failing provenance regression
  and fixing acquisition, all three passed (11.81s execution).
- `cargo fmt` applied one test formatting change; `cargo fmt --check` passed.
- `cargo clippy --all-targets -- -D warnings`: passed.

Only formatting and documentation changed after the final behavioral run.
Earlier joined execution, Python recovery, history and accounting passes remain
their recorded evidence; they were not rerun for this lock repair. No full
affected-suite gate, hosted matrix, UI automation, install/sync, cron trigger,
production mutation, PM handoff, PR publication, landing, or Task completion
occurred. The review is complete; the Task's implementation and configured
acceptance remain open.
