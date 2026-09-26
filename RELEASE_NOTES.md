# v0.12.22

v0.12.22 keeps work moving through review, revision, and delivery without requiring a fresh launch at every step. Tasks continue their saved Flows, blocked PR landings can recover on the same commit, and Waves run one governance attempt per wake. Requests also carry names into agent context so saved decisions can preserve who asked for what.

## Continue from review to the next useful step

`lf task advance <issue>` drives a Task's saved Flow until it needs input, encounters a blocker or interruption, or finishes. Recovery retains captured definitions, selected branches, and completed review feedback.

- The `feature` Flow runs design review followed by implementation, compression, behavioral and concept reviews, explicit loop decisions, and an interactive demo. Demo feedback can send work through another implementation pass.
- **Complete** replaces Approve/Iterate in CLI and desktop Sessions. Completing the Session returns feedback; the following decision step chooses where the Flow goes next.
- Blocked decisions reuse one Ask and reassess its answer. Decisions take effect only after their originating Run succeeds, and stale workers cannot settle newer work.
- Ordinary Flows are resumable too. Finishing a Flow does not implicitly merge its PR or complete its Task.

## Recover delivery without an empty commit

After resolving an external blocker, rerun `lf pr land` on the same head. Landing now reconciles fresh GitHub evidence after repairs and reports the repair agent's concrete conclusion.

- Blocked landings can resume without a lifetime repair ceiling or a required SHA change. GitHub's confirmed merge takes precedence over provider errors.
- Repair publication preserves active checkout placement and the intended Task disposition, including `-c` or `--next <slug>`.
- Rebase recreates deleted remote branches despite stale tracking refs, while leases continue protecting unseen remote work.
- Preparing a replacement PR head explicitly removes the PR from GitHub's merge queue before publication.

## See what happened during Wave operation

Each Wave wake now runs one `wave/operate` attempt, with failures, input claims, and provider sessions recorded in the turn journal. Tasks and ordinary Flows retain their execution engines and captured definitions.

- Wave stack traversal, queue/skip controls, and playhead state are removed. A later unrelated successful wake cannot erase an earlier recorded failure.
- `lf wave recover <name>` displays the original historical continuation. Canceling it requires its exact source sequence and a reason; unknown provider liveness blocks replacement.
- Desktop Task terminals use Ghostty with a terminal pool owned by each workspace. The tmux plugin and desktop tmux shells are removed.

## Preserve who requested the work

Personal `user.name` configuration and request-author context help agents name people in Tasks, PRs, and summaries while keeping direct conversation conversational. Unknown authors remain unknown, and display names do not establish authorization.

- Inspect the configured name with `lf name --json`. Names travel through launches and SSH; native resume updates participant names and clears stale ones.
- Request authors survive CLI and Mac chat, journal replay, Discord, and Linear without being reassigned to publishers or editors.
- Specialized operating instructions now live in the skills that use them, reducing universal prompt guidance. Durable lessons live in Wave memory; retired direction files and orphaned attribution notes are removed, while chapter archives remain in `.lf/chapters/`.

## Operational notes

- Existing Flow invocations retain their captured definitions. Start a fresh invocation to replace obsolete review navigation policies. Inspect interrupted external operations before retrying them.
- Wave listeners and residents must use matching binaries. Historical queues are not automatically transferred into ordinary Flows. Unclosed attempts still require authoritative termination reconciliation; this release does not add that command.
- Minor releases now share a product snapshot with their closing patch: publish a new patch when changes remain, otherwise reuse the latest completed patch. Patch notes cover the preceding release; minor notes cover the cycle since the preceding `.0` tag. Persisted pair selection supports retries, but recovery receipts remain local to the controller.
- `lf release notes VERSION --preview` generates notes without changing release archives or manifests. Existing versions use their historical tag and context. Minor publication stops before tagging if the merged candidate differs from the prepared snapshot.
- Recorded checks include Rust, Python, website, Swift, and Mac build coverage; live GitHub branch recreation and configured release-note previews also succeeded. Landing/provider recovery and release publication proofs used simulations. Configured live provider/client execution and hosted UI acceptance remain outstanding. The default local gate still has a Flow Session fixture conflict between `LF_HOME` and `LF_CONTROL_HOME`; the recorded broad Rust pass used an isolated Home without the conflicting pin.

## Small changes

- Development Session reopen commands retain the development executable.
- Stored Codex OAuth no longer overrides native login.
- Terminal test polling reads fresh state before evaluating its deadline.
- Seven Rust dependencies are updated.
