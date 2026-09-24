# Notes ownership slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Notes
inheritance and retained input advance the full design through the existing
CLI/provider path. The local demonstration passes; complete child ownership,
prerequisite recovery and configured acceptance remain open. No additional
bounded defect was established, and no executable code changed during this review.

Starting head: `4ecb83977d19f65990d38484f4a02a2e886b9fdc`.
Obtained the complete Task patch with `lf task diff LOO-285 --json`: 754,517
characters, `binary: false`, `truncated: false`. Reviewed the directive, full
design, current slice, forbidden outcomes and Done when. Source inspection
concentrated on the notes change, its launch and cleanup paths, and the
accounting/verification owners. Only the compression report changed after the
implementation checkpoint. Earlier validation remains historical evidence.

## Demonstration

`cargo test -p loopflow --test release_lock_tests surviving_release_notes_provider_retains_target_checkout_and_context -- --nocapture`
passed both scenarios in 18.23 seconds after 24.26 seconds compilation.
The built release CLI reaches a nested built CLI and the real Codex harness.
A simulated app-server completes initialization and receives `turn/start`
before pausing. One scenario kills the release controller and nested CLI; the
other lets a failed launcher leave the nested CLI/provider running while the
release controller performs its real error cleanup.

While the provider is paused, another release defers and ordinary checkout
removal fails. After interruption, the provider reads the retained JSON input
and writes notes. The fixture checks the selected version, resulting notes,
and restored target/removal access after the holders exit. Caller HEAD, branch,
raw index bytes, staged/unstaged/untracked work, unpublished commit and previous
notes remain unchanged.

Git, bare origin, CLI dispatch, harness protocol, processes and OS locks are
real. GitHub and the provider executable are simulated. This proves the
application's provider-launch boundary and the resulting local work. It does
not prove arbitrary vendor descendants retaining descriptors, live model
execution, hosted publication, installed scheduling, UI-host verification or
either configured automatic settlement. No production mutation supplied proof.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Notes provider survives interruption | Retain target exclusion, checkout and exact input | Existing descriptors reach nested CLI/provider; unique input uses runtime prompt storage | Two-case built-CLI/Codex-harness demonstration | Pass locally |
| Failed-launcher cleanup | Preserve the surviving child's source and input | Cleanup drops its lease and independently reacquires; context has no launcher-scoped destructor | Demonstration and cleanup/prompt source | Pass locally |
| Notes policy and ordinary callers | Preserve validation/degradation; standalone supplies no release capability | One notes stage with explicit inheritance callback; policy unchanged | Source; prior six notes-policy and normal candidate passes | Retained evidence |
| Complete mutation exclusion | Every side-effect child retains required ownership | Task compensation and source/worktree mutation children remain uncovered | Reachable calls below | Gap |
| On-time, unchanged sync and repeated firing | Preserve activation, one due key and accepted settlement | Deterministic identities and exact attempt fencing | Accounting writers and prior focused cases | Retained local evidence |
| Delayed wake and execution crossing another due | Freeze catch-up coverage; later dues wait | Atomic obligation document retains links/coverage; completion materializes later waits | `begin`, `finish_process`, prior joined proof | Retained local evidence |
| Interrupted collapse and candidate retry | Preserve original owner, selection and failures | Whole-document replacement; selection survives preflight | Accounting source and prior regression | Retained local evidence |
| Manual trigger, repair and timing | Preserve provenance and original first attempt; no false autonomous pair | History reads collapsed provenance and frozen coverage | History source and prior regressions | Local evidence; live trigger race unproven |
| Overlap | One mutator with exact continuation | Exclusion works at covered boundaries; some continuation remains prose | Demonstration and target-lock error path | Partial |
| Crashes around tag/publication | Resume exact candidate and preserve external effects | Existing recovery and publisher reconciliation | Prior same-tag/publisher cases | Full interruption/configured proof gap |
| Late result or zero process exit | No terminal regression or invented success | Atomic fenced `settle`; wrapper only fills missing process result | Writer inspection and prior preservation cases | Retained local evidence |
| Failed telemetry and repair ownership | Retain original prerequisites, current recovery and dated owners | Current telemetry gates mutation; original-due associations and bounded retry absent | `verify_scheduled_telemetry`; retained 36-failure baseline | Gap |
| No-change and resumed candidate checks | Exact source, empty range and complete applicable proof | Shared completion and verified baseline remain required | Prior joined/wrong-source cases; producer source | Retained local evidence |
| Draft, wrong hash, missing asset or failed smoke | Reject incomplete publication; retain external effects | Publisher requires stages, hashes, UI proof and public read-back | Prior publisher and joined counterexamples | Local evidence; actual public/UI gap |
| Corrupt persistence | Fail with path and retain evidence | Strict readers and atomic replacement | Accounting source and prior corruption case | Retained local evidence |
| Schedule/timezone/DST/Home changes | Preserve denominator and actionable old work | Calendar/segments retained; closed unfinished attempts lack continuation | `close`, `receipt_context`, prior calendar cases | Continuation gap |
| Caller preservation and independent scopes | Preserve bytes across every exit; unrelated work progresses | Current caller proof plus prior isolation cases; remaining mutation paths uncovered | Demonstration and prior preservation/isolation receipts | Partial |
| Supported cutover and installed history | Preserve cadence/old evidence; expose missing coverage and repair age | Local projections exist; installed cutover and complete history/recovery proof remain pending | Acceptance ledger and retained Home observations | Gap |
| Two adjacent automatic settlements | Distinct executions, at least one publication, all checks, no manual repair | No configured qualifying pair demonstrated | Acceptance ledger; simulations ineligible | Gap |

## Source and negative architecture

Followed both notes-stage callers, context construction, provider failure
classification, validation and previous-note restoration. Preparation supplies
its existing target lock and exact checkout lease; standalone notes supplies a
no-op callback. The callback configures the existing child rather than acquiring
authority. The nested CLI and harness preserve descriptors made inheritable at
that boundary. Their environment handling does not turn those descriptor names
into a new process role.

The bounded JSON remains one notes input, written by `write_prompt_log` under a
UUID-bearing invocation name. It cannot be replaced by another same-second
notes invocation. The existing writer also maintains the prompts ignore entry.
The full prior-note backup still serves restoration, whereas bounded prior-note
text serves authorship; neither substitutes for the other. No new context
registry, cleanup service, notes policy or fallback writer was introduced.

The target lock, checkout lease and cron receipt descriptor retain separate
scopes: release mutation, exact checkout removal and scheduled attribution.
The repository Flow remains one `op: release run patch`. `settle` remains the
typed product-success writer; `finish_process` cannot turn process success into
publication or overwrite accepted success. Searches found no restored
`PublicationEvidence`, `NoChangeEvidence`, `record_verification`, separate
Python candidate/publish receipt classes, or Swift consumer of the release
history/opportunity/public-receipt/notes-context types.

Direct release worktree creation still disables default-branch synchronization.
That scoped finding does not establish child inheritance in its shared Git
helpers. Source materialization/removal, branch fetching/deletion and rebuild's
ordinary `git reset --hard` remain reachable gaps. Task revocation and
compensation still call `disable_auto_merge` with no-op inheritance. The notes
demonstration does not extend ownership proof to those paths or to arbitrary
programs that close inherited descriptors.

## Next implementation direction

1. Carry existing capabilities through source checkout creation/reset/removal
   and Task revocation/compensation. Reuse the shared Git/worktree/PR owners;
   preserve exact-source classification, Task settlement authority and
   independent target/checkout scope. Prove controller death and failed-launcher
   descendants at the concrete mutation boundaries, including actual resulting
   state and caller preservation.
2. Retain the telemetry prerequisite for every original covered due, including
   missing/failed evidence and linked current recovery. Implement the approved
   once-per-wake bounded retry and dated repair ownership. The current lookup
   still uses the latest interval and two days of receipts. The observed missing
   `agent_turns` scorecard table remains a blocker; no Intelligence handoff is
   accepted. The retained 36 failures include the original 35.
3. Give closed unfinished obligations a supported continuation or disposition
   without transferring old Home authority. Closing retains attempts while
   receipt-context validation rejects reuse; old Running rows remain unresolved.
4. Finish remaining interruption/isolation proof before supported install/sync
   and configured acceptance. Preserve 09:00 telemetry and 10:00 release. Required
   UI-host/public exact-tag proof and two adjacent automatic executions, at least
   one publishing without manual repair, remain mandatory. Collapsed misses do
   not count as additional settlements. No independent new live publication
   blocker or sibling Task was identified here.

Only review documentation changed. No other tests or static checks were rerun;
the implementation's focused passes retain their recorded scope. No affected-suite
gate, full CI, install/sync, cron trigger, production publication, PM handoff,
PR publication, landing or Task completion occurred.
