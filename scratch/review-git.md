# Git ownership slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Shared
staging, commit and branch-push inheritance advances the full design through
the existing Git implementation. No additional bounded defect was established
in this slice; no executable code changed during this review. Complete child
ownership, prerequisite recovery and configured acceptance remain open.

Starting head: `0144c19e2b9cd8761add7fa169612169a97ec135`.
Obtained the complete Task patch through `lf task diff LOO-285 --json`:
694,156 characters, `binary: false`, `truncated: false`. Reviewed the directive,
full design, current slice, forbidden outcomes and Done when, concentrating
source inspection on the Git change and its callers and evidence consumers.
Only the compression report changed after the implementation checkpoint.
Earlier validation receipts remain historical evidence.

## Demonstration

The built-CLI fixture
`surviving_release_git_mutation_retains_target_and_checkout` passed all ten
scenarios in 43.50 seconds after 23.82 seconds compilation. Staging, commit,
upstream establishment, ordinary push and force-with-lease fallback each run
with a killed controller and with a failed launcher leaving a descendant.

Each scenario observes another release defer, ordinary checkout removal fail,
and release-note bytes remain accessible. Failed-controller cleanup also retains
the checkout. After the child completes, the fixture inspects the actual index
or committed notes; push cases compare the bare-origin branch with checkout
HEAD. Target access and ordinary removal then succeed.

Git, bare origins, processes and OS locks are real. Commit cases pause real Git
in a pre-commit hook; other cases pause a shell wrapper before executing real
Git. GitHub and notes generation are simulated. The force case simulates the
initial push failure, then performs a real force-with-lease push. These are local
ownership and resulting-state proofs, not hosted GitHub, arbitrary provider
descriptor propagation, installed scheduling, UI or public-artifact evidence.
They supply neither configured automatic settlement.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Git child survival | Retain target and checkout protection through child exit | Both held descriptors reach stage, commit and all branch-push paths | Ten-case built-CLI demonstration | Pass locally |
| Actual Git result | Protected child can finish its intended mutation | Existing shared Git commands remain authoritative | Index/commit notes and bare-origin SHA assertions | Pass locally |
| One commit/push implementation | Preserve ordinary behavior and Task fence | Shared helpers; ordinary callers supply no release capability; revocation precedes push | Source and prior ordinary commit, fallback-identity and Task revocation tests | Retained evidence |
| Complete mutation exclusion | Every side-effect child retains required ownership | Task compensation, notes, lockfile and source/worktree children remain uncovered | Reachable calls below | Gap |
| Stable dues and repeated firing | Preserve activation; one accepted settlement | Deterministic keys and exact attempt fencing | Accounting writers and prior cases | Retained local evidence |
| Delayed wake and later dues | Frozen catch-up set, one execution/result; later dues wait | Atomic obligation document retains coverage and links; completion records later waits | `begin`, `finish_process`, prior joined proof | Retained local evidence |
| Interrupted collapse and candidate retry | Preserve owner, selected candidate and failures | Whole-document replacement and retained selection through preflight | Writer source and prior regressions | Retained local evidence |
| Manual trigger, repair and timing | No false autonomous pair or lost original timing | History reads collapsed provenance and first covering attempt | History source and prior regressions | Local evidence; live trigger race unproven |
| Overlap | One mutator with exact continuation | Target exclusion works at covered boundaries; some continuations remain prose | Demonstration and lock error path | Partial |
| Crash around tag/publication | Resume exact candidate and retain external effects | Existing recovery and publisher reconciliation | Earlier same-tag and publisher proofs | Full interruption/configured proof gap |
| Late result and process exit | No terminal regression or fabricated product success | Fenced atomic `settle`; wrapper fills only missing process outcome | Writer source and prior preservation cases | Retained local evidence |
| Failed verification and repair ownership | Retain original prerequisites, recovery links and dated owners | Current telemetry gates mutation; historical associations and bounded retry absent | `verify_scheduled_telemetry`, retained 36-failure baseline | Gap |
| No-change and resumed candidate | Exact source, empty range and complete applicable checks | Shared completion and verified baseline remain required | Earlier joined/wrong-source proofs and source | Retained local evidence |
| Incomplete public result | Missing/wrong assets or failed smoke cannot qualify; retain external effects | Publisher requires stages, hashes, UI proof and public read-back | Earlier publisher and joined counterexamples | Local evidence; actual public/UI gap |
| Corrupt records | Fail with path and preserve evidence | Strict readers and atomic replacement | Accounting source and prior corruption proof | Retained local evidence |
| Schedule, timezone, DST and Home changes | Preserve denominator and actionable unfinished work | Calendar/segments retained; closed unfinished attempts lack continuation | `observe`, `close`, `receipt_context`, prior calendar cases | Continuation gap |
| Caller preservation and independent scopes | Preserve caller bytes at every exit; unrelated scopes progress | Earlier joined/hook cases preserve caller state; separate target/checkout locks | Prior preservation/isolation receipts and current checkout proof | Partial; remaining interruption paths open |
| Two adjacent automatic settlements | Distinct executions, at least one publication, all checks, no repair | No configured qualifying pair demonstrated | Acceptance ledger; simulations ineligible | Gap |

## Source and negative architecture

Followed release preparation into `commit_workflow`, both clean and dirty push
paths, configured and fallback commit identities, upstream establishment and
ordinary/force-with-lease pushes. They share command construction and error
conversion. The release call supplies both held capabilities, an explicit commit
message and `create_draft_pr: false`; it does not enter commit-message generation
or best-effort draft creation. Ordinary CLI, Flow, PM, Task and PR callers pass
no-op inheritance explicitly. No parallel release Git implementation was added.

The Task settlement fence still precedes pushing. Its remote revocation, merge
replacement and stale-head compensation use no-op PR inheritance. The current
fixture does not exercise those managed Task paths. Passing Git inheritance
cannot establish their surviving-child ownership. Likewise, the callback does
not protect notes generation, manifest tools or worktree mutation merely because
they run before or after the protected commit.

Checked explicit cron context through CLI/Flow, accounting settlement, history
qualification and the history fixture, publisher receipt/descriptor consumers,
and release documentation. The repository flow remains one mechanical release
operation. `settle` remains the typed product-success writer; physical wrapper
completion cannot promote process success or overwrite accepted settlement.
Target exclusion, exact checkout protection and cron attribution remain separate
capabilities. Searches found no restored duplicate success-proof wrappers,
`record_verification` writer, separate Python candidate/publish receipt classes,
or Swift consumer of the release-history/opportunity/public-receipt types.
Direct release worktree creation still disables default-branch synchronization.
These scoped source findings do not prove every indirect child forwards ownership.

## Next implementation direction

1. Continue existing capability propagation through Task revocation/compensation,
   `run_release_notes_stage`, manifest `cargo update`/`uv lock`, and source checkout
   creation, reset and removal. Rebuild still executes `git reset --hard` through
   ordinary `run_stdout`; branch fetching and materialization have ordinary Git
   children too. Prove controller death and failed-launcher descendants at those
   boundaries, including resulting state and preserved caller/source bytes.
   Preserve Task settlement authority and independent target/checkout scopes.
2. Retain the telemetry prerequisite for each original covered due, including
   missing/failed evidence and linked current recovery. Implement the approved
   once-per-wake bounded retry and record dated repair ownership. Current lookup
   still searches two days for the latest interval. The observed missing
   `agent_turns` scorecard table remains a blocker; no Intelligence handoff has
   been accepted. The retained 36 failures include the original 35.
3. Give closed unfinished obligations a supported continuation or disposition
   without transferring old Home authority. Closing retains attempts while
   `receipt_context` rejects reuse, leaving old Running rows unresolved.
4. Complete interruption/isolation proof before supported installation/sync and
   configured acceptance. Required UI-host/public exact-tag proof and two adjacent
   automatic executions, at least one publishing without manual repair, remain
   mandatory. No new independent publication blocker or sibling Task was
   identified here.

## Validation

`cargo test -p loopflow --test release_lock_tests surviving_release_git_mutation_retains_target_and_checkout -- --nocapture`
passed all ten scenarios in 43.50 seconds. No other tests or static checks were
rerun for this documentation-only review; prior focused passes retain their
stated scope. No affected-suite gate, full CI, installation/sync, cron trigger,
production publication, PM handoff, PR publication, landing or Task completion
occurred.
