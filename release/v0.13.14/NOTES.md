# v0.13.14

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.14 makes it easier to set work aside, return to interrupted conversations, and deliver changes within a repository's existing conventions. Tasks can pause without losing their work, Session opening is explicit, and delivery preserves existing scratch documents. Contributors also get less repeated build work, faster verification, and bounded cleanup of completed build output.

## Leave work intact while taking a break

Pausing an unfinished Task now keeps its checkout, PR, conversations, Workflow and execution history together while holding new automated work. Conversation recovery also preserves saved native history when a provider exits without its exit being recorded.

- Use `lf task pause <issue>` and `lf task resume <issue>`, or Pause Task and Resume Task in the Desktop Task header's context menu. Paused Tasks remain in the Wave plan and leave the default active sidebar, including those with retained Sessions.
- Pause holds new Task Flow runs, CI repair and automatic completion reconciliation. It does not stop running processes or move the Workflow; resume starts nothing. The hold persists through planning exchange, independently of Linear disposition.
- `lf session ensure` and `lf session replace` prepare conversations; `lf session connect SESSION` opens them in the current terminal. This replaces the tmux conversation launcher.
- Recovery requires the recorded provider's exact PID and birth time to establish its death. A live or unknown provider does not permit a duplicate launch, and observed death is not recorded as success.
- Explicitly reconnecting to a replaced Session preserves its completion marker and the successor's primary selection, even when the first launch produced no native history.

## Deliver within the repository's conventions

Worktree creation and integration now follow more of the repository's existing Git setup. Review handoffs are explicit, and delivery cleanup removes notes introduced by the current work while preserving existing documents.

- When at least two live branch checkouts share a parent, new worktrees follow that placement. Existing checkouts are reused without fetching; bare and separate Git directories are supported. Checkout-hook failures are reported without discarding the checkout.
- Integration discovers the default branch through Git and follows its configured upstream. Explicit local sync targets require no remote fetch.
- Use `lf pr request-review`, `lf pr reviews` and `lf pr checks --required` for review handoffs and required-check inspection. Submission leaves reviewers and assignees unchanged; auto-merge follows GitHub's allowed methods, repository preferences and merge queues.
- Rejected pushes no longer trigger an automatic force-push fallback. Release regeneration preserves published ancestry, and required CI failures remain actionable regardless of job name.
- Submit and land preparation use `lf scratch clear` to remove committed scratch additions. Existing files, edits, detected renames—including documents moved into scratch—and `.gitkeep` survive. Direct cleanup leaves untracked and staged-only additions untouched.
- Scratch files commit and merge normally in persistent and stacked checkouts; children retain inherited notes. PR open and publish do not perform general scratch cleanup.

## Give agents guidance that fits the work

Standalone launches no longer receive Task-management and operator procedures by default. Associated work receives shared guidance without that association selecting an operator or Workflow.

- Task and Wave launches preload Work guidance, including associations recovered from existing checkouts and resumed Sessions. Existing conversations can read `lf help skill work`.
- Operating procedures stay in selected skills and Flows. Association alone does not authorize Wave memory maintenance; standalone contributions keep proposed updates in scratch.
- `lf history show SESSION --instructions` explains recorded instruction inclusion, omission and deduplication, with JSON and earlier-input support. Missing historical evidence remains unavailable rather than reconstructed.
- Automatic scratch and repository-memory gathering no longer follows symlinks.

## Spend less time rebuilding and reclaim finished output

Verification reuses prepared CLIs, compiled tests and independent fixtures, then runs independent checks in parallel. Local cleanup now requires evidence that work is completed and inactive before deleting regenerable build output.

- CI compiles and archives Rust once, consolidates ordinary integration targets and partitions execution. Installation checks reuse dependency preparation while retaining disposable account/Home isolation and a distinct published-title build; installation verdicts are not cached.
- macOS cache misses overlap control CLI and Xcode builds. Local `scripts/test.py` prepares the verification CLI with test targets and pins Cargo's executable artifact.
- The CI restructuring recorded a full Actions workload pass in 4:25, against a roughly 20-minute baseline. Matched local measurements showed a small edit/test median of 70.0 seconds versus 106.3 seconds, and fresh prepared allocation of 3.39 GiB versus 4.47 GiB. These are recorded measurements, not a release-wide timing guarantee.
- `lf wt prune --builds --dry-run` explains cleanup candidates and retention reasons. Cleanup defaults are 24 hours, eight roots and 64 GiB; completed ownership, fresh execution/process inspection and a worktree lease govern eligibility.
- Unknown or busy work retains output. Cleanup preserves source, dirty and untracked edits, history and registered checkouts, and replaces the Python age-only deletion path. Direct build or clean commands racing cleanup outside Loopflow's lease are not universally excluded.

## Operational notes

- **Move authored Wave files to `.lf/wave/`.** Goals, memory and metrics now use that location across CLI, Desktop, context gathering and skills. Update references when upgrading existing repositories: there is no automatic migration or old-path fallback. Top-level `wave/` is ordinary repository content.
- The Task-pause SQLite migration leaves existing Tasks unpaused. Install through the normal release path before exercising pause against a live store; native controls and configured live-store acceptance remain unverified in the supplied evidence.
- Session recovery has stand-in-provider coverage, but installed recovery and real Codex terminal interaction remain unverified. Live model behavior for the revised instruction scoping also remains unverified.
- Worktree placement does not register workspaces with another host or run its setup scripts. PR publication and stacked remotes retain GitHub/origin assumptions.
- Installation image acquisition now consistently uses `docker.io/library/rust:trixie` after ECR exhausted its public data quota. Stable/trixie policy and isolated installation checks remain; full disposable installation acceptance on that repair was still pending in the supplied record.
- The supplied checks do not establish a passing full hosted matrix for the combined release. Ten consecutive sub-five-minute CI runs, merge-queue timing and push-to-merge timing remain unproved. Local storage measurements do not establish exclusive APFS savings.

## Small changes

- Remove duplicate architecture scans and brittle metric-prose, title and Session-history assertions while retaining checker fixtures, portable HTML freshness and behavioral coverage. The cleanup removes 21 assertions and three cases; migration, recovery, Task ownership, network, account and process-safety proofs remain unchanged.
- Replace timing and display-order assumptions in planning and rate-limit tests with identity, state-preservation and convergence checks.