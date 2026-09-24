# Lockfile ownership slice review

## Verdict

**Iterate. Keep the serial PR unpublished and the Task incomplete.** Manifest
tool inheritance advances the full design through the existing updater. The
local proof passes, but complete child ownership, prerequisite recovery and
configured acceptance remain open. No additional bounded defect was established
in this slice; no executable code changed during this review.

Starting head: `4b225f5fa6136782bcbe8e9cff33d60e13924a02`.
Obtained the complete Task patch with `lf task diff LOO-285 --json`: 723,625
characters, `binary: false`, `truncated: false`. Reviewed the directive, full
design, current slice, forbidden outcomes and Done when; compared the latest
implementation with the preceding reviewed checkpoint. Only the compression
report changed after implementation. Earlier validation remains historical
evidence, with the fresh demonstration distinguished below.

## Demonstration

`cargo test -p loopflow --test release_lock_tests surviving_release_lockfile_tool_retains_target_and_checkout -- --nocapture`
passed all four scenarios in 16.17 seconds after 26.85 seconds compilation.
Cargo and uv each run with a killed controller and with a failed launcher leaving
a descendant. The built CLI reaches the real preparation path in disposable
repositories. While the launcher is blocked, another release defers and ordinary
checkout removal fails. Failed-controller cleanup also preserves the checkout.

After the barrier opens, real offline Cargo/uv updates a dependency-free
lockfile to the selected version. The fixture checks that result and exact
caller HEAD, branch, index bytes, staged/unstaged and untracked work, unpublished
commit and original lockfile version. Release access and checkout removal return
after the child exits.

Git, bare origins, tools, processes and OS locks are real. GitHub is simulated;
a shell wrapper supplies the interruption barrier before invoking each tool.
This proves the wrapper lifetime and resulting real lockfile update. It does not
prove arbitrary tool descendants retaining descriptors after their own parent
exits, network dependency resolution, provider-agent execution, installed cron
timing, UI-host verification or either configured automatic settlement.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Lockfile child survival | Keep target exclusion and checkout protection through exit | Both existing descriptors reach Cargo and uv | Four-case built-CLI demonstration | Pass locally |
| Actual update and caller preservation | Update selected source; preserve caller branch/index/bytes | Shared updater runs in the owned checkout | Real lockfile contents and exact caller-state assertions | Pass at exercised boundaries |
| Ordinary bump and rebuild | Preserve selection, ordering and error behavior | One updater; standalone supplies no capability; preparation/rebuild borrow both locks | Source; prior bump/reintegration passes | Retained evidence |
| Complete mutation exclusion | Every surviving side-effect child retains ownership | Notes, Task compensation and source/worktree children remain uncovered | Reachable calls below | Gap |
| Same-minute replay and unchanged sync | One due key and accepted settlement; preserve activation | Deterministic keys and fenced settlement | Accounting source and prior focused cases | Retained local evidence |
| Delayed wake and execution crossing another due | Frozen coverage, one execution/result; later due waits | Whole obligation document retains coverage and links; completion records later waits | `begin`, `finish_process`, prior joined proof | Retained local evidence |
| Interrupted collapse and candidate retry | Preserve owner, candidate and failed attempts | Atomic document replacement; selection survives preflight | Writer source and prior regressions | Retained local evidence |
| Manual trigger, repair and timing | Retain intervention and original first attempt; no false autonomous pair | History reads collapsed provenance and frozen coverage | History source and prior regressions | Local evidence; live trigger race unproven |
| Overlap | One mutator and exact continuation | OS exclusion works at covered boundaries; some continuations remain prose | Demonstration and target-lock error path | Partial |
| Crashes around tag/publication | Resume exact candidate and preserve external effects | Existing recovery and publisher reconciliation | Prior same-tag and publisher cases | Full interruption/configured proof gap |
| Late result or zero process exit | No terminal regression or invented success | Fenced atomic `settle`; wrapper only fills missing process result | Writer source and prior preservation cases | Retained local evidence |
| Failed telemetry and repair ownership | Retain original prerequisites, current recovery and dated owners | Current telemetry gates mutation; historical associations and bounded retry absent | `verify_scheduled_telemetry`; retained 36-failure baseline | Gap |
| No-change and resume verification | Exact source, empty range, complete applicable checks | Shared completion and verified baseline remain required | Prior joined/wrong-source cases and producer source | Retained local evidence |
| Draft, wrong hash, missing asset or smoke failure | Reject incomplete publication; retain external effects | Publisher requires stages, hashes, UI proof and public read-back | Prior publisher and joined counterexamples | Local evidence; actual public/UI gap |
| Corrupt persistence | Fail with path and retain valid evidence | Strict readers and atomic replacement | Accounting source and prior corruption case | Retained local evidence |
| Schedule/timezone/DST/Home changes | Preserve denominator and actionable old work | Calendar/segments retained; closed unfinished attempts lack continuation | `close`, `receipt_context`, prior calendar cases | Continuation gap |
| All caller exits and independent scopes | Preserve bytes and permit unrelated work | Current caller proof plus earlier isolation cases; remaining child paths uncovered | Current demonstration and prior preservation/isolation receipts | Partial |
| Two adjacent automatic settlements | Two executions, at least one publication, all checks, no repair | No configured qualifying pair demonstrated | Acceptance ledger; fixtures ineligible | Gap |

## Source and negative architecture

Followed both manifest updater callers and both lockfile commands. Preparation
and rebuild use `prepare_release_in_worktree`, which supplies its existing target
lock and checkout lease. Standalone `release_bump` supplies a no-op callback.
Manifest rewriting, changed-file conditions, Cargo-before-uv ordering and
`command_stdout` error conversion remain unchanged. No second updater, tool
registry, ownership record or ambient capability lookup was introduced.

The callback covers only the tool commands. `run_release_notes_stage` constructs
an ordinary `lf` child; rebuild still runs `git reset --hard` through ordinary
`run_stdout`; source materialization/removal uses shared Git helpers without
these capabilities. Task revocation and compensation still call
`disable_auto_merge` with no-op inheritance. Their mere adjacency to protected
commands is not surviving-child proof.

Checked cron attribution, atomic settlement, history qualification and fixture
consumers, and the release documentation against the unchanged model. The
repository Flow remains one mechanical release operation. `settle` remains the
typed product-success writer; process completion cannot promote zero exit or
overwrite accepted success. Searches found no restored duplicate success-proof
wrappers, `record_verification` writer or separate Python candidate/publish
receipt classes in the inspected production paths. Direct release worktree
creation disables default-branch sync; the shared helper performs it only when
requested. These scoped findings do not establish every indirect child's
ownership behavior.

## Next implementation direction

1. Continue explicit ownership through `run_release_notes_stage` and its real
   CLI/provider launch chain. Prove controller death and failed-launcher
   descendants preserve target exclusion, source and caller bytes; a shell-only
   notes substitute cannot establish the whole provider path. Keep Task
   revocation/compensation and source checkout creation/reset/removal in the same
   serial Task, preserving their existing authority and independent scopes.
2. Retain the telemetry prerequisite for every original covered due, including
   missing/failed evidence and linked current recovery. Implement the approved
   once-per-wake bounded retry and dated repair ownership. Current lookup still
   uses the latest interval and two days of receipts. The observed missing
   `agent_turns` scorecard table and unaccepted Intelligence handoff remain;
   the retained 36 failures include the original 35.
3. Give closed unfinished obligations a supported continuation or disposition
   without transferring old Home authority. Closing retains attempts while
   receipt-context validation rejects reuse; Running rows can remain unresolved.
4. Complete remaining interruption/isolation proof before supported install/sync
   and configured acceptance. Required UI-host/public exact-tag proof and two
   adjacent automatic executions, at least one publishing without manual repair,
   remain mandatory. No new independent publication blocker or sibling Task was
   identified here.

Only review documentation changed. No other tests or static checks were rerun;
the prior focused passes retain their recorded scope. No affected-suite gate,
full CI, install/sync, cron trigger, production publication, PM handoff, PR
publication, landing or Task completion occurred.
