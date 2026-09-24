# Session Run recovery — review and implementation

2026-09-24. The human invoked review-slice, then implement during the review.
This pass fixes the bounded recovery defect found in the required Session → Run
slice. It does not implement or approve the remaining Monitor/performance work.

## Remaining scope

The complete target remains one compressible outline, Task-active-Run Monitor
content in the existing multiplexer, and repeatable hierarchy/workspace
measurements. The Task directive still requires the external workflow/edit,
long-lived-registry trials and human demo. The latest human canvas direction
supersedes its historical A/D and cardinality assumptions.

`lf task status LOO-291 --json` and `lf roadmap --all --json` supplied the Task
identity and cached directive. `lf task diff` refused while another branch-head
mutation held the worktree lock. Read-only git diff supplied the unrestricted
tracked patch at `/tmp/loo291-session-review-tracked.patch`: 460 sections, 458
identical to the existing review-18 snapshot. Its two changed sections were
concurrent outline projection/tests. The Session contract implementation was
already present in that snapshot; its entire local Rust delta was additionally
captured in `/tmp/loo291-session-review-local.patch` and traced through preparation,
capture, child launch, resume, actions and Flow settlement. Concurrent outline
edits are preserved and remain their own contribution.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Required Run reference | Every Session names its own Run | Required Rust/Swift field; explicit Ask/Flow preparation | Previous required-field fixture receipts; unchanged DTOs | pass, contract |
| Prelaunch identity | Lookup succeeds before a provider starts | Explicit JSON open prepares an old boundary; listing never writes | Existing CLI case, passing in this pass's three-test run | pass, isolated CLI |
| Child uses prepared identity | No second Run minted during first launch | CLI child consumes the prepared manifest and adds context | New first-launch case observes exact Run ID in provider process; context exists and preparation is consumed | pass, local stand-in provider |
| Resume preserves access | A waiting conversation cannot block metadata/opening | Native resume releases the preparation lock before waiting on the provider | Reproduced failure, corrected CLI test while child waits on stdin | pass after fix |
| Live configured population | Existing Sessions resolve to their Runs | Selected Home returns no Sessions, scoped or all | Fresh read-only CLI queries; `session-run-live-review.json` | gap: zero comparisons |
| One authority | Run capture owns identity; boundary owns human decisions | Existing owners retained, no second mapping or post-launch identity writer | Source trace and negative searches | pass, source |
| Complete canvas | Mixed panes, truthful active Runs and two measured experiences | Still separate outstanding core work | Canvas design and current slice ledger | gap |

## Reproduced finding and correction

`open_boundary` held `lock_session_launch` across `resume_native_run`, which
waits for the provider to exit. A second `session open <id> --json` therefore
waited for the entire conversation. That blocks the app's preparation request
before it can show an existing Session. This was introduced by the preceding
Run-identity slice.

The resume path now releases the preparation lock after resolving native history
and stopping the preceding owned client, before entering the resumed conversation.
Preparation without history retains the lock through initial publication. No
new lifecycle, owner, storage field or provider adapter was introduced.

The first test attempt incorrectly treated the provider's `--version` availability
check as the real launch; it failed with provider exit 1. The fixture was corrected
to handle that check. The corrected pre-fix test then failed specifically because
the second open did not return within its deadline. Both owned processes were
released before assertion. No user-owned process or Session was touched.

The final regression covers both existing native history and first launch through
the real CLI, temporary Home/store, production Run capture and subprocess path.
Only the provider executable/history are fixtures. It verifies exact child Run ID,
unchanged required reference, active metadata while waiting, and first-launch
context publication. This does not prove a real provider's native conversation
history or AppKit interaction.

## Verification and disposition

- Before: `/tmp/loo291-resume-lock-before-corrected.log`, one failing test at
  `Session metadata open waited for the resumed provider to exit`.
- After: `cargo test -p loopflow --test session_cli_tests`, three tests passed;
  `/tmp/loo291-resume-lock-after.log`.
- Expanded final proof: `cargo test -p loopflow --test session_cli_tests
  boundary_launch_and_resume_remain_openable_while_provider_waits`, one test
  covering both first launch and resume passed;
  `/tmp/loo291-session-launch-resume-proof.log`.
- `cargo clippy --all-targets -- -D warnings` passed;
  `/tmp/loo291-session-recovery-clippy.log`. Formatting and diff whitespace checks
  pass. No production edit followed the passing expanded proof.
- Negative searches find no former temporary Run-binding environment/path writer,
  SessionScope or FlowResolutionAction. Resume still uses the existing native
  provider history operation and exact Run reference.

The fix advances the required identity slice. The latest implement request was
handled locally; no PR operation, installation, PM write, Task completion or
worktree closure occurred. Earlier slice claims are retained only at their
recorded proof levels. Monitor, performance measurements and the new human demo
remain open; the full Task is not ready for publication.
